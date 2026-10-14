//! Reports checkpoint positions that do not fit the supplied input.

use std::fmt::{Display, Formatter};

/// Reports an invalid byte offset in a scanner checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumeError {
    /// The offset outside the input or inside a UTF-8 codepoint.
    pub offset: usize,
}

impl Display for ResumeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid checkpoint position at {}", self.offset)
    }
}

impl std::error::Error for ResumeError {}
