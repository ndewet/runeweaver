//! Compiles byte patterns without selecting a runtime source type.

use crate::automata::{BuildError, StateId, dfa, encoding::ByteRange, nfa};

/// Describes a regular language over bytes.
#[derive(Clone, Debug)]
pub enum BytePattern {
    /// Matches the empty byte sequence.
    Empty,
    /// Matches one byte in an inclusive range.
    Range(u8, u8),
    /// Matches each pattern in sequence.
    Concat(Vec<Self>),
    /// Matches one of the patterns.
    Alternation(Vec<Self>),
    /// Repeats a pattern with inclusive bounds.
    Repeat {
        /// The repeated pattern.
        pattern: Box<Self>,
        /// The minimum number of repetitions.
        min: usize,
        /// The maximum number of repetitions, or no upper bound.
        max: Option<usize>,
    },
}

/// Stores a source-independent DFA with all accepting rule identifiers.
pub struct ByteAutomaton {
    /// The start state identifier.
    pub start: usize,
    /// Inclusive byte ranges and target identifiers, indexed by state.
    pub transitions: Vec<Vec<(u8, u8, usize)>>,
    /// Accepting rule identifiers, indexed by state and in declaration order.
    pub accepts: Vec<Vec<usize>>,
}

impl ByteAutomaton {
    /// Compiles patterns into a minimized byte automaton.
    ///
    /// Retains overlapping accepts so callers can apply their own priorities.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bounds, empty matches, or capacity limits.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver_codegen::{ByteAutomaton, BytePattern};
    /// let automaton = ByteAutomaton::compile(&[BytePattern::Range(b'a', b'z')])?;
    /// assert!(automaton.accepts[automaton.start].is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn compile(patterns: &[BytePattern]) -> Result<Self, ByteCompileError> {
        let mut budget = 100_000;
        for pattern in patterns {
            if validate(pattern, &mut budget)? {
                return Err(ByteCompileError("a byte rule matches empty input".into()));
            }
        }
        let mut builder = nfa::Builder::<ByteRange, usize>::new();
        let start = builder.add_state();
        for (rule, pattern) in patterns.iter().enumerate() {
            let (entry, exit) = fragment(pattern, &mut builder);
            builder.add_epsilon_transition(start, entry);
            builder.set_accept(exit, rule);
        }
        let nfa = builder.build(&[start])?;
        let dfa = dfa::subset::construct(&nfa, |states| {
            let mut accepts: Vec<_> = states
                .iter()
                .filter_map(|state| nfa.accept(*state).copied())
                .collect();
            accepts.sort_unstable();
            accepts.dedup();
            accepts
        })?
        .minimize()?;
        let mut transitions = Vec::new();
        let mut accepts = Vec::new();
        for index in 0..dfa.state_count() {
            let state = StateId::new(index);
            transitions.push(
                dfa.transitions(state)
                    .iter()
                    .map(|edge| (edge.label.low, edge.label.high, edge.target.index()))
                    .collect(),
            );
            accepts.push(dfa.accept(state).cloned().unwrap_or_default());
        }
        Ok(Self {
            start: dfa.start_states()[0].index(),
            transitions,
            accepts,
        })
    }
}

/// Reports an invalid byte pattern or an automaton capacity limit.
#[derive(Debug)]
pub struct ByteCompileError(String);

impl std::fmt::Display for ByteCompileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ByteCompileError {}

impl From<BuildError> for ByteCompileError {
    fn from(error: BuildError) -> Self {
        Self(error.to_string())
    }
}

fn validate(pattern: &BytePattern, budget: &mut usize) -> Result<bool, ByteCompileError> {
    *budget = budget
        .checked_sub(1)
        .ok_or_else(|| ByteCompileError("byte pattern exceeds the construction budget".into()))?;
    match pattern {
        BytePattern::Empty => Ok(true),
        BytePattern::Range(low, high) => {
            if low > high {
                return Err(ByteCompileError("invalid byte range".into()));
            }
            Ok(false)
        }
        BytePattern::Concat(parts) | BytePattern::Alternation(parts) => {
            let concat = matches!(pattern, BytePattern::Concat(_));
            let mut nullable = concat;
            for part in parts {
                let child = validate(part, budget)?;
                nullable = if concat {
                    nullable && child
                } else {
                    nullable || child
                };
            }
            Ok(nullable)
        }
        BytePattern::Repeat { pattern, min, max } => {
            if max.is_some_and(|max| max < *min) {
                return Err(ByteCompileError("invalid repetition bounds".into()));
            }
            let mut child_budget = *budget;
            let nullable = validate(pattern, &mut child_budget)?;
            let copies = max.unwrap_or_else(|| min.saturating_add(1));
            let cost = (budget.saturating_sub(child_budget))
                .saturating_add(2)
                .saturating_mul(copies);
            *budget = budget.checked_sub(cost).ok_or_else(|| {
                ByteCompileError("byte pattern exceeds the construction budget".into())
            })?;
            Ok(*min == 0 || nullable)
        }
    }
}

