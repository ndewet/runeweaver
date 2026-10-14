//! End-to-end tests for generated lexers.

#![deny(dead_code)]

use runeweaver::lexer::{Automaton, ModeAction, ScanStep};
use runeweaver::{Lexer, ScanError, Scanner, Span, Spanned};
use std::convert::Infallible;
use std::str::FromStr;

struct InvalidAutomaton;

impl<'input> Automaton<'input> for InvalidAutomaton {
    type Extras = bool;
    type Error = String;

    fn next_token(scanner: &mut Scanner<'input, Self, bool>, _: usize) -> ScanStep<Self> {
        if scanner.extras {
            ScanStep::End
        } else {
            ScanStep::Matched {
                result: Ok(None),
                action: ModeAction::Stay,
            }
        }
    }

    fn mode_name(_: usize) -> &'static str {
        "INITIAL"
    }
}

#[test]
#[should_panic(expected = "a generated match must consume input")]
fn scanner_rejects_a_match_without_progress() {
    let _ = InvalidAutomaton::scan_with_extras("a", false).next();
}

#[test]
#[should_panic(expected = "invalid generated end position")]
fn scanner_rejects_end_before_the_input_length() {
    let _ = InvalidAutomaton::scan_with_extras("a", true).next();
}

fn first<'input, T: Lexer<'input>>(input: &'input str) -> Option<(T, usize)>
where
    <T as Automaton<'input>>::Extras: Default,
{
    T::scan(input)
        .spanned()
        .next()?
        .ok()
        .map(|spanned| (spanned.token, spanned.span.end))
}

#[test]
fn scanner_exposes_and_updates_its_source_state() {
    let mut scanner = Token::scan("aaa!");

    assert_eq!(scanner.source(), "aaa!");
    assert_eq!(scanner.remainder(), "aaa!");
    assert_eq!(scanner.next(), Some(Ok(Token::As)));
    assert_eq!(scanner.position(), 3);
    assert_eq!(scanner.remainder(), "!");

    scanner.bump(1);
    assert_eq!(scanner.span(), Span::new(0, 4));
    assert_eq!(scanner.remainder(), "");
    assert_eq!(scanner.next(), None);
}

#[test]
fn scanner_keeps_user_state_while_it_scans() {
    let mut scanner = Token::scan_with_extras("aaa b", Vec::new());

    let first = scanner.next();
    scanner.extras.push(first);
    let second = scanner.next();
    scanner.extras.push(second);

    assert_eq!(
        scanner.extras,
        vec![
            Some(Ok(Token::As)),
            Some(Err(ScanError::Unrecognized {
                span: Span::new(3, 4),
            })),
        ]
    );
    assert_eq!(scanner.remainder(), "b");
}

#[test]
fn scanner_defaults_use_the_tokens_declared_extras() {
    let mut scanner: Scanner<'_, StatefulCallbacks> = Scanner::new("word");
    assert_eq!(scanner.next(), Some(Ok(StatefulCallbacks::Word)));
    assert_eq!(scanner.extras, 4);
    let mut spanned: runeweaver::SpannedScan<'_, StatefulCallbacks> = scanner.spanned();
    spanned.scanner_mut().extras = 7;
    assert_eq!(spanned.into_scanner().extras, 7);
}

#[derive(Debug, PartialEq, Lexer)]
enum GenericExtrasToken<Extras: FromStr<Err = std::num::ParseIntError>> {
    #[token("[0-9]+")]
    Number(Extras),
}

#[test]
fn scanner_generics_do_not_shadow_payload_type_parameters() {
    assert_eq!(
        GenericExtrasToken::<u32>::scan("42").next(),
        Some(Ok(GenericExtrasToken::Number(42))),
    );
}

#[derive(Debug, PartialEq, Eq)]
struct RuleOutput<T> {
    token: Option<T>,
    mode_action: ModeAction,
}

fn stay<T>(token: Option<T>) -> RuleOutput<T> {
    RuleOutput {
        token,
        mode_action: ModeAction::Stay,
    }
}

type GeneratedMatch<T> = Result<(RuleOutput<T>, usize, usize), (ScanError, usize, usize)>;

fn generated_match<'input, T: Automaton<'input, Error = String> + InspectAutomaton>(
    input: &'input str,
    mode: usize,
) -> Option<GeneratedMatch<T>>
where
    <T as Automaton<'input>>::Extras: Default,
{
    T::start_state(mode)?;
    let mut scanner = Scanner::<T, <T as Automaton<'input>>::Extras>::new(input);
    match T::next_token(&mut scanner, mode) {
        ScanStep::Matched {
            result: Ok(token),
            action: mode_action,
        } => Some(Ok((
            RuleOutput { token, mode_action },
            0,
            scanner.position(),
        ))),
        ScanStep::Matched {
            result: Err(error), ..
        } => Some(Err((error, scanner.span().start, scanner.span().end))),
        ScanStep::End if scanner.position() != 0 => Some(Ok((stay(None), 0, scanner.position()))),
        ScanStep::End => None,
    }
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(extras = Vec<Option<Result<Token, ScanError>>>)]
enum Token {
    #[token("a+")]
    As,
    #[token("b")]
    B,
}

#[test]
fn derived_lexer_returns_the_longest_prefix() {
    assert_eq!(first::<Token>("aaab"), Some((Token::As, 3)));
    assert_eq!(first::<Token>("b"), Some((Token::B, 1)));
    assert_eq!(first::<Token>("c"), None);
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Lexer)]
enum Priority {
    #[token("a")]
    First,
    #[token("[a-z]")]
    Second,
}

