//! Shows recoverable scan errors and UTF-8 byte spans.

use runeweaver::{Lexer, ScanError, Span, Spanned};

#[derive(Debug, PartialEq, Lexer)]
#[lexer(skip = r"\s+")]
enum Token {
    #[token("[a-z]+")]
    Word,
}

fn main() {
    let input = "one @ two";
    let scanned: Vec<_> = Token::scan(input).spanned().collect();

    assert_eq!(
        scanned,
        vec![
            Ok(Spanned {
                token: Token::Word,
                span: Span::new(0, 3),
            }),
            Err(ScanError::Unrecognized {
                span: Span::new(4, 5)
            }),
            Ok(Spanned {
                token: Token::Word,
                span: Span::new(6, 9),
            }),
        ],
    );

    for item in scanned {
        match item {
            Ok(spanned) => {
                let lexeme = spanned.span.text(input).expect("the span is in the input");
                println!("{lexeme:?}: {:?}", spanned.token);
            }
            Err(error) => println!("scan error: {error:?}"),
        }
    }
}
