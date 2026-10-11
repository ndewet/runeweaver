//! Shows regex building blocks that runeweaver supports.

use runeweaver::Lexer;

#[derive(Debug, PartialEq, Lexer)]
#[lexer(skip = r"\s+")]
enum Token {
    #[token(r"(true|false)")]
    Boolean,
    #[token(r"[A-Za-z_][A-Za-z0-9_]*")]
    Identifier,
    #[token(r"\d{1,3}(\.\d{1,3}){3}")]
    Address,
    #[token(r"\x{1F600}+")]
    Emoji,
    #[token(r"[^,\s]+")]
    Field,
    #[token(",")]
    Comma,
}

fn main() {
    let input = "true 127.0.0.1 😀😀 value,other";
    let scanned: Vec<_> = Token::scan(input).collect();
    println!("{scanned:#?}");
}
