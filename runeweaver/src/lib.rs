//! A compile-time lexer generator for UTF-8 strings and raw bytes.
//!
//! Derive `Lexer` for a token enum and place one `#[token("pattern")]`
//! attribute on each variant.

#![deny(dead_code)]

extern crate self as runeweaver;

pub use runeweaver_derive::Lexer;

#[doc(hidden)]
pub mod lexer;
#[doc(hidden)]
pub mod payload;
mod scan;
mod source;
mod span;

pub use lexer::Lexer;
pub use scan::ByteScanner;
pub use scan::{Scan, ScanError, Scanner, SpannedScan};
pub use source::Source;
pub use span::{Span, Spanned};
