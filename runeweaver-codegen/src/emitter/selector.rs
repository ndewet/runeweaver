//! Emits region-local loops and acyclic control flow.

use super::regions::{Context, Regions};
use crate::automata::{StateId, dfa::Dfa, encoding::ByteRange};
use crate::lexer::RuleId;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::BTreeMap;

pub(super) fn emit(dfa: &Dfa<ByteRange, RuleId>) -> TokenStream {
    let regions = Regions::new(dfa);
    let headers = regions
        .regions
        .iter()
        .map(|region| {
            if region.cyclic && region.states.len() > 1 {
                super::cycles::headers(dfa, region)
            } else {
                region.entries.clone()
            }
        })
        .collect();
    let generator = Selector {
        dfa,
        regions,
        headers,
    };
    let starts = dfa.start_states().iter().enumerate().map(|(mode, state)| {
        let call = generator.call(
            state.index(),
            &quote!(None::<(::core::num::NonZeroUsize, usize)>),
            &quote!(0),
        );
        quote! { #mode => #call, }
    });
    let functions = (0..generator.regions.regions.len()).map(|id| generator.function(id));
    quote! {
        fn __runeweaver_select(input: &[u8], start_condition: usize) -> Option<(::core::num::NonZeroUsize, usize)> {
            match start_condition {
                #(#starts)*
                _ => None,
            }
        }
        #(#functions)*
    }
}

struct Selector<'a> {
    dfa: &'a Dfa<ByteRange, RuleId>,
    regions: Regions,
    headers: Vec<Vec<usize>>,
}

