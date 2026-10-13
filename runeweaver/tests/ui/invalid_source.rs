use runeweaver::Lexer;

#[derive(Lexer)]
#[lexer(source = u16)]
enum Token {
    #[token("a")]
    A,
}

fn main() {}
