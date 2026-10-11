//! The public lexer trait and the automaton interface used by generated lexers.

use crate::Scan;
use crate::payload::PayloadError;

/// A rule's effect after the runtime selects it.
#[doc(hidden)]
pub struct RuleOutput<T> {
    pub token: Option<T>,
    pub mode_action: ModeAction,
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

/// The byte-oriented automaton supplied by a generated lexer.
#[doc(hidden)]
pub trait Automaton: Sized {
    fn start_state(mode: usize) -> Option<usize>;
    fn next_state(state: usize, byte: u8) -> Option<usize>;
    fn accepting_rule(state: usize) -> Option<usize>;
    fn execute_rule(rule: usize, text: &str) -> Result<RuleOutput<Self>, PayloadError>;
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
/// assert_eq!(token.token, Token::Word);
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
    fn scan(input: &str) -> Scan<'_, Self> {
        Scan::new(input)
    }
}

impl<T: Automaton> Lexer for T {}