impl Selector<'_> {
    fn function(&self, id: usize) -> TokenStream {
        let region = &self.regions.regions[id];
        let name = format_ident!("__runeweaver_region_{id}");
        let changes_context = region.cyclic
            && (self.headers[id].len() < region.states.len()
                && self.headers[id]
                    .iter()
                    .any(|&state| self.dfa.accept(StateId::new(state)).is_none())
                || region.states.iter().any(|&state| {
                    self.dfa.accept(StateId::new(state)).is_some()
                        && self
                            .dfa
                            .transitions(StateId::new(state))
                            .iter()
                            .any(|edge| {
                                self.regions.owner[edge.target.index()] == id
                                    && self.dfa.accept(edge.target).is_none()
                            })
                }));
        let context_mut = changes_context.then(|| quote!(mut));
        let context_parameter = match region.context {
            Context::Empty => quote!(),
            Context::Fixed {
                optional: false, ..
            } => quote!(, #context_mut saved_end: usize),
            Context::Fixed { optional: true, .. } => {
                quote!(, #context_mut saved_end: Option<usize>)
            }
            Context::General => {
                quote!(, #context_mut latest: Option<(::core::num::NonZeroUsize, usize)>)
            }
        };
        let saved = match region.context {
            Context::Empty => quote!(None::<(::core::num::NonZeroUsize, usize)>),
            Context::Fixed {
                rule,
                optional: false,
            } => {
                let rule = rule_tag(rule);
                quote!(Some((#rule, saved_end)))
            }
            Context::Fixed {
                rule,
                optional: true,
            } => {
                let rule = rule_tag(rule);
                quote!(saved_end.map(|end| (#rule, end)))
            }
            Context::General => quote!(latest),
        };
        let index_mut = region
            .states
            .iter()
            .any(|&state| !self.dfa.transitions(StateId::new(state)).is_empty())
            .then(|| quote!(mut));
        let multi_entry = region.cyclic && region.entries.len() != 1;
        let entry_parameter = multi_entry.then(|| quote!(, mut state: usize));
        let body = if region.cyclic && region.states.len() > 1 && self.headers[id].len() == 1 {
            let code = self.state(id, self.headers[id][0], &saved);
            quote! { 'region: loop { #code } }
        } else if region.cyclic && region.states.len() > 1 {
            let init = (!multi_entry).then(|| {
                let state = self.headers[id]
                    .binary_search(&region.entries[0])
                    .expect("the region contains its entry");
                quote!(let mut state = #state;)
            });
            let arms = self.headers[id]
                .iter()
                .enumerate()
                .map(|(ordinal, &state)| {
                    let code = self.state(id, state, &saved);
                    quote! { #ordinal => { #code }, }
                });
            quote! {
                #init
                'region: loop {
                    match state {
                        #(#arms)*
                        _ => unreachable!("generated region selected an unknown state"),
                    }
                }
            }
        } else {
            self.state(id, region.entries[0], &saved)
        };
        quote! {
            fn #name(input: &[u8], #index_mut index: usize #context_parameter #entry_parameter)
                -> Option<(::core::num::NonZeroUsize, usize)>
            {
                #body
            }
        }
    }

    fn call(&self, state: usize, saved: &TokenStream, index: &TokenStream) -> TokenStream {
        let id = self.regions.owner[state];
        let region = &self.regions.regions[id];
        let name = format_ident!("__runeweaver_region_{id}");
        let context = match (
            region.context,
            self.dfa.accept(StateId::new(state)).is_some(),
        ) {
            (
                Context::Fixed {
                    optional: false, ..
                },
                true,
            ) => quote!(, 0),
            (Context::Fixed { optional: true, .. }, true) => quote!(, None),
            (Context::General, true) => quote!(, None),
            (Context::Empty, _) => quote!(),
            (
                Context::Fixed {
                    optional: false, ..
                },
                false,
            ) => quote!(, (#saved).expect("the saved rule is live").1),
            (Context::Fixed { optional: true, .. }, false) => {
                quote!(, (#saved).map(|(_, end)| end))
            }
            (Context::General, false) => quote!(, #saved),
        };
        let entry = (region.cyclic && region.entries.len() != 1).then(|| {
            let ordinal = self.headers[id]
                .binary_search(&state)
                .expect("the region contains its entry");
            quote!(, #ordinal)
        });
        quote!(Self::#name(input, #index #context #entry))
    }

    fn state(&self, id: usize, state: usize, saved: &TokenStream) -> TokenStream {
        let state_id = StateId::new(state);
        let accept = self.dfa.accept(state_id).map(|rule| rule.index());
        let failure = accept.map_or_else(
            || saved.clone(),
            |rule| {
                let rule = rule_tag(rule);
                quote!(Some((#rule, index)))
            },
        );
        let transitions = self.dfa.transitions(state_id);
        if transitions.is_empty() {
            return quote!(return #failure;);
        }
        let self_ranges: Vec<_> = transitions
            .iter()
            .filter(|edge| edge.target == state_id)
            .map(|edge| edge.label)
            .collect();
        let fast_loop = (!self_ranges.is_empty()).then(|| {
            let table = (u8::MIN..=u8::MAX).map(|byte| {
                self_ranges
                    .iter()
                    .any(|range| (range.low..=range.high).contains(&byte))
            });
            let tests = (0usize..8).step_by(2).map(|offset| {
                let next = offset + 1;
                quote! {
                    if SELF_LOOP[bytes[#offset] as usize] & SELF_LOOP[bytes[#next] as usize] == 0 {
                        index += #offset;
                        break 'fast;
                    }
                }
            });
            quote! {
                const SELF_LOOP: &[u8; 256] = &[#(#table as u8),*];
                'fast: while index + 8 <= input.len() {
                    let bytes: &[u8; 8] = input[index..index + 8]
                        .try_into().expect("an eight-byte slice has eight bytes");
                    #(#tests)*
                    index += 8;
                }
                while index < input.len() && SELF_LOOP[input[index] as usize] != 0 {
                    index += 1;
                }
            }
        });
        let mut targets: BTreeMap<usize, Vec<ByteRange>> = BTreeMap::new();
        for edge in transitions.iter().filter(|edge| edge.target != state_id) {
            targets
                .entry(edge.target.index())
                .or_default()
                .push(edge.label);
        }
        let fork = if targets.len() > 2 || targets.values().any(|ranges| ranges.len() > 2) {
            let dead = targets.len();
            let width = super::emitter::local_width(dead);
            let table = (u8::MIN..=u8::MAX).map(|byte| {
                let ordinal = targets
                    .values()
                    .position(|ranges| {
                        ranges
                            .iter()
                            .any(|range| (range.low..=range.high).contains(&byte))
                    })
                    .unwrap_or(dead);
                quote!(#ordinal as #width)
            });
            let branches = targets.keys().enumerate().map(|(ordinal, &target)| {
                let ordinal = proc_macro2::Literal::usize_unsuffixed(ordinal);
                let code = self.transfer(id, target, &failure, accept.is_some());
                quote!(#ordinal => { #code },)
            });
            quote! {
                const FORK: &[#width; 256] = &[#(#table),*];
                match FORK[byte as usize] {
                    #(#branches)*
                    _ => return #failure,
                }
            }
        } else {
            let branches = targets.iter().map(|(&target, ranges)| {
                let tests = ranges.iter().map(|range| {
                    let low = range.low;
                    let high = range.high;
                    quote!((#low..=#high).contains(&byte))
                });
                let code = self.transfer(id, target, &failure, accept.is_some());
                quote! { if false #(|| #tests)* { #code } }
            });
            quote! { #(#branches)* return #failure; }
        };
        let read = (!targets.is_empty()).then(|| {
            quote! {
                let byte = input[index];
            }
        });
        quote! {
            #fast_loop
            if index == input.len() { return #failure; }
            #read
            #fork
        }
    }

    fn transfer(
        &self,
        id: usize,
        target: usize,
        saved: &TokenStream,
        accepts: bool,
    ) -> TokenStream {
        let target_id = StateId::new(target);
        let region = &self.regions.regions[id];
        if self.dfa.transitions(target_id).is_empty() {
            return self.dfa.accept(target_id).map_or_else(
                || quote!(return #saved;),
                |rule| {
                    let rule = rule_tag(rule.index());
                    quote!(return Some((#rule, index + 1));)
                },
            );
        }
        if self.regions.owner[target] != id {
            let call = self.call(target, saved, &quote!(index + 1));
            return quote!(return #call;);
        }
        if region.cyclic && self.headers[id].binary_search(&target).is_ok() {
            let ordinal = self.headers[id]
                .binary_search(&target)
                .expect("the target is a loop header");
            let inherited = match region.context {
                Context::Empty => quote!(None::<(::core::num::NonZeroUsize, usize)>),
                Context::Fixed {
                    rule,
                    optional: false,
                } => {
                    let rule = rule_tag(rule);
                    quote!(Some((#rule, saved_end)))
                }
                Context::Fixed {
                    rule,
                    optional: true,
                } => {
                    let rule = rule_tag(rule);
                    quote!(saved_end.map(|end| (#rule, end)))
                }
                Context::General => quote!(latest),
            };
            let update = if (accepts || saved.to_string() != inherited.to_string())
                && self.dfa.accept(target_id).is_none()
            {
                match region.context {
                    Context::Empty => quote!(),
                    Context::Fixed {
                        optional: false, ..
                    } => quote!(saved_end = (#saved).expect("the saved rule is live").1;),
                    Context::Fixed { optional: true, .. } => {
                        quote!(saved_end = (#saved).map(|(_, end)| end);)
                    }
                    Context::General => quote!(latest = #saved;),
                }
            } else {
                quote!()
            };
            let select = (self.headers[id].len() > 1).then(|| quote!(state = #ordinal;));
            quote! {
                #update
                index += 1;
                #select
                continue 'region;
            }
        } else {
            let needs_saved = self.dfa.accept(target_id).is_none()
                && self.regions.incoming[target].iter().any(Option::is_some);
            let bind = needs_saved.then(|| quote!(let saved = #saved;));
            let context = if needs_saved {
                quote!(saved)
            } else {
                quote!(None::<(::core::num::NonZeroUsize, usize)>)
            };
            let code = self.state(id, target, &context);
            quote! { #bind index += 1; #code }
        }
    }
}

fn rule_tag(rule: usize) -> TokenStream {
    let tag = rule
        .checked_add(1)
        .expect("a generated rule index fits its tag");
    quote!(::core::num::NonZeroUsize::new(#tag).expect("a generated rule tag is nonzero"))
}
