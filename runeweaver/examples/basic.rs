//! Shows basic token rules, skips, and rule selection.

use runeweaver::Lexer;

#[derive(Debug, PartialEq, Lexer)]
#[lexer(skip = r"[ \t\r\n]+")]
enum Token {
    #[token("let")]
    Let,
    #[token("[A-Za-z_][A-Za-z0-9_]*")]
    Identifier,
    #[token("[0-9]+")]
    Integer,
    #[token("=")]
    Equal,
}

fn main() {
    let first = Token::scan("letter")
        .next()
        .expect("one token")
        .expect("valid input");
    assert_eq!(first, Token::Identifier, "the longer identifier rule wins");

    let tokens: Vec<_> = Token::scan("let answer = 42").collect();
    println!("{tokens:#?}");
}
