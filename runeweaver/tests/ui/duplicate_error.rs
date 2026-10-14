use runeweaver::Lexer;

#[derive(Lexer)]
#[lexer(error = String)]
#[lexer(error = String)]
enum Token {
    #[token("a")]
    A,
}

fn main() {}
