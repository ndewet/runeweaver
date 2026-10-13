//! A compile-time lexer generator for UTF-8 strings.
//!
//! Derive [`Lexer`] for a token enum and place one `#[token("pattern")]`
//! attribute on each variant.
//!
//! The runtime uses `core` and `alloc`. Disable the default `derive` feature
//! when the runtime does not need the derive macro.

#![deny(dead_code)]
#![no_std]

extern crate alloc;

extern crate self as runeweaver;

#[cfg(feature = "derive")]
pub use runeweaver_derive::Lexer;

#[doc(hidden)]
pub mod lexer;
#[doc(hidden)]
pub mod payload;
mod scan;
mod span;

pub use lexer::Lexer;
pub use scan::{Scan, ScanError, Scanner, SpannedScan};
pub use span::{Span, Spanned};
