use crate::support::token_definition;

pub const SOURCE: &str = include_str!("../sources/mode_actions.txt");

token_definition! {
    #[lexer(mode = Directive)]
    #[lexer(mode = Nested)]
    #[lexer(skip = r"[ \t\r\n]+", modes = [INITIAL, Directive, Nested])]
    pub enum Token {
        #[token("@", begin = Directive)]
        DirectiveStart,
        #[token(";", modes = Directive, begin = INITIAL)]
        DirectiveEnd,
        #[token(r"\{", modes = [INITIAL, Nested], push = Nested)]
        Open,
        #[token(r"\}", modes = Nested, pop)]
        Close,
        #[token("[a-z][a-z_]*")]
        Word,
        #[token("[A-Z]+", modes = Directive)]
        Command,
        #[token("[a-z][a-z_]*", modes = Nested)]
        NestedWord,
    }
}
