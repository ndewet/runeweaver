//! Shows automatic and custom token payload conversion.

use runeweaver::{Lexer, ScanError, Span};

#[derive(Debug, PartialEq, Lexer)]
#[lexer(skip = r"\s+")]
enum Token {
    #[token("[0-9]+")]
    Integer(u64),
    #[token("![a-z]+", with = strip_bang)]
    Shouted(String),
    #[token(r"\?[a-z]+", with = reject_question)]
    Checked(String),
}

/// A converter that cannot fail returns the payload.
fn strip_bang(text: &str) -> String {
    text[1..].to_uppercase()
}

/// A converter returns `Err` to explain a rejection.
fn reject_question(text: &str) -> Result<String, String> {
    Err(format!("a question is not a value: {text}"))
}

fn main() {
    let scanned: Vec<_> = Token::scan("42 !hello").collect();
    println!("{scanned:#?}");

    let failed: Vec<_> = Token::scan("18446744073709551616 7").collect();
    assert!(matches!(
        failed.first(),
        Some(Err(ScanError::InvalidPayload { span, .. })) if span == &Span::new(0, 20)
    ));

    let rejected: Vec<_> = Token::scan("?why").collect();
    assert!(matches!(
        rejected.first(),
        Some(Err(ScanError::InvalidPayload { .. }))
    ));
    println!("{failed:#?}");
    println!("{rejected:#?}");
}