#[test]
fn derived_lexer_breaks_equal_length_ties_by_rule_order() {
    assert_eq!(first::<Priority>("a"), Some((Priority::First, 1)));
}

#[derive(Debug, PartialEq, Lexer)]
enum Unicode {
    #[token("é+")]
    EAcute,
}

#[test]
fn derived_lexer_counts_utf8_bytes() {
    assert_eq!(first::<Unicode>("éé!"), Some((Unicode::EAcute, 4)));
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(skip = "[ \\t\\n]+")]
#[lexer(skip = "//[^\\n]*")]
enum WithTrivia {
    #[token("[a-z]+")]
    Identifier,
}

#[test]
fn generated_single_token_scan_fuses_stay_mode_trivia() {
    assert_eq!(
        generated_match::<WithTrivia>(" \tword tail", 0),
        Some(Ok((stay(Some(WithTrivia::Identifier)), 0, 6)))
    );
    assert_eq!(
        generated_match::<WithTrivia>(" \t", 0),
        Some(Ok((stay(None), 0, 2)))
    );
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(mode = Comment)]
#[lexer(skip = r"[ \t\r\n]+")]
#[lexer(skip = r"/\*", push = Comment)]
#[lexer(skip = r"/\*", modes = Comment, push = Comment)]
#[lexer(skip = r"\*/", modes = Comment, pop)]
#[lexer(skip = r"[^*/]+|[*/]", modes = Comment)]
enum WithNestedComments {
    #[token("[a-z]+")]
    Identifier,
}

#[test]
fn derived_lexer_skips_arbitrarily_nested_block_comments() {
    let input = "one /* outer /* nested */ still outer */ two";
    let scanned: Vec<_> = WithNestedComments::scan(input).spanned().collect();

    assert_eq!(
        scanned,
        vec![
            Ok(Spanned {
                token: WithNestedComments::Identifier,
                span: Span::new(0, 3)
            }),
            Ok(Spanned {
                token: WithNestedComments::Identifier,
                span: Span::new(41, 44)
            }),
        ]
    );
}

#[test]
fn derived_lexer_reports_an_unterminated_nested_comment() {
    let input = "one /* outer /* nested */";
    let scanned: Vec<_> = WithNestedComments::scan(input).spanned().collect();

    assert_eq!(
        scanned,
        vec![
            Ok(Spanned {
                token: WithNestedComments::Identifier,
                span: Span::new(0, 3)
            }),
            Err(ScanError::UnterminatedMode {
                span: Span::new(4, 25),
                mode: "Comment"
            }),
        ]
    );
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(mode = String)]
enum ModeTokens {
    #[token("[a-z]+")]
    Identifier,
    #[token("\"", push = String)]
    StringStart,
    #[token(r#"[^"\\]+"#, modes = String)]
    StringText,
    #[token("\"", modes = String, pop)]
    StringEnd,
}

#[test]
fn derived_lexer_only_enables_tokens_in_their_declared_mode() {
    let scanned: Vec<_> = ModeTokens::scan("name \"text\" tail")
        .spanned()
        .map(|item| item.map(|spanned| spanned.token))
        .collect();
    assert_eq!(
        scanned,
        vec![
            Ok(ModeTokens::Identifier),
            Err(ScanError::Unrecognized {
                span: Span::new(4, 5)
            }),
            Ok(ModeTokens::StringStart),
            Ok(ModeTokens::StringText),
            Ok(ModeTokens::StringEnd),
            Err(ScanError::Unrecognized {
                span: Span::new(11, 12)
            }),
            Ok(ModeTokens::Identifier),
        ]
    );
}

#[test]
fn derived_lexer_skips_whitespace_and_comments() {
    assert_eq!(
        first::<WithTrivia>(" \t// note\nname"),
        Some((WithTrivia::Identifier, 14))
    );
}

#[test]
fn scan_keeps_spans_and_recovers_after_invalid_input() {
    let scanned: Vec<_> = WithTrivia::scan("one @ two").spanned().collect();

    assert_eq!(
        scanned,
        vec![
            Ok(Spanned {
                token: WithTrivia::Identifier,
                span: Span::new(0, 3),
            }),
            Err(ScanError::Unrecognized {
                span: Span::new(4, 5)
            }),
            Ok(Spanned {
                token: WithTrivia::Identifier,
                span: Span::new(6, 9),
            }),
        ]
    );
}

