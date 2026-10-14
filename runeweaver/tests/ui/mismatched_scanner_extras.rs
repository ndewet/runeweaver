use runeweaver::{Lexer, Scanner};

#[derive(Lexer)]
#[lexer(extras = usize)]
enum Token {
    #[token("a")]
    A,
}

fn main() {
    let _ = Scanner::<Token, bool>::with_extras("a", false);
}
