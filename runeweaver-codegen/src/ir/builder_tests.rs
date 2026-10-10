//! Checks decisions in the completed execution plan.

use super::*;
use crate::automata::dfa::Builder as DfaBuilder;

fn linear(selector: &Selector) -> &Block {
    let FunctionBody::Linear(block) = &selector.functions[selector.starts[0].region].body else {
        panic!("the test start has linear control flow");
    };
    block
}

#[test]
fn direct_transitions_inline_final_accepts() {
    let mut builder = DfaBuilder::new();
    let start = builder.add_state();
    let target = builder.add_state();
    builder.add_transition(start, ByteRange::new(b'a', b'z'), target);
    builder.set_accept(target, RuleId::new(0));
    let dfa = builder.build(&[start]).expect("the test DFA is valid");
    let selector = Builder::new(&dfa).build();
    let Block::Scan {
        failure,
        dispatch: Dispatch::Ranges(branches),
        ..
    } = linear(&selector)
    else {
        panic!("one range uses a direct branch");
    };
    assert_eq!(*failure, MatchValue::Empty);
    assert_eq!(branches[0].0, vec![ByteRange::new(b'a', b'z')]);
    assert!(matches!(
        branches[0].1,
        Transfer::Return(MatchValue::Accepted {
            rule: 0,
            end: Position::Next
        })
    ));
}

#[test]
fn local_forks_select_all_targets_and_return_the_saved_match_on_failure() {
    let mut builder = DfaBuilder::new();
    let start = builder.add_state();
    for byte in b'a'..=b'c' {
        let target = builder.add_state();
        builder.add_transition(start, ByteRange::new(byte, byte), target);
        builder.set_accept(target, RuleId::new(usize::from(byte - b'a')));
    }
    let dfa = builder.build(&[start]).expect("the test DFA is valid");
    let selector = Builder::new(&dfa).build();
    let Block::Scan {
        failure,
        dispatch:
            Dispatch::Table {
                width,
                values,
                branches,
            },
        ..
    } = linear(&selector)
    else {
        panic!("three targets use a table");
    };
    assert_eq!(*failure, MatchValue::Empty);
    assert_eq!(*width, Width::U8);
    for byte in u8::MIN..=u8::MAX {
        let expected = if (b'a'..=b'c').contains(&byte) {
            usize::from(byte - b'a')
        } else {
            3
        };
        assert_eq!(values[usize::from(byte)], expected);
    }
    for (ordinal, branch) in branches.iter().enumerate() {
        assert!(
            matches!(branch, Transfer::Return(MatchValue::Accepted { rule, end: Position::Next }) if *rule == ordinal)
        );
    }
}

#[test]
fn fragmented_ranges_use_a_table_even_with_one_target() {
    let mut builder = DfaBuilder::new();
    let start = builder.add_state();
    let target = builder.add_state();
    for byte in *b"ace" {
        builder.add_transition(start, ByteRange::new(byte, byte), target);
    }
    let dfa = builder.build(&[start]).expect("the test DFA is valid");
    let selector = Builder::new(&dfa).build();
    let Block::Scan {
        dispatch: Dispatch::Table {
            values, branches, ..
        },
        ..
    } = linear(&selector)
    else {
        panic!("three ranges use a table");
    };
    assert_eq!(branches.len(), 1);
    assert_eq!(values[usize::from(b'a')], 0);
    assert_eq!(values[usize::from(b'b')], 1);
}

#[test]
fn start_conditions_call_their_regions() {
    let mut builder: DfaBuilder<ByteRange, RuleId> = DfaBuilder::new();
    let first = builder.add_state();
    let second = builder.add_state();
    let dfa = builder
        .build(&[second, first])
        .expect("the test DFA is valid");
    let selector = Builder::new(&dfa).build();
    assert_eq!(selector.starts[0].region, 1);
    assert_eq!(selector.starts[1].region, 0);
    assert!(
        selector
            .starts
            .iter()
            .all(|call| call.index == Position::Zero
                && matches!(call.context, Argument::Omitted)
                && call.entry.is_none())
    );
}

