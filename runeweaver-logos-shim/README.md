# Runeweaver Logos shim

This crate supplies the common Logos API with Runeweaver as its lexer engine.
Rename the package to `logos` so existing imports and derives stay unchanged:

```toml
[dependencies]
logos = { package = "runeweaver-logos-shim", version = "0.2" }
```

The shim supports these interfaces:

- `#[derive(Logos)]` with `#[token]` and `#[regex]` rules.
- UTF-8 `str` sources and borrowed `&str` payloads.
- Rule callbacks, skip callbacks, extras, and custom error types.
- `Lexer`, `Span`, `SpannedIter`, `Skip`, `Filter`, and `FilterResult`.
- `slice`, `remainder`, `span`, `bump`, `morph`, and user extras.

The shim does not support these Logos interfaces yet:

- Byte sources and custom source implementations in derived lexers.
- Subpatterns, case-insensitive rules, and Logos-specific regex syntax.
- Custom error callbacks and the `#[logos(crate = ...)]` override.
- Skip priorities and exact Logos priority calculation for regex rules.

Explicit variant priorities and literal priorities work. Equal regex matches
use declaration order. Runeweaver reports unsupported regex syntax during the
derive.
