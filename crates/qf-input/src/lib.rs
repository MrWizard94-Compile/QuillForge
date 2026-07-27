//! Translate OS/input events into Commands. Never mutates kernel or journal.
//!
//! Optimistic caret updates are local only; `EditorController` (app layer)
//! re-synchronizes selection from canonical state after each commit.

use qf_core::{BlockId, Command, DocRange, TextOffset, TextTarget};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub block_id: Option<BlockId>,
    pub offset: TextOffset,
}

impl Selection {
    pub fn at(block_id: BlockId, offset: TextOffset) -> Self {
        Self {
            block_id: Some(block_id),
            offset,
        }
    }

    pub fn clear(&mut self) {
        self.block_id = None;
        self.offset = TextOffset(0);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyEvent {
    Char(char),
    Backspace,
    Enter,
    Undo,
    Redo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Click {
        block_id: BlockId,
        offset: TextOffset,
    },
}

#[derive(Debug, Default)]
pub struct InputState {
    pub selection: Selection,
}

impl InputState {
    pub fn focus(&mut self, block_id: BlockId, offset: TextOffset) {
        self.selection = Selection::at(block_id, offset);
    }

    pub fn on_pointer(&mut self, ev: PointerEvent) -> Vec<Command> {
        match ev {
            PointerEvent::Click { block_id, offset } => {
                self.focus(block_id, offset);
                vec![]
            }
        }
    }

    /// Map a key event to zero or more commands. Does not commit.
    pub fn on_key(&mut self, ev: KeyEvent) -> Vec<Command> {
        match ev {
            KeyEvent::Char(c) => {
                if c.is_control() {
                    return vec![];
                }
                let Some(block_id) = self.selection.block_id else {
                    return vec![];
                };
                let cmd = Command::InsertText {
                    target: TextTarget {
                        block_id,
                        span_id: None,
                        offset: self.selection.offset,
                    },
                    text: c.to_string(),
                };
                self.selection.offset = TextOffset(self.selection.offset.0.saturating_add(1));
                vec![cmd]
            }
            KeyEvent::Backspace => {
                let Some(block_id) = self.selection.block_id else {
                    return vec![];
                };
                if self.selection.offset.0 == 0 {
                    return vec![];
                }
                let end = self.selection.offset;
                let start = TextOffset(end.0 - 1);
                self.selection.offset = start;
                vec![Command::DeleteRange {
                    range: DocRange {
                        start: TextTarget {
                            block_id,
                            span_id: None,
                            offset: start,
                        },
                        end: TextTarget {
                            block_id,
                            span_id: None,
                            offset: end,
                        },
                    },
                }]
            }
            // Enter is expanded by the editor layer (needs parent index).
            KeyEvent::Enter => vec![],
            KeyEvent::Undo => vec![Command::Undo { steps: 1 }],
            KeyEvent::Redo => vec![Command::Redo { steps: 1 }],
        }
    }

    pub fn on_text(&mut self, s: &str) -> Vec<Command> {
        if s.is_empty() {
            return vec![];
        }
        let Some(block_id) = self.selection.block_id else {
            return vec![];
        };
        let len = s.chars().count() as u32;
        let cmd = Command::InsertText {
            target: TextTarget {
                block_id,
                span_id: None,
                offset: self.selection.offset,
            },
            text: s.to_string(),
        };
        self.selection.offset = TextOffset(self.selection.offset.0.saturating_add(len));
        vec![cmd]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::SeqIdGen;

    #[test]
    fn typing_emits_insert_and_advances_caret() {
        let mut gen = SeqIdGen::default();
        let bid = BlockId::new(&mut gen);
        let mut input = InputState::default();
        input.focus(bid, TextOffset(0));
        let cmds = input.on_key(KeyEvent::Char('a'));
        assert_eq!(cmds.len(), 1);
        assert_eq!(input.selection.offset, TextOffset(1));
    }

    #[test]
    fn no_selection_yields_no_commands() {
        let mut input = InputState::default();
        assert!(input.on_key(KeyEvent::Char('x')).is_empty());
        assert!(input.on_text("hi").is_empty());
    }
}
