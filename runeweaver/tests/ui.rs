//! Checks the compiler diagnostics for invalid lexer declarations.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Case {
    name: &'static str,
    message: &'static str,
    location: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "borrowed_payload",
        message: "error: token payloads must be owned",
        location: "src/bin/borrowed_payload.rs:6:10",
    },
    Case {
        name: "duplicate_converter",
        message: "error: `with` can be specified only once",
        location: "src/bin/duplicate_converter.rs:9:34",
    },
    Case {
        name: "duplicate_mode_action",
        message: "error: a lexer rule can have only one mode action",
        location: "src/bin/duplicate_mode_action.rs:6:23",
    },
    Case {
        name: "duplicate_modes",
        message: "error: `modes` can be specified only once",
        location: "src/bin/duplicate_modes.rs:6:35",
    },
    Case {
        name: "invalid_pattern",
        message: "error: unclosed '[' at position 0",
        location: "src/bin/invalid_pattern.rs:5:13",
    },
    Case {
        name: "missing_pattern",
        message: "error: lexer token variants need one `#[token(\"pattern\")]` attribute",
        location: "src/bin/missing_pattern.rs:5:5",
    },
];

fn make_test_project(root: &Path) -> Result<(), Box<dyn Error>> {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dependency_path = source_dir.to_string_lossy().replace('\\', "/");
    let manifest = format!(
        r#"[package]
name = "runeweaver-ui-tests"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
runeweaver = {{ path = "{dependency_path}" }}
"#,
    );

    fs::create_dir_all(root.join("src/bin"))?;
    fs::write(root.join("Cargo.toml"), manifest)?;
    for case in CASES {
        fs::copy(
            source_dir.join(format!("tests/ui/{}.rs", case.name)),
            root.join(format!("src/bin/{}.rs", case.name)),
        )?;
    }
    Ok(())
}

#[test]
fn lexer_declaration_errors_name_the_invalid_input() -> Result<(), Box<dyn Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("runeweaver-ui-{}-{unique}", std::process::id()));
    make_test_project(&root)?;

    for case in CASES {
        let output = Command::new(env!("CARGO"))
            .args(["check", "--quiet", "--offline", "--bin", case.name])
            .current_dir(&root)
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env("CARGO_TERM_COLOR", "never")
            .output()?;
        let stderr = String::from_utf8(output.stderr)?.replace('\\', "/");

        assert!(
            !output.status.success(),
            "{} compiled successfully",
            case.name
        );
        assert!(
            stderr.contains(case.message),
            "{} did not report `{}`:\n{stderr}",
            case.name,
            case.message,
        );
        assert!(
            stderr.contains(case.location),
            "{} did not point at `{}`:\n{stderr}",
            case.name,
            case.location,
        );
    }

    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn derive_supports_a_renamed_runeweaver_dependency() -> Result<(), Box<dyn Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "runeweaver-renamed-{}-{unique}",
        std::process::id()
    ));
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dependency_path = source_dir.to_string_lossy().replace('\\', "/");
    let manifest = format!(
        r#"[package]
name = "runeweaver-renamed-test"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
renamed_runeweaver = {{ package = "runeweaver", path = "{dependency_path}" }}
"#,
    );
    let source = r#"use renamed_runeweaver::Lexer;

#[derive(Lexer)]
enum Token {
    #[token("[a-z]+")]
    Word,
}

fn main() {
    assert_eq!(Token::scan("word").count(), 1);
}
"#;

    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("Cargo.toml"), manifest)?;
    fs::write(root.join("src/main.rs"), source)?;
    let output = Command::new(env!("CARGO"))
        .args(["check", "--quiet", "--offline"])
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("CARGO_TERM_COLOR", "never")
        .output()?;

    assert!(
        output.status.success(),
        "renamed runeweaver dependency did not compile:\n{}",
        String::from_utf8(output.stderr)?,
    );
    fs::remove_dir_all(root)?;
    Ok(())
}
