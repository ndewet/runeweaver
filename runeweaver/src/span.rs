//! Locates a lexeme in the input by byte offset.

use core::{fmt, ops::Range};

/// A half-open range of UTF-8 byte offsets.
///
/// An offset is absolute, and it starts at zero at the beginning of a scan.
/// An offset carries no input identity. Use it with the string that produced
/// the span.
///
/// # Examples
///
/// ```
/// use runeweaver::Span;
///
/// let span = Span::new(0, 4);
/// assert_eq!(span.text("name 42"), Some("name"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    /// The inclusive start of the range.
    pub start: usize,
    /// The exclusive end of the range.
    pub end: usize,
}

impl Span {
    /// Creates an empty span at `offset`.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Span;
    ///
    /// assert_eq!(Span::at(3), Span::new(3, 3));
    /// ```
    #[must_use]
    pub const fn at(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    /// Creates a span from its two byte offsets.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Span;
    ///
    /// assert_eq!(Span::new(2, 5).start, 2);
    /// ```
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the number of bytes in the range.
    ///
    /// A reversed span has a length of zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Span;
    ///
    /// assert_eq!(Span::new(2, 5).len(), 3);
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Reports whether the range contains no bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Span;
    ///
    /// assert!(Span::new(2, 2).is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// Returns the smallest span that covers both spans.
    ///
    /// The result starts at the earlier start and ends at the later end. The
    /// spans do not need to overlap or occur in a particular order.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Span;
    ///
    /// let left = Span::new(2, 5);
    /// let right = Span::new(8, 11);
    /// assert_eq!(left.cover(right), Span::new(2, 11));
    /// ```
    #[must_use]
    pub const fn cover(self, other: Self) -> Self {
        Self::new(
            if self.start < other.start {
                self.start
            } else {
                other.start
            },
            if self.end > other.end {
                self.end
            } else {
                other.end
            },
        )
    }

    /// Returns this span as a range suitable for indexing the scanned string.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Span;
    ///
    /// assert_eq!(Span::new(2, 5).range(), 2..5);
    /// ```
    #[must_use]
    pub const fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Returns the lexeme that this span identifies in `input`.
    ///
    /// Returns `None` if the range is outside `input`, if the range does not
    /// start and end at a character boundary. Give the same input that was
    /// scanned.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::Span;
    ///
    /// assert_eq!(Span::new(5, 7).text("name 42"), Some("42"));
    /// assert_eq!(Span::new(5, 99).text("name 42"), None);
    /// ```
    #[must_use]
    pub fn text<'a>(&self, input: &'a str) -> Option<&'a str> {
        input.get(self.range())
    }
}

impl From<Range<usize>> for Span {
    fn from(range: Range<usize>) -> Self {
        Self::new(range.start, range.end)
    }
}

impl From<Span> for Range<usize> {
    fn from(span: Span) -> Self {
        span.start..span.end
    }
}

/// A token together with its byte range in the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    /// The token accepted by the lexer.
    pub token: T,
    /// The half-open UTF-8 byte range matched by this token.
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Transforms the token while preserving its span.
    #[must_use]
    pub fn map<U>(self, operation: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            token: operation(self.token),
            span: self.span,
        }
    }

    /// Tries to transform the token while preserving its span.
    ///
    /// # Errors
    ///
    /// Returns the error from `transformation` when the token cannot be
    /// transformed.
    pub fn try_map<U, E>(
        self,
        transformation: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<Spanned<U>, E> {
        transformation(self.token).map(|token| Spanned {
            token,
            span: self.span,
        })
    }
}

impl fmt::Display for Span {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}..{}", self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::Span;

    #[test]
    fn an_offset_makes_an_empty_span() {
        assert_eq!(Span::at(9), Span::new(9, 9));
        assert!(Span::at(9).is_empty());
    }

    #[test]
    fn covering_spans_uses_the_outer_bounds_in_either_order() {
        let first = Span::new(8, 12);
        let second = Span::new(3, 10);

        assert_eq!(first.cover(second), Span::new(3, 12));
        assert_eq!(second.cover(first), Span::new(3, 12));
    }

    #[test]
    fn covering_an_empty_span_includes_its_position() {
        assert_eq!(Span::new(3, 5).cover(Span::at(8)), Span::new(3, 8));
        assert_eq!(Span::at(1).cover(Span::new(3, 5)), Span::new(1, 5));
    }
}
