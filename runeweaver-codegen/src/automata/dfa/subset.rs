use std::collections::HashMap;

use crate::automata::{
    BuildError, StateId,
    dfa::{Builder, Dfa},
    label::Partitionable,
    nfa::{Nfa, epsilon_closure},
    state_set::StateSet,
};

pub(crate) fn construct<L, A, B>(
    nfa: &Nfa<L, A>,
    select: impl FnMut(&[StateId]) -> B,
) -> Result<Dfa<L, B>, BuildError>
where
    L: Partitionable,
{
    let mut construction = Construction::new(nfa, select);
    let starts = construction.seed();
    construction.expand();
    construction.builder.build(&starts)
}

struct Construction<'a, L, A, B, F> {
    nfa: &'a Nfa<L, A>,
    builder: Builder<L, B>,
    selector: F,
    subsets: Vec<Subset>,
    state_by_subset: HashMap<Subset, StateId>,
    next_subset: usize,
    closure: StateSet,
    targets: Vec<StateId>,
}

type Subset = Vec<StateId>;

impl<'a, L, A, B, F> Construction<'a, L, A, B, F>
where
    L: Partitionable,
    F: FnMut(&[StateId]) -> B,
{
    fn new(nfa: &'a Nfa<L, A>, selector: F) -> Self {
        Self {
            nfa,
            builder: Builder::new(),
            selector,
            subsets: Vec::new(),
            state_by_subset: HashMap::new(),
            next_subset: 0,
            closure: StateSet::new(nfa.state_count()),
            targets: Vec::new(),
        }
    }

    fn intern(&mut self, subset: Subset) -> StateId {
        if let Some(existing) = self.state_by_subset.get(&subset) {
            return *existing;
        }
        let state = self.builder.add_state();
        let subset_is_accepting = subset
            .iter()
            .any(|state_id| self.nfa.accept(*state_id).is_some());
        if subset_is_accepting {
            let accept = (self.selector)(&subset);
            self.builder.set_accept(state, accept);
        }
        self.state_by_subset.insert(subset.clone(), state);
        self.subsets.push(subset);
        state
    }

    fn seed(&mut self) -> Vec<StateId> {
        self.nfa
            .start_states()
            .iter()
            .map(|&start| self.intern_closure([start]))
            .collect()
    }

    fn intern_closure(&mut self, seeds: impl IntoIterator<Item = StateId>) -> StateId {
        self.targets.clear();
        self.targets.extend(seeds);
        epsilon_closure(self.nfa, &self.targets, &mut self.closure);
        self.intern(self.closure.members().to_vec())
    }

    fn expand_next(&mut self) -> bool {
        if self.next_subset == self.subsets.len() {
            return false;
        }

        let source = StateId::new(self.next_subset);
        let subset = self.subsets[self.next_subset].clone();
        self.next_subset += 1;
        let mut labels = Vec::new();
        let mut transition_targets = Vec::new();
        for state in subset {
            for transition in self.nfa.transitions(state) {
                labels.push(transition.label.clone());
                transition_targets.push(transition.target);
            }
        }

        for class in L::partition(&labels) {
            let target = self.intern_closure(
                class
                    .get_matching_labels()
                    .iter()
                    .map(|&index| transition_targets[index]),
            );
            self.builder
                .add_transition(source, class.get_label().clone(), target);
        }
        true
    }

    fn expand(&mut self) {
        while self.expand_next() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::nfa::Builder as NfaBuilder;
    use crate::automata::{Label, encoding::ByteRange};
    use std::cell::Cell;

    fn nfa() -> (Nfa<ByteRange, u32>, StateId, StateId) {
        let mut builder: NfaBuilder<ByteRange, u32> = NfaBuilder::new();
        let start = builder.add_state();
        let accept = builder.add_state();
        builder.set_accept(accept, 7);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");
        (nfa, start, accept)
    }

    #[test]
    fn a_construction_starts_with_empty_result_storage() {
        let (nfa, _, _) = nfa();
        let construction = Construction::new(&nfa, |_| ());

        assert_eq!(construction.builder.state_count(), 0);
        assert!(construction.subsets.is_empty());
        assert!(construction.state_by_subset.is_empty());
        assert_eq!(construction.next_subset, 0);
        assert!(construction.closure.is_empty());
        assert!(construction.targets.is_empty());
    }

    #[test]
    fn interning_a_subset_records_one_dfa_state() {
        let (nfa, start, _) = nfa();
        let mut construction = Construction::new(&nfa, |_| ());

        let state = construction.intern(vec![start]);

        assert_eq!(state, StateId::new(0));
        assert_eq!(construction.builder.state_count(), 1);
        assert_eq!(construction.subsets, vec![vec![start]]);
        assert_eq!(construction.state_by_subset[&vec![start]], state);
    }

    #[test]
    fn interning_the_same_subset_reuses_its_dfa_state() {
        let (nfa, start, _) = nfa();
        let mut construction = Construction::new(&nfa, |_| ());

        let first = construction.intern(vec![start]);
        let second = construction.intern(vec![start]);

        assert_eq!(first, second);
        assert_eq!(construction.builder.state_count(), 1);
        assert_eq!(construction.subsets, vec![vec![start]]);
    }

    #[test]
    fn interning_an_accepting_subset_selects_its_accept_once() {
        let (nfa, _, accept) = nfa();
        let calls = Cell::new(0);
        let mut construction = Construction::new(&nfa, |_| {
            calls.set(calls.get() + 1);
        });

        construction.intern(vec![accept]);
        construction.intern(vec![accept]);

        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn seeding_closes_each_nfa_start() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        let middle = builder.add_state();
        let end = builder.add_state();
        builder.add_epsilon_transition(start, middle);
        builder.add_epsilon_transition(middle, end);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");
        let mut construction = Construction::new(&nfa, |_| ());

        let starts = construction.seed();

        assert_eq!(starts, &[StateId::new(0)]);
        assert_eq!(construction.subsets, vec![vec![start, middle, end]]);
    }

    #[test]
    fn seeding_preserves_equivalent_nfa_start_entries() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let first = builder.add_state();
        let second = builder.add_state();
        builder.add_epsilon_transition(first, second);
        builder.add_epsilon_transition(second, first);
        let nfa = builder
            .build(&[first, second])
            .expect("the test NFA is below its capacity");
        let mut construction = Construction::new(&nfa, |_| ());

        let starts = construction.seed();

        assert_eq!(starts, &[StateId::new(0), StateId::new(0)]);
        assert_eq!(construction.builder.state_count(), 1);
        assert_eq!(construction.subsets, vec![vec![first, second]]);
    }

    #[test]
    fn construction_makes_a_dfa_for_one_labeled_path() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        let accept = builder.add_state();
        builder.add_transition(start, ByteRange::new(b'a', b'a'), accept);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");

        let dfa = construct(&nfa, |_| ()).expect("the DFA is below its capacity");

        let start = dfa.start_state(0);
        assert_eq!(dfa.state_count(), 2);
        assert_eq!(dfa.step(start, b'a'), Some(StateId::new(1)));
        assert_eq!(dfa.step(start, b'b'), None);
    }

    #[test]
    fn construction_closes_each_side_of_a_labeled_transition() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        let labeled = builder.add_state();
        let entered = builder.add_state();
        let accept = builder.add_state();
        builder.add_epsilon_transition(start, labeled);
        builder.add_transition(labeled, ByteRange::new(b'a', b'a'), entered);
        builder.add_epsilon_transition(entered, accept);
        builder.mark_accept(accept);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");

        let dfa = construct(&nfa, |_| ()).expect("the DFA is below its capacity");

        let target = dfa
            .step(dfa.start_state(0), b'a')
            .expect("the labeled transition is present");
        assert_eq!(dfa.state_count(), 2);
        assert!(dfa.accepts(target));
    }

    #[test]
    fn construction_partitions_overlapping_labels() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        let left = builder.add_state();
        let right = builder.add_state();
        builder.add_transition(start, ByteRange::new(b'a', b'f'), left);
        builder.add_transition(start, ByteRange::new(b'd', b'z'), right);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");

        let dfa = construct(&nfa, |_| ()).expect("the DFA is below its capacity");

        let start = dfa.start_state(0);
        assert_eq!(dfa.state_count(), 4);
        assert_eq!(dfa.step(start, b'a'), Some(StateId::new(1)));
        assert_eq!(dfa.step(start, b'd'), Some(StateId::new(2)));
        assert_eq!(dfa.step(start, b'z'), Some(StateId::new(3)));
    }

    #[test]
    fn expanding_one_subset_adds_its_labeled_move() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        let target = builder.add_state();
        builder.add_transition(start, ByteRange::new(b'a', b'a'), target);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");
        let mut construction = Construction::new(&nfa, |_| ());
        let starts = construction.seed();

        assert!(construction.expand_next());
        assert!(construction.expand_next());
        assert!(!construction.expand_next());
        let dfa = construction
            .builder
            .build(&starts)
            .expect("the DFA is below its capacity");

        assert_eq!(dfa.state_count(), 2);
        assert_eq!(dfa.step(starts[0], b'a'), Some(StateId::new(1)));
        assert_eq!(dfa.step(starts[0], b'b'), None);
    }

    #[test]
    fn expanding_closes_the_target_of_a_labeled_move() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        let labeled = builder.add_state();
        let entered = builder.add_state();
        let accept = builder.add_state();
        builder.add_epsilon_transition(start, labeled);
        builder.add_transition(labeled, ByteRange::new(b'a', b'a'), entered);
        builder.add_epsilon_transition(entered, accept);
        builder.mark_accept(accept);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");
        let mut construction = Construction::new(&nfa, |_| ());
        let starts = construction.seed();

        assert!(construction.expand_next());
        let dfa = construction
            .builder
            .build(&starts)
            .expect("the DFA is below its capacity");
        let target = dfa
            .step(starts[0], b'a')
            .expect("the labeled transition is present");

        assert!(dfa.accepts(target));
    }

    #[test]
    fn expanding_partitions_overlapping_labels() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        let left = builder.add_state();
        let right = builder.add_state();
        builder.add_transition(start, ByteRange::new(b'a', b'f'), left);
        builder.add_transition(start, ByteRange::new(b'd', b'z'), right);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");
        let mut construction = Construction::new(&nfa, |_| ());
        let starts = construction.seed();

        assert!(construction.expand_next());
        let dfa = construction
            .builder
            .build(&starts)
            .expect("the DFA is below its capacity");

        assert_eq!(dfa.state_count(), 4);
        assert_eq!(dfa.step(starts[0], b'a'), Some(StateId::new(1)));
        assert_eq!(dfa.step(starts[0], b'd'), Some(StateId::new(2)));
        assert_eq!(dfa.step(starts[0], b'z'), Some(StateId::new(3)));
    }

    #[test]
    fn construction_terminates_on_a_labeled_self_loop() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        builder.add_transition(start, ByteRange::new(b'a', b'a'), start);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");

        let dfa = construct(&nfa, |_| ()).expect("the DFA is below its capacity");

        let start = dfa.start_state(0);
        assert_eq!(dfa.state_count(), 1);
        assert_eq!(dfa.step(start, b'a'), Some(start));
    }

    #[test]
    fn construction_reuses_a_target_reached_by_duplicate_transitions() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        let target = builder.add_state();
        builder.add_transition(start, ByteRange::new(b'a', b'a'), target);
        builder.add_transition(start, ByteRange::new(b'a', b'a'), target);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");

        let dfa = construct(&nfa, |_| ()).expect("the DFA is below its capacity");

        assert_eq!(dfa.state_count(), 2);
        assert_eq!(dfa.transitions(dfa.start_state(0)).len(), 1);
    }

    #[test]
    fn construction_keeps_each_nfa_start_state_separate() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let first_start = builder.add_state();
        let second_start = builder.add_state();
        let first_target = builder.add_state();
        let second_target = builder.add_state();
        builder.add_transition(first_start, ByteRange::new(b'a', b'a'), first_target);
        builder.add_transition(second_start, ByteRange::new(b'b', b'b'), second_target);
        let nfa = builder
            .build(&[first_start, second_start])
            .expect("the test NFA is below its capacity");

        let dfa = construct(&nfa, |_| ()).expect("the DFA is below its capacity");

        let first_start = dfa.start_state(0);
        let second_start = dfa.start_state(1);
        assert_eq!(dfa.step(first_start, b'a'), Some(StateId::new(2)));
        assert_eq!(dfa.step(first_start, b'b'), None);
        assert_eq!(dfa.step(second_start, b'a'), None);
        assert_eq!(dfa.step(second_start, b'b'), Some(StateId::new(3)));
    }

    #[test]
    fn construction_selects_once_from_all_accepts_in_a_subset() {
        let mut builder: NfaBuilder<ByteRange, u32> = NfaBuilder::new();
        let start = builder.add_state();
        let first_accept = builder.add_state();
        let second_accept = builder.add_state();
        builder.add_epsilon_transition(start, first_accept);
        builder.add_epsilon_transition(start, second_accept);
        builder.set_accept(first_accept, 7);
        builder.set_accept(second_accept, 3);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");
        let calls = Cell::new(0);

        let dfa = construct(&nfa, |states| {
            calls.set(calls.get() + 1);
            states
                .iter()
                .filter_map(|&state| nfa.accept(state))
                .copied()
                .min()
                .expect("the subset accepts")
        })
        .expect("the DFA is below its capacity");

        assert_eq!(calls.get(), 1);
        assert_eq!(dfa.accept(dfa.start_state(0)), Some(&3));
    }

    #[test]
    fn construction_does_not_select_an_unreachable_accept() {
        let mut builder: NfaBuilder<ByteRange, u32> = NfaBuilder::new();
        let start = builder.add_state();
        let unreachable = builder.add_state();
        builder.set_accept(unreachable, 7);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");
        let calls = Cell::new(0);

        let dfa = construct(&nfa, |_| {
            calls.set(calls.get() + 1);
        })
        .expect("the DFA is below its capacity");

        assert_eq!(calls.get(), 0);
        assert_eq!(dfa.state_count(), 1);
    }

    #[test]
    fn construction_keeps_distinct_classes_that_share_a_target() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        let target = builder.add_state();
        builder.add_transition(start, ByteRange::new(b'a', b'c'), target);
        builder.add_transition(start, ByteRange::new(b'e', b'g'), target);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");

        let dfa = construct(&nfa, |_| ()).expect("the DFA is below its capacity");

        let start = dfa.start_state(0);
        assert_eq!(dfa.transitions(start).len(), 2);
        assert_eq!(dfa.step(start, b'a'), dfa.step(start, b'e'));
    }

    #[test]
    fn construction_matches_nfa_acceptance_for_short_inputs() {
        let mut builder: NfaBuilder<ByteRange> = NfaBuilder::new();
        let start = builder.add_state();
        let left = builder.add_state();
        let right = builder.add_state();
        let accept = builder.add_state();
        let repeat = builder.add_state();
        builder.add_epsilon_transition(start, left);
        builder.add_epsilon_transition(start, right);
        builder.add_transition(left, ByteRange::new(b'a', b'c'), accept);
        builder.add_transition(right, ByteRange::new(b'b', b'd'), accept);
        builder.add_epsilon_transition(accept, repeat);
        builder.add_transition(repeat, ByteRange::new(b'a', b'b'), repeat);
        builder.mark_accept(accept);
        let nfa = builder
            .build(&[start])
            .expect("the test NFA is below its capacity");
        let dfa = construct(&nfa, |_| ()).expect("the DFA is below its capacity");
        let alphabet = *b"abcx";

        for length in 0usize..=4 {
            for encoded in 0..alphabet.len().pow(length as u32) {
                let mut input = vec![alphabet[0]; length];
                let mut encoded = encoded;
                for byte in input.iter_mut().rev() {
                    *byte = alphabet[encoded % alphabet.len()];
                    encoded /= alphabet.len();
                }

                let mut execution = nfa.execution();
                execution.restart(0);
                let mut state = Some(dfa.start_state(0));
                assert_eq!(
                    execution.accepts(),
                    state.is_some_and(|state| dfa.accepts(state))
                );

                for byte in input {
                    execution.step(byte);
                    state = state.and_then(|state| dfa.step(state, byte));
                    assert_eq!(
                        execution.accepts(),
                        state.is_some_and(|state| dfa.accepts(state))
                    );
                }
            }
        }

        for state in 0..dfa.state_count() {
            let state = StateId::new(state);
            for byte in u8::MIN..=u8::MAX {
                let matches = dfa
                    .transitions(state)
                    .iter()
                    .filter(|transition| transition.label.matches(byte))
                    .count();
                assert!(
                    matches <= 1,
                    "state {state:?} has overlapping transitions for {byte}"
                );
            }
        }
    }
}
