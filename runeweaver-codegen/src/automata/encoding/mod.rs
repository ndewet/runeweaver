//! Lowers character sets into automaton labels.
//!
//! [`Encoding`] maps each character class to non-empty label sequences.
//! Thompson construction uses these sequences without knowing the target
//! encoding.

mod bytes;
mod utf8;

pub(crate) use self::bytes::Bytes;
pub(crate) use self::utf8::{ByteRange, Utf8};

use super::label::Label;
use crate::regex::CharSet;

/// Lowers character sets into NFA transition labels.
pub(crate) trait Encoding {
    /// The transition label in the encoded NFA.
    type Label: Label;

    /// One non-empty label sequence.
    type Sequence: AsRef<[Self::Label]>;

    /// Lowers `set` into alternative label sequences.
    ///
    /// The sequences must match exactly the encoded characters in `set`.
    /// Each sequence must be non-empty. An empty set produces no sequence.
    fn encode(&self, set: &CharSet) -> Vec<Self::Sequence>;
}
