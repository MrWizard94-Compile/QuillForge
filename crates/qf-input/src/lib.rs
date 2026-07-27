//! Translate OS/input events into Commands. Never mutates kernel.

use qf_core::{BlockId, Command, TextOffset, TextTarget};

#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub block_id: Option<BlockId>,
    pub offset: TextOffset,
}

#[derive(Clone, Debug)]
pub enum KeyEvent {
    Char(char),
    Backspace,
    Enter,
    Undo,
    Redo,
}

#[derive(Default)]
pub struct InputState {
    pub selection: Selection,
}

impl InputState {
    pub fn on_key(&mut self, ev: KeyEvent) -> Vec<Command> {
        match ev {
            KeyEvent::Char(c) => {
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
                // Optimistic local caret advance (authoritative after commit).
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
                    range: qf_core::DocRange {
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
            KeyEvent::Enter => vec![], // split handled by higher layer
            KeyEvent::Undo => vec![Command::Undo { steps: 1 }],
            KeyEvent::Redo => vec![Command::Redo { steps: 1 }],
        }
    }

    pub fn on_text(&mut self, s: &str) -> Vec<Command> {
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
