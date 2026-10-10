//! Renders minimized lexer automata as Rust source.

use crate::automata::{dfa::Dfa, encoding::ByteRange};
use crate::lexer::{Lexer, ResolvedTransition, RuleId};
use proc_macro2::TokenStream;
use quote::quote;

/// Renders a minimized byte-oriented lexer DFA as Rust matcher tokens.
pub(crate) fn emit(
    dfa: &Dfa<ByteRange, RuleId>,
    lexer: &Lexer,
    runtime: &TokenStream,
) -> TokenStream {
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
    let selectors: Vec<_> = (0..dfa.state_count())
        .map(|index| {
            let state = crate::automata::StateId::new(index);
            let transitions = dfa.transitions(state);
            let accept = dfa.accept(state).map(|rule| {
                let rule = rule.index();
                quote! { latest = Some((#rule, index)); }
            });
            let self_transitions: Vec<_> = transitions
                .iter()
                .filter(|transition| transition.target == state)
                .collect();
            let fast_loop = (!self_transitions.is_empty()).then(|| {
                let table = (u8::MIN..=u8::MAX).map(|byte| {
                    self_transitions.iter().any(|transition| {
                        (transition.label.low..=transition.label.high).contains(&byte)
                    })
                });
                let tests = (0usize..8).map(|offset| {
                    quote! {
                        if SELF_LOOP[bytes[#offset] as usize] == 0 {
                            index += #offset;
                            break 'fast;
                        }
                    }
                });
                quote! {
                    const SELF_LOOP: &[u8; 256] = &[#(#table as u8),*];
                    'fast: while index + 8 <= input.len() {
                        let bytes: &[u8; 8] = input[index..index + 8]
                            .try_into()
                            .expect("an eight-byte slice has eight bytes");
                        #(#tests)*
                        index += 8;
                    }
                    while index < input.len() && SELF_LOOP[input[index] as usize] != 0 {
                        index += 1;
                    }
                    #accept
                }
            });
            let other_transitions: Vec<_> = transitions
                .iter()
                .filter(|transition| transition.target != state)
                .collect();
            let fork = if other_transitions.len() > 2 {
                let dead = dfa.state_count();
                let table = (u8::MIN..=u8::MAX).map(|byte| {
                    other_transitions
                        .iter()
                        .find(|transition| {
                            (transition.label.low..=transition.label.high).contains(&byte)
                        })
                        .map_or(dead, |transition| transition.target.index())
                });
                quote! {
                    const FORK: &[usize; 256] = &[#(#table),*];
                    let next = FORK[byte as usize];
                    if next != #dead {
                        state = next;
                        index += 1;
                        continue 'scan;
                    }
                }
            } else {
                let branches = other_transitions.iter().map(|transition| {
                    let low = transition.label.low;
                    let high = transition.label.high;
                    let target = transition.target.index();
                    quote! {
                        if (#low..=#high).contains(&byte) {
                            state = #target;
                            index += 1;
                            continue 'scan;
                        }
                    }
                });
                quote! { #(#branches)* }
            };
            quote! {
                #index => {
                    #accept
                    #fast_loop
                    if index == input.len() {
                        return latest;
                    }
                    let byte = input[index];
                    #fork
                    return latest;
                }
            }
        })
        .collect();
    let actions: Vec<_> = lexer
        .rules()
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let action = rule.action().rendered();
            let transition = match rule.transition() {
                ResolvedTransition::Stay => quote!(#runtime::Transition::Stay),
                ResolvedTransition::Begin(id) => {
                    let id = id.index();
                    quote!(#runtime::Transition::Begin(#id))
                }
                ResolvedTransition::Push(id) => {
                    let id = id.index();
                    quote!(#runtime::Transition::Push(#id))
                }
                ResolvedTransition::Pop => quote!(#runtime::Transition::Pop),
            };
            quote! {
                #index => {
                    let result: Result<Option<Self>, #runtime::PayloadError> = #action;
                    result
                        .map(|token| (token, #transition))
                        .map_err(|error| error.with_transition(#transition))
                },
            }
        })
        .collect();
    let mode_names = lexer
        .start_conditions()
        .iter()
        .enumerate()
        .map(|(index, mode)| {
            let name = mode.name();
            quote!(#index => #name,)
        });

    quote! {
        fn __runeweaver_start(start_condition: usize) -> Option<usize> {
            match start_condition {
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

        fn __runeweaver_select(
            input: &[u8],
            start_condition: usize,
        ) -> Option<(usize, usize)> {
            let mut state = Self::__runeweaver_start(start_condition)?;
            let mut index = 0;
            let mut latest = None;
            'scan: loop {
                match state {
                    #(#selectors)*
                    _ => return latest,
                }
            }
        }

        fn __runeweaver_action(
            rule: usize,
            text: &str,
        ) -> Result<(Option<Self>, #runtime::Transition), #runtime::PayloadError> {
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
    use crate::lexer::{Rule, RuleAction, StartCondition};
    use crate::regex::Expression;
    use std::str::FromStr;

    fn lexer(action: TokenStream) -> Lexer {
        Lexer::new(
            vec![Rule::new(
                Expression::from_str("a").expect("the test pattern is valid"),
                RuleAction::Emit(action),
                vec![crate::lexer::StartConditionId::new(0)],
                ResolvedTransition::Stay,
            )],
            vec![StartCondition::new("INITIAL")],
        )
    }

    #[test]
    fn emitting_a_dfa_without_transitions_stops_after_selecting_its_start() {
        let mut builder: Builder<ByteRange, RuleId> = Builder::new();
        let start = builder.add_state();
        let dfa = builder
            .build(&[start])
            .expect("the test DFA is below its capacity");

        let actual = emit(&dfa, &Lexer::new(vec![], vec![]), &quote!(::runeweaver)).to_string();
        assert!(actual.contains("0usize => Some (0usize)"));
        assert!(actual.contains("Result < (Option < Self > , :: runeweaver :: Transition)"));
        assert!(actual.contains("fn __runeweaver_mode_name"));
        assert!(actual.contains("fn __runeweaver_transition"));
        assert!(actual.contains("fn __runeweaver_accept"));
        assert!(actual.contains("fn __runeweaver_select"));
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
        assert!(emitted.contains("Transition :: Stay"));
    }
}
