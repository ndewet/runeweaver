//! Verifies the supported Logos compatibility surface.

use logos::{Filter, Logos};

#[derive(Clone, Debug, Default, PartialEq)]
enum LexError {
    Number,
    #[default]
    Invalid,
}

impl From<std::num::ParseIntError> for LexError {
    fn from(_: std::num::ParseIntError) -> Self {
        Self::Number
    }
}

#[derive(Debug, PartialEq, Logos)]
#[logos(extras = usize)]
#[logos(error = LexError)]
#[logos(skip(r"\s+", |lexer| lexer.extras += 10))]
enum Token<'source> {
    #[token("+")]
    Plus,
    #[regex("[a-z]+")]
    Word(&'source str),
    #[regex("[0-9]+", |lexer| {
        lexer.extras += 1;
        lexer.slice().parse::<u8>()
    })]
    Number(u8),
    #[token("@", |lexer| {
        lexer.bump(1);
    })]
    Marker,
    #[token("omit", |_| Filter::<()>::Skip, priority = 20)]
    Omitted,
}

#[derive(Debug, PartialEq, Logos)]
#[logos(extras = usize)]
#[logos(skip r"\s+")]
enum Tail {
    #[token("+")]
    Plus,
}

#[test]
fn the_shim_scans_common_logos_rules_and_callbacks() {
    let source = String::from("word + 42 @! omit 999 ?");
    let mut lexer = Token::lexer(&source);

    assert_eq!(lexer.next(), Some(Ok(Token::Word("word"))));
    assert_eq!(lexer.span(), 0..4);
    assert_eq!(lexer.next(), Some(Ok(Token::Plus)));
    assert_eq!(lexer.span(), 5..6);
    assert_eq!(lexer.next(), Some(Ok(Token::Number(42))));
    assert_eq!(lexer.extras, 21);
    assert_eq!(lexer.next(), Some(Ok(Token::Marker)));
    assert_eq!(lexer.slice(), "@!");
    assert_eq!(lexer.next(), Some(Err(LexError::Number)));
    assert_eq!(lexer.span(), 18..21);
    assert_eq!(lexer.next(), Some(Err(LexError::Invalid)));
    assert_eq!(lexer.span(), 22..23);
    assert_eq!(lexer.next(), None);
}

#[test]
fn the_spanned_iterator_returns_logos_ranges() {
    let results = Token::lexer("one +").spanned().collect::<Vec<_>>();

    assert_eq!(
        results,
        vec![(Ok(Token::Word("one")), 0..3), (Ok(Token::Plus), 4..5)]
    );
}

#[test]
fn morph_preserves_the_source_position_and_extras() {
    let mut lexer = Token::lexer_with_extras("word +", 7);
    assert_eq!(lexer.next(), Some(Ok(Token::Word("word"))));

    let mut tail = lexer.morph::<Tail>();

    assert_eq!(tail.extras, 7);
    assert_eq!(tail.next(), Some(Ok(Tail::Plus)));
    assert_eq!(tail.span(), 5..6);
}
