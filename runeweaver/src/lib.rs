//! A compile-time lexer generator for UTF-8 strings.
//!
//! Derive [`Lexer`] for a token enum and place one `#[token("pattern")]`
//! attribute on each variant.

#![deny(dead_code)]

extern crate self as runeweaver;

pub use runeweaver_derive::Lexer;

mod checkpoint;
#[doc(hidden)]
pub mod lexer;
#[doc(hidden)]
pub mod payload;
mod resume_error;
mod scan;
mod span;
mod state;

pub use checkpoint::Checkpoint;
pub use lexer::Lexer;
pub use resume_error::ResumeError;
pub use scan::{Scan, ScanError, Scanner, SpannedScan};
pub use span::{Span, Spanned};
