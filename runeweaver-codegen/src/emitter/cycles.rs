//! Selects loop headers that bound nested paths inside DFA cycles.

use super::regions::Region;
use crate::automata::{StateId, dfa::Dfa, encoding::ByteRange};
use crate::lexer::RuleId;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(super) fn headers(dfa: &Dfa<ByteRange, RuleId>, region: &Region) -> Vec<usize> {
    let members: BTreeSet<_> = region.states.iter().copied().collect();
    let mut predecessors: BTreeMap<_, _> = region
        .states
        .iter()
        .map(|&state| (state, BTreeSet::new()))
        .collect();
    for &source in &region.states {
        for edge in dfa.transitions(StateId::new(source)) {
            let target = edge.target.index();
            if target != source && members.contains(&target) {
                predecessors
                    .get_mut(&target)
                    .expect("the target belongs to the region")
                    .insert(source);
            }
        }
    }
    let mut headers: BTreeSet<_> = region.entries.iter().copied().collect();
    headers.extend(
        region
            .states
            .iter()
            .copied()
            .filter(|&state| predecessors[&state].len() != 1),
    );
    for header in headers.iter().copied().collect::<Vec<_>>() {
        if region.entries.contains(&header) {
            continue;
        }
        let mut source = header;
        for _ in 0..8 {
            let transitions = dfa.transitions(StateId::new(source));
            let targets: BTreeSet<_> = transitions.iter().map(|edge| edge.target.index()).collect();
            if targets.len() != 1 || targets.contains(&source) {
                break;
            }
            let target = *targets.first().expect("one target exists");
            if !members.contains(&target) || target == header {
                break;
            }
            if headers.contains(&target) {
                headers.remove(&header);
                break;
            }
            source = target;
        }
    }
    // Removing a linear tail retains a header on every cycle.
    // The remaining graph is acyclic, so its longest nested paths are finite.
    let roots = headers.clone();
    let mut depth: BTreeMap<_, usize> = region.states.iter().map(|&state| (state, 0)).collect();
    let mut pending: BTreeMap<_, _> = predecessors
        .iter()
        .map(|(&state, sources)| (state, sources.len()))
        .collect();
    let mut queue: VecDeque<_> = roots.iter().copied().collect();
    let mut processed = 0;
    while let Some(source) = queue.pop_front() {
        processed += 1;
        let targets: BTreeSet<_> = dfa
            .transitions(StateId::new(source))
            .iter()
            .map(|edge| edge.target.index())
            .collect();
        for target in targets {
            if target == source || !members.contains(&target) || roots.contains(&target) {
                continue;
            }
            let next = depth[&source] + 1;
            let target_depth = depth
                .get_mut(&target)
                .expect("the target belongs to the region");
            *target_depth = (*target_depth).max(next);
            let remaining = pending
                .get_mut(&target)
                .expect("the target belongs to the region");
            *remaining -= 1;
            if *remaining == 0 {
                if *target_depth >= 64 {
                    headers.insert(target);
                    *target_depth = 0;
                }
                queue.push_back(target);
            }
        }
    }
    assert_eq!(
        processed,
        region.states.len(),
        "every cycle contains a loop header"
    );
    headers.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::super::regions::Regions;
    use super::*;
    use crate::automata::dfa::Builder;

    fn cycle(count: usize, starts: &[usize], join: bool) -> Vec<usize> {
        let mut builder = Builder::new();
        let states: Vec<_> = (0..count).map(|_| builder.add_state()).collect();
        for index in 0..count {
            builder.add_transition(
                states[index],
                ByteRange::new(b'a', b'a'),
                states[(index + 1) % count],
            );
        }
        if join {
            builder.add_transition(states[0], ByteRange::new(b'b', b'b'), states[2]);
        }
        let starts: Vec<_> = starts.iter().map(|&index| states[index]).collect();
        let dfa = builder.build(&starts).expect("the test DFA is valid");
        let regions = Regions::new(&dfa);
        headers(&dfa, &regions.regions[0])
    }

    #[test]
    fn simple_cycles_need_only_their_entry_header() {
        assert_eq!(cycle(9, &[0], false), vec![0]);
    }

    #[test]
    fn linear_shared_tails_can_be_nested_but_external_entries_remain_headers() {
        assert_eq!(cycle(4, &[0], true), vec![0]);
        assert_eq!(cycle(4, &[0, 2], false), vec![0, 2]);
    }

    #[test]
    fn long_cycles_have_bounded_nested_paths() {
        let headers = cycle(1024, &[0], false);
        assert_eq!(headers.len(), 16);
        assert!(headers.windows(2).all(|pair| pair[1] - pair[0] == 64));
    }
}
