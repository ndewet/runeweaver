//! Configures the generated scanner interface.

use crate::SourceLifetime;
use proc_macro2::TokenStream;

/// Supplies named options for generated scanner methods.
#[doc(hidden)]
pub struct CompileOptions {
    /// Path to the runtime crate.
    pub runtime: TokenStream,
    /// User state type.
    pub extras: TokenStream,
    /// Payload conversion error type.
    pub error: TokenStream,
    /// Input lifetime and its bounds.
    pub source: SourceLifetime,
    /// Additional lexical modes.
    pub modes: Vec<String>,
    /// Whether actions use scanner state.
    pub uses_scanner: bool,
    /// Whether tokens borrow the input.
    pub uses_source: bool,
}
