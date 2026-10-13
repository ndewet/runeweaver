# Runeweaver Logos shim

This crate supplies the Logos 0.16.1 API with Runeweaver as its lexer engine.
Rename the package to `logos` so existing imports and derives stay unchanged:

```toml
[dependencies]
logos = { package = "runeweaver-logos-shim", version = "0.2" }
```

The shim supports these interfaces:

- `#[derive(Logos)]` with `#[token]` and `#[regex]` rules.
- UTF-8 strings, byte sources, custom sources, and borrowed payloads.
- Rule callbacks, skip callbacks, extras, and custom error types.
- `Lexer`, `Span`, `SpannedIter`, `Skip`, `Filter`, and `FilterResult`.
- `slice`, `remainder`, `span`, `bump`, `morph`, and user extras.

- Subpatterns, Unicode properties, case-insensitive rules, and byte regexes.
- Custom error callbacks, crate overrides, and concrete generic substitutions.
- Skip priorities, Logos priority calculation, and ambiguity diagnostics.
- `Source`, `Chunk`, and source wrappers that implement `Deref`.
- `no_std` builds and the `export_derive` and `forbid_unsafe` features.
- DFA exports through `#[logos(export_dir = ...)]`.

The `logos-tests` workspace runs the complete upstream integration suite.
Its Rust test sources and assertions stay unchanged. Compile-fail snapshots
use Runeweaver diagnostics. CI checks default and safe source configurations.

Runeweaver builds and minimizes the DFA. The shim uses `regex-syntax` to
parse Logos patterns. The runtime uses Runeweaver's shared byte matcher.
It does not depend on Logos or a regex engine, and needs no allocator.
The `debug` and `state_machine_codegen` feature names are accepted. Runeweaver
always generates its own state machine. Graph exports show the Runeweaver DFA.

The source interfaces contain code adapted from Logos. See `licenses/` for
the upstream licenses.
