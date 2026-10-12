//! Builds lexer data for the runeweaver derive macro.
//!
//! This crate parses lexer patterns and builds automata for generated matchers.
//! Lexer authors use the derive API instead of this crate.
//!
//! Functions return a [`Result`] for invalid lexer input. A panic reports a
//! defect in runeweaver.

#![cfg_attr(test, allow(clippy::unwrap_used))]
#![deny(dead_code)]

// The derive entry point will use these modules when the lexer pipeline exists.
mod automata;
mod compiler;
mod emitter;
mod ir;
mod regex;
mod selector;
mod specification;

pub use compiler::{
    CompileError, compile, compile_with_modes, compile_with_modes_and_extras_at,
    compile_with_modes_at,
};
pub use specification::{ModeAction, RuleSpec};
