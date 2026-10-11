//! Renders minimized lexer automata as Rust source.

use crate::automata::{dfa::Dfa, encoding::ByteRange};
use crate::compiler::{LexerDefinition, ResolvedModeAction, RuleId};
use proc_macro2::TokenStream;
use quote::quote;

/// Renders a minimized byte-oriented lexer DFA as Rust matcher tokens.
pub(crate) fn emit(
    dfa: &Dfa<ByteRange, RuleId>,
    lexer: &LexerDefinition,
    runtime: &TokenStream,
) -> TokenStream {
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
            let state = crate::automata::StateId::new(index);
            let transitions = dfa.transitions(state);
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
            let state = crate::automata::StateId::new(index);
            dfa.accept(state).map(|accept| {
                let accept = accept.index();
                quote! { #index => Some(#accept), }
            })
        })
        .collect();
    let actions: Vec<_> = lexer
        .rules()
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let action = rule.action().rendered();
            let mode_action = match rule.mode_action() {
                ResolvedModeAction::Stay => quote!(#lexer_runtime::ModeAction::Stay),
                ResolvedModeAction::Begin(id) => {
                    let id = id.index();
                    quote!(#lexer_runtime::ModeAction::Begin(#id))
                }
                ResolvedModeAction::Push(id) => {
                    let id = id.index();
                    quote!(#lexer_runtime::ModeAction::Push(#id))
                }
                ResolvedModeAction::Pop => quote!(#lexer_runtime::ModeAction::Pop),
            };
            quote! {
                #index => {
                    let result: Result<Option<Self>, #payload_runtime::PayloadError> = #action;
                    result
                        .map(|token| #lexer_runtime::RuleOutput {
                            token,
                            mode_action: #mode_action,
                        })
                        .map_err(|error| error.with_mode_action(#mode_action))
                },
            }
        })
        .collect();
    let mode_names = lexer.modes().iter().enumerate().map(|(index, mode)| {
        let name = mode.name();
        quote!(#index => #name,)
    });

    quote! {
        fn __runeweaver_start(mode: usize) -> Option<usize> {
            match mode {
                #(#starts)*
                _ => None,
            }
        }

        fn __runeweaver_transition(state: usize, byte: u8) -> Option<usize> {
            match state {
                #(#transitions)*
                _ => None,
            }
        }

        fn __runeweaver_accept(state: usize) -> Option<usize> {
            match state {
                #(#accepts)*
                _ => None,
            }
        }

        fn __runeweaver_action(
            rule: usize,
            text: &str,
        ) -> Result<#lexer_runtime::RuleOutput<Self>, #payload_runtime::PayloadError> {
            match rule {
                #(#actions)*
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
    use crate::automata::dfa::Builder;
    use crate::compiler::{Mode, Rule};
    use crate::regex::Expression;
    use crate::specification::RuleAction;
    use std::str::FromStr;

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
        assert!(actual.contains("Result < :: runeweaver :: lexer :: RuleOutput < Self >"));
        assert!(actual.contains("fn __runeweaver_mode_name"));
        assert!(actual.contains("fn __runeweaver_transition"));
        assert!(actual.contains("fn __runeweaver_accept"));
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
    }
}
