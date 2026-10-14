//! The public lexer trait and the automaton interface used by generated lexers.

use crate::Scanner;

/// Identifies a completed match or the end of input.
#[doc(hidden)]
pub enum ScanStep<T, E = String> {
    /// Consumes a rule or an unrecognized character.
    Matched {
        /// The emitted token, skipped input, or scan error.
        result: Result<Option<T>, crate::ScanError<E>>,
        /// The mode action, including when conversion fails.
        action: ModeAction,
    },
    /// Ends the scan at the input length.
    End,
}

/// A generated lexer rule's update to the mode stack.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeAction {
    Stay,
    Begin(usize),
    Push(usize),
    Pop,
}

/// Connects a generated lexer to its scanner.
///
/// This trait belongs to the generated-code interface. Handwritten
/// implementations are outside the supported public API.
///
/// A generated match consumes input and sets a valid UTF-8 span. A generated
/// end result moves the position to the input length. Mode identifiers refer
/// to the generated lexer's mode table.
#[doc(hidden)]
pub trait Automaton<'input>: Sized {
    /// User state stored in this token type's scanner.
    type Extras;
    /// Payload conversion error type.
    type Error;

    fn next_token(
        scanner: &mut Scanner<'input, Self, Self::Extras>,
        mode: usize,
    ) -> ScanStep<Self, Self::Error>;
    fn mode_name(mode: usize) -> &'static str;
}

/// Scans UTF-8 input with a generated lexer.
///
/// Use the [`Lexer`](macro@crate::Lexer) derive macro to implement this trait.
/// Handwritten implementations of its generated-code interface are unsupported.
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
/// assert_eq!(token, Token::Word);
/// # Ok::<(), runeweaver::ScanError>(())
/// ```
pub trait Lexer<'input>: Automaton<'input> + Sized {
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
    fn scan(input: &'input str) -> Scanner<'input, Self, Self::Extras>
    where
        Self::Extras: Default,
    {
        Scanner::with_extras(input, Self::Extras::default())
    }

    /// Scans tokens from `input` with user state.
    ///
    /// The scanner keeps `extras` while it scans the input.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Lexer;
    ///
    /// #[derive(Lexer)]
    /// #[lexer(extras = u32)]
    /// enum Token {
    ///     #[token("[a-z]+")]
    ///     Word,
    /// }
    ///
    /// let mut scanner = Token::scan_with_extras("word", 3);
    /// scanner.extras += 1;
    /// assert_eq!(scanner.extras, 4);
    /// ```
    fn scan_with_extras(
        input: &'input str,
        extras: Self::Extras,
    ) -> Scanner<'input, Self, Self::Extras> {
        Scanner::with_extras(input, extras)
    }
}

impl<'input, T: Automaton<'input>> Lexer<'input> for T {}
