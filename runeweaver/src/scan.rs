//! Token iteration and recoverable scan errors.

use crate::lexer::ModeAction;
use crate::payload::PayloadError;
use crate::{Lexer, Span, Spanned};
use alloc::{string::String, vec::Vec};
use core::fmt::{Display, Formatter};
use core::marker::PhantomData;

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
    /// Creates an error for an input character that no rule accepted.
    #[doc(hidden)]
    #[cold]
    #[inline(never)]
    #[must_use]
    pub fn unrecognized(span: Span) -> Self {
        Self::Unrecognized { span }
    }

    /// Creates an error for a failed payload conversion.
    #[doc(hidden)]
    #[cold]
    #[inline(never)]
    #[must_use]
    pub fn invalid_payload(span: Span, error: PayloadError) -> Self {
        Self::InvalidPayload {
            span,
            message: error.into_message(),
        }
    }

    /// Creates an error for an unclosed lexer mode.
    #[doc(hidden)]
    #[cold]
    #[inline(never)]
    #[must_use]
    pub fn unterminated_mode(span: Span, mode: &'static str) -> Self {
        Self::UnterminatedMode { span, mode }
    }

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
    fn fmt(&self, formatter: &mut Formatter<'_>) -> core::fmt::Result {
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

impl core::error::Error for ScanError {}

/// Holds the mutable state for one scan of an input string.
///
/// Invalid input and payload errors consume their byte ranges and scanning
/// continues. A rule's mode action still takes effect when its
/// payload conversion fails.
pub struct Scanner<'input, T, Extras = ()> {
    input: &'input str,
    offset: usize,
    span: Span,
    mode: ModeFrame,
    parent_modes: Vec<ModeFrame>,
    finished: bool,
    marker: PhantomData<T>,
    /// User state kept for the lifetime of the scan.
    pub extras: Extras,
}

/// An iterator over tokens and scanning errors.
///
/// This alias preserves the original name for [`Scanner`]. New code can name
/// the stateful object directly.
pub type Scan<'input, T> = Scanner<'input, T, <T as crate::lexer::Automaton<'input>>::Extras>;

#[derive(Clone, Copy)]
struct ModeFrame {
    id: usize,
    opened_at: usize,
}

impl<'input, T: crate::lexer::Automaton<'input>> Scanner<'input, T, T::Extras>
where
    T::Extras: Default,
{
    /// Creates a scanner for `input`.
    ///
    /// [`Lexer::scan`](crate::Lexer::scan) usually provides better type
    /// inference.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{Lexer, Scanner};
    ///
    /// #[derive(Lexer)]
    /// enum Token {
    ///     #[token("[a-z]+")]
    ///     Word,
    /// }
    ///
    /// let scanner = Scanner::<Token>::new("word");
    /// assert_eq!(scanner.position(), 0);
    /// ```
    #[must_use]
    pub fn new(input: &'input str) -> Self {
        Self::with_extras(input, T::Extras::default())
    }
}

impl<'input, T, Extras> Scanner<'input, T, Extras> {
    /// Creates a scanner for `input` with user state.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{Lexer, Scanner};
    ///
    /// #[derive(Lexer)]
    /// enum Token {
    ///     #[token("[a-z]+")]
    ///     Word,
    /// }
    ///
    /// let scanner = Scanner::<Token, _>::with_extras("word", 7_u32);
    /// assert_eq!(scanner.extras, 7);
    /// ```
    pub fn with_extras(input: &'input str, extras: Extras) -> Self {
        Self {
            input,
            offset: 0,
            span: Span::at(0),
            mode: ModeFrame {
                id: 0,
                opened_at: 0,
            },
            parent_modes: Vec::new(),
            finished: false,
            marker: PhantomData,
            extras,
        }
    }

