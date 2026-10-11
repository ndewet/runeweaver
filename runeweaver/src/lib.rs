//! Provides runtime support for generated runeweaver lexers.
//!
//! Derive [`Lexer`] for a token enum and place one `#[token("pattern")]`
//! attribute on each variant.

#![deny(dead_code)]

extern crate self as runeweaver;

pub use runeweaver_derive::Lexer;

mod payload;
mod span;

pub use payload::PayloadResult;
pub use span::Span;

use std::marker::PhantomData;
use std::str::FromStr;
use std::{
    error::Error,
    fmt::{Display, Formatter},
};

/// A token together with its byte range in the input.
///
/// # Examples
///
/// ```
/// use runeweaver::{Span, Spanned};
///
/// let token = Spanned {
///     token: "name",
///     span: Span::new(0, 4),
/// };
/// assert_eq!(token.span.text("name 42"), Some("name"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    /// The token accepted by the lexer.
    pub token: T,
    /// The half-open UTF-8 byte range matched by this token.
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Transforms the token while preserving its span.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{Span, Spanned};
    ///
    /// let token = Spanned {
    ///     token: "name",
    ///     span: Span::new(2, 6),
    /// };
    /// let length = token.map(str::len);
    /// assert_eq!(length.token, 4);
    /// assert_eq!(length.span, Span::new(2, 6));
    /// ```
    #[must_use]
    pub fn map<U>(self, operation: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            token: operation(self.token),
            span: self.span,
        }
    }

    /// Tries to transform the token while preserving its span.
    ///
    /// # Errors
    ///
    /// Returns the error from `transformation` when the token cannot be
    /// transformed.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{Span, Spanned};
    ///
    /// let token = Spanned {
    ///     token: "42",
    ///     span: Span::new(5, 7),
    /// };
    /// let number = token.try_map(str::parse::<u8>)?;
    /// assert_eq!(number.token, 42);
    /// assert_eq!(number.span, Span::new(5, 7));
    /// # Ok::<(), std::num::ParseIntError>(())
    /// ```
    pub fn try_map<U, E>(
        self,
        transformation: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<Spanned<U>, E> {
        transformation(self.token).map(|token| Spanned {
            token,
            span: self.span,
        })
    }
}

/// Holds the items that generated code names, and that callers do not.
#[doc(hidden)]
pub mod __private {
    /// Restricts [`Lexer`](crate::Lexer) to the types that the derive makes.
    pub trait Sealed {}
}

/// A failure produced while converting an accepted lexeme into a payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadError {
    message: String,
    transition: Transition,
}

/// The result of selecting and executing one lexer rule.
#[doc(hidden)]
pub type RuleScan<T> = Result<(Option<T>, usize, usize, Transition), (PayloadError, usize, usize)>;

/// A generated lexer rule's update to the start-condition stack.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Stay,
    Begin(usize),
    Push(usize),
    Pop,
}

impl PayloadError {
    /// Creates a payload-conversion error with a human-readable message.
    #[doc(hidden)]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            transition: Transition::Stay,
        }
    }

    /// Associates the winning rule's lexical-mode transition with this error.
    #[doc(hidden)]
    pub fn with_transition(mut self, transition: Transition) -> Self {
        self.transition = transition;
        self
    }

    fn transition(&self) -> Transition {
        self.transition
    }
}

impl Display for PayloadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for PayloadError {}

/// Parses one owned token payload with its [`FromStr`] implementation.
#[doc(hidden)]
pub fn parse_payload<T>(text: &str) -> Result<T, PayloadError>
where
    T: FromStr,
    T::Err: Display,
{
    text.parse()
        .map_err(|error: T::Err| PayloadError::new(error.to_string()))
}