fn fragment(
    pattern: &BytePattern,
    builder: &mut nfa::Builder<ByteRange, usize>,
) -> (StateId, StateId) {
    let entry = builder.add_state();
    let exit = builder.add_state();
    match pattern {
        BytePattern::Empty => builder.add_epsilon_transition(entry, exit),
        BytePattern::Range(low, high) => {
            builder.add_transition(entry, ByteRange::new(*low, *high), exit)
        }
        BytePattern::Concat(parts) => {
            let mut previous = entry;
            for part in parts {
                let (start, end) = fragment(part, builder);
                builder.add_epsilon_transition(previous, start);
                previous = end;
            }
            builder.add_epsilon_transition(previous, exit);
        }
        BytePattern::Alternation(parts) => {
            for part in parts {
                let (start, end) = fragment(part, builder);
                builder.add_epsilon_transition(entry, start);
                builder.add_epsilon_transition(end, exit);
            }
        }
        BytePattern::Repeat { pattern, min, max } => {
            let mut previous = entry;
            for _ in 0..*min {
                let (start, end) = fragment(pattern, builder);
                builder.add_epsilon_transition(previous, start);
                previous = end;
            }
            builder.add_epsilon_transition(previous, exit);
            match max {
                Some(max) => {
                    for _ in *min..*max {
                        let (start, end) = fragment(pattern, builder);
                        builder.add_epsilon_transition(previous, start);
                        builder.add_epsilon_transition(end, exit);
                        previous = end;
                    }
                }
                None => {
                    let (start, end) = fragment(pattern, builder);
                    builder.add_epsilon_transition(previous, start);
                    builder.add_epsilon_transition(end, previous);
                }
            }
        }
    }
    (entry, exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepts(automaton: &ByteAutomaton, bytes: &[u8]) -> Vec<usize> {
        let mut state = automaton.start;
        for byte in bytes {
            let Some((_, _, target)) = automaton.transitions[state]
                .iter()
                .find(|(low, high, _)| low <= byte && byte <= high)
            else {
                return Vec::new();
            };
            state = *target;
        }
        automaton.accepts[state].clone()
    }

    #[test]
    fn binary_repetition_retains_overlapping_rules() {
        let patterns = [
            BytePattern::Range(0x80, 0xff),
            BytePattern::Repeat {
                pattern: Box::new(BytePattern::Range(0, 255)),
                min: 1,
                max: Some(3),
            },
        ];
        let automaton = ByteAutomaton::compile(&patterns).unwrap();
        assert_eq!(accepts(&automaton, &[0xff]), [0, 1]);
        assert_eq!(accepts(&automaton, &[0, 0xff, 1]), [1]);
        assert!(accepts(&automaton, &[0; 4]).is_empty());
    }

    #[test]
    fn unbounded_repetition_and_alternation_preserve_the_language() {
        let automaton = ByteAutomaton::compile(&[BytePattern::Concat(vec![
            BytePattern::Range(b'a', b'a'),
            BytePattern::Repeat {
                pattern: Box::new(BytePattern::Alternation(vec![
                    BytePattern::Range(b'b', b'b'),
                    BytePattern::Range(b'c', b'c'),
                ])),
                min: 0,
                max: None,
            },
        ])])
        .unwrap();
        assert_eq!(accepts(&automaton, b"a"), [0]);
        assert_eq!(accepts(&automaton, b"abcb"), [0]);
        assert!(accepts(&automaton, b"abd").is_empty());
    }

    #[test]
    fn invalid_and_excessive_patterns_return_errors() {
        for pattern in [
            BytePattern::Empty,
            BytePattern::Range(2, 1),
            BytePattern::Repeat {
                pattern: Box::new(BytePattern::Range(0, 255)),
                min: usize::MAX,
                max: None,
            },
        ] {
            assert!(ByteAutomaton::compile(&[pattern]).is_err());
        }
        assert!(ByteAutomaton::compile(&[]).is_ok());
    }
}
