#![deny(warnings)]

use runeweaver::Lexer;

#[derive(Lexer)]
enum Token {}

fn main() {
    assert_eq!(Token::scan("input").count(), 5);
}
