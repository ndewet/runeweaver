//! Checks typed conversion failures and scan recovery.

use runeweaver::{Lexer, ScanError, Scanner, Span};
use std::num::{IntErrorKind, ParseIntError};

#[derive(Debug, PartialEq)]
enum Failure {
    Number(ParseIntError),
    Callback(&'static str),
}

impl From<ParseIntError> for Failure {
    fn from(error: ParseIntError) -> Self {
        Self::Number(error)
    }
}

impl From<&'static str> for Failure {
    fn from(error: &'static str) -> Self {
        Self::Callback(error)
    }
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(extras = usize)]
#[lexer(mode = Group)]
#[lexer(skip = " +", with_scanner = skip)]
#[lexer(error = Failure)]
enum Token<'input> {
    #[token("[0-9]+")]
    Number(u8),
    #[token("[a-z]+")]
    Word(&'input str),
    #[token("[A-Z]+", with = self::length)]
    Length(usize),
    #[token("@", with_scanner = reject, push = Group)]
    Open,
    #[token("!", modes = Group, pop)]
    Close,
    #[token("x", modes = Group, with = reject_text)]
    Rejected(String),
}

fn length(text: &str) -> usize {
    text.len()
}

fn skip<'input>(scanner: &mut Scanner<'input, Token<'input>>) -> Result<(), &'static str> {
    scanner.extras += 1;
    if scanner.slice().len() > 1 {
        Err("skip")
    } else {
        Ok(())
    }
}

fn reject<'input>(_: &mut Scanner<'input, Token<'input>>) -> Result<(), &'static str> {
    Err("marker")
}

fn reject_text(_: &str) -> Result<String, &'static str> {
    Err("text")
}

#[test]
fn automatic_conversion_preserves_the_original_error_and_recovers() {
    let mut scan = Token::scan("300 7");
    let error = scan
        .next()
        .expect("conversion failure")
        .expect_err("overflow");
    assert_eq!(error.span(), Span::new(0, 3));
    let ScanError::InvalidPayload {
        message: Failure::Number(error),
        ..
    } = error
    else {
        panic!("expected the integer conversion error");
    };
    assert_eq!(error.kind(), &IntErrorKind::PosOverflow);
    assert_eq!(scan.next(), Some(Ok(Token::Number(7))));
}

#[test]
fn typed_errors_support_infallible_payloads_and_borrowed_text() {
    let input = String::from("ABC word");
    let mut scan = Token::scan(&input);
    assert_eq!(scan.next(), Some(Ok(Token::Length(3))));
    assert_eq!(scan.next(), Some(Ok(Token::Word(&input[4..]))));
    assert_eq!(scan.extras, 1);
}

#[test]
fn skip_callbacks_retain_errors_and_user_state() {
    let mut scan = Token::scan("  word");
    assert_eq!(
        scan.next(),
        Some(Err(ScanError::InvalidPayload {
            span: Span::new(0, 2),
            message: Failure::Callback("skip"),
        }))
    );
    let mut resumed = Scanner::resume("  word", scan.checkpoint()).expect("valid prefix");
    assert_eq!(resumed.extras, 1);
    assert_eq!(resumed.next(), Some(Ok(Token::Word("word"))));
}

#[test]
fn typed_callback_failures_still_apply_mode_actions() {
    let mut scan = Token::scan("@x!word");
    assert_eq!(
        scan.next(),
        Some(Err(ScanError::InvalidPayload {
            span: Span::new(0, 1),
            message: Failure::Callback("marker"),
        }))
    );
    assert_eq!(
        scan.next(),
        Some(Err(ScanError::InvalidPayload {
            span: Span::new(1, 2),
            message: Failure::Callback("text"),
        }))
    );
    assert_eq!(scan.next(), Some(Ok(Token::Close)));
    assert_eq!(scan.next(), Some(Ok(Token::Word("word"))));
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(error = &'input str)]
enum BorrowedError<'input> {
    #[token("[a-z]+", with = reject_borrowed)]
    Word(&'input str),
}

fn reject_borrowed(text: &str) -> Result<&str, &str> {
    Err(text)
}

#[test]
fn conversion_errors_can_borrow_the_source() {
    let input = String::from("word");
    assert_eq!(
        BorrowedError::scan(&input).next(),
        Some(Err(ScanError::InvalidPayload {
            span: Span::new(0, 4),
            message: &input[..],
        }))
    );
}
