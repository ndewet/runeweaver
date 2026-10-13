//! Checks source contracts and callback results outside the upstream suite.

#![allow(unsafe_code)]

use logos::{Filter, FilterResult, Logos, Source};
use std::ops::{Deref, Range};

struct Custom<'a>(&'a str);

impl<'source> Source for Custom<'source> {
    type Slice<'a>
        = &'a str
    where
        'source: 'a;
    fn len(&self) -> usize {
        self.0.len()
    }
    fn read<'a, C: logos::source::Chunk<'a>>(&'a self, offset: usize) -> Option<C> {
        self.0.read(offset)
    }
    fn slice(&self, range: Range<usize>) -> Option<&str> {
        self.0.get(range)
    }
    fn find_boundary(&self, index: usize) -> usize {
        self.0.find_boundary(index)
    }
    fn is_boundary(&self, index: usize) -> bool {
        self.0.is_char_boundary(index)
    }
    #[cfg(not(feature = "forbid_unsafe"))]
    unsafe fn slice_unchecked(&self, range: Range<usize>) -> &str {
        &self.0[range]
    }
}

#[derive(Debug, PartialEq, Logos)]
#[logos(source = Custom<'a>)]
enum Token<'a> {
    #[regex(r"\p{Letter}+")]
    Word(&'a str),
}

#[test]
fn a_custom_source_preserves_borrowed_slices_and_utf8_error_boundaries() {
    let source = Custom("λ😀abc");
    let mut lexer = Token::lexer(&source);
    assert_eq!(lexer.next(), Some(Ok(Token::Word("λ"))));
    assert_eq!(lexer.next(), Some(Err(())));
    assert_eq!(lexer.span(), 2..6);
    assert_eq!(lexer.next(), Some(Ok(Token::Word("abc"))));
    assert_eq!(lexer.next(), None);
}

struct Owned(String);
impl Deref for Owned {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

#[test]
fn source_wrappers_and_chunks_follow_the_source_contract() {
    let source = Owned("abcλ".into());
    assert_eq!(source.read::<&[u8; 3]>(0), Some(b"abc"));
    assert_eq!(source.read::<u8>(3), Some(0xce));
    assert_eq!(source.read::<&[u8; 3]>(3), None);
    assert_eq!(Source::slice(&source, 3..5), Some("λ"));
    assert_eq!(source.find_boundary(4), 5);
    assert!(!source.is_boundary(4));
    assert_eq!(
        <[u8] as Source>::read::<&[u8; 2]>(&[0, 255], 0),
        Some(&[0, 255])
    );
}

#[derive(Debug, PartialEq, Logos)]
enum Selected {
    #[token("a", |_| Filter::Emit(Selected::Chosen))]
    #[token("b", |_| FilterResult::<Selected, ()>::Emit(Selected::Chosen))]
    #[token("c", |_| FilterResult::<Selected, ()>::Error(()))]
    #[token("d", |_| Filter::<Selected>::Skip)]
    Input,
    Chosen,
}

#[test]
fn filters_emit_whole_tokens_and_skips_reset_the_span() {
    let mut lexer = Selected::lexer("abcd");
    assert_eq!(lexer.next(), Some(Ok(Selected::Chosen)));
    assert_eq!(lexer.next(), Some(Ok(Selected::Chosen)));
    assert_eq!(lexer.next(), Some(Err(())));
    assert_eq!(lexer.next(), None);
    assert_eq!(lexer.span(), 4..4);
}

#[derive(Debug, PartialEq, Logos)]
enum Prefix {
    #[token("abc")]
    Abc,
}

#[test]
fn failed_prefixes_consume_the_traversed_bytes() {
    let mut lexer = Prefix::lexer("ab!abc");
    assert_eq!(lexer.next(), Some(Err(())));
    assert_eq!(lexer.span(), 0..2);
    assert_eq!(lexer.next(), Some(Err(())));
    assert_eq!(lexer.span(), 2..3);
    assert_eq!(lexer.next(), Some(Ok(Prefix::Abc)));
}
