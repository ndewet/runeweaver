//! Describes rule effects used by execution planning.

use crate::lexer::ResolvedTransition;

/// Supplies rule effects without carrying an action's source syntax.
pub(crate) struct RuleEffect {
    pub skips: bool,
    pub transition: ResolvedTransition,
}
