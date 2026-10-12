//! Renders completed selector control flow as Rust source.

mod inlining;

use crate::ir::{
    Block, Call, Context, Dispatch, Function, FunctionBody, MatchValue, Position, Selector,
    Transfer,
};
use proc_macro2::TokenStream;
use std::fmt::Write;

pub(super) fn emit(selector: &Selector) -> TokenStream {
    let functions: Vec<_> = selector
        .functions
        .iter()
        .enumerate()
        .map(|(id, function)| Renderer::render_function(selector, id, function))
        .collect();
    let sizes: Vec<_> = functions.iter().map(String::len).collect();
    let forced_inline = inlining::select(selector, &sizes);
    let mut renderer = Renderer::new(selector);
    renderer.entry(selector);
    for (source, forced) in functions.iter().zip(forced_inline) {
        if forced {
            renderer.source.push_str("#[inline(always)] ");
        }
        renderer.source.push_str(source);
    }
    renderer
        .source
        .parse()
        .expect("generated selector source is valid Rust tokens")
}

struct Renderer<'a> {
    source: String,
    selector: &'a Selector,
}

impl<'a> Renderer<'a> {
    fn new(selector: &'a Selector) -> Self {
        Self {
            source: String::new(),
            selector,
        }
    }

    fn entry(&mut self, selector: &Selector) {
        self.source.push_str(
            "#[inline(always)] fn __runeweaver_select(input: &[u8], start_condition: usize) \
             -> Option<(::core::num::NonZeroUsize, usize)> { match start_condition {",
        );
        for (mode, start) in selector.starts.iter().enumerate() {
            write!(self.source, "{mode} => ").expect("writing to a string succeeds");
            self.call(start);
            self.source.push(',');
        }
        self.source.push_str("_ => None, } }");
    }

    fn render_function(selector: &'a Selector, id: usize, plan: &Function) -> String {
        let mut renderer = Self::new(selector);
        renderer.function(id, plan);
        renderer.source
    }

    fn function(&mut self, id: usize, plan: &Function) {
        let mutable_context = body_updates_context(&plan.body);
        write!(self.source, "fn __runeweaver_region_{id}(input: &[u8], ")
            .expect("writing to a string succeeds");
        if body_scans(&plan.body) {
            self.source.push_str("mut ");
        }
        self.source.push_str("index: usize");
        match plan.context {
            Context::Empty => {}
            Context::Fixed {
                optional: false, ..
            } => {
                self.source.push_str(", ");
                if mutable_context {
                    self.source.push_str("mut ");
                }
                self.source.push_str("saved_end: usize");
            }
            Context::Fixed { optional: true, .. } => {
                self.source.push_str(", ");
                if mutable_context {
                    self.source.push_str("mut ");
                }
                self.source.push_str("saved_end: Option<usize>");
            }
            Context::General => {
                self.source.push_str(", ");
                if mutable_context {
                    self.source.push_str("mut ");
                }
                self.source
                    .push_str("latest: Option<(::core::num::NonZeroUsize, usize)>");
            }
        }
        if matches!(plan.body, FunctionBody::Dispatch { initial: None, .. }) {
            self.source.push_str(", mut state: usize");
        }
        self.source
            .push_str(") -> Option<(::core::num::NonZeroUsize, usize)> {");
        match &plan.body {
            FunctionBody::Linear(block) => self.block(block),
            FunctionBody::Loop(block) => {
                self.source.push_str("'region: loop {");
                self.block(block);
                self.source.push('}');
            }
            FunctionBody::Dispatch { initial, arms } => {
                if let Some(state) = initial {
                    write!(self.source, "let mut state = {state};")
                        .expect("writing to a string succeeds");
                }
                self.source.push_str("'region: loop { match state {");
                for (ordinal, block) in arms.iter().enumerate() {
                    write!(self.source, "{ordinal} => {{").expect("writing to a string succeeds");
                    self.block(block);
                    self.source.push_str("},");
                }
                self.source.push_str(
                    "_ => unreachable!(\"generated region selected an unknown state\"), } }",
                );
            }
        }
        self.source.push('}');
    }

    fn call(&mut self, plan: &Call) {
        write!(
            self.source,
            "Self::__runeweaver_region_{}(input, ",
            plan.region
        )
        .expect("writing to a string succeeds");
        self.position(plan.index);
        match self.selector.functions[plan.region].context {
            Context::Empty => {}
            Context::Fixed {
                optional: false, ..
            } => {
                self.source.push_str(", (");
                self.match_value(plan.saved);
                self.source
                    .push_str(").expect(\"the saved rule is live\").1");
            }
            Context::Fixed { optional: true, .. } => {
                self.source.push_str(", (");
                self.match_value(plan.saved);
                self.source.push_str(").map(|(_, end)| end)");
            }
            Context::General => {
                self.source.push_str(", ");
                self.match_value(plan.saved);
            }
        }
        if let Some(entry) = plan.entry {
            write!(self.source, ", {entry}").expect("writing to a string succeeds");
        }
        self.source.push(')');
    }

