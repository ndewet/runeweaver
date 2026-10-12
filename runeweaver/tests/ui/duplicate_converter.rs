use runeweaver::Lexer;

fn convert(_: &str) -> u8 {
    1
}

#[derive(Lexer)]
enum Token {
    #[token("a", with = convert, with = convert)]
    A(u8),
}

fn main() {}
