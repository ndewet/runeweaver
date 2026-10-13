//! Input source types for the compatibility lexer.

use core::ops::Range;

/// Supplies the operations that a lexer needs from its input.
pub trait Source {
    /// The slice returned for one source range.
    type Slice<'source>: 'source
    where
        Self: 'source;

    /// Returns the source length in bytes.
    fn len(&self) -> usize;

    /// Returns true if the source is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the specified source range.
    fn slice(&self, range: Range<usize>) -> Option<Self::Slice<'_>>;

    /// Returns true if `index` is a valid slice boundary.
    fn is_boundary(&self, index: usize) -> bool;
}

impl Source for str {
    type Slice<'source> = &'source str;

    fn len(&self) -> usize {
        str::len(self)
    }

    fn slice(&self, range: Range<usize>) -> Option<Self::Slice<'_>> {
        self.get(range)
    }

    fn is_boundary(&self, index: usize) -> bool {
        self.is_char_boundary(index)
    }
}

impl Source for [u8] {
    type Slice<'source> = &'source [u8];

    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    fn slice(&self, range: Range<usize>) -> Option<Self::Slice<'_>> {
        self.get(range)
    }

    fn is_boundary(&self, index: usize) -> bool {
        index <= self.len()
    }
}
