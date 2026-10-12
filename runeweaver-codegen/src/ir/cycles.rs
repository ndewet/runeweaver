//! Selects loop headers that bound nested paths inside DFA cycles.

use super::regions::Regions;
use std::collections::{BTreeSet, VecDeque};

pub(super) fn headers(regions: &Regions, id: usize) -> Vec<usize> {
    let region = &regions.regions[id];
    let internal_predecessors = |state: usize| {
        regions.states[state]
            .predecessors
            .iter()
            .filter(move |&&source| source != state && regions.states[source].owner == id)
    };
    let mut headers: BTreeSet<_> = region.entries.iter().copied().collect();
    headers.extend(
        region
            .states
            .iter()
            .copied()
            .filter(|&state| internal_predecessors(state).count() != 1),
    );
    for header in headers.iter().copied().collect::<Vec<_>>() {
        if region.entries.contains(&header) {
            continue;
        }
        let mut source = header;
        for _ in 0..super::limits::MAX_SHARED_TAIL {
            let targets = &regions.states[source].successors;
            if targets.len() != 1 || targets.contains(&source) {
                break;
            }
            let target = *targets.first().expect("one target exists");
            if regions.states[target].owner != id || target == header {
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
    let mut depth = vec![0; regions.states.len()];
    let mut pending = vec![0; regions.states.len()];
    for &state in &region.states {
        pending[state] = internal_predecessors(state).count();
    }
    let mut queue: VecDeque<_> = roots.iter().copied().collect();
    let mut processed = 0;
    while let Some(source) = queue.pop_front() {
        processed += 1;
        for &target in &regions.states[source].successors {
            if target == source || regions.states[target].owner != id || roots.contains(&target) {
                continue;
            }
            depth[target] = depth[target].max(depth[source] + 1);
            pending[target] -= 1;
            if pending[target] == 0 {
                if depth[target] >= super::limits::MAX_INLINE_DEPTH {
                    headers.insert(target);
                    depth[target] = 0;
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
    use super::*;
    use crate::automata::dfa::Builder;
    use crate::automata::encoding::ByteRange;

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
        headers(&regions, 0)
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
