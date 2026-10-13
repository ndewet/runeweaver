use crate::{Logos, Source};
use core::fmt::{self, Debug};
use core::ops::{Deref, DerefMut, Range};

/// A byte range in the input source.
pub type Span = Range<usize>;

/// Holds the state for one compatible Logos scan.
pub struct Lexer<'source, Token: Logos<'source>> {
    source: &'source Token::Source,
    token_start: usize,
    token_end: usize,
    /// User state stored for the scan.
    pub extras: Token::Extras,
}

impl<'source, Token: Logos<'source>> Lexer<'source, Token> {
    /// Creates a lexer for `source`.
    pub fn new(source: &'source Token::Source) -> Self
    where
        Token::Extras: Default,
    {
        Self::with_extras(source, Token::Extras::default())
    }

    /// Creates a lexer for `source` with user state.
    pub const fn with_extras(source: &'source Token::Source, extras: Token::Extras) -> Self {
        Self {
            source,
            token_start: 0,
            token_end: 0,
            extras,
        }
    }

    /// Returns the complete input source.
    pub const fn source(&self) -> &'source Token::Source {
        self.source
    }

    /// Returns the current byte range.
    #[doc(hidden)]
    #[deprecated(since = "0.2.0", note = "use `span` instead")]
    pub fn range(&self) -> Span {
        self.span()
    }

    /// Returns the current byte range.
    pub fn span(&self) -> Span {
        self.token_start..self.token_end
    }

    /// Returns the current source slice.
    ///
    /// # Panics
    ///
    /// This function panics if a custom source rejects the current range.
    pub fn slice(&self) -> <Token::Source as Source>::Slice<'source> {
        self.source
            .slice(self.span())
            .expect("the lexer span is in the source")
    }

    /// Returns the source after the current range.
    ///
    /// # Panics
    ///
    /// This function panics if a custom source rejects the remaining range.
    pub fn remainder(&self) -> <Token::Source as Source>::Slice<'source> {
        self.source
            .slice(self.token_end..self.source.len())
            .expect("the lexer position is in the source")
    }

    /// Converts this lexer to another token type over the same source.
    pub fn morph<Token2>(self) -> Lexer<'source, Token2>
    where
        Token2: Logos<'source, Source = Token::Source>,
        Token::Extras: Into<Token2::Extras>,
    {
        Lexer {
            source: self.source,
            token_start: self.token_start,
            token_end: self.token_end,
            extras: self.extras.into(),
        }
    }

    /// Extends the current range by `bytes`.
    ///
    /// # Panics
    ///
    /// This function panics if the new position is not a source boundary.
    pub fn bump(&mut self, bytes: usize) {
        let end = self.token_end.checked_add(bytes);
        assert!(
            end.is_some_and(|end| self.source.is_boundary(end)),
            "invalid lexer bump"
        );
        self.token_end = end.expect("the lexer position was checked");
    }

    /// Adds byte ranges to the scan results.
    pub fn spanned(self) -> SpannedIter<'source, Token> {
        SpannedIter { lexer: self }
    }

    #[doc(hidden)]
    pub const fn __position(&self) -> usize {
        self.token_end
    }

    #[doc(hidden)]
    pub fn __set_span(&mut self, start: usize, end: usize) {
        assert!(start <= end && self.source.is_boundary(start) && self.source.is_boundary(end));
        self.token_start = start;
        self.token_end = end;
    }
}

impl<'source, Token> Iterator for Lexer<'source, Token>
where
    Token: Logos<'source>,
{
    type Item = Result<Token, Token::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.token_start = self.token_end;
        Token::lex(self)
    }
}

impl<'source, Token> Clone for Lexer<'source, Token>
where
    Token: Logos<'source> + Clone,
    Token::Extras: Clone,
{
    fn clone(&self) -> Self {
        Self {
            source: self.source,
            token_start: self.token_start,
            token_end: self.token_end,
            extras: self.extras.clone(),
        }
    }
}

impl<'source, Token> Debug for Lexer<'source, Token>
where
    Token: Logos<'source>,
    Token::Source: Debug,
    Token::Extras: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Lexer")
            .field("source", &self.source)
            .field("extras", &self.extras)
            .finish_non_exhaustive()
    }
}

/// Iterates over scan results and their byte ranges.
pub struct SpannedIter<'source, Token: Logos<'source>> {
    lexer: Lexer<'source, Token>,
}

impl<'source, Token: Logos<'source>> Iterator for SpannedIter<'source, Token> {
    type Item = (Result<Token, Token::Error>, Span);

    fn next(&mut self) -> Option<Self::Item> {
        self.lexer.next().map(|result| (result, self.lexer.span()))
    }
}

impl<'source, Token> Clone for SpannedIter<'source, Token>
where
    Token: Logos<'source> + Clone,
    Token::Extras: Clone,
{
    fn clone(&self) -> Self {
        Self {
            lexer: self.lexer.clone(),
        }
    }
}

impl<'source, Token: Logos<'source>> Deref for SpannedIter<'source, Token> {
    type Target = Lexer<'source, Token>;

    fn deref(&self) -> &Self::Target {
        &self.lexer
    }
}

impl<'source, Token: Logos<'source>> DerefMut for SpannedIter<'source, Token> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lexer
    }
}