    fn block(&mut self, plan: &Block) {
        let Block::Scan {
            self_loop,
            failure,
            dispatch,
        } = plan
        else {
            let Block::Return(value) = plan else {
                unreachable!("all selector blocks have known variants")
            };
            self.source.push_str("return ");
            self.match_value(*value);
            self.source.push(';');
            return;
        };
        if let Some(loop_plan) = self_loop {
            let table: Vec<_> = loop_plan
                .values
                .iter()
                .map(|&value| u8::from(value))
                .collect();
            let table = proc_macro2::Literal::byte_string(&table);
            write!(self.source, "const SELF_LOOP: &[u8; 256] = {table};")
                .expect("writing to a string succeeds");
            write!(
                self.source,
                "'fast: while index + {} <= input.len() {{ \
                 let bytes: &[u8; {}] = input[index..index + {}].try_into()\
                 .expect(\"an eight-byte slice has eight bytes\");",
                crate::ir::SELF_LOOP_CHUNK,
                crate::ir::SELF_LOOP_CHUNK,
                crate::ir::SELF_LOOP_CHUNK
            )
            .expect("writing to a string succeeds");
            for offset in (0..crate::ir::SELF_LOOP_CHUNK).step_by(2) {
                let next = offset + 1;
                write!(
                    self.source,
                    "if SELF_LOOP[bytes[{offset}] as usize] & \
                     SELF_LOOP[bytes[{next}] as usize] == 0 {{ \
                     index += {offset}; break 'fast; }}"
                )
                .expect("writing to a string succeeds");
            }
            write!(
                self.source,
                "index += {}; }} while index < input.len() && \
                 SELF_LOOP[input[index] as usize] != 0 {{ index += 1; }}",
                crate::ir::SELF_LOOP_CHUNK
            )
            .expect("writing to a string succeeds");
        }
        self.source.push_str("if index == input.len() { return ");
        self.match_value(*failure);
        self.source.push_str("; }");
        match dispatch {
            Dispatch::Fail => {
                self.source.push_str("return ");
                self.match_value(*failure);
                self.source.push(';');
            }
            Dispatch::Table { values, branches } => {
                self.source.push_str("let byte = input[index];");
                self.table(branches.len(), values);
                self.source.push_str("match FORK[byte as usize] {");
                for (ordinal, transfer) in branches.iter().enumerate() {
                    write!(self.source, "{ordinal} => {{").expect("writing to a string succeeds");
                    self.transfer(transfer);
                    self.source.push_str("},");
                }
                self.source.push_str("_ => return ");
                self.match_value(*failure);
                self.source.push_str(", }");
            }
            Dispatch::Ranges(branches) => {
                self.source.push_str("let byte = input[index];");
                for (ranges, transfer) in branches {
                    self.source.push_str("if false");
                    for range in ranges {
                        write!(
                            self.source,
                            " || ({}u8..={}u8).contains(&byte)",
                            range.low, range.high
                        )
                        .expect("writing to a string succeeds");
                    }
                    self.source.push_str(" {");
                    self.transfer(transfer);
                    self.source.push('}');
                }
                self.source.push_str("return ");
                self.match_value(*failure);
                self.source.push(';');
            }
        }
    }

    fn table(&mut self, dead: usize, values: &[usize; 256]) {
        let Some(width) = table_width(dead) else {
            let values: Vec<_> = values
                .iter()
                .map(|&ordinal| u8::try_from(ordinal).expect("a u8 table value fits in u8"))
                .collect();
            let values = proc_macro2::Literal::byte_string(&values);
            write!(self.source, "const FORK: &[u8; 256] = {values};")
                .expect("writing to a string succeeds");
            return;
        };
        write!(self.source, "const FORK: &[{width}; 256] = &[")
            .expect("writing to a string succeeds");
        for value in values {
            write!(self.source, "{value} as {width},").expect("writing to a string succeeds");
        }
        self.source.push_str("]; ");
    }

