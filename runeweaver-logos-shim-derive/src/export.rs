use runeweaver_codegen::ByteAutomaton;
use std::fmt::Write;
use std::path::Path;
use syn::{Error, LitStr, Result};

pub(super) fn graphs(destination: &LitStr, name: &str, automaton: &ByteAutomaton) -> Result<()> {
    let value = destination.value();
    let path = Path::new(&value);
    let paths = match path.extension().and_then(|extension| extension.to_str()) {
        Some("dot") => vec![(path.to_owned(), true)],
        Some("mmd") => vec![(path.to_owned(), false)],
        None => vec![
            (path.join(format!("{name}.dot")), true),
            (path.join(format!("{name}.mmd")), false),
        ],
        _ => {
            return Err(Error::new_spanned(
                destination,
                "the export path must name a directory, .dot file, or .mmd file",
            ));
        }
    };
    for (path, dot) in paths {
        let mut text = if dot { "digraph {\n" } else { "flowchart LR\n" }.to_owned();
        for (state, accepts) in automaton.accepts.iter().enumerate() {
            if dot {
                writeln!(text, "s{state} [label=\"{state}: {accepts:?}\"];")
                    .expect("writing to a String succeeds");
            } else {
                writeln!(text, "s{state}[\"{state}: {accepts:?}\"]")
                    .expect("writing to a String succeeds");
            }
            for (low, high, target) in &automaton.transitions[state] {
                if dot {
                    writeln!(
                        text,
                        "s{state} -> s{target} [label=\"{low:02X}-{high:02X}\"];"
                    )
                    .expect("writing to a String succeeds");
                } else {
                    writeln!(text, "s{state} -->|{low:02X}-{high:02X}| s{target}")
                        .expect("writing to a String succeeds");
                }
            }
        }
        if dot {
            text.push_str("}\n");
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| Error::new_spanned(destination, error.to_string()))?;
        }
        std::fs::write(path, text)
            .map_err(|error| Error::new_spanned(destination, error.to_string()))?;
    }
    Ok(())
}
