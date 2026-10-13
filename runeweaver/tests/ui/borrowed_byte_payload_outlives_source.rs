use runeweaver::Lexer;

#[derive(Debug, Lexer)]
#[lexer(source = [u8])]
enum Token<'input> {
    #[token("[a-z]+")]
    Word(&'input [u8]),
}

fn main() {
    let token;
    {
        let input = b"word".to_vec();
        token = Token::scan(&input).next().expect("one token");
    }
    println!("{token:?}");
}
