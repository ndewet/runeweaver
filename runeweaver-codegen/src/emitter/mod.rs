//! Emits Rust source for minimized lexer automata.
//!
//! This module owns generated matcher layout and source rendering. Automaton
//! construction and minimization remain in [`crate::automata`].

mod cycles;
#[allow(clippy::module_inception)]
mod emitter;
mod regions;
mod selector;

pub(crate) use self::emitter::emit;
