//! A compile-time lexer generator for UTF-8 strings.
//!
//! Derive `Lexer` for a token enum and place one `#[token("pattern")]`
//! attribute on each variant.
//!
//! The runtime uses `core` and `alloc`. Disable the default `derive` feature
//! when the runtime does not need the derive macro.

#![deny(dead_code)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

extern crate self as runeweaver;

#[cfg(feature = "derive")]
pub use runeweaver_derive::Lexer;

#[doc(hidden)]
#[cfg(feature = "alloc")]
pub mod lexer;
#[doc(hidden)]
#[cfg(feature = "alloc")]
pub mod payload;
#[cfg(feature = "alloc")]
mod scan;
mod span;

#[cfg(feature = "alloc")]
pub use lexer::Lexer;
#[cfg(feature = "alloc")]
pub use scan::{Scan, ScanError, Scanner, SpannedScan};
pub use span::{Span, Spanned};