#[test]
fn nested_accepts_save_their_original_end_before_returning_to_a_header() {
    let mut builder = DfaBuilder::new();
    let states: Vec<_> = (0..3).map(|_| builder.add_state()).collect();
    for index in 0..3 {
        builder.add_transition(
            states[index],
            ByteRange::new(b'a', b'a'),
            states[(index + 1) % 3],
        );
    }
    builder.set_accept(states[1], RuleId::new(7));
    let dfa = builder.build(&[states[0]]).expect("the test DFA is valid");
    let selector = Builder::new(&dfa).build();
    let function = &selector.functions[0];
    let context = Context::Fixed {
        rule: 7,
        optional: true,
    };
    assert_eq!(function.context, context);
    assert!(function.mutable_context);
    let FunctionBody::Loop(Block::Scan {
        dispatch: Dispatch::Ranges(first),
        ..
    }) = &function.body
    else {
        panic!("the cycle uses its single header");
    };
    let Transfer::Inline { bind: None, body } = &first[0].1 else {
        panic!("the accept needs no incoming match");
    };
    let Block::Scan {
        dispatch: Dispatch::Ranges(second),
        ..
    } = body.as_ref()
    else {
        panic!("the accept has a successor");
    };
    let Transfer::Inline { bind, body } = &second[0].1 else {
        panic!("the nonaccept keeps the previous match");
    };
    assert_eq!(
        *bind,
        Some(MatchValue::Accepted {
            rule: 7,
            end: Position::Current
        })
    );
    let Block::Scan {
        dispatch: Dispatch::Ranges(third),
        failure,
        ..
    } = body.as_ref()
    else {
        panic!("the tail returns to its header");
    };
    assert_eq!(*failure, MatchValue::Local);
    assert!(
        matches!(third[0].1, Transfer::Continue { update: Some((saved_context, MatchValue::Local)), header: None } if saved_context == context)
    );
}

fn depth(block: &Block) -> usize {
    let Block::Scan { dispatch, .. } = block else {
        return 1;
    };
    let transfers: Vec<_> = match dispatch {
        Dispatch::Fail => vec![],
        Dispatch::Ranges(branches) => branches.iter().map(|(_, transfer)| transfer).collect(),
        Dispatch::Table { branches, .. } => branches.iter().collect(),
    };
    1 + transfers
        .into_iter()
        .map(|transfer| match transfer {
            Transfer::Inline { body, .. } => depth(body),
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

#[test]
fn completed_bodies_bound_duplicated_paths_in_long_cycles_and_acyclic_paths() {
    for cyclic in [false, true] {
        let mut builder: DfaBuilder<ByteRange, RuleId> = DfaBuilder::new();
        let states: Vec<_> = (0..1024).map(|_| builder.add_state()).collect();
        for pair in states.windows(2) {
            builder.add_transition(pair[0], ByteRange::new(b'a', b'a'), pair[1]);
        }
        if cyclic {
            builder.add_transition(states[1023], ByteRange::new(b'a', b'a'), states[0]);
        }
        let dfa = builder.build(&[states[0]]).expect("the test DFA is valid");
        let selector = Builder::new(&dfa).build();
        for function in &selector.functions {
            match &function.body {
                FunctionBody::Linear(block) | FunctionBody::Loop(block) => {
                    assert!(depth(block) <= 64)
                }
                FunctionBody::Dispatch { arms, .. } => {
                    assert!(arms.iter().all(|block| depth(block) <= 64))
                }
            }
        }
    }
}

#[test]
fn self_loops_have_a_complete_membership_table_and_bounded_chunk_probes() {
    let mut builder: DfaBuilder<ByteRange, RuleId> = DfaBuilder::new();
    let start = builder.add_state();
    builder.add_transition(start, ByteRange::new(b'a', b'z'), start);
    let dfa = builder.build(&[start]).expect("the test DFA is valid");
    let selector = Builder::new(&dfa).build();
    let Block::Scan {
        self_loop: Some(plan),
        dispatch: Dispatch::Fail,
        ..
    } = linear(&selector)
    else {
        panic!("the self loop ends without another target");
    };
    for byte in u8::MIN..=u8::MAX {
        assert_eq!(plan.values[usize::from(byte)], byte.is_ascii_lowercase());
    }
    assert_eq!(plan.chunk_size, 8);
    assert_eq!(plan.probes, vec![(0, 1), (2, 3), (4, 5), (6, 7)]);
}

#[test]
fn external_entries_select_distinct_cycle_headers_without_recursive_calls() {
    let mut builder: DfaBuilder<ByteRange, RuleId> = DfaBuilder::new();
    let first = builder.add_state();
    let second = builder.add_state();
    builder.add_transition(first, ByteRange::new(b'a', b'a'), second);
    builder.add_transition(second, ByteRange::new(b'b', b'b'), first);
    let dfa = builder
        .build(&[second, first])
        .expect("the test DFA is valid");
    let selector = Builder::new(&dfa).build();
    assert_eq!(selector.starts[0].entry, Some(1));
    assert_eq!(selector.starts[1].entry, Some(0));
    assert_eq!(selector.starts[0].region, selector.starts[1].region);
    let function = &selector.functions[selector.starts[0].region];
    assert!(function.entry_parameter);
    let FunctionBody::Dispatch {
        initial: None,
        arms,
    } = &function.body
    else {
        panic!("both entries use header dispatch");
    };
    assert_eq!(arms.len(), 2);
    for (ordinal, block) in arms.iter().enumerate() {
        let Block::Scan {
            dispatch: Dispatch::Ranges(branches),
            ..
        } = block
        else {
            panic!("each header has one direct branch");
        };
        assert!(
            matches!(branches[0].1, Transfer::Continue { update: None, header: Some(header) } if header == 1 - ordinal)
        );
    }
}
