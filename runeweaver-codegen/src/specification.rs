//! Public input types for a lexer specification.

use proc_macro2::TokenStream;
use quote::quote;

/// One rule supplied to the code generator.
pub struct RuleSpec {
    pub(crate) pattern: String,
    pub(crate) action: RuleAction,
    pub(crate) modes: Vec<String>,
    pub(crate) mode_action: ModeAction,
}

/// An action applied to the lexer mode stack after a rule is accepted.
///
/// # Examples
///
/// ```
/// use runeweaver_codegen::ModeAction;
///
/// let action = ModeAction::Push("String".into());
/// assert!(matches!(action, ModeAction::Push(name) if name == "String"));
/// ```
#[derive(Clone)]
pub enum ModeAction {
    /// Leave the mode stack unchanged.
    Stay,
    /// Replace the current mode.
    Begin(String),
    /// Push a mode onto the stack.
    Push(String),
    /// Pop the current mode.
    Pop,
}

impl RuleSpec {
    /// Creates a rule from its regex pattern and generated Rust action.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver_codegen::RuleSpec;
    ///
    /// let rule = RuleSpec::emit("[a-z]+", quote::quote!(Ok(Some(Self::Word))));
    /// let _ = rule;
    /// ```
    pub fn emit(pattern: impl Into<String>, action: TokenStream) -> Self {
        Self {
            pattern: pattern.into(),
            action: RuleAction::Emit(action),
            modes: vec!["INITIAL".into()],
            mode_action: ModeAction::Stay,
        }
    }

    /// Creates a rule that consumes input without producing a token.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver_codegen::RuleSpec;
    ///
    /// let rule = RuleSpec::skip(r"\s+");
    /// let _ = rule;
    /// ```
    pub fn skip(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            action: RuleAction::Skip,
            modes: vec!["INITIAL".into()],
            mode_action: ModeAction::Stay,
        }
    }

    /// Restricts this rule to named lexer modes.
    pub fn in_modes(mut self, modes: Vec<String>) -> Self {
        self.modes = modes;
        self
    }

    /// Sets the mode-stack action after this rule matches.
    pub fn mode_action(mut self, action: ModeAction) -> Self {
        self.mode_action = action;
        self
    }
}

/// The generated expression returned when a rule wins a match.
pub(crate) enum RuleAction {
    Emit(TokenStream),
    Skip,
}

impl RuleAction {
    /// Reports whether the rule consumes input without producing a token.
    pub(crate) const fn is_skip(&self) -> bool {
        matches!(self, Self::Skip)
    }

    /// Renders the generated action for the selected rule.
    pub(crate) fn rendered(&self) -> TokenStream {
        match self {
            Self::Emit(tokens) => quote!(#tokens),
            Self::Skip => quote!(Ok(None)),
        }
    }
}
