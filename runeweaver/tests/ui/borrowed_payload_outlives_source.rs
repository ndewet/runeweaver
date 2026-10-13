use runeweaver::Lexer;

#[derive(Debug, Lexer)]
enum Token<'input> {
    #[token("[a-z]+")]
    Word(&'input str),
}

fn main() {
    let token;
    {
        let input = String::from("word");
        token = Token::scan(&input).next().expect("one token");
    }
    println!("{token:?}");
}
