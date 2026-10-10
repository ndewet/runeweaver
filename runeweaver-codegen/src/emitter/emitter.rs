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
    let selector = super::selector::emit(dfa);
    let fused_skips = lexer
        .rules()
        .iter()
        .enumerate()
        .filter(|(_, rule)| {
            rule.action().is_skip() && matches!(rule.transition(), ResolvedTransition::Stay)
        })
        .map(|(index, _)| {
            quote! {
                #index if length != 0 => {
                    offset = end;
                    if offset == input.len() {
                        return Some(Ok((None, offset, #runtime::Transition::Stay)));
                    }
                }
            }
        });
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

    // The UTF-8 DFA accepts complete codepoints, so selected lengths end at character boundaries.
    // Each skipped prefix has the same guarantee; offset + length never exceeds input.len().
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

        #selector

        fn __runeweaver_scan_one(
            input: &str,
            start_condition: usize,
        ) -> Option<#runtime::RuleScan<Self>> {
            let mut offset = 0;
            loop {
                let Some((rule, length)) = Self::__runeweaver_select(
                    &input.as_bytes()[offset..],
                    start_condition,
                ) else {
                    return (offset != 0).then_some(Ok((
                        None,
                        offset,
                        #runtime::Transition::Stay,
                    )));
                };
                let rule = rule.get() - 1;
                let end = offset + length;
                match rule {
                    #(#fused_skips)*
                    _ => {
                        let text = unsafe { input.get_unchecked(offset..end) };
                        return Some(
                            Self::__runeweaver_action(rule, text)
                                .map(|(token, transition)| (token, end, transition))
                                .map_err(|error| (error, end)),
                        );
                    }
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

pub(super) fn local_width(dead: usize) -> TokenStream {
    if u8::try_from(dead).is_ok() {
        quote!(u8)
    } else if u16::try_from(dead).is_ok() {
        quote!(u16)
    } else if u32::try_from(dead).is_ok() {
        quote!(u32)
    } else {
        quote!(usize)
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
    fn local_width_includes_the_dead_value() {
        for (dead, expected) in [
            (0, "u8"),
            (255, "u8"),
            (256, "u16"),
            (65535, "u16"),
            (65536, "u32"),
            (u32::MAX as usize, "u32"),
        ] {
            assert_eq!(local_width(dead).to_string(), expected);
        }
        #[cfg(target_pointer_width = "64")]
        assert_eq!(local_width(u32::MAX as usize + 1).to_string(), "usize");
    }

    #[test]
    fn direct_transitions_inline_final_accepts() {
        let mut builder: Builder<ByteRange, RuleId> = Builder::new();
        let start = builder.add_state();
        let target = builder.add_state();
        builder.add_transition(start, ByteRange::new(b'a', b'z'), target);
        builder.set_accept(target, RuleId::new(0));
        let dfa = builder.build(&[start]).expect("the test DFA is valid");
        let emitted = emit(&dfa, &lexer(quote!(Rule::Token(7))), &quote!(::runeweaver)).to_string();
        assert!(emitted.contains("return Some ((:: core :: num :: NonZeroUsize :: new (1usize)"));
        assert!(!emitted.contains("latest = Some"));
        assert!(!emitted.contains("fn __runeweaver_state_"));
    }

    #[test]
    fn local_forks_select_all_targets_and_return_the_saved_match_on_failure() {
        let mut builder: Builder<ByteRange, RuleId> = Builder::new();
        let start = builder.add_state();
        for byte in b'a'..=b'c' {
            let target = builder.add_state();
            builder.add_transition(start, ByteRange::new(byte, byte), target);
            builder.set_accept(target, RuleId::new(usize::from(byte - b'a')));
        }
        let dfa = builder.build(&[start]).expect("the test DFA is valid");
        let emitted = emit(&dfa, &lexer(quote!(Rule::Token(7))), &quote!(::runeweaver)).to_string();
        assert!(emitted.contains("const FORK : & [u8 ; 256]"));
        for ordinal in 0..3 {
            assert!(emitted.contains(&format!(
                "{ordinal} => {{ return Some ((:: core :: num :: NonZeroUsize :: new ({}usize)",
                ordinal + 1
            )));
        }
        assert!(emitted.contains("3usize as u8"));
        assert!(
            emitted.contains("_ => return None :: < (:: core :: num :: NonZeroUsize , usize) >")
        );
        assert!(!emitted.contains("let next"));
    }

    #[test]
    fn start_conditions_call_their_regions() {
        let mut builder: Builder<ByteRange, RuleId> = Builder::new();
        let first = builder.add_state();
        let second = builder.add_state();
        let dfa = builder
            .build(&[second, first])
            .expect("the test DFA is valid");
        let emitted = emit(&dfa, &lexer(quote!(Rule::Token(7))), &quote!(::runeweaver)).to_string();
        assert!(emitted.contains("0usize => Self :: __runeweaver_region_1 (input , 0)"));
        assert!(emitted.contains("1usize => Self :: __runeweaver_region_0 (input , 0)"));
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
