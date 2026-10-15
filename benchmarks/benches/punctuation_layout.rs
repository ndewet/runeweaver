#![allow(dead_code)]

#[path = "../cases/punctuation.rs"]
mod case;
#[path = "../support/mod.rs"]
mod support;

use runeweaver::Lexer;
use std::hint::black_box;

#[inline(never)]
fn placement_count(input: &str) -> usize {
    case::Token::scan(black_box(input))
        .try_fold(0, |count, token| token.map(|_| count + 1))
        .expect("the source contains valid tokens")
}

#[inline(never)]
fn placement_spans(input: &str) -> usize {
    case::Token::scan(black_box(input))
        .spanned()
        .map(|result| {
            let token = result.expect("the source contains valid tokens");
            token.span.end - token.span.start
        })
        .sum()
}

#[inline(never)]
fn placement_values(input: &str) -> usize {
    case::Token::scan(black_box(input))
        .map(|token| token.expect("the source contains valid tokens") as usize + 1)
        .sum()
}

fn main() {
    let input = case::SOURCE.repeat((512 * 1024usize).div_ceil(case::SOURCE.len()));
    println!(
        "{{\"bytes\":{},\"count\":{},\"spans\":{},\"values\":{}}}",
        input.len(),
        placement_count(black_box(input.as_str())),
        placement_spans(black_box(input.as_str())),
        placement_values(black_box(input.as_str())),
    );
}
