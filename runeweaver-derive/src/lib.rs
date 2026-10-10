//! Defines the runeweaver derive macro.
//!
//! The macro reads token enums and emits their lexer implementations.

#![deny(dead_code)]

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span};
use quote::quote;
use runeweaver_codegen::{RuleSpec, Transition, compile_with_modes_at};
use syn::{
    Data, DeriveInput, Error, Expr, Fields, LitInt, LitStr, Result, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// Derives the `runeweaver::Lexer` trait for an enum of token kinds.
///
/// Every variant needs one `#[token("pattern")]` attribute. Unit variants
/// emit no payload; a single-field tuple variant parses an owned payload.
/// Rules are considered in declaration order when equal-length matches tie.
/// The scanner accepts candidate tokens up to 16 MiB by default. Set
/// `#[lexer(max_token_bytes = N)]` on the enum to choose another limit.
///
/// # Examples
///
/// ```
/// # extern crate self as runeweaver;
/// # pub struct PayloadError;
/// # impl PayloadError {
/// #     pub fn new(_: impl Into<String>) -> Self {
/// #         Self
/// #     }
/// #     pub fn with_transition(self, _: Transition) -> Self {
/// #         self
/// #     }
/// # }
/// # pub enum Transition {
/// #     Stay,
/// #     Begin(usize),
/// #     Push(usize),
/// #     Pop,
/// # }
/// # pub type RuleScan<T> = Result<(Option<T>, usize, Transition), (PayloadError, usize)>;
/// # pub mod __private { pub trait Sealed {} }
/// # pub trait Lexer: __private::Sealed + Sized {
/// #     fn max_token_bytes() -> usize { 16 * 1024 * 1024 }
/// #     fn start_state(mode: usize) -> Option<usize>;
/// #     fn next_state(state: usize, byte: u8) -> Option<usize>;
/// #     fn accepting_rule(state: usize) -> Option<usize>;
/// #     fn scan_one(input: &str, mode: usize) -> Option<RuleScan<Self>>;
/// #     fn run_action(rule: usize, text: &str) -> Result<(Option<Self>, Transition), PayloadError>;
/// #     fn mode_name(mode: usize) -> &'static str;
/// # }
/// # fn main() {
/// use runeweaver_derive::Lexer;
///
/// #[derive(Debug, PartialEq, Lexer)]
/// #[lexer(max_token_bytes = 1_048_576)]
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
        max_token_bytes,
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
                            #runtime::PayloadResult::<#payload_type>::into_payload(
                                (#converter)(text),
                            )
                            .map(|payload| Some(Self::#variant_ident(payload)))
                        },
                        None => quote! {
                            #runtime::parse_payload::<#payload_type>(text)
                                .map(|payload| Some(Self::#variant_ident(payload)))
                        },
                    };
                    rules.push(rule_spec(
                        RuleSpec::emit(pattern.value(), action),
                        &attribute.modes,
                        &attribute.transition,
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
            &attribute.transition,
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
    let max_token_bytes = max_token_bytes.map(|limit| {
        quote! {
            fn max_token_bytes() -> usize {
                #limit
            }
        }
    });

    Ok(quote! {
        impl #impl_generics #ident #type_generics #where_clause {
            #matcher
        }

        impl #impl_generics #runtime::__private::Sealed for #ident #type_generics #where_clause {}

        impl #impl_generics #runtime::Lexer for #ident #type_generics #where_clause {
            #max_token_bytes

            fn start_state(mode: usize) -> Option<usize> {
                Self::__runeweaver_start(mode)
            }

            fn next_state(state: usize, byte: u8) -> Option<usize> {
                Self::__runeweaver_transition(state, byte)
            }

            fn accepting_rule(state: usize) -> Option<usize> {
                Self::__runeweaver_accept(state)
            }

            fn scan_one(input: &str, mode: usize) -> Option<#runtime::RuleScan<Self>> {
                Self::__runeweaver_scan_one(input, mode)
            }

            fn run_action(
                rule: usize,
                text: &str,
            ) -> Result<(Option<Self>, #runtime::Transition), #runtime::PayloadError> {
                Self::__runeweaver_action(rule, text)
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
    max_token_bytes: Option<LitInt>,
}

fn lexer_attributes(attributes: &[syn::Attribute]) -> Result<LexerConfig> {
    let mut modes = Vec::new();
    let mut rules = Vec::new();
    let mut patterns = Vec::new();
    let mut max_token_bytes = None;
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
            if config.name == "max_token_bytes" {
                let limit = config
                    .max_token_bytes
                    .expect("max_token_bytes attribute has an integer");
                if limit.base10_parse::<usize>()? == 0 {
                    return Err(Error::new_spanned(
                        limit,
                        "`max_token_bytes` must be greater than zero",
                    ));
                }
                if max_token_bytes.replace(limit).is_some() {
                    return Err(Error::new_spanned(
                        attribute,
                        "a lexer can declare `max_token_bytes` only once",
                    ));
                }
                return Ok(());
            }
            if config.name != "skip" {
                return Err(Error::new_spanned(
                    attribute,
                    "expected `mode`, `skip`, or `max_token_bytes`",
                ));
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
                &config.rule.transition,
            ));
            patterns.push(pattern);
            Ok(())
        })?;
    Ok(LexerConfig {
        modes,
        rules,
        patterns,
        max_token_bytes,
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
    max_token_bytes: Option<LitInt>,
    rule: RuleConfig,
}

struct RuleConfig {
    modes: Vec<String>,
    transition: Transition,
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
                max_token_bytes: None,
                rule: RuleConfig::stay(),
            });
        }
        if name == "max_token_bytes" {
            let max_token_bytes: LitInt = input.parse()?;
            if !input.is_empty() {
                return Err(input.error("unexpected max_token_bytes attribute content"));
            }
            return Ok(Self {
                name: name.to_string(),
                mode: None,
                pattern: None,
                max_token_bytes: Some(max_token_bytes),
                rule: RuleConfig::stay(),
            });
        }
        let pattern: LitStr = input.parse()?;
        let rule = parse_rule_modifiers(input)?;
        Ok(Self {
            name: name.to_string(),
            mode: None,
            pattern: Some(pattern),
            max_token_bytes: None,
            rule,
        })
    }
}

