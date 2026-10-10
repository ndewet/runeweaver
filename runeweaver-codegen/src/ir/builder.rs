//! Lowers graph regions into bounded selector control flow.

use super::context::Context;
use super::regions::Regions;
use super::selector::{
    Argument, Block, Call, Dispatch, Function, FunctionBody, MatchValue, Position, Selector,
    SelfLoop, Transfer, Width,
};
use crate::automata::{StateId, dfa::Dfa, encoding::ByteRange};
use crate::lexer::RuleId;
use std::collections::BTreeMap;

pub(super) struct Builder<'a> {
    dfa: &'a Dfa<ByteRange, RuleId>,
    regions: Regions,
    headers: Vec<Vec<usize>>,
}

impl<'a> Builder<'a> {
    pub(super) fn new(dfa: &'a Dfa<ByteRange, RuleId>) -> Self {
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
        Self {
            dfa,
            regions,
            headers,
        }
    }

    pub(super) fn build(&self) -> Selector {
        Selector {
            starts: self
                .dfa
                .start_states()
                .iter()
                .map(|state| self.call(state.index(), MatchValue::Empty, Position::Zero))
                .collect(),
            functions: (0..self.regions.regions.len())
                .map(|id| self.function(id))
                .collect(),
        }
    }

    fn function(&self, id: usize) -> Function {
        let region = &self.regions.regions[id];
        let mutable_context = region.cyclic
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
        let saved = inherited(region.context);
        let entry_parameter = region.cyclic && region.entries.len() != 1;
        let body = if region.cyclic && region.states.len() > 1 && self.headers[id].len() == 1 {
            FunctionBody::Loop(self.state(id, self.headers[id][0], saved))
        } else if region.cyclic && region.states.len() > 1 {
            FunctionBody::Dispatch {
                initial: (!entry_parameter).then(|| {
                    self.headers[id]
                        .binary_search(&region.entries[0])
                        .expect("the region contains its entry")
                }),
                arms: self.headers[id]
                    .iter()
                    .map(|&state| self.state(id, state, saved))
                    .collect(),
            }
        } else {
            FunctionBody::Linear(self.state(id, region.entries[0], saved))
        };
        Function {
            context: region.context,
            mutable_context,
            mutable_index: region
                .states
                .iter()
                .any(|&state| !self.dfa.transitions(StateId::new(state)).is_empty()),
            entry_parameter,
            body,
        }
    }

    fn call(&self, state: usize, saved: MatchValue, index: Position) -> Call {
        let id = self.regions.owner[state];
        let region = &self.regions.regions[id];
        let context = match (
            region.context,
            self.dfa.accept(StateId::new(state)).is_some(),
        ) {
            (
                Context::Fixed {
                    optional: false, ..
                },
                true,
            ) => Argument::ZeroEnd,
            (Context::Fixed { optional: true, .. }, true) => Argument::EmptyEnd,
            (Context::General, true) => Argument::EmptyMatch,
            (Context::Empty, _) => Argument::Omitted,
            (
                Context::Fixed {
                    optional: false, ..
                },
                false,
            ) => Argument::RequiredEnd(saved),
            (Context::Fixed { optional: true, .. }, false) => Argument::OptionalEnd(saved),
            (Context::General, false) => Argument::Match(saved),
        };
        Call {
            region: id,
            index,
            context,
            entry: (region.cyclic && region.entries.len() != 1).then(|| {
                self.headers[id]
                    .binary_search(&state)
                    .expect("the region contains its entry")
            }),
        }
    }

    fn state(&self, id: usize, state: usize, saved: MatchValue) -> Block {
        let state_id = StateId::new(state);
        let accept = self.dfa.accept(state_id).map(|rule| rule.index());
        let failure = accept.map_or(saved, |rule| MatchValue::Accepted {
            rule,
            end: Position::Current,
        });
        let transitions = self.dfa.transitions(state_id);
        if transitions.is_empty() {
            return Block::Return(failure);
        }
        let self_ranges: Vec<_> = transitions
            .iter()
            .filter(|edge| edge.target == state_id)
            .map(|edge| edge.label)
            .collect();
        let self_loop = (!self_ranges.is_empty()).then(|| SelfLoop {
            values: Box::new(std::array::from_fn(|byte| {
                self_ranges
                    .iter()
                    .any(|range| (usize::from(range.low)..=usize::from(range.high)).contains(&byte))
            })),
            chunk_size: super::limits::SELF_LOOP_CHUNK,
            probes: (0..super::limits::SELF_LOOP_CHUNK)
                .step_by(2)
                .map(|offset| (offset, offset + 1))
                .collect(),
        });
        let mut targets: BTreeMap<usize, Vec<ByteRange>> = BTreeMap::new();
        for edge in transitions.iter().filter(|edge| edge.target != state_id) {
            targets
                .entry(edge.target.index())
                .or_default()
                .push(edge.label);
        }
        let dispatch = if targets.is_empty() {
            Dispatch::Fail
        } else if targets.len() > 2 || targets.values().any(|ranges| ranges.len() > 2) {
            let dead = targets.len();
            Dispatch::Table {
                width: Width::for_dead(dead),
                values: Box::new(std::array::from_fn(|byte| {
                    targets
                        .values()
                        .position(|ranges| {
                            ranges.iter().any(|range| {
                                (usize::from(range.low)..=usize::from(range.high)).contains(&byte)
                            })
                        })
                        .unwrap_or(dead)
                })),
                branches: targets
                    .keys()
                    .map(|&target| self.transfer(id, target, failure, accept.is_some()))
                    .collect(),
            }
        } else {
            Dispatch::Ranges(
                targets
                    .into_iter()
                    .map(|(target, ranges)| {
                        (ranges, self.transfer(id, target, failure, accept.is_some()))
                    })
                    .collect(),
            )
        };
        Block::Scan {
            self_loop,
            failure,
            dispatch,
        }
    }

    fn transfer(&self, id: usize, target: usize, saved: MatchValue, accepts: bool) -> Transfer {
        let target_id = StateId::new(target);
        let region = &self.regions.regions[id];
        if self.dfa.transitions(target_id).is_empty() {
            return Transfer::Return(self.dfa.accept(target_id).map_or(saved, |rule| {
                MatchValue::Accepted {
                    rule: rule.index(),
                    end: Position::Next,
                }
            }));
        }
        if self.regions.owner[target] != id {
            return Transfer::Call(self.call(target, saved, Position::Next));
        }
        if region.cyclic && self.headers[id].binary_search(&target).is_ok() {
            let ordinal = self.headers[id]
                .binary_search(&target)
                .expect("the target is a loop header");
            let update = ((accepts || saved != inherited(region.context))
                && self.dfa.accept(target_id).is_none()
                && region.context != Context::Empty)
                .then_some((region.context, saved));
            Transfer::Continue {
                update,
                header: (self.headers[id].len() > 1).then_some(ordinal),
            }
        } else {
            let needs_saved = self.dfa.accept(target_id).is_none()
                && self.regions.incoming[target].iter().any(Option::is_some);
            Transfer::Inline {
                bind: needs_saved.then_some(saved),
                body: Box::new(self.state(
                    id,
                    target,
                    if needs_saved {
                        MatchValue::Local
                    } else {
                        MatchValue::Empty
                    },
                )),
            }
        }
    }
}

fn inherited(context: Context) -> MatchValue {
    match context {
        Context::Empty => MatchValue::Empty,
        _ => MatchValue::Context(context),
    }
}

#[cfg(test)]
#[path = "builder_tests.rs"]
mod tests;
