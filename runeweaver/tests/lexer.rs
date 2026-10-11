//! End-to-end tests for generated lexers.

#![deny(dead_code)]

use runeweaver::{Lexer, ScanError, Span, Spanned};
use std::convert::Infallible;
use std::str::FromStr;

fn first<T: Lexer>(input: &str) -> Option<(T, usize)> {
    T::scan(input)
        .next()?
        .ok()
        .map(|spanned| (spanned.token, spanned.span.end as usize))
}

#[derive(Debug, PartialEq, Lexer)]
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
        WithTrivia::scan_one(" \tword tail", 0),
        Some(Ok((
            Some(WithTrivia::Identifier),
            2,
            6,
            runeweaver::Transition::Stay,
        )))
    );
    assert_eq!(
        WithTrivia::scan_one(" \t", 0),
        Some(Ok((None, 2, 2, runeweaver::Transition::Stay)))
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
    let scanned: Vec<_> = WithNestedComments::scan(input).collect();

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
    let scanned: Vec<_> = WithNestedComments::scan(input).collect();

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
    let scanned: Vec<_> = WithTrivia::scan("one @ two").collect();

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
    let scanned: Vec<_> = Value::scan("42 name !hello").collect();

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
    let scanned: Vec<_> = Value::scan("18446744073709551616 7").collect();

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
    let scanned: Vec<_> = Value::scan("#no 7").collect();

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
fn a_failed_payload_still_applies_its_mode_transition() {
    let scanned: Vec<_> = FailedModePayload::scan("[xa").collect();

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
    assert_eq!(Repetition::scan("aaab"), Some((Repetition::OpenEnded, 4)));
    assert_eq!(Repetition::scan("b"), Some((Repetition::NullableLoop, 1)));
    assert_eq!(Repetition::scan("aaa"), None);
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
    let scanned: Vec<_> = UnicodeBoundaries::scan("\u{7f}\u{80}é€𐐷@").collect();

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
    let scanned: Vec<_> = NegatedClass::scan("a\né").collect();

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
        Converters::scan("!hi").next(),
        Some(Ok(Spanned {
            token: Converters::Direct("HI".to_owned()),
            span: Span::new(0, 3),
        }))
    );
    assert_eq!(
        Converters::scan("42").next(),
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

#[test]
fn scan_errors_expose_their_location_without_matching_variants() {
    let error = ScanError::InvalidPayload {
        span: Span::new(3, 7),
        message: "invalid value".to_owned(),
    };
    assert_eq!(error.span(), Span::new(3, 7));
    assert_eq!(error.offset(), 3);
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
    let (token, _, length, _) = Continuation::scan_one(input, 0)?.ok()?;
    Some((token?, length))
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
        Priority::scan_one("a", 0),
        Some(Ok((
            Some(Priority::First),
            0,
            1,
            runeweaver::Transition::Stay
        )))
    );
    assert_eq!(
        ModeTokens::scan_one("text", 0),
        Some(Ok((
            Some(ModeTokens::Identifier),
            0,
            4,
            runeweaver::Transition::Stay
        )))
    );
    assert_eq!(
        ModeTokens::scan_one("text", 1),
        Some(Ok((
            Some(ModeTokens::StringText),
            0,
            4,
            runeweaver::Transition::Stay
        )))
    );
    assert_eq!(ModeTokens::scan_one("text", 2), None);
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

fn interpreted_token<T: Lexer>(input: &str) -> Option<(T, usize)> {
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
    let (token, _) = T::run_action(rule, &input[..end]).ok()?;
    Some((token?, end))
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

#[test]
fn token_actions_receive_complete_utf8_text_after_unicode_skips() {
    for prefix in ["", "\u{2003}", "\u{1f642}", "\u{2003}\u{1f642}\u{2003}"] {
        for text in ["a", "\u{e9}", "\u{1f600}", "a\u{e9}\u{1f600}"] {
            let input = format!("{prefix}{text}!");
            assert_eq!(
                BoundaryToken::scan_one(&input, 0),
                Some(Ok((
                    Some(BoundaryToken::Text(text.to_owned())),
                    prefix.len(),
                    prefix.len() + text.len(),
                    runeweaver::Transition::Stay
                )))
            );
        }
        if !prefix.is_empty() {
            assert_eq!(
                BoundaryToken::scan_one(prefix, 0),
                Some(Ok((
                    None,
                    prefix.len(),
                    prefix.len(),
                    runeweaver::Transition::Stay
                )))
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
            let selected = CycleAcceptToken::scan_one(input, 0)
                .and_then(Result::ok)
                .and_then(|(token, _, end, _)| token.map(|token| (token, end)));
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
            let selected = MembershipToken::scan_one(&input, 0)
                .and_then(Result::ok)
                .and_then(|(token, _, end, _)| token.map(|token| (token, end)));
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
        let selected = MembershipToken::scan_one(input, 0)
            .and_then(Result::ok)
            .and_then(|(token, _, end, _)| token.map(|token| (token, end)));
        assert_eq!(
            selected,
            interpreted_token::<MembershipToken>(input),
            "{input}"
        );
    }
}

fn selected_region(input: &str) -> Option<(RegionToken, usize)> {
    let (token, _, end, _) = RegionToken::scan_one(input, 0)?.ok()?;
    Some((token?, end))
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
