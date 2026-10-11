//! Holds the complete execution plan for a lexer DFA.

use super::builder::Builder;
use super::rule::RuleEffect;
use super::selector::Selector;
use crate::automata::{dfa::Dfa, encoding::ByteRange};
use crate::lexer::{ResolvedTransition, RuleId};

#[derive(Debug)]
/// Holds the specialized selector plan and fused skip rules.
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
        rules: impl IntoIterator<Item = RuleEffect>,
    ) -> Self {
        Self {
            selector: Builder::new(dfa).build(),
            fused_skips: rules
                .into_iter()
                .enumerate()
                .filter_map(|(index, rule)| {
                    (rule.skips && matches!(rule.transition, ResolvedTransition::Stay))
                        .then_some(index)
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automata::dfa::Builder;
    use crate::lexer::StartConditionId;

    #[test]
    fn only_stay_mode_skips_are_fused() {
        let mut builder: Builder<ByteRange, RuleId> = Builder::new();
        let start = builder.add_state();
        let dfa = builder.build(&[start]).expect("the test DFA is valid");
        let rules = [
            RuleEffect {
                skips: true,
                transition: ResolvedTransition::Stay,
            },
            RuleEffect {
                skips: false,
                transition: ResolvedTransition::Stay,
            },
            RuleEffect {
                skips: true,
                transition: ResolvedTransition::Begin(StartConditionId::new(0)),
            },
            RuleEffect {
                skips: true,
                transition: ResolvedTransition::Push(StartConditionId::new(0)),
            },
            RuleEffect {
                skips: true,
                transition: ResolvedTransition::Pop,
            },
        ];
        assert_eq!(Matcher::new(&dfa, rules).fused_skips, vec![0]);
    }
}
