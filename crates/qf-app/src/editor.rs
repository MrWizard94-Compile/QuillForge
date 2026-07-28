//! Primary user workflow: input events drive commands through DocumentSession.
//!
//! Layout is always derived after successful commits (never authoritative).

use qf_core::{BlockId, Command, CommandError, OpSeq, ParentRef, TextOffset};
use qf_input::{InputState, KeyEvent, PointerEvent, Selection};
use qf_layout::{LayoutConfig, LayoutEngine, LayoutTree};
use qf_search::{SearchEngine, SearchHit, SearchIngestor, SearchMode, SearchQuery};
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

/// Wires input → session commit → layout reflow (+ derived search index).
pub struct EditorController {
    pub session: DocumentSession,
    pub input: InputState,
    pub layout_engine: LayoutEngine,
    pub layout_config: LayoutConfig,
    pub layout_tree: LayoutTree,
    pub search: SearchEngine,
}

impl EditorController {
    pub fn new(session: DocumentSession) -> Self {
        let layout_engine = LayoutEngine::new();
        let layout_config = LayoutConfig::default();
        let layout_tree = layout_engine.full_layout(session.state(), &layout_config);
        let mut search = SearchEngine::new();
        search.rebuild(session.state());
        Self {
            session,
            input: InputState::default(),
            layout_engine,
            layout_config,
            layout_tree,
            search,
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
    pub(crate) fn resync_selection_from_canonical(&mut self) {
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

    /// Rebuild layout + search from current canonical state (derived only).
    pub fn reflow(&mut self) {
        self.layout_tree = self
            .layout_engine
            .full_layout(self.session.state(), &self.layout_config);
        // Search index is derived — always rebuildable from canonical state.
        self.search.rebuild(self.session.state());
    }

    /// Hybrid search (token ∪ phrase ∪ TF–IDF semantic). Derived index only.
    pub fn search_text(&mut self, query: &str) -> Vec<SearchHit> {
        if self.search.state_gen != self.session.state().op_seq {
            self.search.rebuild(self.session.state());
        }
        self.search.query(&SearchQuery::hybrid(query))
    }

    pub fn search_with_mode(&mut self, query: &str, mode: SearchMode) -> Vec<SearchHit> {
        if self.search.state_gen != self.session.state().op_seq {
            self.search.rebuild(self.session.state());
        }
        self.search.query(&SearchQuery {
            text: query.into(),
            mode,
            limit: 50,
        })
    }

    /// Focus a block from a search hit (caret at start).
    pub fn focus_search_hit(&mut self, hit: &SearchHit) -> Result<(), EditorError> {
        self.focus(hit.block_id, TextOffset(0))
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

    /// Plain text of the document (paragraphs/headings joined by newlines).
    pub fn document_plain(&self) -> String {
        let mut lines = Vec::new();
        let doc = &self.session.state().document;
        for sid in &doc.section_order {
            let Some(sec) = doc.sections.get(sid) else {
                continue;
            };
            for bid in &sec.blocks {
                if let Some(t) = doc.project_text(*bid) {
                    lines.push(t);
                }
            }
        }
        lines.join("\n")
    }

    /// Word count for status bar (Unicode whitespace split).
    pub fn word_count(&self) -> u32 {
        self.document_plain()
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .count() as u32
    }

    pub fn page_count(&self) -> u32 {
        self.layout_tree.page_count()
    }

    /// Plain lines for each print page (shared with PDF export).
    pub fn page_plain_lines(&self) -> Vec<Vec<String>> {
        self.layout_tree.page_plain_lines(self.session.state())
    }

    /// Lines for a 1-based page index.
    pub fn page_lines(&self, page_1based: u32) -> Vec<String> {
        let pages = self.page_plain_lines();
        let idx = page_1based.saturating_sub(1) as usize;
        pages.get(idx).cloned().unwrap_or_default()
    }

    /// Export PDF bytes using layout-aligned pagination.
    pub fn export_pdf_bytes(&self) -> Result<Vec<u8>, EditorError> {
        use qf_export::{CoreExporter, ExportFormat, ExportRequest, Exporter};
        let out = CoreExporter
            .export(
                self.session.state(),
                &ExportRequest {
                    format: ExportFormat::Pdf,
                    config: qf_export::ExportConfig {
                        include_bibliography: false,
                        include_timestamps: false,
                    },
                },
            )
            .map_err(EditorError::Other)?;
        Ok(out.bytes)
    }

    /// Replace focused block text via journaled ReplaceRange (Word-class typing path).
    pub fn set_focused_plain(&mut self, text: &str) -> Result<EditResult, EditorError> {
        let block_id = self.input.selection.block_id.ok_or(EditorError::NoFocus)?;
        let current = self
            .session
            .state()
            .document
            .project_text(block_id)
            .unwrap_or_default();
        if current == text {
            return Ok(EditResult {
                committed: false,
                outcome: None,
                layout_changed: false,
            });
        }
        let end_off = TextOffset(current.chars().count() as u32);
        let target = |offset: TextOffset| qf_core::TextTarget {
            block_id,
            span_id: None,
            offset,
        };
        let outcome = self.session.commit_command(Command::ReplaceRange {
            range: qf_core::DocRange {
                start: target(TextOffset(0)),
                end: target(end_off),
            },
            text: text.to_string(),
        })?;
        self.input.selection.offset = TextOffset(text.chars().count() as u32);
        self.reflow();
        Ok(EditResult {
            committed: true,
            outcome: Some(outcome),
            layout_changed: true,
        })
    }

    /// Import DOCX blocks via journaled InsertBlock commands (P4 round-trip path).
    ///
    /// Replaces current document body: existing blocks are removed, then import
    /// blocks are inserted in order. Layout reflows after commit.
    pub fn import_docx_bytes(&mut self, bytes: &[u8]) -> Result<usize, EditorError> {
        use qf_blocks::{BlockBody, CodeBlock};
        use qf_export::{import_docx_blocks, DocxImportBlock};
        use qf_kernel::encode_body;

        let blocks = import_docx_blocks(bytes).map_err(EditorError::Other)?;
        // Remove existing root-section blocks (newest first so indices stay valid).
        let existing: Vec<BlockId> = {
            let doc = &self.session.state().document;
            let root = doc.root_section_id;
            doc.sections
                .get(&root)
                .map(|s| s.blocks.clone())
                .unwrap_or_default()
        };
        for bid in existing.into_iter().rev() {
            let _ = self.session.commit_command(Command::DeleteBlock { block_id: bid });
        }

        let mut count = 0u32;
        for block in &blocks {
            let (body_type, body_payload) = match block {
                DocxImportBlock::Paragraph { text } => ("paragraph".into(), text.as_bytes().to_vec()),
                DocxImportBlock::Heading { level, text } => {
                    let mut payload = vec![*level];
                    payload.extend_from_slice(text.as_bytes());
                    ("heading".into(), payload)
                }
                DocxImportBlock::Quote { text } => ("quote".into(), text.as_bytes().to_vec()),
                DocxImportBlock::Code { text } => {
                    let bb = encode_body(&BlockBody::Code(CodeBlock {
                        language: None,
                        text: text.clone(),
                        show_line_numbers: false,
                    }))
                    .map_err(|e| EditorError::Other(e.to_string()))?;
                    (bb.type_name, bb.payload)
                }
                DocxImportBlock::PageBreak => ("page_break".into(), Vec::new()),
            };
            self.session.commit_command(Command::InsertBlock {
                parent: ParentRef::DocumentRoot,
                index: count,
                body_type,
                body_payload,
            })?;
            count = count.saturating_add(1);
        }

        if count == 0 {
            self.ensure_editable_paragraph()?;
        } else {
            // Focus first text-bearing block when possible.
            let first = {
                let doc = &self.session.state().document;
                let root = doc.root_section_id;
                doc.sections
                    .get(&root)
                    .and_then(|s| s.blocks.first().copied())
            };
            if let Some(id) = first {
                let _ = self.focus(id, TextOffset(0));
            }
        }
        self.reflow();
        Ok(count as usize)
    }

    pub fn import_docx_path(&mut self, path: &Path) -> Result<usize, EditorError> {
        let bytes = std::fs::read(path).map_err(|e| EditorError::Other(e.to_string()))?;
        self.import_docx_bytes(&bytes)
    }

    /// Export current document as DOCX bytes (round-trip partner for import).
    pub fn export_docx_bytes(&self) -> Result<Vec<u8>, EditorError> {
        use qf_export::{CoreExporter, ExportFormat, ExportRequest, Exporter};
        let out = CoreExporter
            .export(
                self.session.state(),
                &ExportRequest {
                    format: ExportFormat::Docx,
                    config: qf_export::ExportConfig {
                        include_bibliography: false,
                        include_timestamps: false,
                    },
                },
            )
            .map_err(EditorError::Other)?;
        Ok(out.bytes)
    }

    pub fn save(&mut self, path: &Path) -> Result<(), EditorError> {
        self.session.save(path)?;
        Ok(())
    }

    pub fn save_encrypted(
        &mut self,
        path: &Path,
        passphrase: &str,
    ) -> Result<(), EditorError> {
        self.session.save_encrypted(path, passphrase)?;
        Ok(())
    }
}
