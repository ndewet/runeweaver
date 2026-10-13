//! A compile-time lexer generator for UTF-8 strings and raw bytes.
//!
//! Derive `Lexer` for a token enum and place one `#[token("pattern")]`
//! attribute on each variant.
//!
//! The scanner uses `core` and `alloc`. Disable default features to use the
//! byte matcher and spans without an allocator. The `derive` feature enables
//! the macro and allocation support.

#![deny(dead_code)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

extern crate self as runeweaver;

#[cfg(feature = "derive")]
pub use runeweaver_derive::Lexer;

pub mod bytes;
#[doc(hidden)]
#[cfg(feature = "alloc")]
pub mod lexer;
#[doc(hidden)]
#[cfg(feature = "alloc")]
pub mod payload;
#[cfg(feature = "alloc")]
mod scan;
mod source;
mod span;

#[cfg(feature = "alloc")]
pub use lexer::Lexer;
#[cfg(feature = "alloc")]
pub use scan::ByteScanner;
#[cfg(feature = "alloc")]
pub use scan::{Scan, ScanError, Scanner, SpannedScan};
pub use source::Source;
pub use span::{Span, Spanned};
