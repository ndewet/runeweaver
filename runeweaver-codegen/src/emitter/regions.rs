//! Groups DFA cycles and shared paths into execution regions.

use crate::automata::{StateId, dfa::Dfa, encoding::ByteRange};
use crate::lexer::RuleId;
use std::collections::{BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Context {
    Empty,
    Fixed { rule: usize, optional: bool },
    General,
}

#[derive(Debug)]
pub(super) struct Region {
    pub states: Vec<usize>,
    pub entries: Vec<usize>,
    pub cyclic: bool,
    pub context: Context,
}

#[derive(Debug)]
pub(super) struct Regions {
    pub regions: Vec<Region>,
    pub owner: Vec<usize>,
    pub incoming: Vec<BTreeSet<Option<usize>>>,
}

impl Regions {
    pub(super) fn new(dfa: &Dfa<ByteRange, RuleId>) -> Self {
        let count = dfa.state_count();
        let successors: Vec<Vec<usize>> = (0..count)
            .map(|index| {
                dfa.transitions(StateId::new(index))
                    .iter()
                    .map(|edge| edge.target.index())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            })
            .collect();
        let mut predecessors = vec![BTreeSet::new(); count];
        for (source, targets) in successors.iter().enumerate() {
            for &target in targets {
                predecessors[target].insert(source);
            }
        }
        let mut visited = vec![false; count];
        let mut finish = Vec::with_capacity(count);
        for root in 0..count {
            if visited[root] {
                continue;
            }
            visited[root] = true;
            let mut stack = vec![(root, 0)];
            while let Some((state, next)) = stack.last_mut() {
                if *next == successors[*state].len() {
                    finish.push(*state);
                    stack.pop();
                } else {
                    let target = successors[*state][*next];
                    *next += 1;
                    if !visited[target] {
                        visited[target] = true;
                        stack.push((target, 0));
                    }
                }
            }
        }
        let mut components = Vec::new();
        let mut component = vec![usize::MAX; count];
        for &root in finish.iter().rev() {
            if component[root] != usize::MAX {
                continue;
            }
            let id = components.len();
            let mut states = Vec::new();
            let mut stack = vec![root];
            component[root] = id;
            while let Some(state) = stack.pop() {
                states.push(state);
                for &source in &predecessors[state] {
                    if component[source] == usize::MAX {
                        component[source] = id;
                        stack.push(source);
                    }
                }
            }
            states.sort_unstable();
            components.push(states);
        }
        let starts: BTreeSet<_> = dfa.start_states().iter().map(|s| s.index()).collect();
        let cyclic_components: Vec<_> = components
            .iter()
            .map(|states| states.len() > 1 || successors[states[0]].contains(&states[0]))
            .collect();
        let mut regions = Vec::new();
        let mut owner = vec![usize::MAX; count];
        for states in &components {
            let first = states[0];
            let cyclic = cyclic_components[component[first]];
            if cyclic {
                let id = regions.len();
                let mut entries: Vec<_> = states
                    .iter()
                    .copied()
                    .filter(|state| {
                        starts.contains(state)
                            || predecessors[*state]
                                .iter()
                                .any(|source| component[*source] != component[*state])
                    })
                    .collect();
                if entries.is_empty() {
                    entries.push(first);
                }
                for &state in states {
                    owner[state] = id;
                }
                regions.push(Region {
                    states: states.clone(),
                    entries,
                    cyclic,
                    context: Context::Empty,
                });
            }
        }
        for state in 0..count {
            if owner[state] == usize::MAX
                && (starts.contains(&state)
                    || predecessors[state].len() != 1
                    || cyclic_components
                        [component[*predecessors[state].first().expect("one predecessor exists")]])
            {
                let id = regions.len();
                owner[state] = id;
                regions.push(Region {
                    states: vec![state],
                    entries: vec![state],
                    cyclic: false,
                    context: Context::Empty,
                });
            }
        }
        let mut depth = vec![0; count];
        for state in 0..count {
            if owner[state] != usize::MAX {
                continue;
            }
            let mut path = Vec::new();
            let mut source = state;
            while owner[source] == usize::MAX {
                path.push(source);
                source = *predecessors[source]
                    .first()
                    .expect("an unassigned acyclic state has one predecessor");
            }
            let mut id = owner[source];
            for member in path.into_iter().rev() {
                depth[member] = depth[source] + 1;
                if depth[member] == 64 {
                    id = regions.len();
                    depth[member] = 0;
                    regions.push(Region {
                        states: Vec::new(),
                        entries: vec![member],
                        cyclic: false,
                        context: Context::Empty,
                    });
                }
                owner[member] = id;
                regions[id].states.push(member);
                source = member;
            }
        }
        let mut incoming = vec![BTreeSet::new(); count];
        let mut queue = VecDeque::new();
        let mut queued = vec![false; count];
        for &start in &starts {
            incoming[start].insert(None);
            queue.push_back(start);
            queued[start] = true;
        }
        while let Some(source) = queue.pop_front() {
            queued[source] = false;
            let outgoing = dfa.accept(StateId::new(source)).map_or_else(
                || incoming[source].clone(),
                |rule| BTreeSet::from([Some(rule.index())]),
            );
            for &target in &successors[source] {
                let old = incoming[target].len();
                incoming[target].extend(&outgoing);
                if incoming[target].len() != old && !queued[target] {
                    queued[target] = true;
                    queue.push_back(target);
                }
            }
        }
        for region in &mut regions {
            let live: BTreeSet<_> = region
                .states
                .iter()
                .filter(|&&state| {
                    (region.cyclic || region.entries.contains(&state))
                        && dfa.accept(StateId::new(state)).is_none()
                })
                .flat_map(|&state| incoming[state].iter().copied())
                .collect();
            let rules: Vec<_> = live.iter().filter_map(|rule| *rule).collect();
            region.context = match rules.as_slice() {
                [] => Context::Empty,
                [rule] => Context::Fixed {
                    rule: *rule,
                    optional: live.contains(&None),
                },
                _ => Context::General,
            };
            region.states.sort_unstable();
        }
        Self {
            regions,
            owner,
            incoming,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::Builder;

    fn edge(builder: &mut Builder<ByteRange, RuleId>, from: StateId, byte: u8, to: StateId) {
        builder.add_transition(from, ByteRange::new(byte, byte), to);
    }

    #[test]
    fn a_cycle_stays_in_one_region_and_a_unique_path_stays_in_its_parent() {
        let mut builder = Builder::new();
        let states: Vec<_> = (0..5).map(|_| builder.add_state()).collect();
        edge(&mut builder, states[0], b'a', states[1]);
        edge(&mut builder, states[1], b'b', states[2]);
        edge(&mut builder, states[2], b'c', states[3]);
        edge(&mut builder, states[3], b'd', states[2]);
        edge(&mut builder, states[3], b'e', states[4]);
        let dfa = builder.build(&[states[0]]).expect("the test DFA is valid");
        let regions = Regions::new(&dfa);
        assert_eq!(regions.owner[0], regions.owner[1]);
        assert_eq!(regions.owner[2], regions.owner[3]);
        assert_ne!(regions.owner[1], regions.owner[2]);
        assert_ne!(regions.owner[3], regions.owner[4]);
        assert!(regions.regions[regions.owner[2]].cyclic);
    }

    fn joined_context(first: Option<usize>, second: Option<usize>) -> Context {
        let mut builder = Builder::new();
        let states: Vec<_> = (0..4).map(|_| builder.add_state()).collect();
        edge(&mut builder, states[0], b'a', states[1]);
        edge(&mut builder, states[0], b'b', states[2]);
        edge(&mut builder, states[1], b'c', states[3]);
        edge(&mut builder, states[2], b'c', states[3]);
        if let Some(rule) = first {
            builder.set_accept(states[1], RuleId::new(rule));
        }
        if let Some(rule) = second {
            builder.set_accept(states[2], RuleId::new(rule));
        }
        let dfa = builder.build(&[states[0]]).expect("the test DFA is valid");
        let regions = Regions::new(&dfa);
        regions.regions[regions.owner[3]].context
    }

    #[test]
    fn joins_distinguish_empty_fixed_optional_and_general_matches() {
        assert_eq!(joined_context(None, None), Context::Empty);
        assert_eq!(
            joined_context(Some(0), Some(0)),
            Context::Fixed {
                rule: 0,
                optional: false
            }
        );
        assert_eq!(
            joined_context(Some(0), None),
            Context::Fixed {
                rule: 0,
                optional: true
            }
        );
        assert_eq!(joined_context(Some(0), Some(1)), Context::General);
    }

    #[test]
    fn an_accept_inside_a_cycle_makes_its_rule_live_at_nonaccept_states() {
        let mut builder = Builder::new();
        let first = builder.add_state();
        let second = builder.add_state();
        edge(&mut builder, first, b'a', second);
        edge(&mut builder, second, b'b', first);
        builder.set_accept(first, RuleId::new(3));
        let dfa = builder.build(&[first]).expect("the test DFA is valid");
        let regions = Regions::new(&dfa);
        assert_eq!(regions.regions.len(), 1);
        assert_eq!(
            regions.regions[0].context,
            Context::Fixed {
                rule: 3,
                optional: false
            }
        );
    }

    #[test]
    fn every_external_entry_into_a_cycle_is_retained() {
        let mut builder = Builder::new();
        let start = builder.add_state();
        let first = builder.add_state();
        let second = builder.add_state();
        edge(&mut builder, start, b'a', first);
        edge(&mut builder, start, b'b', second);
        edge(&mut builder, first, b'c', second);
        edge(&mut builder, second, b'd', first);
        let dfa = builder.build(&[start]).expect("the test DFA is valid");
        let regions = Regions::new(&dfa);
        assert_eq!(
            regions.regions[regions.owner[first.index()]].entries,
            vec![1, 2]
        );
    }

    #[test]
    fn long_acyclic_paths_have_bounded_region_depth() {
        let mut builder = Builder::new();
        let states: Vec<_> = (0..1024).map(|_| builder.add_state()).collect();
        for pair in states.windows(2) {
            edge(&mut builder, pair[0], b'a', pair[1]);
        }
        let dfa = builder.build(&[states[0]]).expect("the test DFA is valid");
        let regions = Regions::new(&dfa);
        assert!(
            regions
                .regions
                .iter()
                .all(|region| region.states.len() <= 64)
        );
        assert!(regions.regions.iter().all(|region| !region.cyclic));
    }
}
