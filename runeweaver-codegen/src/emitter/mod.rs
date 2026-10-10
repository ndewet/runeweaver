//! Renders completed lexer execution plans as Rust source.
//!
//! Execution planning belongs to [`crate::ir`]. This module owns Rust syntax,
//! generated identifiers, and token construction.

#[allow(clippy::module_inception)]
mod emitter;
mod selector;

pub(crate) use self::emitter::emit;
