use crate::support::generation::Input;
use quote::quote;
use runeweaver_codegen::{ModeAction, RuleSpec};
use syn::parse::{Parse, ParseStream};
use syn::{Data, DeriveInput, Expr, Fields, LitStr, Type};

struct Rule {
    modes: Vec<String>,
    mode_action: ModeAction,
    converter: Option<Expr>,
}

impl Rule {
    fn stay() -> Self {
        Self {
            modes: vec!["INITIAL".into()],
            mode_action: ModeAction::Stay,
            converter: None,
        }
    }
}

struct LexerAttribute {
    name: String,
    mode: Option<String>,
    pattern: Option<LitStr>,
    rule: Rule,
}

struct TokenAttribute {
    pattern: LitStr,
    rule: Rule,
}

impl Parse for TokenAttribute {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            pattern: input.parse()?,
            rule: parse_rule(input)?,
        })
    }
}

impl Parse for LexerAttribute {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        input.parse::<syn::Token![=]>()?;

        if name == "mode" {
            let mode: syn::Ident = input.parse()?;
            return Ok(Self {
                name: name.to_string(),
                mode: Some(mode.to_string()),
                pattern: None,
                rule: Rule::stay(),
            });
        }

        let pattern = input.parse()?;
        Ok(Self {
            name: name.to_string(),
            mode: None,
            pattern: Some(pattern),
            rule: parse_rule(input)?,
        })
    }
}

fn parse_rule(input: ParseStream<'_>) -> syn::Result<Rule> {
    let mut rule = Rule::stay();
    while !input.is_empty() {
        input.parse::<syn::Token![,]>()?;
        let name: syn::Ident = input.parse()?;
        match name.to_string().as_str() {
            "modes" => {
                input.parse::<syn::Token![=]>()?;
                rule.modes = parse_modes(input)?;
            }
            "push" => {
                input.parse::<syn::Token![=]>()?;
                let mode: syn::Ident = input.parse()?;
                rule.mode_action = ModeAction::Push(mode.to_string());
            }
            "begin" => {
                input.parse::<syn::Token![=]>()?;
                let mode: syn::Ident = input.parse()?;
                rule.mode_action = ModeAction::Begin(mode.to_string());
            }
            "with" => {
                input.parse::<syn::Token![=]>()?;
                rule.converter = Some(input.parse()?);
            }
            "pop" => rule.mode_action = ModeAction::Pop,
            _ => return Err(input.error("unsupported lexer rule modifier")),
        }
    }
    Ok(rule)
}

fn parse_modes(input: ParseStream<'_>) -> syn::Result<Vec<String>> {
    if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let modes = content.parse_terminated(syn::Ident::parse, syn::Token![,])?;
        Ok(modes.into_iter().map(|mode| mode.to_string()).collect())
    } else {
        let mode: syn::Ident = input.parse()?;
        Ok(vec![mode.to_string()])
    }
}

fn configure(spec: RuleSpec, rule: Rule) -> RuleSpec {
    spec.in_modes(rule.modes).mode_action(rule.mode_action)
}

pub fn parse(source: &str) -> Input {
    let input: DeriveInput = syn::parse_str(source).expect("the benchmark token definition parses");
    let Data::Enum(data) = input.data else {
        panic!("a benchmark token definition must be an enum");
    };

    let mut modes = Vec::new();
    let mut rules = Vec::new();
    for attribute in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("lexer"))
    {
        let attribute: LexerAttribute = attribute
            .parse_args()
            .expect("the benchmark lexer attribute parses");
        match attribute.name.as_str() {
            "mode" => modes.push(attribute.mode.expect("a mode has a name")),
            "skip" => rules.push(configure(
                RuleSpec::skip(attribute.pattern.expect("a skip has a pattern").value()),
                attribute.rule,
            )),
            _ => panic!("unsupported benchmark lexer attribute"),
        }
    }

    for variant in data.variants {
        let attribute = variant
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("token"))
            .expect("a benchmark token variant has a token attribute");
        let attribute: TokenAttribute = attribute
            .parse_args()
            .expect("the benchmark token attribute parses");
        let pattern = attribute.pattern.value();
        let ident = variant.ident;
        let action = match variant.fields {
            Fields::Unit => quote!(Ok(Some(Self::#ident))),
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let payload = &fields.unnamed[0].ty;
                if matches!(payload, Type::Reference(_)) {
                    panic!("benchmark token payloads must be owned");
                }
                if let Some(converter) = attribute.rule.converter.as_ref() {
                    quote! {
                        ::runeweaver::payload::PayloadResult::<#payload>::into_payload(
                            (#converter)(text),
                        )
                        .map(|payload| Some(Self::#ident(payload)))
                    }
                } else {
                    quote! {
                        ::runeweaver::parse_payload::<#payload>(text)
                            .map(|payload| Some(Self::#ident(payload)))
                    }
                }
            }
            _ => panic!("a benchmark token variant must have zero or one tuple field"),
        };
        rules.push(configure(RuleSpec::emit(pattern, action), attribute.rule));
    }

    Input { modes, rules }
}
