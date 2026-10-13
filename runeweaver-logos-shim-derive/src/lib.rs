//! Implements the Logos compatibility derive macro.

mod config;
mod export;
mod pattern;

use config::{Config, Definition};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::quote;
use runeweaver_codegen::ByteAutomaton;
use syn::visit_mut::{self, VisitMut};
use syn::{Data, DeriveInput, Error, Fields, Lifetime, Result, parse_macro_input};

/// Derives the compatible `logos::Logos` trait.
#[proc_macro_derive(Logos, attributes(logos, token, regex))]
pub fn derive_logos(input: TokenStream) -> TokenStream {
    derive(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn derive(input: DeriveInput) -> Result<Tokens> {
    let Data::Enum(data) = &input.data else {
        return Err(Error::new_spanned(&input.ident, "`Logos` requires an enum"));
    };
    let config = Config::parse(&input.attrs)?;
    let root = &config.root;
    let token = &input.ident;
    let lifetime = Lifetime::new("'__logos_source", Span::call_site());
    if input.generics.lifetimes().count() > 1 || input.generics.const_params().next().is_some() {
        return Err(Error::new_spanned(
            &input.generics,
            "Logos supports one lifetime and concrete type substitutions",
        ));
    }
    let mut generics = Vec::new();
    for parameter in &input.generics.params {
        match parameter {
            syn::GenericParam::Lifetime(_) => generics.push(quote!(#lifetime)),
            syn::GenericParam::Type(parameter) => {
                let Some(ty) = config.types.get(&parameter.ident.to_string()) else {
                    return Err(Error::new_spanned(
                        parameter,
                        "a generic type needs #[logos(type T = Type)]",
                    ));
                };
                let mut ty = ty.clone();
                Substitutions {
                    config: &config,
                    lifetime: &lifetime,
                }
                .visit_type_mut(&mut ty);
                generics.push(quote!(#ty));
            }
            _ => unreachable!(),
        }
    }
    let token_type = if generics.is_empty() {
        quote!(#token)
    } else {
        quote!(#token<#(#generics),*>)
    };
    let mut definitions = Vec::new();
    for (index, variant) in data.variants.iter().enumerate() {
        for attribute in &variant.attrs {
            if attribute.path().is_ident("token") || attribute.path().is_ident("regex") {
                let args = attribute.parse_args()?;
                definitions.push(Definition {
                    args,
                    variant: Some(index),
                    literal: attribute.path().is_ident("token"),
                });
            }
        }
    }
    definitions.extend(config.skips.iter().cloned().map(|args| Definition {
        args,
        variant: None,
        literal: false,
    }));
    let mut patterns = Vec::new();
    let mut priorities = Vec::new();
    let mut actions = Vec::new();
    for (index, definition) in definitions.iter().enumerate() {
        let (hir, priority) = pattern::parse(definition, &config)?;
        if config.utf8 && !hir.properties().is_utf8() {
            return Err(Error::new_spanned(
                &definition.args.pattern,
                "a string source cannot match invalid UTF-8",
            ));
        }
        priorities.push(priority);
        patterns.push(
            pattern::lower(&hir)
                .map_err(|message| Error::new_spanned(&definition.args.pattern, message))?,
        );
        let callback = &definition.args.callback;
        let action = if let Some(variant_index) = definition.variant {
            let variant = &data.variants[variant_index];
            let name = &variant.ident;
            let constructor = match &variant.fields {
                Fields::Unit => quote!(|()| Self::#name),
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => quote!(Self::#name),
                _ => {
                    return Err(Error::new_spanned(
                        variant,
                        "a Logos token has zero fields or one unnamed field",
                    ));
                }
            };
            if let Some(callback) = callback {
                quote!(#root::internal::CallbackRetVal::construct(#root::internal::call_callback(#callback, lexer), #constructor))
            } else {
                match &variant.fields {
                    Fields::Unit => quote!(#root::internal::CallbackResult::Emit(Self::#name)),
                    _ => quote!(#root::internal::CallbackResult::Emit(Self::#name(lexer.slice()))),
                }
            }
        } else if let Some(callback) = callback {
            quote!(#root::internal::construct_skip(#root::internal::call_callback(#callback, lexer), lexer).into())
        } else {
            quote!(#root::internal::CallbackResult::Skip)
        };
        actions.push(quote!(#index => #action,));
    }
    let automaton = ByteAutomaton::compile(&patterns)
        .map_err(|error| Error::new_spanned(token, error.to_string()))?;
    if let Some(destination) = &config.export {
        export::graphs(destination, &token.to_string(), &automaton)?;
    }
    let mut accepts = Vec::new();
    for (state, rules) in automaton.accepts.iter().enumerate() {
        if let Some(priority) = rules.iter().map(|rule| priorities[*rule]).max() {
            let winners: Vec<_> = rules
                .iter()
                .copied()
                .filter(|rule| priorities[*rule] == priority)
                .collect();
            if winners.len() != 1 {
                return Err(Error::new_spanned(
                    token,
                    "multiple rules match with the same priority; set an explicit priority",
                ));
            }
            let rule = winners[0];
            accepts.push(quote!(#state => ::core::option::Option::Some(#rule),));
        }
    }
    let transitions: Vec<_> = automaton.transitions.iter().enumerate().filter(|(_, edges)| !edges.is_empty()).map(|(state, edges)| {
        let edges = edges.iter().map(|(low, high, target)| quote!(#low..=#high => ::core::option::Option::Some(#target),));
        quote!(#state => match byte { #(#edges)* _ => ::core::option::Option::None },)
    }).collect();
    let start = automaton.start;
    let runtime = quote!(#root::__private::runeweaver);
    let mut extras = config.extras.clone();
    let mut error = config.error.clone();
    let mut source = config.source.clone().unwrap_or_else(|| {
        if config.utf8 {
            syn::parse_quote!(str)
        } else {
            syn::parse_quote!([u8])
        }
    });
    let mut substitutions = Substitutions {
        config: &config,
        lifetime: &lifetime,
    };
    substitutions.visit_type_mut(&mut extras);
    substitutions.visit_type_mut(&mut error);
    substitutions.visit_type_mut(&mut source);
    let default_error = config.error_callback.as_ref().map_or_else(
        || quote!(<#error as ::core::default::Default>::default()),
        |callback| quote!(#root::internal::call_callback(#callback, lexer)),
    );
    let scanner = if definitions.is_empty() {
        quote! {
            let base = lexer.__position();
            if base == #root::Source::len(lexer.source()) { return ::core::option::Option::None; }
            let end = #root::Source::find_boundary(lexer.source(), base + 1);
            lexer.__set_span(base, end);
            ::core::option::Option::Some(::core::result::Result::Err(#default_error))
        }
    } else {
        quote! {
            loop {
                let base = lexer.__position();
                if base == #root::Source::len(lexer.source()) { return ::core::option::Option::None; }
                struct Rules;
                impl #runtime::bytes::ByteMatcher for Rules {
                    fn start() -> usize { #start }
                    fn transition(state: usize, byte: u8) -> ::core::option::Option<usize> {
                        match state { #(#transitions)* _ => ::core::option::Option::None }
                    }
                    fn accept(state: usize) -> ::core::option::Option<usize> {
                        match state { #(#accepts)* _ => ::core::option::Option::None }
                    }
                }
                let matched = <Rules as #runtime::bytes::ByteMatcher>::match_source(
                    |offset| #root::Source::read::<u8>(lexer.source(), offset), base,
                );
                let ::core::option::Option::Some(rule) = matched.rule else {
                    let end = #root::Source::find_boundary(lexer.source(), matched.traversed.max(base + 1));
                    lexer.__set_span(base, end);
                    return ::core::option::Option::Some(::core::result::Result::Err(#default_error));
                };
                lexer.__set_span(base, matched.end);
                let action = match rule { #(#actions)* _ => unreachable!("invalid Runeweaver rule") };
                match action {
                    #root::internal::CallbackResult::Emit(token) => return ::core::option::Option::Some(::core::result::Result::Ok(token)),
                    #root::internal::CallbackResult::Error(error) => return ::core::option::Option::Some(::core::result::Result::Err(error)),
                    #root::internal::CallbackResult::DefaultError => return ::core::option::Option::Some(::core::result::Result::Err(#default_error)),
                    #root::internal::CallbackResult::Skip => {
                        let position = lexer.__position();
                        lexer.__set_span(position, position);
                        continue;
                    },
                }
            }
        }
    };
    Ok(quote! {
        impl<#lifetime> #root::Logos<#lifetime> for #token_type {
            type Extras = #extras;
            type Source = #source;
            type Error = #error;

            fn lex(lexer: &mut #root::Lexer<#lifetime, Self>) -> ::core::option::Option<::core::result::Result<Self, #error>> {
                #scanner
            }
        }
    })
}

struct Substitutions<'a> {
    config: &'a Config,
    lifetime: &'a Lifetime,
}

impl VisitMut for Substitutions<'_> {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        if let syn::Type::Path(path) = ty {
            if let Some(ident) = path.path.get_ident() {
                if let Some(replacement) = self.config.types.get(&ident.to_string()) {
                    *ty = replacement.clone();
                }
            }
        }
        visit_mut::visit_type_mut(self, ty);
        if let syn::Type::Reference(reference) = ty {
            if reference.lifetime.is_none() {
                reference.lifetime = Some(self.lifetime.clone());
            }
        }
    }

    fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
        if lifetime.ident != "static" {
            *lifetime = self.lifetime.clone();
        }
    }
}
