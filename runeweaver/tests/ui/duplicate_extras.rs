use runeweaver::Lexer;

#[derive(Lexer)]
#[lexer(extras = usize)]
#[lexer(extras = String)]
enum Token {
    #[token("word")]
    Word,
}

fn main() {}
