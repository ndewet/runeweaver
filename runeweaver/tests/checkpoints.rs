//! Checks resumed scans against complete scans.

use runeweaver::{Lexer, ScanError, Scanner, Span};

#[derive(Debug, PartialEq, Lexer)]
#[lexer(extras = usize)]
#[lexer(mode = Group)]
#[lexer(skip = " +", modes = [Initial, Group])]
enum Token {
    #[token(r"\(", modes = [Initial, Group], push = Group)]
    Open,
    #[token(r"\)", modes = Group, pop)]
    Close,
    #[token("[a-zé]+", modes = [Initial, Group], with_scanner = count_word)]
    Word,
}

fn count_word(scanner: &mut Scanner<'_, Token>) {
    scanner.extras += 1;
}

#[test]
fn resumed_suffix_matches_a_complete_scan_after_an_edit() {
    let mut scan = Token::scan("one two");
    assert_eq!(scan.next(), Some(Ok(Token::Word)));
    let checkpoint = scan.checkpoint();
    let resumed: Vec<_> = Scanner::resume("one (three)", checkpoint)
        .expect("the prefix is unchanged")
        .spanned()
        .collect();
    let expected: Vec<_> = Token::scan("one (three)").spanned().skip(1).collect();
    assert_eq!(resumed, expected);
}

#[test]
fn checkpoints_preserve_nested_modes_and_diagnostic_origins() {
    let input = "((é";
    let mut scan = Token::scan(input);
    let _ = scan.next();
    let _ = scan.next();
    let checkpoint = scan.checkpoint().clone();
    let expected: Vec<_> = scan.spanned().collect();
    let actual: Vec<_> = Scanner::resume(input, checkpoint)
        .expect("valid checkpoint")
        .spanned()
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(
        actual.last(),
        Some(&Err(ScanError::UnterminatedMode {
            span: Span::new(0, input.len()),
            mode: "Group",
        }))
    );
}

#[test]
fn checkpoints_preserve_extras_and_resume_after_errors() {
    let mut scan = Token::scan("one!two");
    let _ = scan.next();
    assert!(matches!(
        scan.next(),
        Some(Err(ScanError::Unrecognized { .. }))
    ));
    let mut resumed = Scanner::resume("one!two", scan.checkpoint()).expect("valid checkpoint");
    assert_eq!(resumed.extras, 1);
    assert_eq!(resumed.next(), Some(Ok(Token::Word)));
    assert_eq!(resumed.extras, 2);
}

#[test]
fn resume_rejects_short_input_and_split_codepoints() {
    let mut scan = Token::scan("a!");
    let _ = scan.next();
    let checkpoint = scan.checkpoint();
    assert_eq!(
        Scanner::resume("", checkpoint.clone())
            .err()
            .expect("short input")
            .offset,
        1
    );
    assert_eq!(
        Scanner::resume("é", checkpoint)
            .err()
            .expect("split codepoint")
            .offset,
        1
    );
}

#[test]
fn context_comparison_excludes_positions_but_includes_modes_and_extras() {
    let mut first = Token::scan("(");
    let mut second = Token::scan("  (");
    let _ = first.next();
    let _ = second.next();
    assert!(first.checkpoint().same_context(&second.checkpoint()));
    second.extras = 1;
    assert!(!first.checkpoint().same_context(&second.checkpoint()));
    assert!(
        !first
            .checkpoint()
            .same_context(&Token::scan("").checkpoint())
    );
}

struct NonClone;

#[derive(Lexer)]
#[lexer(extras = NonClone)]
enum MoveToken {
    #[token("a")]
    A,
}

#[test]
fn moving_a_checkpoint_does_not_require_clone_or_default_extras() {
    let scan = MoveToken::scan_with_extras("a", NonClone);
    assert_eq!(
        Scanner::resume("a", scan.into_checkpoint())
            .expect("valid checkpoint")
            .count(),
        1
    );
}

#[derive(Lexer)]
enum CallbackToken {
    #[token("a", with_scanner = save_in_callback)]
    A,
}

fn save_in_callback(scanner: &mut Scanner<'_, CallbackToken>) {
    let _ = scanner.checkpoint();
}

#[test]
#[should_panic(expected = "a checkpoint requires a completed rule")]
fn callbacks_cannot_save_partially_applied_rule_state() {
    let _ = CallbackToken::scan("a").next();
}

#[test]
fn a_checkpoint_at_end_does_not_retain_iterator_completion() {
    let mut scan = Token::scan("(");
    let _ = scan.next();
    assert!(matches!(
        scan.next(),
        Some(Err(ScanError::UnterminatedMode { .. }))
    ));
    assert_eq!(scan.next(), None);
    let mut resumed = Scanner::resume("()", scan.checkpoint()).expect("valid prefix");
    assert_eq!(resumed.next(), Some(Ok(Token::Close)));
    assert_eq!(resumed.next(), None);
}
