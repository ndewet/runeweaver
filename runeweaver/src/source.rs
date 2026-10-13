//! Source operations shared by string and byte scanners.

use core::ops::Range;

/// Supplies bytes and valid slice boundaries to a scanner.
///
/// Implemented for `str` and `[u8]`. String scanners preserve UTF-8 boundaries.
pub trait Source: private::Sealed {
    /// Returns the source bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Source;
    /// assert_eq!(Source::as_bytes("word"), b"word");
    /// ```
    fn as_bytes(&self) -> &[u8];

    /// Returns the source length in bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Source;
    /// assert_eq!(Source::len("λ"), 2);
    /// ```
    fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// Returns whether the source is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Source;
    /// assert!(Source::is_empty(""));
    /// ```
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a source slice when both offsets are valid boundaries.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Source;
    /// assert_eq!(Source::slice("λx", 0..2), Some("λ"));
    /// ```
    fn slice(&self, range: Range<usize>) -> Option<&Self>;

    /// Returns whether an offset is a valid slice boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Source;
    /// assert!(!Source::is_boundary("λ", 1));
    /// assert!(Source::is_boundary(&b"\xff"[..], 1));
    /// ```
    fn is_boundary(&self, offset: usize) -> bool;
}

impl Source for str {
    fn as_bytes(&self) -> &[u8] {
        str::as_bytes(self)
    }
    fn slice(&self, range: Range<usize>) -> Option<&str> {
        self.get(range)
    }
    fn is_boundary(&self, offset: usize) -> bool {
        self.is_char_boundary(offset)
    }
}

impl Source for [u8] {
    fn as_bytes(&self) -> &[u8] {
        self
    }
    fn slice(&self, range: Range<usize>) -> Option<&[u8]> {
        self.get(range)
    }
    fn is_boundary(&self, offset: usize) -> bool {
        offset <= self.len()
    }
}

mod private {
    pub trait Sealed {}
    impl Sealed for str {}
    impl Sealed for [u8] {}
}