impl RuleConfig {
    fn stay() -> Self {
        Self {
            modes: vec!["INITIAL".into()],
            transition: Transition::Stay,
            converter: None,
        }
    }
}

fn parse_rule_modifiers(input: ParseStream<'_>) -> Result<RuleConfig> {
    let mut config = RuleConfig::stay();
    while !input.is_empty() {
        input.parse::<syn::Token![,]>()?;
        let name: syn::Ident = input.parse()?;
        match name.to_string().as_str() {
            "modes" => {
                input.parse::<syn::Token![=]>()?;
                config.modes = parse_modes(input)?;
            }
            "push" => {
                input.parse::<syn::Token![=]>()?;
                config.transition = Transition::Push(input.parse::<syn::Ident>()?.to_string());
            }
            "begin" => {
                input.parse::<syn::Token![=]>()?;
                config.transition = Transition::Begin(input.parse::<syn::Ident>()?.to_string());
            }
            "with" => {
                input.parse::<syn::Token![=]>()?;
                config.converter = Some(input.parse()?);
            }
            "pop" => config.transition = Transition::Pop,
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

fn rule_spec(spec: RuleSpec, modes: &[String], transition: &Transition) -> RuleSpec {
    spec.in_modes(modes.to_vec()).transition(transition.clone())
}

struct RuleAttribute {
    pattern: LitStr,
    converter: Option<Expr>,
    modes: Vec<String>,
    transition: Transition,
}

impl Parse for RuleAttribute {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let pattern = input.parse()?;
        let config = parse_rule_modifiers(input)?;
        Ok(Self {
            pattern,
            converter: config.converter,
            modes: config.modes,
            transition: config.transition,
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
