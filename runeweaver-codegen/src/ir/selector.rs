//! Describes selector control flow without source-language syntax.

use super::context::Context;
use crate::automata::encoding::ByteRange;

#[derive(Debug)]
/// Holds completed region functions and calls for each start condition.
pub(crate) struct Selector {
    pub starts: Vec<Call>,
    pub functions: Vec<Function>,
}

#[derive(Debug)]
/// Executes one region with a specialized saved-match parameter.
pub(crate) struct Function {
    pub context: Context,
    pub mutable_context: bool,
    pub mutable_index: bool,
    pub entry_parameter: bool,
    pub body: FunctionBody,
}

#[derive(Debug)]
/// Selects linear control flow, one loop header, or explicit header dispatch.
pub(crate) enum FunctionBody {
    Linear(Block),
    Loop(Block),
    Dispatch {
        initial: Option<usize>,
        arms: Vec<Block>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Locates an end relative to the current input index.
pub(crate) enum Position {
    Zero,
    Current,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Supplies a selected match without constructing a source expression.
pub(crate) enum MatchValue {
    Empty,
    Accepted {
        rule: usize,
        end: Position,
    },
    Context(Context),
    /// Uses the match bound before the preceding inline transfer consumes a byte.
    Local,
}

#[derive(Debug, Clone, Copy)]
/// Adapts a saved match to the destination region's parameter.
pub(crate) enum Argument {
    Omitted,
    ZeroEnd,
    EmptyEnd,
    EmptyMatch,
    RequiredEnd(MatchValue),
    OptionalEnd(MatchValue),
    Match(MatchValue),
}

#[derive(Debug)]
/// Calls a region with a completed context and entry selection.
pub(crate) struct Call {
    pub region: usize,
    pub index: Position,
    pub context: Argument,
    pub entry: Option<usize>,
}

#[derive(Debug)]
/// Returns a match or scans from one planned state.
pub(crate) enum Block {
    Return(MatchValue),
    Scan {
        self_loop: Option<SelfLoop>,
        failure: MatchValue,
        dispatch: Dispatch,
    },
}

#[derive(Debug)]
/// Selects a completed transfer after self-loop scanning.
pub(crate) enum Dispatch {
    Fail,
    Ranges(Vec<(Vec<ByteRange>, Transfer)>),
    Table {
        width: Width,
        values: Box<[usize; 256]>,
        branches: Vec<Transfer>,
    },
}

#[derive(Debug)]
/// Holds membership values and the probes for a bounded scan chunk.
pub(crate) struct SelfLoop {
    pub values: Box<[bool; 256]>,
    pub chunk_size: usize,
    pub probes: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Holds the smallest integer width that includes the dead branch value.
pub(crate) enum Width {
    U8,
    U16,
    U32,
    Usize,
}

impl Width {
    pub(super) fn for_dead(dead: usize) -> Self {
        if u8::try_from(dead).is_ok() {
            Self::U8
        } else if u16::try_from(dead).is_ok() {
            Self::U16
        } else if u32::try_from(dead).is_ok() {
            Self::U32
        } else {
            Self::Usize
        }
    }
}

#[derive(Debug)]
/// Completes a transition with a return, region call, loop continuation, or inline body.
pub(crate) enum Transfer {
    Return(MatchValue),
    Call(Call),
    Continue {
        update: Option<(Context, MatchValue)>,
        header: Option<usize>,
    },
    Inline {
        bind: Option<MatchValue>,
        body: Box<Block>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_width_includes_the_dead_value() {
        for (dead, expected) in [
            (0, Width::U8),
            (255, Width::U8),
            (256, Width::U16),
            (65535, Width::U16),
            (65536, Width::U32),
            (u32::MAX as usize, Width::U32),
        ] {
            assert_eq!(Width::for_dead(dead), expected);
        }
        #[cfg(target_pointer_width = "64")]
        assert_eq!(Width::for_dead(u32::MAX as usize + 1), Width::Usize);
    }
}
