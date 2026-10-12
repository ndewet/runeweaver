use runeweaver::Lexer;

#[derive(Lexer)]
#[lexer(mode = Inner)]
enum Token {
    #[token("a", pop, push = Inner)]
    A,
}

fn main() {}
