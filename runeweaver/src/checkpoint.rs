//! Saves scan positions and lexical state for later input snapshots.

use crate::state::LexicalState;
use std::marker::PhantomData;

/// Saves a completed scan position, its lexical modes, and user state.
///
/// A checkpoint contains no input reference unless its user state contains one.
/// Resume it only when the input prefix before its position is unchanged.
/// Callbacks must produce the same results when replayed from saved user state.
pub struct Checkpoint<T, Extras> {
    pub(crate) position: usize,
    pub(crate) state: LexicalState,
    pub(crate) extras: Extras,
    pub(crate) marker: PhantomData<fn() -> T>,
}

impl<T, Extras: Clone> Clone for Checkpoint<T, Extras> {
    fn clone(&self) -> Self {
        Self {
            position: self.position,
            state: self.state.clone(),
            extras: self.extras.clone(),
            marker: PhantomData,
        }
    }
}

impl<T, Extras> Checkpoint<T, Extras> {
    /// Returns the byte offset where scanning resumes.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Lexer;
    /// #[derive(Lexer)]
    /// enum Token { #[token("a")] A }
    /// let mut scan = Token::scan("aa");
    /// let _ = scan.next();
    /// assert_eq!(scan.checkpoint().position(), 1);
    /// ```
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Compares modes and user state that affect subsequent matching.
    ///
    /// This comparison excludes positions used for diagnostics. Equal context
    /// alone does not prove that cached tokens or diagnostics can be reused.
    /// User state equality must include all state that affects callbacks.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Lexer;
    /// #[derive(Lexer)]
    /// enum Token { #[token("a")] A }
    /// let mut scan = Token::scan("aa");
    /// let before = scan.checkpoint();
    /// let _ = scan.next();
    /// assert!(before.same_context(&scan.checkpoint()));
    /// ```
    pub fn same_context(&self, other: &Self) -> bool
    where
        Extras: PartialEq,
    {
        self.state.same_context(&other.state) && self.extras == other.extras
    }
}