#[derive(Debug, PartialEq)]
struct Identifier(String);

impl FromStr for Identifier {
    type Err = Infallible;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ok(Self(text.to_owned()))
    }
}

#[derive(Debug, PartialEq, Lexer)]
enum Value {
    #[token("[0-9]+")]
    Integer(u64),
    #[token("[a-z]+")]
    Identifier(Identifier),
    #[token("![a-z]+", with = strip_bang)]
    Shouted(String),
    #[token("#[a-z]+", with = reject_hash)]
    Rejected(String),
}

fn strip_bang(text: &str) -> Result<String, Infallible> {
    Ok(text[1..].to_uppercase())
}

fn reject_hash(_: &str) -> Result<String, &'static str> {
    Err("hash-prefixed values are not permitted")
}

#[test]
fn derived_lexer_parses_owned_payloads() {
    let scanned: Vec<_> = Value::scan("42 name !hello").spanned().collect();

    assert_eq!(
        scanned,
        vec![
            Ok(Spanned {
                token: Value::Integer(42),
                span: Span::new(0, 2),
            }),
            Err(ScanError::Unrecognized {
                span: Span::new(2, 3)
            }),
            Ok(Spanned {
                token: Value::Identifier(Identifier("name".to_owned())),
                span: Span::new(3, 7),
            }),
            Err(ScanError::Unrecognized {
                span: Span::new(7, 8)
            }),
            Ok(Spanned {
                token: Value::Shouted("HELLO".to_owned()),
                span: Span::new(8, 14),
            }),
        ]
    );
}

#[test]
fn scan_reports_payload_conversion_errors_and_recovers() {
    let scanned: Vec<_> = Value::scan("18446744073709551616 7").spanned().collect();

    assert!(matches!(
        scanned.first(),
        Some(Err(ScanError::InvalidPayload { span, .. })) if span == &Span::new(0, 20)
    ));
    assert_eq!(
        scanned.last(),
        Some(&Ok(Spanned {
            token: Value::Integer(7),
            span: Span::new(21, 22),
        }))
    );
}

#[test]
fn scan_reports_explicit_converter_errors_and_recovers() {
    let scanned: Vec<_> = Value::scan("#no 7").spanned().collect();

    assert_eq!(
        scanned,
        vec![
            Err(ScanError::InvalidPayload {
                span: Span::new(0, 3),
                message: "hash-prefixed values are not permitted".to_owned(),
            }),
            Err(ScanError::Unrecognized {
                span: Span::new(3, 4)
            }),
            Ok(Spanned {
                token: Value::Integer(7),
                span: Span::new(4, 5),
            }),
        ]
    );
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(mode = Payload)]
#[lexer(skip = r"\[", push = Payload)]
enum FailedModePayload {
    #[token("x", modes = Payload, with = reject_mode_payload, pop)]
    Rejected(String),
    #[token("a")]
    A,
}

fn reject_mode_payload(_: &str) -> Result<String, &'static str> {
    Err("invalid mode payload")
}

#[test]
fn a_failed_payload_still_applies_its_mode_action() {
    let scanned: Vec<_> = FailedModePayload::scan("[xa").spanned().collect();

    assert_eq!(
        scanned,
        vec![
            Err(ScanError::InvalidPayload {
                span: Span::new(1, 2),
                message: "invalid mode payload".to_owned(),
            }),
            Ok(Spanned {
                token: FailedModePayload::A,
                span: Span::new(2, 3),
            }),
        ]
    );
}

#[derive(Debug, PartialEq, Lexer)]
enum Repetition {
    #[token("a{2,}b")]
    OpenEnded,
    #[token("(a?)*b")]
    NullableLoop,
}

#[test]
fn derived_lexer_handles_open_ended_and_nullable_repetition() {
    assert_eq!(
        first::<Repetition>("aaab"),
        Some((Repetition::OpenEnded, 4))
    );
    assert_eq!(
        first::<Repetition>("b"),
        Some((Repetition::NullableLoop, 1))
    );
    assert_eq!(first::<Repetition>("aaa"), None);
}

#[derive(Debug, PartialEq, Lexer)]
enum UnicodeBoundaries {
    #[token(r"[\x7f-\x{80}]")]
    Boundary,
    #[token("(é|€|𐐷)+")]
    Scalar,
}

