//! Renders completed lexer execution plans as Rust source.

use crate::automata::{StateId, dfa::Dfa, encoding::ByteRange};
use crate::compiler::{LexerDefinition, ResolvedModeAction, RuleId};
use crate::ir::Matcher;
use proc_macro2::TokenStream;
use quote::quote;

pub(crate) struct EmitOptions<'a> {
    pub runtime: &'a TokenStream,
    pub extras: &'a TokenStream,
    pub source_bounds: &'a TokenStream,
    pub uses_scanner: bool,
    pub uses_source: bool,
}

fn mode_action(action: ResolvedModeAction, lexer_runtime: &TokenStream) -> TokenStream {
    match action {
        ResolvedModeAction::Stay => quote!(#lexer_runtime::ModeAction::Stay),
        ResolvedModeAction::Begin(mode) => {
            let mode = mode.index();
            quote!(#lexer_runtime::ModeAction::Begin(#mode))
        }
        ResolvedModeAction::Push(mode) => {
            let mode = mode.index();
            quote!(#lexer_runtime::ModeAction::Push(#mode))
        }
        ResolvedModeAction::Pop => quote!(#lexer_runtime::ModeAction::Pop),
    }
}

fn action_arms(
    lexer: &LexerDefinition,
    lexer_runtime: &TokenStream,
    payload_runtime: &TokenStream,
) -> Vec<TokenStream> {
    lexer
        .rules()
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let action = rule.action().rendered();
            let mode_action = mode_action(rule.mode_action(), lexer_runtime);
            quote! {
                #index => {
                    let result: Result<Option<Self>, #payload_runtime::PayloadError> = #action;
                    (result, #mode_action)
                },
            }
        })
        .collect()
}

/// Renders a completed execution plan as Rust matcher tokens.
pub(crate) fn emit(
    dfa: &Dfa<ByteRange, RuleId>,
    matcher: &Matcher,
    lexer: &LexerDefinition,
    options: EmitOptions<'_>,
) -> TokenStream {
    let EmitOptions {
        runtime,
        extras,
        source_bounds,
        uses_scanner,
        uses_source,
    } = options;
    let lexer_runtime = quote!(#runtime::lexer);
    let payload_runtime = quote!(#runtime::payload);
    let starts: Vec<_> = dfa
        .start_states()
        .iter()
        .enumerate()
        .map(|(index, state)| {
            let state = state.index();
            quote! { #index => Some(#state), }
        })
        .collect();
    let transitions: Vec<_> = (0..dfa.state_count())
        .filter_map(|index| {
            let transitions = dfa.transitions(StateId::new(index));
            (!transitions.is_empty()).then(|| {
                let arms = transitions.iter().map(|transition| {
                    let low = transition.label.low;
                    let high = transition.label.high;
                    let target = transition.target.index();
                    quote! { #low..=#high => Some(#target), }
                });
                quote! {
                    #index => match byte {
                        #(#arms)*
                        _ => None,
                    },
                }
            })
        })
        .collect();
    let accepts: Vec<_> = (0..dfa.state_count())
        .filter_map(|index| {
            dfa.accept(StateId::new(index)).map(|accept| {
                let accept = accept.index();
                quote! { #index => Some(#accept), }
            })
        })
        .collect();
    let selector = crate::selector::emit(&matcher.selector);
    let scan_skips: Vec<_> = matcher
        .fused_skips
        .iter()
        .map(|index| {
            quote! {
                #index if length != 0 => {
                    cursor = end;
                }
            }
        })
        .collect();
    let actions = action_arms(lexer, &lexer_runtime, &payload_runtime);
    // Keep actions directly in the scanner so LLVM can specialize each lexer. The
    // dispatch function also supports the generated automaton's test interface.
    let direct_actions = actions.iter();
    let dispatched_actions = actions.iter();
    let mode_names = lexer.modes().iter().enumerate().map(|(index, mode)| {
        let name = mode.name();
        quote!(#index => #name,)
    });
    let scanner_generics = if uses_source {
        quote!(<'__runeweaver_source>)
    } else {
        quote!()
    };
    let source_lifetime = if uses_source {
        quote!('__runeweaver_source)
    } else {
        quote!('_)
    };
    let scanner_extras = uses_scanner
        .then_some(extras)
        .map_or_else(|| quote!(impl Sized), |extras| quote!(#extras));
    let scanner_bounds = uses_source.then(|| {
        quote! {
            where
                Self: #lexer_runtime::Automaton<'__runeweaver_source>,
                #source_bounds
        }
    });
    let action_text = if uses_source {
        quote!(&'__runeweaver_source str)
    } else {
        quote!(&str)
    };

    // The UTF-8 DFA accepts complete codepoints, so selected lengths end at character boundaries.
    // Each skipped prefix has the same guarantee; offset + length never exceeds input.len().
    quote! {
        #[cfg(test)]
        fn __runeweaver_start(mode: usize) -> Option<usize> {
            match mode {
                #(#starts)*
                _ => None,
            }
        }

        #[cfg(test)]
        fn __runeweaver_transition(state: usize, byte: u8) -> Option<usize> {
            match state {
                #(#transitions)*
                _ => None,
            }
        }

        #[cfg(test)]
        fn __runeweaver_accept(state: usize) -> Option<usize> {
            match state {
                #(#accepts)*
                _ => None,
            }
        }

        #selector

        #[inline(always)]
        fn __runeweaver_next #scanner_generics(
            scanner: &mut #runtime::Scanner<#source_lifetime, Self, #scanner_extras>,
            mode: usize,
        ) -> Option<#lexer_runtime::ScanMatch<Self>>
        #scanner_bounds
        {
            let input = #runtime::Scanner::source(scanner);
            let mut cursor = #runtime::Scanner::position(scanner);
            loop {
                if cursor == input.len() {
                    scanner.__set_position(cursor);
                    return None;
                }
                let Some((rule, length)) = Self::__runeweaver_select(
                    &input.as_bytes()[cursor..],
                    mode,
                ) else {
                    let start = cursor;
                    cursor += input[start..]
                        .chars()
                        .next()
                        .expect("the input is not empty")
                        .len_utf8();
                    scanner.__set_match(start, cursor);
                    return Some(Err((
                        #runtime::ScanError::unrecognized(scanner.span()),
                        #lexer_runtime::ModeAction::Stay,
                    )));
                };
                let rule = rule.get() - 1;
                let end = cursor + length;
                match rule {
                    #(#scan_skips)*
                    _ => {
                        let start = cursor;
                        let text = unsafe { input.get_unchecked(start..end) };
                        scanner.__set_match(start, end);
                        let result = match rule {
                            #(#direct_actions)*
                            _ => unreachable!("generated lexer selected an unknown rule"),
                        };
                        let (result, mode_action) = result;
                        return Some(match result {
                            Ok(token) => Ok((token, mode_action)),
                            Err(error) => Err((
                                #runtime::ScanError::invalid_payload(scanner.span(), error),
                                mode_action,
                            )),
                        });
                    }
                }
            }
        }

        #[cfg(test)]
        fn __runeweaver_action(
            rule: usize,
            text: #action_text,
            scanner: &mut #runtime::Scanner<#source_lifetime, Self, #scanner_extras>,
        ) -> (
            Result<Option<Self>, #payload_runtime::PayloadError>,
            #lexer_runtime::ModeAction,
        )
        #scanner_bounds
        {
            match rule {
                #(#dispatched_actions)*
                _ => unreachable!("generated lexer selected an unknown rule"),
            }
        }

        fn __runeweaver_mode_name(mode: usize) -> &'static str {
            match mode { #(#mode_names)* _ => "<unknown>", }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::{
        dfa::{Builder, Dfa},
        encoding::ByteRange,
    };
    use crate::compiler::{LexerDefinition, Mode, Rule, RuleId};
    use crate::regex::Expression;
    use crate::specification::RuleAction;
    use std::str::FromStr;

    fn emit(
        dfa: &Dfa<ByteRange, RuleId>,
        lexer: &LexerDefinition,
        runtime: &TokenStream,
    ) -> TokenStream {
        let extras = quote!(());
        let source_bounds = quote!();
        super::emit(
            dfa,
            &Matcher::new(dfa, []),
            lexer,
            EmitOptions {
                runtime,
                extras: &extras,
                source_bounds: &source_bounds,
                uses_scanner: false,
                uses_source: false,
            },
        )
    }

    fn lexer(action: TokenStream) -> LexerDefinition {
        LexerDefinition::new(
            vec![Rule::new(
                Expression::from_str("a").expect("the test pattern is valid"),
                RuleAction::Emit(action),
                vec![crate::compiler::ModeId::new(0)],
                ResolvedModeAction::Stay,
            )],
            vec![Mode::new("INITIAL")],
        )
    }

    #[test]
    fn emitting_a_dfa_without_transitions_stops_after_selecting_its_start() {
        let mut builder: Builder<ByteRange, RuleId> = Builder::new();
        let start = builder.add_state();
        let dfa = builder
            .build(&[start])
            .expect("the test DFA is below its capacity");

        let actual = emit(
            &dfa,
            &LexerDefinition::new(vec![], vec![]),
            &quote!(::runeweaver),
        )
        .to_string();
        assert!(actual.contains("0usize => Some (0usize)"));
        assert!(actual.contains("Option < :: runeweaver :: lexer :: ScanMatch < Self >>"));
        assert!(actual.contains("fn __runeweaver_mode_name"));
        assert!(actual.contains("fn __runeweaver_transition"));
        assert!(actual.contains("fn __runeweaver_accept"));
        assert!(actual.contains("fn __runeweaver_select"));
        assert!(actual.contains("Scanner < '_ , Self , impl Sized >"));
        assert!(!actual.contains("__runeweaver_select_spanned"));
    }

    #[test]
    fn emitting_a_transition_and_accept_selects_the_rule_before_its_action() {
        let mut builder: Builder<ByteRange, RuleId> = Builder::new();
        let start = builder.add_state();
        let accept = builder.add_state();
        builder.add_transition(start, ByteRange::new(b'a', b'z'), accept);
        builder.set_accept(accept, RuleId::new(0));
        let dfa = builder
            .build(&[start])
            .expect("the test DFA is below its capacity");
        let emitted = emit(&dfa, &lexer(quote!(Rule::Token(7))), &quote!(::runeweaver)).to_string();

        assert!(emitted.contains("97u8 ..= 122u8 => Some (1usize)"));
        assert!(emitted.contains("1usize => Some (0usize)"));
        assert!(emitted.contains("Rule :: Token (7)"));
        assert!(emitted.contains("ModeAction :: Stay"));
        let scan = emitted
            .split("fn __runeweaver_next")
            .nth(1)
            .expect("the generated scanner exists")
            .split("fn __runeweaver_action")
            .next()
            .expect("the generated action follows the scanner");
        assert!(scan.contains("let result = match rule"));
        assert!(!scan.contains("Self :: __runeweaver_action"));
    }
}
