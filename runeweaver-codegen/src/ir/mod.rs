//! Plans execution of minimized byte DFAs before Rust emission.
//!
//! Construction owns graph analysis, saved matches, transition strategies,
//! and bounded path duplication. The completed plan contains no Rust tokens.
//! Inline transfers contain completed bodies. Header transfers continue loops,
//! and region transfers call functions. Emission does not need the DFA graph.

mod builder;
mod context;
mod cycles;
mod limits;
mod matcher;
mod regions;
mod rule;
mod selector;

pub(crate) use context::Context;
pub(crate) use matcher::Matcher;
pub(crate) use rule::RuleEffect;
pub(crate) use selector::{
    Argument, Block, Call, Dispatch, Function, FunctionBody, MatchValue, Position, Selector,
    Transfer, Width,
};