#[test]
fn derived_lexer_matches_utf8_boundaries_and_keeps_byte_spans() {
    let scanned: Vec<_> = UnicodeBoundaries::scan("\u{7f}\u{80}é€𐐷@")
        .spanned()
        .collect();

    assert_eq!(
        scanned,
        vec![
            Ok(Spanned {
                token: UnicodeBoundaries::Boundary,
                span: Span::new(0, 1),
            }),
            Ok(Spanned {
                token: UnicodeBoundaries::Boundary,
                span: Span::new(1, 3),
            }),
            Ok(Spanned {
                token: UnicodeBoundaries::Scalar,
                span: Span::new(3, 12),
            }),
            Err(ScanError::Unrecognized {
                span: Span::new(12, 13)
            }),
        ]
    );
}

#[derive(Debug, PartialEq, Lexer)]
enum NegatedClass {
    #[token("a")]
    A,
    #[token(r"[^a]")]
    NotA,
}

#[test]
fn derived_lexer_matches_negated_classes_including_newlines_and_unicode() {
    let scanned: Vec<_> = NegatedClass::scan("a\né").spanned().collect();

    assert_eq!(
        scanned,
        vec![
            Ok(Spanned {
                token: NegatedClass::A,
                span: Span::new(0, 1),
            }),
            Ok(Spanned {
                token: NegatedClass::NotA,
                span: Span::new(1, 2),
            }),
            Ok(Spanned {
                token: NegatedClass::NotA,
                span: Span::new(2, 4),
            }),
        ]
    );
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(skip = r"\s+")]
enum RepeatedNegatedClass {
    #[token(r"(true|false)")]
    Boolean,
    #[token(r"[A-Za-z_][A-Za-z0-9_]*")]
    Identifier,
    #[token(r"\d{1,3}(\.\d{1,3}){3}")]
    Address,
    #[token(r"\x{1F600}+")]
    Emoji,
    #[token(r"[^,\s]+")]
    Field,
    #[token(",")]
    Comma,
}

#[test]
fn repeated_negated_class_scans_without_a_stack_overflow() {
    let scanned: Result<Vec<_>, _> =
        RepeatedNegatedClass::scan("true 127.0.0.1 \u{1F600}\u{1F600} value,other").collect();

    assert_eq!(
        scanned,
        Ok(vec![
            RepeatedNegatedClass::Boolean,
            RepeatedNegatedClass::Address,
            RepeatedNegatedClass::Emoji,
            RepeatedNegatedClass::Identifier,
            RepeatedNegatedClass::Comma,
            RepeatedNegatedClass::Identifier,
        ])
    );
}

#[derive(Debug, Clone, PartialEq, Lexer)]
#[lexer(skip = r"[ \t\r\n]+")]
#[lexer(skip = r"//[^\n]*")]
enum RustToken {
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("match")]
    Match,
    #[token("[A-Za-z_][A-Za-z0-9_]*")]
    Identifier,
    #[token("[0-9][0-9_]*")]
    Integer,
    #[token(r#""([^"\\]|\\.)*""#)]
    String,
    #[token("->")]
    Arrow,
    #[token("::")]
    PathSeparator,
    #[token(":")]
    Colon,
    #[token("==")]
    EqualEqual,
    #[token("=")]
    Equal,
    #[token("=>")]
    FatArrow,
    #[token(r"\.\.=")]
    RangeInclusive,
    #[token(r"\.\.")]
    Range,
    #[token(r"\.")]
    Dot,
    #[token(r"\(")]
    LeftParen,
    #[token(r"\)")]
    RightParen,
    #[token(r"\{")]
    LeftBrace,
    #[token(r"\}")]
    RightBrace,
    #[token(",")]
    Comma,
    #[token(";")]
    Semicolon,
}

fn rust_tokens(input: &str) -> Vec<Spanned<RustToken>> {
    RustToken::scan(input)
        .spanned()
        .map(|result| result.expect("the Rust-like input is recognized"))
        .collect()
}

