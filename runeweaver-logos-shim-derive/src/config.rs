use std::collections::BTreeMap;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Error, Expr, Ident, Lit, LitBool, LitInt, LitStr, Path, Result, Token, Type};

#[derive(Clone)]
pub(super) struct Arguments {
    pub pattern: Lit,
    pub callback: Option<Expr>,
    pub priority: Option<usize>,
    pub ignore_case: bool,
    pub allow_greedy: bool,
}

impl Parse for Arguments {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let pattern = input.parse()?;
        let mut args = Self {
            pattern,
            callback: None,
            priority: None,
            ignore_case: false,
            allow_greedy: false,
        };
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            if input.peek(Ident) && input.peek2(Token![=]) {
                let name: Ident = input.parse()?;
                input.parse::<Token![=]>()?;
                match name.to_string().as_str() {
                    "priority" => {
                        if args.priority.is_some() {
                            return Err(Error::new_spanned(name, "duplicate priority"));
                        }
                        args.priority = Some(input.parse::<LitInt>()?.base10_parse()?);
                    }
                    "callback" => {
                        if args.callback.is_some() {
                            return Err(Error::new_spanned(name, "duplicate callback"));
                        }
                        args.callback = Some(input.parse()?);
                    }
                    "allow_greedy" => args.allow_greedy = input.parse::<LitBool>()?.value,
                    _ => return Err(Error::new_spanned(name, "unsupported Logos rule option")),
                }
            } else if input.peek(Ident)
                && input.peek2(syn::token::Paren)
                && input.fork().parse::<Ident>()? == "ignore"
            {
                input.parse::<Ident>()?;
                let content;
                syn::parenthesized!(content in input);
                let flag: Ident = content.parse()?;
                if flag != "case" || !content.is_empty() {
                    return Err(Error::new_spanned(flag, "expected ignore(case)"));
                }
                args.ignore_case = true;
            } else {
                if args.callback.is_some() {
                    return Err(input.error("duplicate callback"));
                }
                args.callback = Some(input.parse()?);
            }
        }
        Ok(args)
    }
}

pub(super) struct Definition {
    pub args: Arguments,
    pub variant: Option<usize>,
    pub literal: bool,
}

pub(super) struct Config {
    pub root: Path,
    pub extras: Type,
    pub error: Type,
    pub error_callback: Option<Expr>,
    pub source: Option<Type>,
    pub utf8: bool,
    pub skips: Vec<Arguments>,
    pub types: BTreeMap<String, Type>,
    pub subpatterns: BTreeMap<String, Lit>,
    pub export: Option<LitStr>,
}

impl Config {
    pub(super) fn parse(attributes: &[syn::Attribute]) -> Result<Self> {
        let mut config = Self {
            root: syn::parse_quote!(::logos),
            extras: syn::parse_quote!(()),
            error: syn::parse_quote!(()),
            error_callback: None,
            source: None,
            utf8: true,
            skips: Vec::new(),
            types: BTreeMap::new(),
            subpatterns: BTreeMap::new(),
            export: None,
        };
        let mut seen = BTreeMap::new();
        for attribute in attributes
            .iter()
            .filter(|attr| attr.path().is_ident("logos"))
        {
            let items =
                attribute.parse_args_with(Punctuated::<Item, Token![,]>::parse_terminated)?;
            for item in items {
                let (name, value) = item.0;
                if !matches!(value, Value::Skip(_) | Value::Named(_, _))
                    && seen.insert(name.to_string(), ()).is_some()
                {
                    return Err(Error::new_spanned(name, "duplicate Logos option"));
                }
                match (name.to_string().as_str(), value) {
                    ("crate", Value::Path(path)) => config.root = path,
                    ("extras", Value::Type(ty)) => config.extras = ty,
                    ("source", Value::Type(ty)) => config.source = Some(ty),
                    ("error", Value::Error(ty, callback)) => {
                        config.error = ty;
                        config.error_callback = callback;
                    }
                    ("utf8", Value::Bool(value)) => config.utf8 = value,
                    ("skip", Value::Skip(args)) => config.skips.push(args),
                    ("type", Value::Named(name, Named::Type(ty))) => {
                        if config.types.insert(name.to_string(), *ty).is_some() {
                            return Err(Error::new_spanned(name, "duplicate type substitution"));
                        }
                    }
                    ("subpattern", Value::Named(name, Named::Pattern(pattern))) => {
                        if config
                            .subpatterns
                            .insert(name.to_string(), pattern)
                            .is_some()
                        {
                            return Err(Error::new_spanned(name, "duplicate subpattern"));
                        }
                    }
                    ("export_dir", Value::Export(path)) => config.export = Some(path),
                    _ => return Err(Error::new_spanned(name, "unsupported Logos option")),
                }
            }
        }
        Ok(config)
    }
}

struct Item((Ident, Value));
enum Named {
    Type(Box<Type>),
    Pattern(Lit),
}
enum Value {
    Path(Path),
    Type(Type),
    Error(Type, Option<Expr>),
    Bool(bool),
    Skip(Arguments),
    Named(Ident, Named),
    Export(LitStr),
}

impl Parse for Item {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.call(Ident::parse_any)?;
        let value = match name.to_string().as_str() {
            "type" | "subpattern" => {
                let parameter = input.parse()?;
                input.parse::<Token![=]>()?;
                Value::Named(
                    parameter,
                    if name == "type" {
                        Named::Type(input.parse()?)
                    } else {
                        Named::Pattern(input.parse()?)
                    },
                )
            }
            "skip" => {
                let args = if input.peek(syn::token::Paren) {
                    let content;
                    syn::parenthesized!(content in input);
                    content.parse()?
                } else {
                    Arguments {
                        pattern: input.parse()?,
                        callback: None,
                        priority: None,
                        ignore_case: false,
                        allow_greedy: false,
                    }
                };
                Value::Skip(args)
            }
            "error" if input.peek(syn::token::Paren) => {
                let content;
                syn::parenthesized!(content in input);
                let ty = content.parse()?;
                let callback = if content.is_empty() {
                    None
                } else {
                    content.parse::<Token![,]>()?;
                    if content.peek(Ident) && content.peek2(Token![=]) {
                        let option: Ident = content.parse()?;
                        if option != "callback" {
                            return Err(Error::new_spanned(option, "expected callback"));
                        }
                        content.parse::<Token![=]>()?;
                    }
                    Some(content.parse()?)
                };
                if !content.is_empty() {
                    content.parse::<Token![,]>()?;
                }
                Value::Error(ty, callback)
            }
            "extras" | "source" | "error" => {
                input.parse::<Token![=]>()?;
                let ty = input.parse()?;
                if name == "error" {
                    Value::Error(ty, None)
                } else {
                    Value::Type(ty)
                }
            }
            "crate" => {
                input.parse::<Token![=]>()?;
                Value::Path(input.parse()?)
            }
            "utf8" => {
                input.parse::<Token![=]>()?;
                Value::Bool(input.parse::<LitBool>()?.value)
            }
            "export_dir" => {
                input.parse::<Token![=]>()?;
                Value::Export(input.parse()?)
            }
            _ => return Err(Error::new_spanned(name, "unsupported Logos option")),
        };
        Ok(Self((name, value)))
    }
}

use syn::ext::IdentExt;
