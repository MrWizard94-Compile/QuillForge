//! Primary user workflow: input events drive commands through DocumentSession.
//!
//! Layout is always derived after successful commits (never authoritative).

use qf_core::{BlockId, Command, CommandError, OpSeq, ParentRef, TextOffset};
use qf_input::{InputState, KeyEvent, PointerEvent, Selection};
use qf_layout::{LayoutConfig, LayoutEngine, LayoutTree};
use qf_storage::{CommitOutcome, DocumentSession};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EditorError {
    #[error("session: {0}")]
    Session(#[from] CommandError),
    #[error("no focused block")]
    NoFocus,
    #[error("block not found")]
    MissingBlock,
    #[error("{0}")]
    Other(String),
}

#[derive(Debug)]
pub struct EditResult {
    pub committed: bool,
    pub outcome: Option<CommitOutcome>,
    pub layout_changed: bool,
}

/// Wires input → session commit → layout reflow.
pub struct EditorController {
    pub session: DocumentSession,
    pub input: InputState,
    pub layout_engine: LayoutEngine,
    pub layout_config: LayoutConfig,
    pub layout_tree: LayoutTree,
}

impl EditorController {
    pub fn new(session: DocumentSession) -> Self {
        let layout_engine = LayoutEngine::new();
        let layout_config = LayoutConfig::default();
        let layout_tree = layout_engine.full_layout(session.state(), &layout_config);
        Self {
            session,
            input: InputState::default(),
            layout_engine,
            layout_config,
            layout_tree,
        }
    }

    pub fn create_default(journal_path: &Path) -> Result<Self, EditorError> {
        let session = DocumentSession::create_default(journal_path)?;
        Ok(Self::new(session))
    }

    pub fn op_seq(&self) -> OpSeq {
        self.session.state().op_seq
    }

    pub fn selection(&self) -> &Selection {
        &self.input.selection
    }

    pub fn focus(&mut self, block_id: BlockId, offset: TextOffset) -> Result<(), EditorError> {
        if !self.session.state().document.contains_block(block_id) {
            return Err(EditorError::MissingBlock);
        }
        let max = self
            .session
            .state()
            .document
            .project_text(block_id)
            .map(|t| t.chars().count() as u32)
            .unwrap_or(0);
        let offset = TextOffset(offset.0.min(max));
        self.input.focus(block_id, offset);
        Ok(())
    }

    /// Ensure a paragraph exists and is focused (empty-doc bootstrap).
    pub fn ensure_editable_paragraph(&mut self) -> Result<BlockId, EditorError> {
        if let Some(id) = self.input.selection.block_id {
            if self.session.state().document.contains_block(id) {
                return Ok(id);
            }
        }
        let outcome = self.session.commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: vec![],
        })?;
        let _ = outcome;
        let id = *self
            .session
            .state()
            .document
            .blocks
            .keys()
            .next()
            .ok_or_else(|| EditorError::Other("insert produced no block".into()))?;
        self.input.focus(id, TextOffset(0));
        self.reflow();
        Ok(id)
    }

    pub fn handle_pointer(&mut self, ev: PointerEvent) -> Result<EditResult, EditorError> {
        let _ = self.input.on_pointer(ev);
        Ok(EditResult {
            committed: false,
            outcome: None,
            layout_changed: false,
        })
    }

    pub fn handle_key(&mut self, ev: KeyEvent) -> Result<EditResult, EditorError> {
        if matches!(ev, KeyEvent::Enter) {
            return self.handle_enter();
        }
        let cmds = self.input.on_key(ev);
        self.dispatch_commands(cmds)
    }

    pub fn handle_text(&mut self, s: &str) -> Result<EditResult, EditorError> {
        let cmds = self.input.on_text(s);
        self.dispatch_commands(cmds)
    }

    fn handle_enter(&mut self) -> Result<EditResult, EditorError> {
        let block_id = self.input.selection.block_id.ok_or(EditorError::NoFocus)?;
        let (parent, index) = self.block_parent_index(block_id)?;
        let insert_at = index.saturating_add(1);
        let outcome = self.session.commit_command(Command::InsertBlock {
            parent: parent.clone(),
            index: insert_at,
            body_type: "paragraph".into(),
            body_payload: vec![],
        })?;
        let new_id = self
            .block_at_parent_index(&parent, insert_at as usize)
            .ok_or_else(|| EditorError::Other("enter insert missing block".into()))?;
        self.input.focus(new_id, TextOffset(0));
        self.reflow();
        Ok(EditResult {
            committed: true,
            outcome: Some(outcome),
            layout_changed: true,
        })
    }

    fn dispatch_commands(&mut self, cmds: Vec<Command>) -> Result<EditResult, EditorError> {
        if cmds.is_empty() {
            return Ok(EditResult {
                committed: false,
                outcome: None,
                layout_changed: false,
            });
        }
        let mut last = None;
        for cmd in cmds {
            last = Some(self.session.commit_command(cmd)?);
        }
        self.resync_selection_from_canonical();
        self.reflow();
        Ok(EditResult {
            committed: true,
            outcome: last,
            layout_changed: true,
        })
    }

    /// After commit, clamp caret to valid offset in focused block.
    fn resync_selection_from_canonical(&mut self) {
        let Some(block_id) = self.input.selection.block_id else {
            return;
        };
        if !self.session.state().document.contains_block(block_id) {
            self.input.selection.clear();
            return;
        }
        let max = self
            .session
            .state()
            .document
            .project_text(block_id)
            .map(|t| t.chars().count() as u32)
            .unwrap_or(0);
        if self.input.selection.offset.0 > max {
            self.input.selection.offset = TextOffset(max);
        }
    }

    fn reflow(&mut self) {
        self.layout_tree = self
            .layout_engine
            .full_layout(self.session.state(), &self.layout_config);
    }

    fn block_parent_index(&self, block_id: BlockId) -> Result<(ParentRef, u32), EditorError> {
        for (sid, section) in &self.session.state().document.sections {
            if let Some(idx) = section.blocks.iter().position(|b| *b == block_id) {
                return Ok((ParentRef::Section(*sid), idx as u32));
            }
        }
        Err(EditorError::MissingBlock)
    }

    fn block_at_parent_index(&self, parent: &ParentRef, index: usize) -> Option<BlockId> {
        match parent {
            ParentRef::Section(sid) => self
                .session
                .state()
                .document
                .sections
                .get(sid)
                .and_then(|s| s.blocks.get(index).copied()),
            ParentRef::DocumentRoot => {
                let sid = self.session.state().document.root_section_id;
                self.session
                    .state()
                    .document
                    .sections
                    .get(&sid)
                    .and_then(|s| s.blocks.get(index).copied())
            }
            ParentRef::Block(_) => None,
        }
    }

    pub fn focused_text(&self) -> Option<String> {
        let id = self.input.selection.block_id?;
        self.session.state().document.project_text(id)
    }

    pub fn save(&mut self, path: &Path) -> Result<(), EditorError> {
        self.session.save(path)?;
        Ok(())
    }
}