#[test]
fn rust_like_lexer_recognizes_common_tokens() {
    assert_eq!(first::<RustToken>("fn"), Some((RustToken::Fn, 2)));
    assert_eq!(
        first::<RustToken>("fn_name"),
        Some((RustToken::Identifier, 7)),
        "a keyword must not steal an identifier prefix"
    );
    assert_eq!(
        first::<RustToken>(r#""say: \"é\"""#),
        Some((RustToken::String, 13)),
        "escaped quotes and multi-byte characters stay inside one string literal"
    );
}

#[test]
fn rust_like_lexer_handles_comments_ranges_and_compound_punctuation() {
    let input = "fn crate::main(a: u32) -> String { let value = 12_345; // note\nmatch value { 0..=10 => \"é\", _ => \"other\" } }";
    let scanned = rust_tokens(input);

    assert_eq!(
        scanned
            .iter()
            .map(|spanned| &spanned.token)
            .collect::<Vec<_>>(),
        vec![
            &RustToken::Fn,
            &RustToken::Identifier,
            &RustToken::PathSeparator,
            &RustToken::Identifier,
            &RustToken::LeftParen,
            &RustToken::Identifier,
            &RustToken::Colon,
            &RustToken::Identifier,
            &RustToken::RightParen,
            &RustToken::Arrow,
            &RustToken::Identifier,
            &RustToken::LeftBrace,
            &RustToken::Let,
            &RustToken::Identifier,
            &RustToken::Equal,
            &RustToken::Integer,
            &RustToken::Semicolon,
            &RustToken::Match,
            &RustToken::Identifier,
            &RustToken::LeftBrace,
            &RustToken::Integer,
            &RustToken::RangeInclusive,
            &RustToken::Integer,
            &RustToken::FatArrow,
            &RustToken::String,
            &RustToken::Comma,
            &RustToken::Identifier,
            &RustToken::FatArrow,
            &RustToken::String,
            &RustToken::RightBrace,
            &RustToken::RightBrace,
        ]
    );
    assert_eq!(
        scanned
            .iter()
            .map(|spanned| spanned.span.text(input).expect("the span is in the input"))
            .collect::<Vec<_>>(),
        vec![
            "fn",
            "crate",
            "::",
            "main",
            "(",
            "a",
            ":",
            "u32",
            ")",
            "->",
            "String",
            "{",
            "let",
            "value",
            "=",
            "12_345",
            ";",
            "match",
            "value",
            "{",
            "0",
            "..=",
            "10",
            "=>",
            "\"é\"",
            ",",
            "_",
            "=>",
            "\"other\"",
            "}",
            "}",
        ]
    );
}

#[test]
fn rust_like_lexer_prefers_the_longest_compound_punctuation() {
    let input = "a===b..c...d";
    let scanned = rust_tokens(input);

    assert_eq!(
        scanned
            .iter()
            .map(|spanned| &spanned.token)
            .collect::<Vec<_>>(),
        vec![
            &RustToken::Identifier,
            &RustToken::EqualEqual,
            &RustToken::Equal,
            &RustToken::Identifier,
            &RustToken::Range,
            &RustToken::Identifier,
            &RustToken::Range,
            &RustToken::Dot,
            &RustToken::Identifier,
        ]
    );
    assert_eq!(
        scanned
            .iter()
            .map(|spanned| spanned.span.text(input).expect("the span is in the input"))
            .collect::<Vec<_>>(),
        vec!["a", "==", "=", "b", "..", "c", "..", ".", "d"]
    );
}

#[derive(Debug, PartialEq, Lexer)]
enum Converters {
    #[token("![a-z]+", with = direct)]
    Direct(String),
    #[token("[0-9]+", with = checked)]
    Checked(u8),
    #[token(r"\?[a-z]+", with = failing)]
    Rejected(String),
}

fn direct(text: &str) -> String {
    text[1..].to_uppercase()
}

fn checked(text: &str) -> Result<u8, String> {
    text.parse()
        .map_err(|_| format!("{text} is not a byte value"))
}

fn failing(_: &str) -> Result<String, &'static str> {
    Err("a question is not a value")
}

#[test]
fn a_converter_returns_a_payload_or_a_result() {
    assert_eq!(
        Converters::scan("!hi").spanned().next(),
        Some(Ok(Spanned {
            token: Converters::Direct("HI".to_owned()),
            span: Span::new(0, 3),
        }))
    );
    assert_eq!(
        Converters::scan("42").spanned().next(),
        Some(Ok(Spanned {
            token: Converters::Checked(42),
            span: Span::new(0, 2),
        }))
    );
    assert_eq!(
        Converters::scan("999").next(),
        Some(Err(ScanError::InvalidPayload {
            span: Span::new(0, 3),
            message: "999 is not a byte value".to_owned(),
        }))
    );
    assert_eq!(
        Converters::scan("?why").next(),
        Some(Err(ScanError::InvalidPayload {
            span: Span::new(0, 4),
            message: "a question is not a value".to_owned(),
        }))
    );
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(extras = usize)]
#[lexer(skip = r"\s+")]
enum BorrowedToken<'input> {
    #[token("[a-z]+")]
    Word(&'input str),
    #[token("[0-9]+", with = borrow_text)]
    Number(&'input str),
    #[token("[A-Z]+", with_scanner = borrow_and_count)]
    Upper(&'input str),
}

fn borrow_text(text: &str) -> &str {
    text
}

#[derive(Debug, PartialEq, Lexer)]
enum SourceNamedToken<'__runeweaver_source> {
    #[token("[a-z]+")]
    Word(&'__runeweaver_source str),
}

#[test]
fn borrowed_payloads_do_not_shadow_declared_source_lifetimes() {
    let input = String::from("word");
    assert_eq!(
        SourceNamedToken::scan(&input).next(),
        Some(Ok(SourceNamedToken::Word("word"))),
    );
}

fn borrow_and_count<'input>(
    scanner: &mut Scanner<'input, BorrowedToken<'input>, usize>,
) -> &'input str {
    scanner.extras += 1;
    scanner.slice()
}

