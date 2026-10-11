//! Shows lexer modes and nested block comments.

use runeweaver::{Lexer, ScanError};

#[derive(Debug, PartialEq, Lexer)]
#[lexer(mode = Directive)]
#[lexer(mode = Comment)]
#[lexer(skip = r"\s+", modes = [INITIAL, Directive])]
#[lexer(skip = r"/\*", modes = [INITIAL, Directive], push = Comment)]
#[lexer(skip = r"/\*", modes = Comment, push = Comment)]
#[lexer(skip = r"\*/", modes = Comment, pop)]
#[lexer(skip = r"[^*/]+|[*/]", modes = Comment)]
enum Token {
    #[token("@", begin = Directive)]
    DirectiveStart,
    #[token(";", modes = Directive, begin = INITIAL)]
    DirectiveEnd,
    #[token("[a-z]+")]
    Word,
    #[token("[A-Z]+", modes = Directive)]
    Command,
}

fn main() {
    let input = "word @ RUN /* outer /* inner */ outer */ ; tail";
    let scanned: Vec<_> = Token::scan(input).collect();
    println!("{scanned:#?}");

    let unterminated: Vec<_> = Token::scan("word /* open").collect();
    assert!(matches!(
        unterminated.last(),
        Some(Err(ScanError::UnterminatedMode {
            mode: "Comment",
            ..
        }))
    ));
    println!("{unterminated:#?}");
}