/// An error encountered while scanning input.
///
/// # Examples
///
/// ```
/// use runeweaver::{ScanError, Span};
///
/// let error: ScanError = ScanError::Unrecognized {
///     span: Span::new(4, 5),
/// };
/// assert!(matches!(error, ScanError::Unrecognized { span } if span.len() == 1));
/// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{ScanError, Span};
    ///
    /// let error: ScanError = ScanError::Unrecognized {
    ///     span: Span::new(4, 5),
    /// };
    /// assert_eq!(error.span(), Span::new(4, 5));
    /// ```
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Unrecognized { span }
            | Self::InvalidPayload { span, .. }
            | Self::UnterminatedMode { span, .. } => *span,
        }
    }

    /// Returns the first byte offset associated with this error.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{ScanError, Span};
    ///
    /// let error = ScanError::Unrecognized {
    ///     span: Span::new(7, 8),
    /// };
    /// assert_eq!(error.offset(), 7);
    /// ```
    #[must_use]
    pub const fn offset(&self) -> u64 {
        match self {
            Self::Unrecognized { span }
            | Self::InvalidPayload { span, .. }
            | Self::UnterminatedMode { span, .. } => span.start,
        }
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

impl Error for ScanError {}

/// Iterates over tokens and scanning errors.
///
/// Invalid input and payload errors consume their byte ranges and scanning
/// continues. A rule's lexical-mode transition still takes effect when its
/// payload conversion fails.
///
/// # Examples
///
/// ```
/// use runeweaver::Lexer;
///
/// #[derive(Debug, PartialEq, Lexer)]
/// enum Token {
///     #[token("[a-z]+")]
///     Word,
/// }
///
/// assert_eq!(Token::scan("word").count(), 1);
/// ```
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
    fn new(input: &'input str) -> Self {
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
    fn apply_transition(&mut self, transition: Transition, start: usize) {
        match transition {
            Transition::Stay => {}
            Transition::Begin(id) => {
                let frame = self
                    .modes
                    .last_mut()
                    .expect("the mode stack contains INITIAL");
                frame.id = id;
                frame.opened_at = start;
            }
            Transition::Push(id) => self.modes.push(ModeFrame {
                id,
                opened_at: start,
            }),
            Transition::Pop => {
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
                        span: Span::new(frame.opened_at as u64, self.offset as u64),
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
            let Some(result) = T::scan_one(&self.input[start..], mode) else {
                self.offset += self.input[start..]
                    .chars()
                    .next()
                    .expect("the input is not empty")
                    .len_utf8();
                return Some(Err(ScanError::Unrecognized {
                    span: Span::new(start as u64, self.offset as u64),
                }));
            };

            let (token, token_start, consumed, transition) = match result {
                Ok((token, token_start, consumed, transition)) => {
                    (token, token_start, consumed, transition)
                }
                Err((error, token_start, consumed)) => {
                    self.offset += consumed;
                    self.apply_transition(error.transition(), start + token_start);
                    return Some(Err(ScanError::InvalidPayload {
                        span: Span::new((start + token_start) as u64, self.offset as u64),
                        message: error.to_string(),
                    }));
                }
            };
            assert!(consumed > 0, "a generated lexer consumed an empty match");
            self.offset += consumed;
            self.apply_transition(transition, start + token_start);
            if let Some(token) = token {
                return Some(Ok(Spanned {
                    token,
                    span: Span::new((start + token_start) as u64, self.offset as u64),
                }));
            }
        }
    }
}

/// Scans UTF-8 input with a generated lexer.
///
/// Only the `Lexer` derive macro implements this trait.
///
/// # Examples
///
/// ```
/// use runeweaver::Lexer;
///
/// #[derive(Debug, PartialEq, Lexer)]
/// enum Token {
///     #[token("[a-z]+")]
///     Word,
/// }
///
/// let token = Token::scan("word").next().expect("one token")?;
/// assert_eq!(token.token, Token::Word);
/// # Ok::<(), runeweaver::ScanError>(())
/// ```
pub trait Lexer: __private::Sealed + Sized {
    /// Scans one token or skipped rule from the start of a string.
    ///
    /// `None` means no rule accepts a prefix. A successful result contains a
    /// skipped rule or token plus its consumed byte length. An error means a
    /// winning rule could not convert its payload.
    #[doc(hidden)]
    fn scan_one(input: &str, mode: usize) -> Option<RuleScan<Self>>;

    /// Returns a generated start-condition name for diagnostics.
    #[doc(hidden)]
    fn mode_name(mode: usize) -> &'static str;

    /// Scans tokens from `input`.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Lexer;
    ///
    /// #[derive(Lexer)]
    /// enum Token {
    ///     #[token("[a-z]+")]
    ///     Word,
    /// }
    ///
    /// assert_eq!(Token::scan("word").count(), 1);
    /// ```
    fn scan(input: &str) -> Scan<'_, Self> {
        Scan::new(input)
    }
}