#[test]
fn token_payloads_can_borrow_the_matched_text() {
    let input = String::from("word 42 ABC");
    let mut scanner = BorrowedToken::scan(&input);
    let tokens = scanner.by_ref().collect::<Result<Vec<_>, _>>();

    assert_eq!(
        tokens,
        Ok(vec![
            BorrowedToken::Word("word"),
            BorrowedToken::Number("42"),
            BorrowedToken::Upper("ABC")
        ])
    );
    assert_eq!(scanner.extras, 1);
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(extras = usize)]
#[lexer(skip = r"\s+", with_scanner = count_skip)]
enum StatefulCallbacks {
    #[token("[a-z]+", with_scanner = count_word)]
    Word,
    #[token("[0-9]+", with_scanner = parse_number)]
    Number(u8),
    #[token("@", with_scanner = include_bang)]
    Marker,
}

fn count_skip(scanner: &mut Scanner<'_, StatefulCallbacks, usize>) {
    scanner.extras += 10;
}

fn count_word(scanner: &mut Scanner<'_, StatefulCallbacks, usize>) {
    scanner.extras += scanner.slice().len();
}

fn parse_number(
    scanner: &mut Scanner<'_, StatefulCallbacks, usize>,
) -> Result<u8, std::num::ParseIntError> {
    scanner.extras += 1;
    scanner.slice().parse()
}

fn include_bang(scanner: &mut Scanner<'_, StatefulCallbacks, usize>) {
    scanner.extras += 100;
    if scanner.remainder().starts_with('!') {
        scanner.bump(1);
    }
}

#[test]
fn scanner_callbacks_access_extras_and_can_extend_a_match() {
    let mut scanner = StatefulCallbacks::scan_with_extras("hi 42 @!", 0);

    assert_eq!(scanner.next(), Some(Ok(StatefulCallbacks::Word)));
    assert_eq!(scanner.span(), Span::new(0, 2));
    assert_eq!(scanner.next(), Some(Ok(StatefulCallbacks::Number(42))));
    assert_eq!(scanner.span(), Span::new(3, 5));
    assert_eq!(scanner.next(), Some(Ok(StatefulCallbacks::Marker)));
    assert_eq!(scanner.span(), Span::new(6, 8));
    assert_eq!(scanner.extras, 123);
    assert_eq!(scanner.next(), None);
}

#[test]
fn scan_errors_expose_their_location_without_matching_variants() {
    let ranged: ScanError = ScanError::InvalidPayload {
        span: Span::new(3, 7),
        message: "invalid value".to_owned(),
    };
    assert_eq!(ranged.span(), Span::new(3, 7));
    assert_eq!(ranged.offset(), 3);
}

#[test]
fn spanned_transformations_preserve_the_original_span() {
    let token = Spanned {
        token: "17",
        span: Span::new(4, 6),
    };
    let parsed = token
        .try_map(str::parse::<u8>)
        .expect("the token is an integer");

    assert_eq!(parsed.token, 17);
    assert_eq!(parsed.span, Span::new(4, 6));
    assert_eq!(parsed.map(u16::from).span, Span::new(4, 6));
}

#[test]
fn a_failed_spanned_transformation_returns_its_error() {
    let token = Spanned {
        token: "not a number",
        span: Span::new(2, 14),
    };
    let result = token.try_map(str::parse::<u8>);

    assert!(result.is_err());
}
#[derive(Debug, PartialEq, Lexer)]
enum Continuation {
    #[token("a+")]
    Short,
    #[token("a+bcd")]
    Long,
    #[token("(xy)+")]
    Cycle,
    #[token("\u{e9}+")]
    Unicode,
}

fn continuation(input: &str) -> Option<(Continuation, usize)> {
    let (output, _, length) = generated_match::<Continuation>(input, 0)?.ok()?;
    Some((output.token?, length))
}

#[test]
fn continuations_preserve_matches_after_loop_exits_and_failed_longer_rules() {
    for length in 1..=25 {
        let prefix = "a".repeat(length);
        assert_eq!(
            continuation(&format!("{prefix}!")),
            Some((Continuation::Short, length))
        );
        assert_eq!(
            continuation(&format!("{prefix}bc!")),
            Some((Continuation::Short, length))
        );
        assert_eq!(
            continuation(&format!("{prefix}bcd!")),
            Some((Continuation::Long, length + 3))
        );
    }
    assert_eq!(continuation("!"), None);
    assert_eq!(
        continuation("\u{e9}\u{e9}!"),
        Some((Continuation::Unicode, 4))
    );
}

