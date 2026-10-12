use runeweaver::Lexer;

#[derive(Lexer)]
#[lexer(mode = Inner)]
enum Token {
    #[token("a", modes = Initial, modes = Inner)]
    A,
}

fn main() {}
