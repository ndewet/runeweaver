//! Describes saved matches carried across region boundaries.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Describes the rule information required by a region's saved match.
pub(crate) enum Context {
    Empty,
    Fixed { rule: usize, optional: bool },
    General,
}

impl Context {
    pub(super) const fn accepted(rule: usize) -> Self {
        Self::Fixed {
            rule,
            optional: false,
        }
    }

    pub(super) const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::General, _) | (_, Self::General) => Self::General,
            (Self::Empty, Self::Empty) => Self::Empty,
            (Self::Empty, Self::Fixed { rule, .. }) | (Self::Fixed { rule, .. }, Self::Empty) => {
                Self::Fixed {
                    rule,
                    optional: true,
                }
            }
            (
                Self::Fixed {
                    rule: first,
                    optional: first_optional,
                },
                Self::Fixed {
                    rule: second,
                    optional: second_optional,
                },
            ) => {
                if first == second {
                    Self::Fixed {
                        rule: first,
                        optional: first_optional || second_optional,
                    }
                } else {
                    Self::General
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Context;

    #[test]
    fn joining_saved_match_contexts_preserves_only_relevant_distinctions() {
        let fixed = Context::accepted(3);
        let other = Context::accepted(4);
        let optional = Context::Fixed {
            rule: 3,
            optional: true,
        };

        assert_eq!(Context::Empty.join(Context::Empty), Context::Empty);
        assert_eq!(fixed.join(fixed), fixed);
        assert_eq!(Context::Empty.join(fixed), optional);
        assert_eq!(optional.join(fixed), optional);
        assert_eq!(fixed.join(other), Context::General);
        assert_eq!(Context::General.join(fixed), Context::General);
    }
}