#[test]
fn single_token_continuations_use_the_start_condition_and_rule_priority() {
    assert_eq!(
        generated_match::<Priority>("a", 0),
        Some(Ok((stay(Some(Priority::First)), 0, 1)))
    );
    assert_eq!(
        generated_match::<ModeTokens>("text", 0),
        Some(Ok((stay(Some(ModeTokens::Identifier)), 0, 4)))
    );
    assert_eq!(
        generated_match::<ModeTokens>("text", 1),
        Some(Ok((stay(Some(ModeTokens::StringText)), 0, 4)))
    );
    assert_eq!(generated_match::<ModeTokens>("text", 2), None);
}

#[test]
fn long_self_loops_and_state_cycles_complete_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            let input = "a".repeat(1024 * 1024);
            assert_eq!(
                continuation(&input),
                Some((Continuation::Short, input.len()))
            );
            let input = "xy".repeat(128 * 1024);
            assert_eq!(
                continuation(&input),
                Some((Continuation::Cycle, input.len()))
            );
        })
        .expect("the test thread starts")
        .join()
        .expect("the test thread completes");
}

#[derive(Debug, PartialEq, Lexer)]
enum RegionToken {
    #[token("a")]
    A,
    #[token("b")]
    B,
    #[token("[abx]cd")]
    General,
    #[token("z")]
    Z,
    #[token("[zy]ef")]
    Fixed,
    #[token("q")]
    Q,
    #[token("q(rq)*s")]
    Cycle,
    #[token("p")]
    P,
    #[token("[pt](uv)*w")]
    OptionalCycle,
    #[token("(mn)*m")]
    RepeatedAccept,
    #[token(r#""([^"\\]|\\.)*""#)]
    Quoted,
}

fn interpreted_region(input: &str) -> Option<(RegionToken, usize)> {
    interpreted_token::<RegionToken>(input)
}

trait InspectAutomaton: Sized {
    fn start_state(mode: usize) -> Option<usize>;
    fn next_state(state: usize, byte: u8) -> Option<usize>;
    fn accepting_rule(state: usize) -> Option<usize>;
    fn execute_rule(rule: usize, text: &str) -> Option<RuleOutput<Self>>;
}

fn interpreted_token<T: InspectAutomaton>(input: &str) -> Option<(T, usize)> {
    let mut state = T::start_state(0)?;
    let mut latest = T::accepting_rule(state).map(|rule| (rule, 0));
    for (offset, &byte) in input.as_bytes().iter().enumerate() {
        let Some(next) = T::next_state(state, byte) else {
            break;
        };
        state = next;
        if let Some(rule) = T::accepting_rule(state) {
            latest = Some((rule, offset + 1));
        }
    }
    let (rule, end) = latest?;
    let output = T::execute_rule(rule, &input[..end])?;
    Some((output.token?, end))
}

#[derive(Debug, PartialEq, Lexer)]
enum MembershipToken {
    #[token("private")]
    Private,
    #[token("primitive")]
    Primitive,
    #[token("[a-zA-Z_$][a-zA-Z0-9_$]*")]
    Identifier,
}

#[derive(Debug, PartialEq, Lexer)]
enum CycleAcceptToken {
    #[token("(ab|cdb)*a")]
    First,
    #[token("(ab|cdb)*cd")]
    Second,
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(skip = "[\u{2003}\u{1f642}]+")]
enum BoundaryToken {
    #[token("[a-z\u{e9}\u{1f600}]+", with = boundary_text)]
    Text(String),
}

fn boundary_text(text: &str) -> Result<String, Infallible> {
    Ok(text.to_owned())
}

macro_rules! inspect_automaton {
    ($token:ty) => {
        impl InspectAutomaton for $token {
            fn start_state(mode: usize) -> Option<usize> {
                Self::__runeweaver_start(mode)
            }

            fn next_state(state: usize, byte: u8) -> Option<usize> {
                Self::__runeweaver_transition(state, byte)
            }

            fn accepting_rule(state: usize) -> Option<usize> {
                Self::__runeweaver_accept(state)
            }

            fn execute_rule(rule: usize, text: &str) -> Option<RuleOutput<Self>> {
                let mut scanner = Scanner::<Self, <Self as Automaton>::Extras>::new(text);
                scanner.__set_match(0, text.len());
                let (result, mode_action) = Self::__runeweaver_action(rule, text, &mut scanner);
                result.ok().map(|token| RuleOutput { token, mode_action })
            }
        }
    };
}

inspect_automaton!(RegionToken);
inspect_automaton!(MembershipToken);
inspect_automaton!(CycleAcceptToken);
inspect_automaton!(WithTrivia);
inspect_automaton!(Continuation);
inspect_automaton!(Priority);
inspect_automaton!(ModeTokens);
inspect_automaton!(BoundaryToken);

