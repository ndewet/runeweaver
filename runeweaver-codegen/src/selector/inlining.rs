//! Bounds forced expansion of shared selector regions.

use crate::ir::{Block, Dispatch, Function, FunctionBody, Selector, Transfer};
use std::collections::VecDeque;

const MAX_FORCED_EXPANSION: usize = 128 * 1024;

pub(super) fn select(selector: &Selector, function_sizes: &[usize]) -> Vec<bool> {
    let calls: Vec<_> = selector.functions.iter().map(function_calls).collect();
    let mut starts = vec![0usize; selector.functions.len()];
    for start in &selector.starts {
        starts[start.region] += 1;
    }
    select_with_graph(&calls, &starts, function_sizes)
}

fn select_with_graph(
    calls: &[Vec<usize>],
    starts: &[usize],
    function_sizes: &[usize],
) -> Vec<bool> {
    let mut forced = vec![true; calls.len()];
    loop {
        let copies = expansion_copies(calls, starts, &forced);
        let contributions: Vec<_> = function_sizes
            .iter()
            .zip(&copies)
            .zip(&forced)
            .map(|((&size, &copies), &is_forced)| {
                is_forced.then_some(copies.saturating_sub(1).saturating_mul(size))
            })
            .collect();
        let expansion = contributions
            .iter()
            .flatten()
            .fold(0usize, |sum, &size| sum.saturating_add(size));
        if expansion <= MAX_FORCED_EXPANSION {
            return forced;
        }
        let (function, contribution) = contributions
            .iter()
            .enumerate()
            .filter_map(|(function, contribution)| contribution.map(|size| (function, size)))
            .max_by_key(|&(_, size)| size)
            .expect("forced inlining past its budget has a duplicated function");
        assert!(contribution != 0, "forced inline expansion can decrease");
        forced[function] = false;
    }
}

fn expansion_copies(calls: &[Vec<usize>], starts: &[usize], forced: &[bool]) -> Vec<usize> {
    let mut incoming = vec![0usize; calls.len()];
    for targets in calls {
        let mut unique = targets.clone();
        unique.sort_unstable();
        unique.dedup();
        for target in unique {
            incoming[target] += 1;
        }
    }
    let mut queue: VecDeque<_> = incoming
        .iter()
        .enumerate()
        .filter_map(|(function, &count)| (count == 0).then_some(function))
        .collect();
    let mut copies = starts.to_vec();
    let mut processed = 0;
    while let Some(function) = queue.pop_front() {
        processed += 1;
        let instances = if forced[function] {
            copies[function]
        } else {
            usize::from(copies[function] != 0)
        };
        let mut counts = vec![0usize; calls.len()];
        for &target in &calls[function] {
            counts[target] += 1;
        }
        for (target, count) in counts
            .into_iter()
            .enumerate()
            .filter(|&(_, count)| count != 0)
        {
            copies[target] = copies[target].saturating_add(instances.saturating_mul(count));
            incoming[target] -= 1;
            if incoming[target] == 0 {
                queue.push_back(target);
            }
        }
    }
    assert_eq!(processed, calls.len(), "region calls form an acyclic graph");
    copies
}

fn function_calls(function: &Function) -> Vec<usize> {
    let mut calls = Vec::new();
    match &function.body {
        FunctionBody::Linear(block) | FunctionBody::Loop(block) => block_calls(block, &mut calls),
        FunctionBody::Dispatch { arms, .. } => {
            for block in arms {
                block_calls(block, &mut calls);
            }
        }
    }
    calls
}

fn block_calls(block: &Block, calls: &mut Vec<usize>) {
    let Block::Scan { dispatch, .. } = block else {
        return;
    };
    let transfers: Vec<_> = match dispatch {
        Dispatch::Fail => Vec::new(),
        Dispatch::Ranges(branches) => branches.iter().map(|(_, transfer)| transfer).collect(),
        Dispatch::Table { branches, .. } => branches.iter().collect(),
    };
    for transfer in transfers {
        match transfer {
            Transfer::Call(call) => calls.push(call.region),
            Transfer::Inline { body, .. } => block_calls(body, calls),
            Transfer::Return(_) | Transfer::Continue { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_shared_regions_keep_forced_inlining() {
        let calls = vec![vec![1; 32], vec![]];
        let starts = vec![1, 0];
        let sizes = vec![42_000, 1_600];

        assert_eq!(select_with_graph(&calls, &starts, &sizes), [true, true]);
    }

    #[test]
    fn excessive_shared_region_expansion_stops_forced_inlining() {
        let calls = vec![vec![1; 16], vec![]];
        let starts = vec![1, 0];
        let sizes = vec![20_000, 10_000];

        assert_eq!(select_with_graph(&calls, &starts, &sizes), [true, false]);
    }

    #[test]
    fn a_retained_function_stops_transitive_copy_growth() {
        let calls = vec![vec![1; 10], vec![2; 10], vec![]];
        let starts = vec![1, 0, 0];

        assert_eq!(expansion_copies(&calls, &starts, &[true; 3]), [1, 10, 100]);
        assert_eq!(
            expansion_copies(&calls, &starts, &[true, false, true]),
            [1, 10, 10]
        );
    }
}
