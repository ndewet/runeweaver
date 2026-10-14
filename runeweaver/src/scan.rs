//! Token iteration and recoverable scan errors.

use crate::lexer::ScanStep;
use crate::payload::PayloadError;
use crate::state::LexicalState;
use crate::{Checkpoint, Lexer, ResumeError, Span, Spanned};
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;

/// An error encountered while scanning input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError<E = String> {
    /// No rule accepted the character at this UTF-8 byte range.
    Unrecognized {
        /// The UTF-8 byte range of the unrecognized character.
        span: Span,
    },
    /// A winning rule's payload conversion failed.
    InvalidPayload {
        /// The UTF-8 byte range matched by the rule with the invalid payload.
        span: Span,
        /// The payload conversion error, formatted as text by default.
        message: E,
    },
    /// Input ended while a pushed lexer mode was still active.
    UnterminatedMode {
        /// The range from the mode-opening rule through end of input.
        span: Span,
        /// The unclosed mode's declared name.
        mode: &'static str,
    },
}

impl<E> ScanError<E> {
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
    pub fn invalid_payload(span: Span, error: PayloadError<E>) -> Self {
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

impl<E: Display> Display for ScanError<E> {
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

impl<E: std::fmt::Debug + Display> std::error::Error for ScanError<E> {}

/// Holds the mutable state for one scan of an input string.
///
/// Invalid input and payload errors consume their byte ranges and scanning
/// continues. A rule's mode action still takes effect when its
/// payload conversion fails.
pub struct Scanner<'input, T, Extras = <T as crate::lexer::Automaton<'input>>::Extras> {
    input: &'input str,
    offset: usize,
    span: Span,
    state: LexicalState,
    finished: bool,
    in_rule: bool,
    marker: PhantomData<T>,
    /// User state kept for the lifetime of the scan.
    pub extras: Extras,
}

/// An iterator over tokens and scanning errors.
///
/// This alias preserves the original name for [`Scanner`]. New code can name
/// the stateful object directly.
pub type Scan<'input, T> = Scanner<'input, T, <T as crate::lexer::Automaton<'input>>::Extras>;

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

impl<'input, T: crate::lexer::Automaton<'input>> Scanner<'input, T, T::Extras> {
    /// Creates a scanner for `input` with user state.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{Lexer, Scanner};
    ///
    /// #[derive(Lexer)]
    /// #[lexer(extras = u32)]
    /// enum Token {
    ///     #[token("[a-z]+")]
    ///     Word,
    /// }
    ///
    /// let scanner = Scanner::<Token>::with_extras("word", 7_u32);
    /// assert_eq!(scanner.extras, 7);
    /// ```
    pub fn with_extras(input: &'input str, extras: T::Extras) -> Self {
        Self {
            input,
            offset: 0,
            span: Span::at(0),
            state: LexicalState::default(),
            finished: false,
            in_rule: false,
            marker: PhantomData,
            extras,
        }
    }

    /// Resumes scanning from a checkpoint in an unchanged input prefix.
    ///
    /// The caller must check the input revision and replay safety of callbacks.
    /// The most recent span starts empty at the saved position.
    ///
    /// # Errors
    ///
    /// Returns an error if a saved offset is outside input or splits a codepoint.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{Lexer, Scanner};
    /// #[derive(Debug, PartialEq, Lexer)]
    /// enum Token { #[token("a")] A, #[token("b")] B }
    /// let mut scan = Token::scan("aa");
    /// let _ = scan.next();
    /// let mut resumed = Scanner::resume("ab", scan.checkpoint())?;
    /// assert_eq!(resumed.next(), Some(Ok(Token::B)));
    /// # Ok::<(), runeweaver::ResumeError>(())
    /// ```
    pub fn resume(
        input: &'input str,
        checkpoint: Checkpoint<T, T::Extras>,
    ) -> Result<Self, ResumeError> {
        for offset in std::iter::once(checkpoint.position).chain(checkpoint.state.positions()) {
            if offset > checkpoint.position || !input.is_char_boundary(offset) {
                return Err(ResumeError { offset });
            }
        }
        Ok(Self {
            input,
            offset: checkpoint.position,
            span: Span::at(checkpoint.position),
            state: checkpoint.state,
            finished: false,
            in_rule: false,
            marker: PhantomData,
            extras: checkpoint.extras,
        })
    }
}

impl<'input, T, Extras> Scanner<'input, T, Extras> {
    /// Saves the current position, modes, and a copy of user state.
    ///
    /// # Panics
    ///
    /// This function panics if a rule callback requests a checkpoint.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Lexer;
    /// #[derive(Lexer)]
    /// enum Token { #[token("a")] A }
    /// let scan = Token::scan("a");
    /// assert_eq!(scan.checkpoint().position(), 0);
    /// ```
    pub fn checkpoint(&self) -> Checkpoint<T, Extras>
    where
        Extras: Clone,
    {
        assert!(!self.in_rule, "a checkpoint requires a completed rule");
        Checkpoint {
            position: self.offset,
            state: self.state.clone(),
            extras: self.extras.clone(),
            marker: PhantomData,
        }
    }

    /// Saves scan state without copying user state.
    ///
    /// # Panics
    ///
    /// This function panics if a rule callback requests a checkpoint.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Lexer;
    /// #[derive(Lexer)]
    /// enum Token { #[token("a")] A }
    /// assert_eq!(Token::scan("a").into_checkpoint().position(), 0);
    /// ```
    pub fn into_checkpoint(self) -> Checkpoint<T, Extras> {
        assert!(!self.in_rule, "a checkpoint requires a completed rule");
        Checkpoint {
            position: self.offset,
            state: self.state,
            extras: self.extras,
            marker: PhantomData,
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
    /// This function panics if generated code reports an invalid input span.
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
pub struct SpannedScan<'input, T, Extras = <T as crate::lexer::Automaton<'input>>::Extras> {
    scan: Scanner<'input, T, Extras>,
}

impl<'input, T, Extras> SpannedScan<'input, T, Extras> {
    /// Returns the scanner for source and state inspection.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Lexer;
    /// #[derive(Lexer)]
    /// enum Token { #[token("a")] A }
    /// assert_eq!(Token::scan("a").spanned().scanner().source(), "a");
    /// ```
    pub const fn scanner(&self) -> &Scanner<'input, T, Extras> {
        &self.scan
    }

    /// Returns the scanner to change its state.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Lexer;
    /// #[derive(Lexer)]
    /// #[lexer(extras = usize)]
    /// enum Token { #[token("a")] A }
    /// let mut scan = Token::scan("a").spanned();
    /// scan.scanner_mut().extras = 3;
    /// assert_eq!(scan.scanner().extras, 3);
    /// ```
    pub fn scanner_mut(&mut self) -> &mut Scanner<'input, T, Extras> {
        &mut self.scan
    }

    /// Returns the scanner to continue without attached spans.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Lexer;
    /// #[derive(Lexer)]
    /// enum Token { #[token("a")] A }
    /// assert_eq!(Token::scan("a").spanned().into_scanner().count(), 1);
    /// ```
    pub fn into_scanner(self) -> Scanner<'input, T, Extras> {
        self.scan
    }
}

impl<'input, T: Lexer<'input>> Scanner<'input, T, T::Extras> {
    #[inline(always)]
    fn next_token(&mut self) -> Option<Result<T, ScanError<T::Error>>> {
        loop {
            if self.offset == self.input.len() {
                self.finished = true;
                if let Some(frame) = self.state.unclosed() {
                    self.span = Span::new(frame.opened_at, self.offset);
                    return Some(Err(ScanError::unterminated_mode(
                        self.span,
                        T::mode_name(frame.id),
                    )));
                }
                return None;
            }

            let mode = self.state.mode();
            let position = self.offset;
            self.in_rule = true;
            let step = T::next_token(self, mode);
            self.in_rule = false;
            match step {
                ScanStep::End => {
                    assert_eq!(
                        self.offset,
                        self.input.len(),
                        "invalid generated end position"
                    );
                }
                ScanStep::Matched { result, action } => {
                    assert!(
                        self.offset > position,
                        "a generated match must consume input"
                    );
                    self.state.apply(action, self.span.start);
                    match result {
                        Ok(Some(token)) => return Some(Ok(token)),
                        Ok(None) => {}
                        Err(error) => return Some(Err(error)),
                    }
                }
            }
        }
    }
}

impl<'input, T: Lexer<'input>> Iterator for Scanner<'input, T, T::Extras> {
    type Item = Result<T, ScanError<T::Error>>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        self.next_token()
    }
}

impl<'input, T: Lexer<'input>> Iterator for SpannedScan<'input, T, T::Extras> {
    type Item = Result<Spanned<T>, ScanError<T::Error>>;

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