    fn transfer(&mut self, plan: &Transfer) {
        match plan {
            Transfer::Return(value) => {
                self.source.push_str("return ");
                self.match_value(*value);
                self.source.push(';');
            }
            Transfer::Call(call) => {
                self.source.push_str("return ");
                self.call(call);
                self.source.push(';');
            }
            Transfer::Continue { update, header } => {
                if let Some((context, value)) = update {
                    match context {
                        Context::Empty => {}
                        Context::Fixed {
                            optional: false, ..
                        } => {
                            self.source.push_str("saved_end = (");
                            self.match_value(*value);
                            self.source
                                .push_str(").expect(\"the saved rule is live\").1;");
                        }
                        Context::Fixed { optional: true, .. } => {
                            self.source.push_str("saved_end = (");
                            self.match_value(*value);
                            self.source.push_str(").map(|(_, end)| end);");
                        }
                        Context::General => {
                            self.source.push_str("latest = ");
                            self.match_value(*value);
                            self.source.push(';');
                        }
                    }
                }
                self.source.push_str("index += 1;");
                if let Some(header) = header {
                    write!(self.source, "state = {header};").expect("writing to a string succeeds");
                }
                self.source.push_str("continue 'region;");
            }
            Transfer::Inline { bind, body } => {
                if let Some(value) = bind {
                    self.source.push_str("let saved = ");
                    self.match_value(*value);
                    self.source.push(';');
                }
                self.source.push_str("index += 1;");
                self.block(body);
            }
        }
    }

    fn match_value(&mut self, value: MatchValue) {
        match value {
            MatchValue::Empty | MatchValue::Context(Context::Empty) => self
                .source
                .push_str("None::<(::core::num::NonZeroUsize, usize)>"),
            MatchValue::Accepted { rule, end } => {
                self.source.push_str("Some((");
                self.rule_tag(rule);
                self.source.push_str(", ");
                self.position(end);
                self.source.push_str("))");
            }
            MatchValue::Context(Context::Fixed {
                rule,
                optional: false,
            }) => {
                self.source.push_str("Some((");
                self.rule_tag(rule);
                self.source.push_str(", saved_end))");
            }
            MatchValue::Context(Context::Fixed {
                rule,
                optional: true,
            }) => {
                self.source.push_str("saved_end.map(|end| (");
                self.rule_tag(rule);
                self.source.push_str(", end))");
            }
            MatchValue::Context(Context::General) => self.source.push_str("latest"),
            MatchValue::Local => self.source.push_str("saved"),
        }
    }

    fn position(&mut self, value: Position) {
        self.source.push_str(match value {
            Position::Zero => "0",
            Position::Current => "index",
            Position::Next => "index + 1",
        });
    }

    fn rule_tag(&mut self, rule: usize) {
        let tag = rule
            .checked_add(1)
            .expect("a generated rule index fits its tag");
        write!(
            self.source,
            "::core::num::NonZeroUsize::new({tag})\
             .expect(\"a generated rule tag is nonzero\")"
        )
        .expect("writing to a string succeeds");
    }
}

fn body_scans(body: &FunctionBody) -> bool {
    match body {
        FunctionBody::Linear(block) | FunctionBody::Loop(block) => {
            matches!(block, Block::Scan { .. })
        }
        FunctionBody::Dispatch { arms, .. } => {
            arms.iter().any(|block| matches!(block, Block::Scan { .. }))
        }
    }
}

fn body_updates_context(body: &FunctionBody) -> bool {
    match body {
        FunctionBody::Linear(block) | FunctionBody::Loop(block) => block_updates_context(block),
        FunctionBody::Dispatch { arms, .. } => arms.iter().any(block_updates_context),
    }
}

fn block_updates_context(block: &Block) -> bool {
    let Block::Scan { dispatch, .. } = block else {
        return false;
    };
    match dispatch {
        Dispatch::Fail => false,
        Dispatch::Ranges(branches) => branches
            .iter()
            .any(|(_, transfer)| transfer_updates_context(transfer)),
        Dispatch::Table { branches, .. } => branches.iter().any(transfer_updates_context),
    }
}

fn transfer_updates_context(transfer: &Transfer) -> bool {
    match transfer {
        Transfer::Continue { update, .. } => update.is_some(),
        Transfer::Inline { body, .. } => block_updates_context(body),
        Transfer::Return(_) | Transfer::Call(_) => false,
    }
}

fn table_width(dead: usize) -> Option<&'static str> {
    if u8::try_from(dead).is_ok() {
        None
    } else if u16::try_from(dead).is_ok() {
        Some("u16")
    } else if u32::try_from(dead).is_ok() {
        Some("u32")
    } else {
        Some("usize")
    }
}

#[cfg(test)]
mod tests {
    use super::table_width;

    #[test]
    fn a_table_uses_the_smallest_width_that_includes_its_dead_branch() {
        for (dead, expected) in [
            (0, None),
            (255, None),
            (256, Some("u16")),
            (65535, Some("u16")),
            (65536, Some("u32")),
            (u32::MAX as usize, Some("u32")),
        ] {
            assert_eq!(table_width(dead), expected);
        }
        #[cfg(target_pointer_width = "64")]
        assert_eq!(table_width(u32::MAX as usize + 1), Some("usize"));
    }
}
