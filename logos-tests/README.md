# Logos compatibility tests

This independent workspace runs the complete integration suite from
Logos 0.16.1, commit `8c77ac03c9de98b87d5a8317d441e6a9b4c4b9b2`,
against `runeweaver-logos-shim`.

The Rust test sources and assertions are unchanged. The dependency points
to the shim. The compile-fail snapshots contain Runeweaver diagnostics.
The upstream `source.rs` test remains disabled by its upstream `cfg(any())`.
The shim adds active custom-source tests in its own test directory.

The suite has 123 runtime tests, one compile-test harness, one compile-pass
fixture, and six compile-fail fixtures. It also compiles the upstream
`no_std` fixture. Run both feature configurations:

```sh
cargo test --manifest-path logos-tests/Cargo.toml --locked
cargo test --manifest-path logos-tests/Cargo.toml --locked --all-features
```

The test sources retain the upstream MIT and Apache 2.0 licenses.
The source repository is <https://github.com/maciejhirsz/logos/tree/v0.16.1/tests>.
