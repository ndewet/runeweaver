use crate::{ByteScanner, lexer::Automaton};

/// Scans raw byte sources with a generated lexer.
///
/// Derive `Lexer` with `#[lexer(source = [u8])]` to implement this trait.
pub trait ByteLexer<'input>: Automaton<'input, [u8]> {
    /// Scans tokens from `input` without UTF-8 decoding.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{ByteLexer, Lexer};
    /// #[derive(Debug, PartialEq, Lexer)]
    /// #[lexer(source = [u8])]
    /// enum Token { #[token(r"\xff")] Byte }
    /// assert_eq!(Token::scan_bytes(b"\xff").next(), Some(Ok(Token::Byte)));
    /// ```
    fn scan_bytes(input: &'input [u8]) -> ByteScanner<'input, Self>
    where
        Self::Extras: Default,
    {
        ByteScanner::with_extras(input, Self::Extras::default())
    }

    /// Scans bytes with user state stored in the scanner.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::{ByteLexer, Lexer};
    /// #[derive(Lexer)]
    /// #[lexer(source = [u8])]
    /// #[lexer(extras = usize)]
    /// enum Token { #[token(".")] Byte }
    /// let scanner = Token::scan_bytes_with_extras(b"\xff", 7);
    /// assert_eq!(scanner.extras, 7);
    /// ```
    fn scan_bytes_with_extras(
        input: &'input [u8],
        extras: Self::Extras,
    ) -> ByteScanner<'input, Self> {
        ByteScanner::with_extras(input, extras)
    }
}

impl<'input, T: Automaton<'input, [u8]>> ByteLexer<'input> for T {}
