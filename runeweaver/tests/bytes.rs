//! Checks native byte scanning and shared byte matching.

use runeweaver::{ByteScanner, Lexer, ScanError, Span, bytes::ByteMatcher};

#[derive(Debug, PartialEq, Lexer)]
#[lexer(source = [u8])]
#[lexer(skip = r"\x00+")]
enum Binary<'input> {
    #[token(r"\xca\xfe")]
    Header,
    #[token(r"[\x80-\xff]+")]
    Data(&'input [u8]),
    #[token("[a-z]+")]
    Word(&'input [u8]),
}

#[test]
fn bytes_are_not_decoded_and_payloads_borrow_the_original_input() {
    let input = b"\x00\xca\xfe\x00\x80\xffword!";
    let mut scanner = Binary::scan(input);
    assert_eq!(scanner.next(), Some(Ok(Binary::Header)));
    assert_eq!(scanner.span(), Span::new(1, 3));
    assert_eq!(scanner.next(), Some(Ok(Binary::Data(&input[4..6]))));
    assert_eq!(scanner.slice().as_ptr(), input[4..].as_ptr());
    assert_eq!(scanner.next(), Some(Ok(Binary::Word(b"word"))));
    assert_eq!(
        scanner.next(),
        Some(Err(ScanError::Unrecognized {
            span: Span::new(10, 11)
        }))
    );
    assert_eq!(scanner.next(), None);
    assert_eq!(scanner.source(), input);
    assert_eq!(scanner.remainder(), b"");
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(source = [u8])]
enum AnyByte {
    #[token(r"[\x00-\xff]")]
    Byte,
}

#[test]
fn the_byte_alphabet_includes_all_256_values() {
    let input: Vec<u8> = (0..=255).collect();
    let mut scanner = ByteScanner::<AnyByte>::new(&input);
    for offset in 0..256 {
        assert_eq!(scanner.next(), Some(Ok(AnyByte::Byte)));
        assert_eq!(scanner.span(), Span::new(offset, offset + 1));
        assert_eq!(scanner.slice(), &input[offset..offset + 1]);
    }
    assert_eq!(scanner.next(), None);
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(source = [u8])]
#[lexer(extras = usize)]
#[lexer(mode = Frame)]
#[lexer(skip = r"\x00", with_scanner = count_skip)]
enum Stateful<'input> {
    #[token(r"\xfe", push = Frame)]
    Start,
    #[token(r"\xff", modes = Frame, with_scanner = include_byte, pop)]
    End(&'input [u8]),
}

fn count_skip(scanner: &mut ByteScanner<'_, Stateful<'_>, usize>) {
    scanner.extras += 1;
}

fn include_byte<'input>(
    scanner: &mut ByteScanner<'input, Stateful<'input>, usize>,
) -> &'input [u8] {
    scanner.bump(1);
    scanner.extras += 10;
    scanner.slice()
}

#[test]
fn byte_callbacks_bump_the_input_and_preserve_modes_and_extras() {
    let mut scanner = Stateful::scan_with_extras(b"\x00\xfe\xff\x80\xfe", 5);
    assert_eq!(scanner.next(), Some(Ok(Stateful::Start)));
    assert_eq!(scanner.extras, 6);
    assert_eq!(scanner.next(), Some(Ok(Stateful::End(b"\xff\x80"))));
    assert_eq!(scanner.span(), Span::new(2, 4));
    assert_eq!(scanner.extras, 16);
    assert_eq!(scanner.next(), Some(Ok(Stateful::Start)));
    assert_eq!(
        scanner.next(),
        Some(Err(ScanError::UnterminatedMode {
            span: Span::new(4, 5),
            mode: "Frame"
        }))
    );
    assert_eq!(scanner.next(), None);
}

#[derive(Debug, PartialEq, Lexer)]
#[lexer(source = [u8])]
#[lexer(mode = Frame)]
#[lexer(skip = r"\xfe", push = Frame)]
enum Fallible {
    #[token(r"\xff", modes = Frame, with = reject, pop)]
    Rejected(u8),
    #[token("a")]
    A,
}

fn reject(bytes: &[u8]) -> Result<u8, &'static str> {
    assert_eq!(bytes, b"\xff");
    Err("invalid byte payload")
}

#[test]
fn byte_payload_errors_apply_mode_actions_and_scanning_continues() {
    let mut scanner = Fallible::scan(b"\xfe\xffa");
    assert_eq!(
        scanner.next(),
        Some(Err(ScanError::InvalidPayload {
            span: Span::new(1, 2),
            message: "invalid byte payload".into()
        }))
    );
    assert_eq!(scanner.next(), Some(Ok(Fallible::A)));
    assert_eq!(scanner.next(), None);
}

struct Prefix;
impl ByteMatcher for Prefix {
    fn start() -> usize {
        0
    }
    fn transition(state: usize, byte: u8) -> Option<usize> {
        match (state, byte) {
            (0, b'a') => Some(1),
            (1 | 2, b'b') => Some(2),
            (2, b'c') => Some(3),
            _ => None,
        }
    }
    fn accept(state: usize) -> Option<usize> {
        match state {
            1 => Some(0),
            3 => Some(1),
            _ => None,
        }
    }
}

#[test]
fn shared_matching_preserves_the_last_accept_and_the_failed_prefix() {
    let matched = Prefix::match_bytes(b"abb!");
    assert_eq!(matched.rule, Some(0));
    assert_eq!(matched.end, 1);
    assert_eq!(matched.traversed, 3);
    let matched = Prefix::match_source(|offset| b"!abbc!".get(offset).copied(), 1);
    assert_eq!(matched.rule, Some(1));
    assert_eq!(matched.end, 5);
    let matched = Prefix::match_bytes(b"!");
    assert_eq!(matched.rule, None);
    assert_eq!(matched.end, 0);
    assert_eq!(matched.traversed, 0);
    assert_eq!(Prefix::match_bytes(b"").rule, None);
}

#[test]
fn spanned_byte_scanning_preserves_absolute_offsets() {
    let results: Vec<_> = AnyByte::scan(b"\xff\x80")
        .spanned()
        .collect::<Result<_, _>>()
        .expect("valid bytes");
    assert_eq!(results[0].span, Span::new(0, 1));
    assert_eq!(results[1].span, Span::new(1, 2));
}

#[test]
#[should_panic(expected = "invalid scanner bump")]
fn byte_bumps_cannot_pass_the_end_of_input() {
    AnyByte::scan(b"\xff").bump(2);
}
