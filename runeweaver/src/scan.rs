//! Token iteration and recoverable scan errors.

use crate::lexer::{Automaton, ModeAction, RuleOutput};
use crate::{Lexer, Span, Spanned};
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;

/// An error encountered while scanning input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    /// No rule accepted the character at this UTF-8 byte range.
    Unrecognized {
        /// The UTF-8 byte range of the unrecognized character.
        span: Span,
    },
    /// A winning rule's payload conversion failed.
    InvalidPayload {
        /// The UTF-8 byte range matched by the rule with the invalid payload.
        span: Span,
        /// The payload conversion error's message.
        message: String,
    },
    /// Input ended while a pushed lexer mode was still active.
    UnterminatedMode {
        /// The range from the mode-opening rule through end of input.
        span: Span,
        /// The unclosed mode's declared name.
        mode: &'static str,
    },
}

impl ScanError {
    /// Returns the byte range associated with this error.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Unrecognized { span }
            | Self::InvalidPayload { span, .. }
            | Self::UnterminatedMode { span, .. } => *span,
        }
    }

    /// Returns the first byte offset associated with this error.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.span().start
    }
}

impl Display for ScanError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unrecognized { span } => write!(formatter, "unrecognized input at {span}"),
            Self::InvalidPayload { span, message } => {
                write!(formatter, "invalid token payload at {span}: {message}")
            }
            Self::UnterminatedMode { span, mode } => {
                write!(formatter, "unterminated lexer mode {mode} at {span}")
            }
        }
    }
}

impl std::error::Error for ScanError {}

/// Iterates over tokens and scanning errors.
///
/// Invalid input and payload errors consume their byte ranges and scanning
/// continues. A rule's mode action still takes effect when its
/// payload conversion fails.
pub struct Scan<'input, T> {
    input: &'input str,
    offset: usize,
    modes: Vec<ModeFrame>,
    finished: bool,
    marker: PhantomData<T>,
}

struct ModeFrame {
    id: usize,
    opened_at: usize,
}

impl<'input, T> Scan<'input, T> {
    pub(crate) fn new(input: &'input str) -> Self {
        Self {
            input,
            offset: 0,
            modes: vec![ModeFrame {
                id: 0,
                opened_at: 0,
            }],
            finished: false,
            marker: PhantomData,
        }
    }

    /// Returns the byte offset at which the next token will begin.
    pub const fn position(&self) -> usize {
        self.offset
    }
}

impl<T: Lexer> Scan<'_, T> {
    fn apply_mode_action(&mut self, action: ModeAction, start: usize) {
        match action {
            ModeAction::Stay => {}
            ModeAction::Begin(id) => {
                let frame = self
                    .modes
                    .last_mut()
                    .expect("the mode stack contains INITIAL");
                frame.id = id;
                frame.opened_at = start;
            }
            ModeAction::Push(id) => self.modes.push(ModeFrame {
                id,
                opened_at: start,
            }),
            ModeAction::Pop => {
                if self.modes.len() > 1 {
                    self.modes.pop();
                }
            }
        }
    }
}

impl<T: Lexer> Iterator for Scan<'_, T> {
    type Item = Result<Spanned<T>, ScanError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        loop {
            if self.offset == self.input.len() {
                self.finished = true;
                return self.modes.get(1).map(|frame| {
                    Err(ScanError::UnterminatedMode {
                        span: Span::new(frame.opened_at, self.offset),
                        mode: T::mode_name(frame.id),
                    })
                });
            }

            let start = self.offset;
            let mode = self
                .modes
                .last()
                .expect("the mode stack contains INITIAL")
                .id;
            let (rule, length) = match match_prefix::<T>(&self.input[start..], mode) {
                PrefixMatch::Accepted { rule, length } => (rule, length),
                PrefixMatch::Rejected {
                    examined: _examined,
                } => {
                    // Keep one-character recovery for now. The examined length is
                    // available when the recovery policy is revised.
                    self.offset += self.input[start..]
                        .chars()
                        .next()
                        .expect("the input is not empty")
                        .len_utf8();
                    return Some(Err(ScanError::Unrecognized {
                        span: Span::new(start, self.offset),
                    }));
                }
            };

            assert!(length > 0, "a lexer rule matched empty input");
            let text = &self.input[start..start + length];
            let RuleOutput { token, mode_action } = match T::execute_rule(rule, text) {
                Ok(output) => output,
                Err(error) => {
                    self.offset += length;
                    self.apply_mode_action(error.mode_action(), start);
                    return Some(Err(ScanError::InvalidPayload {
                        span: Span::new(start, self.offset),
                        message: error.to_string(),
                    }));
                }
            };
            self.offset += length;
            self.apply_mode_action(mode_action, start);
            if let Some(token) = token {
                return Some(Ok(Spanned {
                    token,
                    span: Span::new(start, self.offset),
                }));
            }
        }
    }
}

enum PrefixMatch {
    Accepted { rule: usize, length: usize },
    Rejected { examined: usize },
}

fn match_prefix<T: Automaton>(input: &str, mode: usize) -> PrefixMatch {
    let Some(mut state) = T::start_state(mode) else {
        return PrefixMatch::Rejected { examined: 0 };
    };
    let mut latest = T::accepting_rule(state).map(|rule| (rule, 0));
    let mut examined = 0;

    for (index, &byte) in input.as_bytes().iter().enumerate() {
        let Some(next) = T::next_state(state, byte) else {
            break;
        };
        state = next;
        examined = index + 1;
        if let Some(rule) = T::accepting_rule(state) {
            latest = Some((rule, examined));
        }
    }

    latest.map_or(PrefixMatch::Rejected { examined }, |(rule, length)| {
        PrefixMatch::Accepted { rule, length }
    })
}

#[cfg(test)]
mod tests {
    use super::{PrefixMatch, match_prefix};
    use crate::lexer::{Automaton, RuleOutput};
    use crate::payload::PayloadError;

    struct AStarB;

    impl Automaton for AStarB {
        fn start_state(mode: usize) -> Option<usize> {
            (mode == 0).then_some(0)
        }

        fn next_state(state: usize, byte: u8) -> Option<usize> {
            match (state, byte) {
                (0, b'a') => Some(0),
                (0, b'b') => Some(1),
                _ => None,
            }
        }

        fn accepting_rule(state: usize) -> Option<usize> {
            (state == 1).then_some(0)
        }

        fn execute_rule(_: usize, _: &str) -> Result<RuleOutput<Self>, PayloadError> {
            unreachable!("prefix matching does not execute a rule")
        }

        fn mode_name(_: usize) -> &'static str {
            "INITIAL"
        }
    }

    #[test]
    fn a_rejected_prefix_reports_how_many_bytes_the_automaton_examined() {
        assert!(matches!(
            match_prefix::<AStarB>("aaaa!", 0),
            PrefixMatch::Rejected { examined: 4 }
        ));
    }

    #[test]
    fn an_accepted_prefix_reports_the_winning_rule_and_length() {
        assert!(matches!(
            match_prefix::<AStarB>("aaab!", 0),
            PrefixMatch::Accepted { rule: 0, length: 4 }
        ));
    }
}
