//! The public lexer trait and the automaton interface used by generated lexers.

use crate::{Scanner, Span};

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
pub trait Automaton: Sized {
    fn next_token(
        input: &str,
        offset: &mut usize,
        span: &mut Span,
        mode: usize,
    ) -> Option<ScanMatch<Self>>;
    fn mode_name(mode: usize) -> &'static str;
}

/// Scans UTF-8 input with a generated lexer.
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
pub trait Lexer: Automaton + Sized {
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
    fn scan(input: &str) -> Scanner<'_, Self> {
        Scanner::new(input)
    }
}

impl<T: Automaton> Lexer for T {}
