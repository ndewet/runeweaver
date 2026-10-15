# Runeweaver

**A compile-time lexer generator for Rust.**

[![CI](https://github.com/ndewet/runeweaver/actions/workflows/ci.yml/badge.svg)](https://github.com/ndewet/runeweaver/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/runeweaver.svg)](https://crates.io/crates/runeweaver)
[![docs.rs](https://docs.rs/runeweaver/badge.svg)](https://docs.rs/runeweaver)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Define your tokens as an enum. Runeweaver compiles their patterns into a
deterministic finite automaton at build time and gives you an iterator over typed
tokens. Use it to build parsers for programming languages, configuration formats,
and domain-specific languages.

- **Typed tokens:** borrow text directly from the input, parse values with
  `FromStr`, or supply a custom converter.
- **Predictable matching:** the longest match wins; declaration order breaks ties.
- **Context-aware rules:** lexical modes handle strings, directives, and nested
  comments. Scanner callbacks can access and update your own state.
- **Useful diagnostics:** UTF-8 byte spans and recoverable errors connect tokens
  back to their source.
- **Compile-time generation:** no runtime regular-expression engine. Invalid
  patterns, empty matches, and rules that can never win are rejected at build time.

[API documentation](https://docs.rs/runeweaver) ·
[Runnable examples](runeweaver/examples) ·
[Report an issue](https://github.com/ndewet/runeweaver/issues)

## Get started

Add Runeweaver to your project. Rust **1.86 or newer** is required.

```toml
[dependencies]
runeweaver = "0.2"
```

Derive `Lexer` and give each variant a token pattern:

```rust
use runeweaver::{Lexer, Span, Spanned};

#[derive(Debug, PartialEq, Lexer)]
#[lexer(skip = r"\s+")]
enum Token<'input> {
    #[token("let")]
    Let,

    #[token("[a-zA-Z_][a-zA-Z0-9_]*")]
    Identifier(&'input str),

    #[token("=")]
    Equals,

    #[token("[0-9]+")]
    Integer(u64),
}

fn main() -> Result<(), runeweaver::ScanError> {
    let tokens = Token::scan("let answer = 42")
        .spanned()
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(
        tokens,
        vec![
            Spanned { token: Token::Let, span: Span::new(0, 3) },
            Spanned { token: Token::Identifier("answer"), span: Span::new(4, 10) },
            Spanned { token: Token::Equals, span: Span::new(11, 12) },
            Spanned { token: Token::Integer(42), span: Span::new(13, 15) },
        ],
    );
    Ok(())
}
```

The identifier borrows from the input; the integer is parsed into a `u64`.
Skip rules consume text without emitting tokens. Omit `.spanned()` when you only
need token values.

## Matching rules

Every `#[token("...")]` attribute is a regular expression, matched at the current
input position. The rule that consumes the most input wins. For equally long
matches, the rule declared first wins: in the example above, `let` is a keyword,
while `letter` is a single identifier.

Patterns support Unicode literals and character ranges, character classes and
negation, grouping, alternation, and repetition (`?`, `*`, `+`, `{n}`, `{n,}`,
`{n,m}`). Shorthand classes such as `\d`, `\w`, and `\s` use ASCII definitions.
Escape regular-expression punctuation to match it literally, for example
`#[token(r"\+")]` for a plus sign.

Look-around, backreferences, and lazy quantifiers are unsupported. See the
[pattern examples](runeweaver/examples/patterns.rs) for the supported constructs.

## Token values and scanner state

Use a unit variant for a token kind, or a single-field tuple variant for a value.
A `&str` field borrows the matched text. Other fields use `FromStr` by default;
`with` supplies a custom converter that returns either a value or a `Result`:

```rust
use runeweaver::Lexer;

#[derive(Lexer)]
enum Token {
    #[token("0x[0-9a-fA-F]+", with = parse_hex)]
    Hex(u64),
}

fn parse_hex(text: &str) -> Result<u64, std::num::ParseIntError> {
    u64::from_str_radix(&text[2..], 16)
}
```

For conversion that needs scanner access, use `with_scanner`. Callbacks can read
the matched `slice()` and `remainder()`, extend a match with `bump()`, and update
`scanner.extras`. Declare the state type with `#[lexer(extras = MyState)]` and
initialize it with `Token::scan_with_extras(input, state)`.

See the [payload examples](runeweaver/examples/payloads.rs) and
[`Scanner` reference](https://docs.rs/runeweaver/latest/runeweaver/struct.Scanner.html).

## Lexical modes

Modes select which rules are active. Use `begin` to switch modes, `push` to enter
a nested context, and `pop` to return to the previous mode. For example, these
skip rules handle nested block comments:

```rust
use runeweaver::Lexer;

#[derive(Lexer)]
#[lexer(mode = Comment)]
#[lexer(skip = r"\s+")]
#[lexer(skip = r"/\*", push = Comment)]
#[lexer(skip = r"/\*", modes = Comment, push = Comment)]
#[lexer(skip = r"\*/", modes = Comment, pop)]
#[lexer(skip = r"[^*/]+|[*/]", modes = Comment)]
enum Token<'input> {
    #[token("[a-z]+")]
    Word(&'input str),
}
```

Rules without `modes` apply in the initial mode. If input ends while a pushed mode
is still open, the scanner reports its name and the span from its opening rule
through the end of input. The [modes example](runeweaver/examples/modes.rs) also
demonstrates switching between contexts with `begin`.

## Spans and error recovery

`Token::scan(input)` yields `Result<Token, ScanError>`. The scanner exposes the
current match through `span()` and `slice()`; `.spanned()` yields
`Result<Spanned<Token>, ScanError>` instead. Spans are half-open ranges of UTF-8
byte offsets, suitable for slicing the original input or passing to a diagnostic
renderer.

| Error | Meaning |
| --- | --- |
| `Unrecognized` | No rule accepts the next character. |
| `InvalidPayload` | A matched token's value could not be converted. |
| `UnterminatedMode` | Input ended inside a pushed mode. |

Every error includes a span. Unrecognized characters and invalid payloads are
consumed, so you can keep iterating to collect further tokens and diagnostics.
Collecting into `Result<Vec<_>, _>`, as in the quick start, stops at the first error.
See the [error recovery example](runeweaver/examples/errors.rs).

## Explore and contribute

The [examples directory](runeweaver/examples) contains small programs covering
matching, payloads, modes, patterns, and errors. Run one from the repository root:

```console
cargo run -p runeweaver --example modes
```

Bug reports and pull requests are welcome on
[GitHub](https://github.com/ndewet/runeweaver). For a bug report, include a small
token definition, the input, and the expected and actual results.

To check a change locally:

```console
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
```

For performance work, see the [benchmark guide](benchmarks/README.md) for the
generation and scanning suites and instructions for adding cases.

## Compatibility

Runeweaver is pre-1.0. Patch and minor releases preserve the documented public API;
breaking changes require a major release. This guarantee covers the `runeweaver`
runtime crate and its re-exported derive macro. The code generation crate and
hidden interfaces used by generated code are outside that guarantee.

Raising the minimum supported Rust version requires at least a minor release.
Bug fixes target the latest release line.

## License and acknowledgements

Runeweaver is licensed under the [MIT license](LICENSE).

Thanks to [Logos](https://github.com/maciejhirsz/logos) for inspiring the
derive-based token declaration syntax.
