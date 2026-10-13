//! Provides the common Logos API with Runeweaver as the lexer engine.
//!
//! Use this package as `logos` in `Cargo.toml`. The shim supports UTF-8 string
//! sources, token and regex rules, callbacks, extras, errors, and skip rules.

mod lexer;
pub mod source;

#[doc(hidden)]
pub mod internal;

pub use lexer::{Lexer, Span, SpannedIter};
pub use runeweaver_logos_shim_derive::Logos;
pub use source::Source;

use core::fmt::Debug;

/// Defines a token type that the compatibility lexer can scan.
pub trait Logos<'source>: Sized {
    /// User state stored in the lexer.
    type Extras;

    /// The input source type.
    type Source: Source + ?Sized + 'source;

    /// The error returned for invalid input or a failed callback.
    type Error: Default + Clone + PartialEq + Debug + 'source;

    /// Scans the next token.
    fn lex(lexer: &mut Lexer<'source, Self>) -> Option<Result<Self, Self::Error>>;

    /// Creates a lexer for `source`.
    fn lexer(source: &'source Self::Source) -> Lexer<'source, Self>
    where
        Self::Extras: Default,
    {
        Lexer::new(source)
    }

    /// Creates a lexer for `source` with user state.
    fn lexer_with_extras(
        source: &'source Self::Source,
        extras: Self::Extras,
    ) -> Lexer<'source, Self> {
        Lexer::with_extras(source, extras)
    }
}

/// Tells a callback to omit its current match.
pub struct Skip;

/// Selects whether a callback emits or omits its current match.
pub enum Filter<T> {
    /// Emits a payload or token.
    Emit(T),
    /// Omits the current match.
    Skip,
}

/// Selects whether a callback emits, omits, or rejects its current match.
pub enum FilterResult<T, E> {
    /// Emits a payload or token.
    Emit(T),
    /// Omits the current match.
    Skip,
    /// Emits an error.
    Error(E),
}

/// Tells a callback to omit its current match.
pub fn skip<'source, Token: Logos<'source>>(_: &mut Lexer<'source, Token>) -> Skip {
    Skip
}

#[doc(hidden)]
pub mod __private {
    pub use runeweaver;
}
