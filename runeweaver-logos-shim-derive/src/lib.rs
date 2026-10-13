//! Implements the Logos compatibility derive macro.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use runeweaver_codegen::{RuleSpec, compile_with_modes_and_extras_at};
use std::cmp::Reverse;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Data, DeriveInput, Error, Expr, Fields, GenericParam, Ident, Lifetime, LifetimeParam, LitInt,
    LitStr, Result, Token, Type, parse_macro_input,
};

/// Derives the compatible `logos::Logos` trait.
#[proc_macro_derive(Logos, attributes(logos, token, regex))]
pub fn derive_logos(input: TokenStream) -> TokenStream {
    derive(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn derive(input: DeriveInput) -> Result<TokenStream2> {
    let Data::Enum(data) = &input.data else {
        return Err(Error::new_spanned(&input.ident, "`Logos` requires an enum"));
    };
    let config = Config::parse(&input.attrs)?;
    let token_ident = &input.ident;
    let helper_ident = format_ident!("__RuneweaverLogos{}Rules", token_ident);
    let mut definitions = Vec::new();
    for (variant_index, variant) in data.variants.iter().enumerate() {
        for attribute in &variant.attrs {
            let kind = if attribute.path().is_ident("token") {
                Some(RuleKind::Token)
            } else if attribute.path().is_ident("regex") {
                Some(RuleKind::Regex)
            } else {
                None
            };
            if let Some(kind) = kind {
                definitions.push(Definition::parse(
                    attribute.parse_args()?,
                    kind,
                    variant_index,
                )?);
            }
        }
    }
    if definitions.is_empty() {
        return Err(Error::new_spanned(
            token_ident,
            "a Logos enum needs at least one token or regex rule",
        ));
    }

    definitions.sort_by_key(|definition| Reverse(definition.priority));
    let mut helper_variants = Vec::new();
    let mut rules = Vec::new();
    let mut action_arms = Vec::new();
    for (rule_index, definition) in definitions.iter().enumerate() {
        let helper_variant = format_ident!("Rule{rule_index}");
        helper_variants.push(helper_variant.clone());
        let pattern = match definition.kind {
            RuleKind::Token => escape_literal(&definition.pattern.value()),
            RuleKind::Regex => definition.pattern.value(),
        };
        rules.push(RuleSpec::emit(
            pattern,
            quote!(Ok(Some(Self::#helper_variant))),
        ));
        let variant = &data.variants[definition.variant_index];
        action_arms.push(action_arm(
            &helper_ident,
            &helper_variant,
            variant,
            definition.callback.as_ref(),
        )?);
    }
    for (skip_index, skip) in config.skips.iter().enumerate() {
        if let Some(callback) = &skip.callback {
            let helper_variant = format_ident!("Skip{skip_index}");
            helper_variants.push(helper_variant.clone());
            rules.push(RuleSpec::emit(
                skip.pattern.value(),
                quote!(Ok(Some(Self::#helper_variant))),
            ));
            action_arms.push(quote! {
                #helper_ident::#helper_variant => {
                    match ::logos::internal::construct_skip(
                        ::logos::internal::call_callback(#callback, lexer),
                        lexer,
                    ) {
                        ::logos::internal::SkipResult::Skip => {
                            ::logos::internal::CallbackResult::Skip
                        }
                        ::logos::internal::SkipResult::Error(error) => {
                            ::logos::internal::CallbackResult::Error(error)
                        }
                    }
                },
            });
        } else {
            rules.push(RuleSpec::skip(skip.pattern.value()));
        }
    }

    let runtime = quote!(::logos::__private::runeweaver);
    let matcher = compile_with_modes_and_extras_at(
        runtime.clone(),
        quote!(()),
        quote!(),
        Vec::new(),
        rules,
        false,
        false,
    )
    .map_err(|error| Error::new_spanned(token_ident, error.to_string()))?;

    let source_lifetime = Lifetime::new("'__logos_source", Span::call_site());
    let declared_lifetimes: Vec<_> = input
        .generics
        .lifetimes()
        .map(|parameter| parameter.lifetime.clone())
        .collect();
    let token_source = declared_lifetimes.first().cloned();
    let mut impl_generics = input.generics.clone();
    impl_generics.params.insert(
        0,
        GenericParam::Lifetime(LifetimeParam::new(source_lifetime.clone())),
    );
    let where_clause = impl_generics.make_where_clause();
    for lifetime in &declared_lifetimes {
        where_clause
            .predicates
            .push(syn::parse_quote!(#source_lifetime: #lifetime));
    }
    if let Some(lifetime) = token_source {
        where_clause
            .predicates
            .push(syn::parse_quote!(#lifetime: #source_lifetime));
    }
    let (logos_impl_generics, _, logos_where_clause) = impl_generics.split_for_impl();
    let (_, type_generics, _) = input.generics.split_for_impl();
    let extras = config.extras;
    let error = config.error;

    Ok(quote! {
        #[doc(hidden)]
        enum #helper_ident {
            #(#helper_variants),*
        }

        impl #helper_ident {
            #matcher
        }

        impl<'__runeweaver_source> #runtime::lexer::Automaton<'__runeweaver_source>
            for #helper_ident
        {
            type Extras = ();

            fn next_token(
                scanner: &mut #runtime::Scanner<'__runeweaver_source, Self, ()>,
                mode: usize,
            ) -> Option<#runtime::lexer::ScanMatch<Self>> {
                Self::__runeweaver_next(scanner, mode)
            }

            fn mode_name(mode: usize) -> &'static str {
                Self::__runeweaver_mode_name(mode)
            }
        }

        impl #logos_impl_generics ::logos::Logos<#source_lifetime>
            for #token_ident #type_generics #logos_where_clause
        {
            type Extras = #extras;
            type Source = str;
            type Error = #error;

            fn lex(
                lexer: &mut ::logos::Lexer<#source_lifetime, Self>,
            ) -> Option<Result<Self, Self::Error>> {
                loop {
                    let base = lexer.__position();
                    let remaining = lexer.remainder();
                    let mut scanner = #runtime::Scanner::<#helper_ident>::new(remaining);
                    let result = scanner.next();
                    let position = scanner.position();
                    let span = scanner.span();
                    drop(scanner);
                    match result {
                        None => {
                            lexer.__set_span(base + position, base + position);
                            return None;
                        }
                        Some(Err(_)) => {
                            lexer.__set_span(base + span.start, base + span.end);
                            return Some(Err(Self::Error::default()));
                        }
                        Some(Ok(rule)) => {
                            lexer.__set_span(base + span.start, base + span.end);
                            let action = match rule {
                                #(#action_arms)*
                            };
                            match action {
                                ::logos::internal::CallbackResult::Emit(token) => {
                                    return Some(Ok(token));
                                }
                                ::logos::internal::CallbackResult::Error(error) => {
                                    return Some(Err(error));
                                }
                                ::logos::internal::CallbackResult::DefaultError => {
                                    return Some(Err(Self::Error::default()));
                                }
                                ::logos::internal::CallbackResult::Skip => continue,
                            }
                        }
                    }
                }
            }
        }
    })
}

fn action_arm(
    helper: &Ident,
    helper_variant: &Ident,
    variant: &syn::Variant,
    callback: Option<&Expr>,
) -> Result<TokenStream2> {
    let variant_ident = &variant.ident;
    let action = match &variant.fields {
        Fields::Unit => callback.map_or_else(
            || quote!(::logos::internal::CallbackResult::Emit(Self::#variant_ident)),
            |callback| {
                quote! {
                    ::logos::internal::CallbackRetVal::construct(
                        ::logos::internal::call_callback(#callback, lexer),
                        |()| Self::#variant_ident,
                    )
                }
            },
        ),
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let field = fields.unnamed.first().expect("one field was checked");
            if let Some(callback) = callback {
                quote! {
                    ::logos::internal::CallbackRetVal::construct(
                        ::logos::internal::call_callback(#callback, lexer),
                        Self::#variant_ident,
                    )
                }
            } else if is_str_reference(&field.ty) {
                quote! {
                    ::logos::internal::CallbackResult::Emit(
                        Self::#variant_ident(lexer.slice()),
                    )
                }
            } else {
                return Err(Error::new_spanned(
                    field,
                    "a payload rule needs a callback unless its payload is `&str`",
                ));
            }
        }
        _ => {
            return Err(Error::new_spanned(
                variant,
                "a Logos token must be a unit variant or have one tuple payload",
            ));
        }
    };
    Ok(quote!(#helper::#helper_variant => { #action },))
}

fn is_str_reference(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Reference(reference)
            if reference.mutability.is_none()
                && matches!(reference.elem.as_ref(), Type::Path(path) if path.path.is_ident("str"))
    )
}

fn escape_literal(literal: &str) -> String {
    let mut escaped = String::with_capacity(literal.len());
    for character in literal.chars() {
        if matches!(
            character,
            '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '\\'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[derive(Clone, Copy)]
enum RuleKind {
    Token,
    Regex,
}

struct Definition {
    pattern: LitStr,
    callback: Option<Expr>,
    priority: usize,
    kind: RuleKind,
    variant_index: usize,
}

impl Definition {
    fn parse(arguments: DefinitionArgs, kind: RuleKind, variant_index: usize) -> Result<Self> {
        let priority = arguments.priority.unwrap_or_else(|| match kind {
            RuleKind::Token => arguments.pattern.value().chars().count() * 2,
            RuleKind::Regex => 0,
        });
        Ok(Self {
            pattern: arguments.pattern,
            callback: arguments.callback,
            priority,
            kind,
            variant_index,
        })
    }
}

struct DefinitionArgs {
    pattern: LitStr,
    callback: Option<Expr>,
    priority: Option<usize>,
}

impl Parse for DefinitionArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let pattern = input.parse()?;
        let mut callback = None;
        let mut priority = None;
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let fork = input.fork();
            if fork.peek(Ident) && fork.peek2(Token![=]) {
                let name: Ident = input.parse()?;
                input.parse::<Token![=]>()?;
                if name == "priority" {
                    let value: LitInt = input.parse()?;
                    priority = Some(value.base10_parse()?);
                } else if name == "callback" {
                    callback = Some(input.parse()?);
                } else {
                    return Err(Error::new_spanned(name, "unsupported Logos rule option"));
                }
            } else {
                if callback.is_some() {
                    return Err(input.error("a rule can have only one callback"));
                }
                callback = Some(input.parse()?);
            }
        }
        Ok(Self {
            pattern,
            callback,
            priority,
        })
    }
}

struct Config {
    extras: Type,
    error: Type,
    skips: Vec<DefinitionArgs>,
}

impl Config {
    fn parse(attributes: &[syn::Attribute]) -> Result<Self> {
        let mut extras = None;
        let mut error = None;
        let mut skips = Vec::new();
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.path().is_ident("logos"))
        {
            let arguments =
                attribute.parse_args_with(Punctuated::<ConfigItem, Token![,]>::parse_terminated)?;
            for item in arguments {
                match item {
                    ConfigItem::Extras(ty) => extras = Some(ty),
                    ConfigItem::Error(ty) => error = Some(ty),
                    ConfigItem::Skip(pattern) => skips.push(pattern),
                }
            }
        }
        Ok(Self {
            extras: extras.unwrap_or_else(|| syn::parse_quote!(())),
            error: error.unwrap_or_else(|| syn::parse_quote!(())),
            skips,
        })
    }
}

enum ConfigItem {
    Extras(Type),
    Error(Type),
    Skip(DefinitionArgs),
}

impl Parse for ConfigItem {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name: Ident = input.parse()?;
        if name == "extras" || name == "error" {
            input.parse::<Token![=]>()?;
            let ty = input.parse()?;
            return Ok(if name == "extras" {
                Self::Extras(ty)
            } else {
                Self::Error(ty)
            });
        }
        if name == "skip" {
            let definition = if input.peek(syn::token::Paren) {
                let content;
                syn::parenthesized!(content in input);
                content.parse()?
            } else {
                DefinitionArgs {
                    pattern: input.parse()?,
                    callback: None,
                    priority: None,
                }
            };
            if definition.priority.is_some() {
                return Err(Error::new_spanned(
                    &definition.pattern,
                    "skip priorities are not supported yet",
                ));
            }
            return Ok(Self::Skip(definition));
        }
        Err(Error::new_spanned(name, "unsupported Logos option"))
    }
}
