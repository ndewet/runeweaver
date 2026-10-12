//! Holds the complete execution plan for a lexer DFA.

use super::builder::Builder;
use super::selector::Selector;
use crate::automata::{dfa::Dfa, encoding::ByteRange};
use crate::compiler::RuleId;

#[derive(Debug)]
/// Holds the specialized selector plan and rules fused into its scan loop.
pub(crate) struct Matcher {
    pub selector: Selector,
    pub fused_skips: Vec<usize>,
}

impl Matcher {
    /// Plans a byte DFA and the stay-mode skip rules.
    ///
    /// The caller supplies a DFA from UTF-8 lowering and minimization. Its
    /// accepts end at complete codepoints, and each transfer consumes one byte.
    /// Self loops consume only bytes in their labels and stop at the input bound.
    /// These invariants also apply to saved matches and fused skip prefixes.
    /// Thus each selected end is within the remaining input at a UTF-8 boundary.
    /// Adding a selected length to a skipped prefix preserves both slice bounds.
    pub(crate) fn new(
        dfa: &Dfa<ByteRange, RuleId>,
        fused_skips: impl IntoIterator<Item = usize>,
    ) -> Self {
        Self {
            selector: Builder::new(dfa).build(),
            fused_skips: fused_skips.into_iter().collect(),
        }
    }
}