#[test]
fn token_actions_receive_complete_utf8_text_after_unicode_skips() {
    for prefix in ["", "\u{2003}", "\u{1f642}", "\u{2003}\u{1f642}\u{2003}"] {
        for text in ["a", "\u{e9}", "\u{1f600}", "a\u{e9}\u{1f600}"] {
            let input = format!("{prefix}{text}!");
            assert_eq!(
                generated_match::<BoundaryToken>(&input, 0),
                Some(Ok((
                    stay(Some(BoundaryToken::Text(text.to_owned()))),
                    0,
                    prefix.len() + text.len(),
                )))
            );
        }
        if !prefix.is_empty() {
            assert_eq!(
                generated_match::<BoundaryToken>(prefix, 0),
                Some(Ok((stay(None), 0, prefix.len())))
            );
        }
    }
}

#[test]
fn nested_cycle_paths_preserve_different_saved_rules_and_end_offsets() {
    let alphabet = b"abcd";
    for length in 0..=6 {
        for mut number in 0..alphabet.len().pow(length) {
            let mut bytes = vec![b' '; length as usize];
            for byte in &mut bytes {
                *byte = alphabet[number % alphabet.len()];
                number /= alphabet.len();
            }
            let input = std::str::from_utf8(&bytes).expect("the test alphabet is ASCII");
            let selected = generated_match::<CycleAcceptToken>(input, 0)
                .and_then(Result::ok)
                .and_then(|(output, _, end)| output.token.map(|token| (token, end)));
            assert_eq!(
                selected,
                interpreted_token::<CycleAcceptToken>(input),
                "{input}"
            );
        }
    }
}

#[test]
fn local_membership_tables_preserve_all_byte_ranges_and_keyword_fallbacks() {
    for prefix in ["", "p", "pr", "priv", "private", "primitive"] {
        for byte in 0..=127u8 {
            let input = format!("{prefix}{}!", char::from(byte));
            let selected = generated_match::<MembershipToken>(&input, 0)
                .and_then(Result::ok)
                .and_then(|(output, _, end)| output.token.map(|token| (token, end)));
            assert_eq!(
                selected,
                interpreted_token::<MembershipToken>(&input),
                "{input:?}"
            );
        }
    }
    for input in [
        "private",
        "primitive",
        "priv",
        "private_name",
        "primitive2",
        "private\u{e9}",
        "\u{e9}",
    ] {
        let selected = generated_match::<MembershipToken>(input, 0)
            .and_then(Result::ok)
            .and_then(|(output, _, end)| output.token.map(|token| (token, end)));
        assert_eq!(
            selected,
            interpreted_token::<MembershipToken>(input),
            "{input}"
        );
    }
}

fn selected_region(input: &str) -> Option<(RegionToken, usize)> {
    let (output, _, end) = generated_match::<RegionToken>(input, 0)?.ok()?;
    Some((output.token?, end))
}

#[test]
fn regions_agree_with_interpretation_on_all_short_inputs() {
    let alphabet = b"abxcdzyef!";
    for length in 0..=4 {
        for mut number in 0..alphabet.len().pow(length) {
            let mut bytes = vec![b' '; length as usize];
            for byte in &mut bytes {
                *byte = alphabet[number % alphabet.len()];
                number /= alphabet.len();
            }
            let input = std::str::from_utf8(&bytes).expect("the test alphabet is ASCII");
            assert_eq!(selected_region(input), interpreted_region(input), "{input}");
        }
    }
}

#[test]
fn regions_preserve_saved_matches_across_cycles_and_unicode() {
    for input in [
        "q",
        "qr",
        "qrq",
        "qrqrq!",
        "qrqrqs!",
        "puvu!",
        "tuvu!",
        "puvuvw!",
        "tuvuvw!",
        "\"\u{e9}\u{1f600}\"!",
        "\"\u{e9}\u{1f600}!",
        "\"a\\\"b\"!",
        "mnmnm!",
        "mnmn!",
    ] {
        assert_eq!(selected_region(input), interpreted_region(input), "{input}");
    }
}

#[test]
fn read_loops_preserve_each_exit_and_partial_chunk() {
    for length in 0..=40 {
        let prefix = "a".repeat(length);
        for suffix in ["\"!", "\\nrest\"!", "\u{e9}rest\"!", "", "\\"] {
            let input = format!("\"{prefix}{suffix}");
            assert_eq!(
                selected_region(&input),
                interpreted_region(&input),
                "{input}"
            );
        }
        let input = format!("{prefix}!");
        assert_eq!(
            continuation(&input),
            (length != 0).then_some((Continuation::Short, length))
        );
    }
}

#[test]
fn region_cycles_have_bounded_stack_use_in_debug_and_release() {
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            for input in [
                format!("q{}!", "rq".repeat(128 * 1024)),
                format!("p{}!", "uv".repeat(128 * 1024)),
                format!("\"{}\"", "\u{e9}\u{1f600}\\n".repeat(64 * 1024)),
                format!("{}!", "mn".repeat(128 * 1024)),
            ] {
                assert_eq!(selected_region(&input), interpreted_region(&input));
            }
        })
        .expect("the test thread starts")
        .join()
        .expect("the test thread completes");
}