    /// Returns the complete input string.
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
    /// assert_eq!(Token::scan("word").source(), "word");
    /// ```
    #[inline(always)]
    pub const fn source(&self) -> &'input str {
        self.input
    }

    /// Returns the input after the current scan position.
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
    /// let mut scanner = Token::scan("word rest");
    /// let _ = scanner.next();
    /// assert_eq!(scanner.remainder(), " rest");
    /// ```
    pub fn remainder(&self) -> &'input str {
        &self.input[self.offset..]
    }

    /// Returns the byte offset at which the next scan starts.
    #[inline(always)]
    pub const fn position(&self) -> usize {
        self.offset
    }

    /// Moves the current scan position forward by `bytes`.
    ///
    /// This operation also extends the current span. It supports callbacks
    /// that consume input beyond the matched rule.
    ///
    /// # Panics
    ///
    /// This function panics if the new position exceeds the input or splits a
    /// UTF-8 codepoint.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{Lexer, Span};
    ///
    /// #[derive(Lexer)]
    /// enum Token {
    ///     #[token("[a-z]+")]
    ///     Word,
    /// }
    ///
    /// let mut scanner = Token::scan("word!");
    /// let _ = scanner.next();
    /// scanner.bump(1);
    /// assert_eq!(scanner.span(), Span::new(0, 5));
    /// assert_eq!(scanner.remainder(), "");
    /// ```
    pub fn bump(&mut self, bytes: usize) {
        let end = self.offset.checked_add(bytes);
        assert!(
            end.is_some_and(|end| end <= self.input.len() && self.input.is_char_boundary(end)),
            "invalid scanner bump"
        );
        let Some(end) = end else {
            unreachable!("the scanner position was checked")
        };
        self.offset = end;
        self.span.end = end;
    }

    #[doc(hidden)]
    #[inline(always)]
    pub fn __set_position(&mut self, position: usize) {
        debug_assert!(
            position <= self.input.len() && self.input.is_char_boundary(position),
            "invalid scanner position"
        );
        self.offset = position;
    }

    #[doc(hidden)]
    #[inline(always)]
    pub fn __set_match(&mut self, start: usize, end: usize) {
        debug_assert!(
            start <= end
                && end <= self.input.len()
                && self.input.is_char_boundary(start)
                && self.input.is_char_boundary(end),
            "invalid scanner match"
        );
        self.offset = end;
        self.span = Span::new(start, end);
    }

    /// Returns the byte range of the most recent token or error.
    ///
    /// The range is empty at byte zero before the first scan result.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{Lexer, Span};
    ///
    /// #[derive(Lexer)]
    /// enum Token {
    ///     #[token("[a-z]+")]
    ///     Word,
    /// }
    ///
    /// let mut scan = Token::scan("word");
    /// let _ = scan.next();
    /// assert_eq!(scan.span(), Span::new(0, 4));
    /// ```
    #[inline(always)]
    pub const fn span(&self) -> Span {
        self.span
    }

    /// Returns the input text of the most recent token or error.
    ///
    /// The text is empty before the first scan result.
    ///
    /// # Panics
    ///
    /// Panics if a manually implemented [`Lexer`] reports a span outside its input.
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
    /// let mut scan = Token::scan("word");
    /// let _ = scan.next();
    /// assert_eq!(scan.slice(), "word");
    /// ```
    pub fn slice(&self) -> &'input str {
        self.span
            .text(self.input)
            .expect("a scan span is valid for its input")
    }

    /// Adds the current byte range to each scanned token.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{Lexer, Span};
    ///
    /// #[derive(Debug, PartialEq, Lexer)]
    /// enum Token {
    ///     #[token("[a-z]+")]
    ///     Word,
    /// }
    ///
    /// let token = Token::scan("word").spanned().next().expect("one token")?;
    /// assert_eq!(token.token, Token::Word);
    /// assert_eq!(token.span, Span::new(0, 4));
    /// # Ok::<(), runeweaver::ScanError>(())
    /// ```
    pub fn spanned(self) -> SpannedScan<'input, T, Extras> {
        SpannedScan { scan: self }
    }
}

/// Iterates over tokens with their byte ranges and scanning errors.
pub struct SpannedScan<'input, T, Extras = ()> {
    scan: Scanner<'input, T, Extras>,
}

impl<'input, T: Lexer<'input>> Scanner<'input, T, T::Extras> {
    #[inline(always)]
    fn apply_mode_action(&mut self, action: ModeAction, start: usize) {
        match action {
            ModeAction::Stay => {}
            ModeAction::Begin(id) => {
                self.mode.id = id;
                self.mode.opened_at = start;
            }
            ModeAction::Push(id) => {
                self.parent_modes.push(self.mode);
                self.mode = ModeFrame {
                    id,
                    opened_at: start,
                };
            }
            ModeAction::Pop => {
                if let Some(parent) = self.parent_modes.pop() {
                    self.mode = parent;
                }
            }
        }
    }

    #[inline(always)]
    fn next_token(&mut self) -> Option<Result<T, ScanError>> {
        loop {
            if self.offset == self.input.len() {
                self.finished = true;
                if !self.parent_modes.is_empty() {
                    let frame = self.parent_modes.get(1).unwrap_or(&self.mode);
                    self.span = Span::new(frame.opened_at, self.offset);
                    return Some(Err(ScanError::unterminated_mode(
                        self.span,
                        T::mode_name(frame.id),
                    )));
                }
                return None;
            }

            let mode = self.mode.id;
            let result = T::next_token(self, mode);
            let Some(result) = result else {
                continue;
            };

            let start = self.span.start;
            match result {
                Ok((token, mode_action)) => {
                    self.apply_mode_action(mode_action, start);
                    if let Some(token) = token {
                        return Some(Ok(token));
                    }
                }
                Err((error, mode_action)) => {
                    self.apply_mode_action(mode_action, start);
                    return Some(Err(error));
                }
            }
        }
    }
}

impl<'input, T: Lexer<'input>> Iterator for Scanner<'input, T, T::Extras> {
    type Item = Result<T, ScanError>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        self.next_token()
    }
}

impl<'input, T: Lexer<'input>> Iterator for SpannedScan<'input, T, T::Extras> {
    type Item = Result<Spanned<T>, ScanError>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.scan.next().map(|result| {
            result.map(|token| Spanned {
                token,
                span: self.scan.span,
            })
        })
    }
}
