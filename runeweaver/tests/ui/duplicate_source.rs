use runeweaver::Lexer;

#[derive(Lexer)]
#[lexer(source = [u8])]
#[lexer(source = str)]
enum Token {
    #[token("a")]
    A,
}

fn main() {}
