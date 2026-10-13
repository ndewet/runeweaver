use crate::config::{Config, Definition};
use regex_syntax::hir::{Class, Dot, Hir, HirKind};
use regex_syntax::utf8::Utf8Sequences;
use runeweaver_codegen::BytePattern;
use syn::{Error, Lit, Result};

pub(super) fn parse(definition: &Definition, config: &Config) -> Result<(Hir, usize)> {
    let (text, unicode, bytes) = literal(&definition.args.pattern)?;
    let hir = if definition.literal && !definition.args.ignore_case {
        Hir::literal(bytes.clone())
    } else {
        let pattern = if definition.literal {
            bytes
                .iter()
                .map(|byte| format!("\\x{byte:02x}"))
                .collect::<String>()
        } else {
            expand(&text, config, &mut Vec::new())?
        };
        let pattern = if definition.literal && unicode {
            regex_syntax::escape(&text)
        } else {
            pattern
        };
        regex_syntax::ParserBuilder::new()
            .utf8(false)
            .unicode(unicode)
            .case_insensitive(definition.args.ignore_case)
            .build()
            .parse(&pattern)
            .map_err(|error| Error::new_spanned(&definition.args.pattern, error.to_string()))?
    };
    if !definition.args.allow_greedy && greedy_all(&hir) {
        return Err(Error::new_spanned(
            &definition.args.pattern,
            "an unbounded greedy dot needs allow_greedy = true",
        ));
    }
    let priority = definition.args.priority.unwrap_or_else(|| {
        if definition.literal {
            bytes.len() * 2
        } else {
            complexity(&hir)
        }
    });
    Ok((hir, priority))
}

fn literal(literal: &Lit) -> Result<(String, bool, Vec<u8>)> {
    match literal {
        Lit::Str(text) => Ok((text.value(), true, text.value().into_bytes())),
        Lit::ByteStr(bytes) => {
            let value = bytes.value();
            let text = value
                .iter()
                .map(|byte| {
                    if byte.is_ascii() {
                        char::from(*byte).to_string()
                    } else {
                        format!("\\x{byte:02x}")
                    }
                })
                .collect();
            Ok((text, false, value))
        }
        _ => Err(Error::new_spanned(
            literal,
            "expected a string or byte string",
        )),
    }
}

fn expand(text: &str, config: &Config, stack: &mut Vec<String>) -> Result<String> {
    if text.len() > 100_000 {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "subpattern exceeds the expansion budget",
        ));
    }
    let Some(start) = text.find("(?&") else {
        return Ok(text.into());
    };
    let Some(end) = text[start..].find(')').map(|end| end + start) else {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "unclosed subpattern",
        ));
    };
    let name = &text[start + 3..end];
    let Some(pattern) = config.subpatterns.get(name) else {
        return Err(Error::new(
            proc_macro2::Span::call_site(),
            "unknown subpattern",
        ));
    };
    if stack.iter().any(|item| item == name) {
        return Err(Error::new_spanned(pattern, "recursive subpattern"));
    }
    stack.push(name.into());
    let replacement = expand(&literal(pattern)?.0, config, stack)?;
    stack.pop();
    let result = format!("{}(?:{}){}", &text[..start], replacement, &text[end + 1..]);
    expand(&result, config, stack)
}

fn complexity(hir: &Hir) -> usize {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => 0,
        HirKind::Literal(literal) => {
            2 * std::str::from_utf8(&literal.0).map_or(literal.0.len(), |text| text.chars().count())
        }
        HirKind::Class(_) => 2,
        HirKind::Repetition(repeat) => repeat.min as usize * complexity(&repeat.sub),
        HirKind::Capture(capture) => complexity(&capture.sub),
        HirKind::Concat(parts) => parts.iter().map(complexity).sum(),
        HirKind::Alternation(parts) => parts.iter().map(complexity).min().unwrap_or(0),
    }
}

fn greedy_all(hir: &Hir) -> bool {
    match hir.kind() {
        HirKind::Repetition(repeat) => {
            repeat.greedy
                && repeat.max.is_none()
                && [
                    Dot::AnyChar,
                    Dot::AnyByte,
                    Dot::AnyByteExceptLF,
                    Dot::AnyCharExceptLF,
                    Dot::AnyByteExceptCRLF,
                    Dot::AnyCharExceptCRLF,
                ]
                .iter()
                .any(|dot| repeat.sub.as_ref() == &Hir::dot(*dot))
        }
        HirKind::Capture(capture) => greedy_all(&capture.sub),
        HirKind::Concat(parts) | HirKind::Alternation(parts) => parts.iter().any(greedy_all),
        _ => false,
    }
}

pub(super) fn lower(hir: &Hir) -> std::result::Result<BytePattern, &'static str> {
    Ok(match hir.kind() {
        HirKind::Empty => BytePattern::Empty,
        HirKind::Literal(literal) => BytePattern::Concat(
            literal
                .0
                .iter()
                .map(|byte| BytePattern::Range(*byte, *byte))
                .collect(),
        ),
        HirKind::Class(Class::Bytes(class)) => BytePattern::Alternation(
            class
                .iter()
                .map(|range| BytePattern::Range(range.start(), range.end()))
                .collect(),
        ),
        HirKind::Class(Class::Unicode(class)) => BytePattern::Alternation(
            class
                .iter()
                .flat_map(|range| Utf8Sequences::new(range.start(), range.end()))
                .map(|sequence| {
                    BytePattern::Concat(
                        sequence
                            .as_slice()
                            .iter()
                            .map(|range| BytePattern::Range(range.start, range.end))
                            .collect(),
                    )
                })
                .collect(),
        ),
        HirKind::Look(_) => return Err("Logos does not support anchors or word boundaries"),
        HirKind::Capture(capture) => lower(&capture.sub)?,
        HirKind::Concat(parts) => BytePattern::Concat(
            parts
                .iter()
                .map(lower)
                .collect::<std::result::Result<_, _>>()?,
        ),
        HirKind::Alternation(parts) => BytePattern::Alternation(
            parts
                .iter()
                .map(lower)
                .collect::<std::result::Result<_, _>>()?,
        ),
        HirKind::Repetition(repeat) => BytePattern::Repeat {
            pattern: Box::new(lower(&repeat.sub)?),
            min: repeat.min as usize,
            max: repeat.max.map(|max| max as usize),
        },
    })
}
