//! The public lexer trait and the automaton interface used by generated lexers.

use crate::Scanner;

/// A generated scan result and its mode action.
#[doc(hidden)]
pub type ScanMatch<T> = Result<(Option<T>, ModeAction), (crate::ScanError, ModeAction)>;

/// A generated lexer rule's update to the mode stack.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeAction {
    Stay,
    Begin(usize),
    Push(usize),
    Pop,
}

/// The byte-oriented automaton supplied by a generated lexer.
#[doc(hidden)]
pub trait Automaton<'input, Input: crate::Source + ?Sized = str>: Sized {
    /// User state stored in this token type's scanner.
    type Extras;

    fn next_token(
        scanner: &mut Scanner<'input, Self, Self::Extras, Input>,
        mode: usize,
    ) -> Option<ScanMatch<Self>>;
    fn mode_name(mode: usize) -> &'static str;
}

/// Scans strings or raw bytes with a generated lexer.
///
/// Only the [`Lexer`](macro@crate::Lexer) derive macro implements this trait.
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
///
/// Select raw bytes with `#[lexer(source = [u8])]`:
///
/// ```
/// use runeweaver::Lexer;
///
/// #[derive(Debug, PartialEq, Lexer)]
/// #[lexer(source = [u8])]
/// enum Token { #[token(r"\xff")] Byte }
///
/// assert_eq!(Token::scan(b"\xff").next(), Some(Ok(Token::Byte)));
/// ```
pub trait Lexer<'input, Input: crate::Source + ?Sized = str>: Automaton<'input, Input> {
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
    fn scan(input: &'input Input) -> Scanner<'input, Self, Self::Extras, Input>
    where
        Self::Extras: Default,
    {
        Scanner::new(input)
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
        input: &'input Input,
        extras: Self::Extras,
    ) -> Scanner<'input, Self, Self::Extras, Input> {
        Scanner::with_extras(input, extras)
    }
}

impl<'input, Input: crate::Source + ?Sized, T: Automaton<'input, Input>> Lexer<'input, Input>
    for T
{
}
