//! Describes saved matches carried across region boundaries.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Describes the rule information required by a region's saved match.
pub(crate) enum Context {
    Empty,
    Fixed { rule: usize, optional: bool },
    General,
}
