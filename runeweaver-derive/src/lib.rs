//! Defines the runeweaver derive macro.
//!
//! The macro reads token enums and emits their lexer implementations.

#![deny(dead_code)]

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span};
use quote::quote;
use runeweaver_codegen::{ModeAction, RuleSpec, compile_with_modes_at};
use syn::{
    Data, DeriveInput, Error, Expr, Fields, LitStr, Result, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// Derives the `runeweaver::Lexer` trait for an enum of token kinds.
///
/// Every variant needs one `#[token("pattern")]` attribute. Unit variants
/// emit no payload; a single-field tuple variant parses an owned payload.
/// Rules are considered in declaration order when equal-length matches tie.
///
/// # Examples
///
/// ```
/// # extern crate self as runeweaver;
/// # #[derive(Clone, Copy)]
/// # pub struct Span;
/// # pub struct Scanner<'a, T, E = ()> { input: &'a str, offset: usize, span: Span, marker: std::marker::PhantomData<(T, E)> }
/// # impl<'a, T, E> Scanner<'a, T, E> {
/// #     pub fn source(&self) -> &'a str { self.input }
/// #     pub fn position(&self) -> usize { self.offset }
/// #     pub fn span(&self) -> Span { self.span }
/// #     pub fn __set_position(&mut self, position: usize) { self.offset = position; }
/// #     pub fn __set_match(&mut self, _: usize, end: usize) { self.offset = end; }
/// # }
/// # pub struct ScanError;
/// # impl ScanError {
/// #     pub fn unrecognized(_: Span) -> Self { Self }
/// #     pub fn invalid_payload(_: Span, _: payload::PayloadError) -> Self { Self }
/// # }
/// # pub mod lexer {
/// # use crate::payload::PayloadError;
/// # pub enum ModeAction {
/// #     Stay,
/// #     Begin(usize),
/// #     Push(usize),
/// #     Pop,
/// # }
/// # pub type ScanMatch<T> = Result<(Option<T>, ModeAction), (crate::ScanError, ModeAction)>;
/// # pub trait Automaton: Sized {
/// #     fn next_token(scanner: &mut crate::Scanner<'_, Self, impl Sized>, mode: usize) -> Option<ScanMatch<Self>>;
/// #     fn mode_name(mode: usize) -> &'static str;
/// # }
/// # }
/// # pub mod payload {
/// # pub struct PayloadError;
/// # impl PayloadError {
/// #     pub fn new(_: impl Into<String>) -> Self {
/// #         Self
/// #     }
/// # }
/// # }
/// # pub trait Lexer: lexer::Automaton + Sized {}
/// # fn main() {
/// use runeweaver_derive::Lexer;
///
/// #[derive(Debug, PartialEq, Lexer)]
/// enum Token {
///     #[token("[a-z]+")]
///     Word,
/// }
///
/// let _ = Token::Word;
/// # }
/// ```
#[proc_macro_derive(Lexer, attributes(lexer, token))]
pub fn derive_lexer(input: TokenStream) -> TokenStream {
    match derive_lexer_inner(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn derive_lexer_inner(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let Data::Enum(data) = input.data else {
        return Err(Error::new_spanned(
            input.ident,
            "`Lexer` can only be derived for enums",
        ));
    };

    let runtime = runtime_path();
    let LexerConfig {
        modes,
        mut rules,
        mut patterns,
    } = lexer_attributes(&input.attrs)?;
    for variant in data.variants {
        let attribute = rule_attribute(&variant.attrs, &variant.ident)?;
        let pattern = attribute.pattern.clone();
        let converter = attribute.converter;
        let variant_ident = variant.ident;
        let action = match variant.fields {
            Fields::Unit => {
                if converter.is_some() {
                    return Err(Error::new_spanned(
                        variant_ident,
                        "a unit token variant has no payload, thus it cannot have `with`",
                    ));
                }
                quote!(Ok(Some(Self::#variant_ident)))
            }
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let field = fields.unnamed.first().expect("one field was checked");
                let Type::Reference(_) = field.ty else {
                    let payload_type = &field.ty;
                    let action = match converter {
                        Some(converter) => quote! {
                            #runtime::payload::PayloadResult::<#payload_type>::into_payload(
                                (#converter)(text),
                            )
                            .map(|payload| Some(Self::#variant_ident(payload)))
                        },
                        None => quote! {
                            #runtime::payload::parse_payload::<#payload_type>(text)
                                .map(|payload| Some(Self::#variant_ident(payload)))
                        },
                    };
                    rules.push(rule_spec(
                        RuleSpec::emit(pattern.value(), action),
                        &attribute.modes,
                        &attribute.mode_action,
                    ));
                    patterns.push(pattern);
                    continue;
                };
                return Err(Error::new_spanned(field, "token payloads must be owned"));
            }
            Fields::Unnamed(_) => {
                return Err(Error::new_spanned(
                    variant_ident,
                    "lexer token variants may have at most one tuple payload",
                ));
            }
            Fields::Named(_) => {
                return Err(Error::new_spanned(
                    variant_ident,
                    "lexer token variants must be unit variants or have one tuple payload",
                ));
            }
        };
        rules.push(rule_spec(
            RuleSpec::emit(pattern.value(), action),
            &attribute.modes,
            &attribute.mode_action,
        ));
        patterns.push(pattern);
    }

    let matcher = compile_with_modes_at(runtime.clone(), modes, rules).map_err(|error| {
        error.rule_index().map_or_else(
            || Error::new_spanned(input.ident.clone(), error.to_string()),
            |index| {
                let pattern = patterns
                    .get(index)
                    .expect("a code generation rule has a pattern span");
                Error::new_spanned(pattern, error.to_string())
            },
        )
    })?;
    let ident = input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #ident #type_generics #where_clause {
            #matcher
        }

        impl #impl_generics #runtime::lexer::Automaton for #ident #type_generics #where_clause {
            #[inline(always)]
            fn next_token(
                scanner: &mut #runtime::Scanner<'_, Self, impl Sized>,
                mode: usize,
            ) -> Option<#runtime::lexer::ScanMatch<Self>> {
                Self::__runeweaver_next(scanner, mode)
            }

            fn mode_name(mode: usize) -> &'static str {
                Self::__runeweaver_mode_name(mode)
            }
        }
    })
}

struct LexerConfig {
    modes: Vec<String>,
    rules: Vec<RuleSpec>,
    patterns: Vec<LitStr>,
}

fn lexer_attributes(attributes: &[syn::Attribute]) -> Result<LexerConfig> {
    let mut modes = Vec::new();
    let mut rules = Vec::new();
    let mut patterns = Vec::new();
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("lexer"))
        .try_for_each(|attribute| {
            let config: LexerAttribute = attribute.parse_args()?;
            if config.name == "mode" {
                let Some(mode) = config.mode else {
                    unreachable!()
                };
                modes.push(mode);
                return Ok(());
            }
            if config.name != "skip" {
                return Err(Error::new_spanned(attribute, "expected `mode` or `skip`"));
            }
            let pattern = config.pattern.expect("skip attribute has a pattern");
            if config.rule.converter.is_some() {
                return Err(Error::new_spanned(
                    attribute,
                    "a `skip` rule emits no token, thus it cannot have `with`",
                ));
            }
            rules.push(rule_spec(
                RuleSpec::skip(pattern.value()),
                &config.rule.modes,
                &config.rule.mode_action,
            ));
            patterns.push(pattern);
            Ok(())
        })?;
    Ok(LexerConfig {
        modes,
        rules,
        patterns,
    })
}

fn runtime_path() -> proc_macro2::TokenStream {
    match crate_name("runeweaver") {
        Ok(FoundCrate::Itself) => quote!(::runeweaver),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Err(_) => quote!(::runeweaver),
    }
}

struct LexerAttribute {
    name: String,
    mode: Option<String>,
    pattern: Option<LitStr>,
    rule: RuleConfig,
}

struct RuleConfig {
    modes: Vec<String>,
    mode_action: ModeAction,
    converter: Option<Expr>,
}

impl Parse for LexerAttribute {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name: syn::Ident = input.parse()?;
        input.parse::<syn::Token![=]>()?;
        if name == "mode" {
            let mode: syn::Ident = input.parse()?;
            if !input.is_empty() {
                return Err(input.error("unexpected lexer mode attribute content"));
            }
            return Ok(Self {
                name: name.to_string(),
                mode: Some(mode.to_string()),
                pattern: None,
                rule: RuleConfig::stay(),
            });
        }
        let pattern: LitStr = input.parse()?;
        let rule = parse_rule_modifiers(input)?;
        Ok(Self {
            name: name.to_string(),
            mode: None,
            pattern: Some(pattern),
            rule,
        })
    }
}

impl RuleConfig {
    fn stay() -> Self {
        Self {
            modes: vec!["INITIAL".into()],
            mode_action: ModeAction::Stay,
            converter: None,
        }
    }
}

fn parse_rule_modifiers(input: ParseStream<'_>) -> Result<RuleConfig> {
    let mut config = RuleConfig::stay();
    let mut has_modes = false;
    let mut has_mode_action = false;
    let mut has_converter = false;
    while !input.is_empty() {
        input.parse::<syn::Token![,]>()?;
        let name: syn::Ident = input.parse()?;
        match name.to_string().as_str() {
            "modes" => {
                if has_modes {
                    return Err(Error::new_spanned(
                        name,
                        "`modes` can be specified only once",
                    ));
                }
                has_modes = true;
                input.parse::<syn::Token![=]>()?;
                config.modes = parse_modes(input)?;
            }
            "push" => {
                if has_mode_action {
                    return Err(Error::new_spanned(
                        name,
                        "a lexer rule can have only one mode action",
                    ));
                }
                has_mode_action = true;
                input.parse::<syn::Token![=]>()?;
                config.mode_action = ModeAction::Push(input.parse::<syn::Ident>()?.to_string());
            }
            "begin" => {
                if has_mode_action {
                    return Err(Error::new_spanned(
                        name,
                        "a lexer rule can have only one mode action",
                    ));
                }
                has_mode_action = true;
                input.parse::<syn::Token![=]>()?;
                config.mode_action = ModeAction::Begin(input.parse::<syn::Ident>()?.to_string());
            }
            "with" => {
                if has_converter {
                    return Err(Error::new_spanned(
                        name,
                        "`with` can be specified only once",
                    ));
                }
                has_converter = true;
                input.parse::<syn::Token![=]>()?;
                config.converter = Some(input.parse()?);
            }
            "pop" => {
                if has_mode_action {
                    return Err(Error::new_spanned(
                        name,
                        "a lexer rule can have only one mode action",
                    ));
                }
                has_mode_action = true;
                config.mode_action = ModeAction::Pop;
            }
            _ => {
                return Err(Error::new_spanned(
                    name,
                    "expected `modes`, `with`, `push`, `begin`, or `pop`",
                ));
            }
        }
    }
    Ok(config)
}

fn parse_modes(input: ParseStream<'_>) -> Result<Vec<String>> {
    if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let modes = content.parse_terminated(syn::Ident::parse, syn::Token![,])?;
        Ok(modes.into_iter().map(|mode| mode.to_string()).collect())
    } else {
        Ok(vec![input.parse::<syn::Ident>()?.to_string()])
    }
}

fn rule_spec(spec: RuleSpec, modes: &[String], mode_action: &ModeAction) -> RuleSpec {
    spec.in_modes(modes.to_vec())
        .mode_action(mode_action.clone())
}

struct RuleAttribute {
    pattern: LitStr,
    converter: Option<Expr>,
    modes: Vec<String>,
    mode_action: ModeAction,
}

impl Parse for RuleAttribute {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let pattern = input.parse()?;
        let config = parse_rule_modifiers(input)?;
        Ok(Self {
            pattern,
            converter: config.converter,
            modes: config.modes,
            mode_action: config.mode_action,
        })
    }
}

fn rule_attribute(attributes: &[syn::Attribute], variant: &syn::Ident) -> Result<RuleAttribute> {
    let mut patterns = attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("token"));
    let Some(attribute) = patterns.next() else {
        return Err(Error::new_spanned(
            variant,
            "lexer token variants need one `#[token(\"pattern\")]` attribute",
        ));
    };
    if patterns.next().is_some() {
        return Err(Error::new_spanned(
            variant,
            "lexer token variants can have only one `#[token(...)]` attribute",
        ));
    }
    attribute.parse_args()
}
