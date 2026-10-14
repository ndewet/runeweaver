//! Stores lexical modes independently of scanner input.

use crate::lexer::ModeAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LexicalState {
    mode: ModeFrame,
    parents: Vec<ModeFrame>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ModeFrame {
    pub id: usize,
    pub opened_at: usize,
}

impl Default for LexicalState {
    fn default() -> Self {
        Self {
            mode: ModeFrame {
                id: 0,
                opened_at: 0,
            },
            parents: Vec::new(),
        }
    }
}

impl LexicalState {
    pub(crate) fn mode(&self) -> usize {
        self.mode.id
    }

    pub(crate) fn unclosed(&self) -> Option<ModeFrame> {
        (!self.parents.is_empty()).then(|| *self.parents.get(1).unwrap_or(&self.mode))
    }

    pub(crate) fn positions(&self) -> impl Iterator<Item = usize> + '_ {
        std::iter::once(self.mode.opened_at).chain(self.parents.iter().map(|frame| frame.opened_at))
    }

    pub(crate) fn same_context(&self, other: &Self) -> bool {
        self.mode.id == other.mode.id
            && self.parents.len() == other.parents.len()
            && self
                .parents
                .iter()
                .zip(&other.parents)
                .all(|(a, b)| a.id == b.id)
    }

    pub(crate) fn apply(&mut self, action: ModeAction, start: usize) {
        match action {
            ModeAction::Stay => {}
            ModeAction::Begin(id) => {
                self.mode = ModeFrame {
                    id,
                    opened_at: start,
                }
            }
            ModeAction::Push(id) => {
                self.parents.push(self.mode);
                self.mode = ModeFrame {
                    id,
                    opened_at: start,
                };
            }
            ModeAction::Pop => {
                if let Some(parent) = self.parents.pop() {
                    self.mode = parent;
                }
            }
        }
    }
}
