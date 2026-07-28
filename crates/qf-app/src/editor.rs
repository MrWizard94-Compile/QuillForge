//! Primary user workflow: input events drive commands through DocumentSession.
//!
//! Layout is always derived after successful commits (never authoritative).

use qf_core::{
    ActorId, BlockId, Command, CommandError, OpSeq, ParentRef, SeqIdGen, SessionId, TextOffset,
};
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

    /// Open a `.qfdoc` package into a working journal path.
    pub fn open_package(package_path: &Path, journal_path: &Path) -> Result<Self, EditorError> {
        let mut gen = SeqIdGen::default();
        let actor = ActorId::new(&mut gen);
        let sess = SessionId::new(&mut gen);
        let (session, _notes) = DocumentSession::open_package(
            package_path,
            journal_path,
            actor,
            sess,
            Box::new(gen),
        )
        .map_err(|e| EditorError::Other(e.to_string()))?;
        let mut ed = Self::new(session);
        // Focus first text-bearing block when present.
        let first = {
            let doc = &ed.session.state().document;
            let root = doc.root_section_id;
            doc.sections
                .get(&root)
                .and_then(|s| s.blocks.first().copied())
        };
        if let Some(id) = first {
            let _ = ed.focus(id, TextOffset(0));
        } else {
            let _ = ed.ensure_editable_paragraph();
        }
        Ok(ed)
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

    /// Export PDF bytes using layout-aligned pagination (+ package images as XObjects).
    ///
    /// Embeds the local-first pin font [`qf_export::FontPack::pin_default`] so
    /// PDF text does not depend on system fonts.
    pub fn export_pdf_bytes(&self) -> Result<Vec<u8>, EditorError> {
        use qf_export::{export_pdf_with_assets_and_font, FontPack, ExportConfig};
        export_pdf_with_assets_and_font(
            self.session.state(),
            &qf_export::sources_from_state(self.session.state()),
            &ExportConfig {
                include_bibliography: false,
                include_timestamps: false,
            },
            Some(&self.session.assets),
            Some(&FontPack::pin_default()),
        )
        .map_err(EditorError::Other)
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

    /// Apply import intermediate blocks via journaled InsertBlock (shared by all importers).
    pub fn apply_import_blocks(
        &mut self,
        blocks: &[qf_export::DocxImportBlock],
    ) -> Result<usize, EditorError> {
        use qf_blocks::{BlockBody, CodeBlock, ImageBlock};
        use qf_core::AssetId;
        use qf_export::DocxImportBlock;
        use qf_kernel::encode_body;

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
        for block in blocks {
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
                DocxImportBlock::Image {
                    alt,
                    bytes: img_bytes,
                    width_px,
                    height_px,
                } => {
                    let asset_id = AssetId::new(self.session.ids.as_mut());
                    self.session
                        .assets
                        .insert(asset_id.as_uuid().to_string(), img_bytes.clone());
                    let bb = encode_body(&BlockBody::Image(ImageBlock {
                        asset_id,
                        alt: alt.clone(),
                        caption: vec![],
                        width_px: *width_px,
                        height_px: *height_px,
                    }))
                    .map_err(|e| EditorError::Other(e.to_string()))?;
                    (bb.type_name, bb.payload)
                }
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

    /// Import DOCX blocks via journaled InsertBlock commands.
    pub fn import_docx_bytes(&mut self, bytes: &[u8]) -> Result<usize, EditorError> {
        use qf_export::import_docx_blocks;
        let blocks = import_docx_blocks(bytes).map_err(EditorError::Other)?;
        self.apply_import_blocks(&blocks)
    }

    pub fn import_docx_path(&mut self, path: &Path) -> Result<usize, EditorError> {
        let bytes = std::fs::read(path).map_err(|e| EditorError::Other(e.to_string()))?;
        self.import_docx_bytes(&bytes)
    }

    /// Import Markdown / HTML / plain text by path extension.
    pub fn import_text_path(&mut self, path: &Path) -> Result<usize, EditorError> {
        use qf_export::import_text_path;
        let blocks = import_text_path(path).map_err(EditorError::Other)?;
        self.apply_import_blocks(&blocks)
    }

    /// Import raw text with an explicit format: `md` | `html` | `txt`.
    pub fn import_text_format(&mut self, text: &str, format: &str) -> Result<usize, EditorError> {
        use qf_export::{import_html, import_markdown, import_plain};
        let blocks = match format {
            "md" | "markdown" => import_markdown(text),
            "html" | "htm" => import_html(text),
            "txt" | "text" | "plain" => import_plain(text),
            other => {
                return Err(EditorError::Other(format!(
                    "unknown text format {other}"
                )));
            }
        };
        self.apply_import_blocks(&blocks)
    }

    /// Open any supported document path into this editor (replaces body; does not change package path).
    pub fn import_any_path(&mut self, path: &Path) -> Result<usize, EditorError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "docx" => self.import_docx_path(path),
            "md" | "markdown" | "html" | "htm" | "txt" | "text" => self.import_text_path(path),
            "qfdoc" => Err(EditorError::Other(
                "open .qfdoc via package open, not import_any_path".into(),
            )),
            "pdf" => Err(EditorError::Other(
                "PDF is export-only in foundation (no PDF import)".into(),
            )),
            other => Err(EditorError::Other(format!(
                "unsupported open format .{other}; use qfdoc|docx|md|html|txt"
            ))),
        }
    }

    /// Export current document as DOCX bytes (DrawingML images + pin TrueType font).
    pub fn export_docx_bytes(&self) -> Result<Vec<u8>, EditorError> {
        use qf_export::{export_docx_with_assets_and_font, FontPack, ExportConfig};
        export_docx_with_assets_and_font(
            self.session.state(),
            &qf_export::sources_from_state(self.session.state()),
            &ExportConfig {
                include_bibliography: false,
                include_timestamps: false,
            },
            Some(&self.session.assets),
            Some(&FontPack::pin_default()),
        )
        .map_err(EditorError::Other)
    }

    fn export_core(&self, format: qf_export::ExportFormat) -> Result<Vec<u8>, EditorError> {
        use qf_export::{CoreExporter, ExportConfig, ExportRequest, Exporter};
        // Prefer asset-aware paths for PDF/DOCX.
        match format {
            qf_export::ExportFormat::Pdf => self.export_pdf_bytes(),
            qf_export::ExportFormat::Docx => self.export_docx_bytes(),
            other => CoreExporter
                .export(
                    self.session.state(),
                    &ExportRequest {
                        format: other,
                        config: ExportConfig {
                            include_bibliography: false,
                            include_timestamps: false,
                        },
                    },
                )
                .map(|o| o.bytes)
                .map_err(EditorError::Other),
        }
    }

    pub fn export_markdown_bytes(&self) -> Result<Vec<u8>, EditorError> {
        self.export_core(qf_export::ExportFormat::Markdown)
    }

    pub fn export_html_bytes(&self) -> Result<Vec<u8>, EditorError> {
        self.export_core(qf_export::ExportFormat::Html)
    }

    pub fn export_plain_bytes(&self) -> Result<Vec<u8>, EditorError> {
        self.export_core(qf_export::ExportFormat::PlainText)
    }

    /// Export by extension: pdf|docx|md|html|txt (and aliases).
    pub fn export_by_extension(&self, ext: &str) -> Result<Vec<u8>, EditorError> {
        match ext.to_ascii_lowercase().as_str() {
            "pdf" => self.export_pdf_bytes(),
            "docx" => self.export_docx_bytes(),
            "md" | "markdown" => self.export_markdown_bytes(),
            "html" | "htm" => self.export_html_bytes(),
            "txt" | "text" | "plain" => self.export_plain_bytes(),
            other => Err(EditorError::Other(format!(
                "unsupported export extension .{other}"
            ))),
        }
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

    fn focused_range(&self) -> Result<qf_core::DocRange, EditorError> {
        use qf_core::{DocRange, TextTarget};
        let block_id = self.input.selection.block_id.ok_or(EditorError::NoFocus)?;
        let len = self
            .session
            .state()
            .document
            .project_text(block_id)
            .map(|t| t.chars().count() as u32)
            .unwrap_or(0);
        let target = |off: u32| TextTarget {
            block_id,
            span_id: None,
            offset: TextOffset(off),
        };
        Ok(DocRange {
            start: target(0),
            end: target(len),
        })
    }

    /// Toggle a character mark on the focused block (whole-block foundation formatting).
    pub fn toggle_mark(&mut self, mark: qf_blocks::Mark) -> Result<(), EditorError> {
        let range = self.focused_range()?;
        let kind = match &mark {
            qf_blocks::Mark::Bold => "bold",
            qf_blocks::Mark::Italic => "italic",
            qf_blocks::Mark::Underline => "underline",
            qf_blocks::Mark::Strike => "strike",
            qf_blocks::Mark::Code => "code",
            _ => "unknown",
        };
        let block_id = range.start.block_id;
        let has = self
            .session
            .state()
            .document
            .blocks
            .get(&block_id)
            .map(|b| match &b.body {
                qf_blocks::BlockBody::Paragraph(p) => p
                    .spans
                    .first()
                    .map(|s| s.marks.iter().any(|m| mark_kind(m) == kind))
                    .unwrap_or(false),
                qf_blocks::BlockBody::Heading(h) => h
                    .spans
                    .first()
                    .map(|s| s.marks.iter().any(|m| mark_kind(m) == kind))
                    .unwrap_or(false),
                _ => false,
            })
            .unwrap_or(false);
        if has {
            self.session.commit_command(Command::RemoveMark {
                range,
                mark_kind: kind.into(),
            })?;
        } else {
            let mut buf = Vec::new();
            ciborium::into_writer(&mark, &mut buf)
                .map_err(|e| EditorError::Other(e.to_string()))?;
            self.session.commit_command(Command::ApplyMark {
                range,
                mark_payload: buf,
            })?;
        }
        self.reflow();
        Ok(())
    }

    pub fn insert_page_break(&mut self) -> Result<(), EditorError> {
        let index = self.root_block_count();
        self.session.commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index,
            body_type: "page_break".into(),
            body_payload: Vec::new(),
        })?;
        self.reflow();
        Ok(())
    }

    pub fn insert_heading(&mut self, level: u8, text: &str) -> Result<(), EditorError> {
        let mut payload = vec![level.clamp(1, 6)];
        payload.extend_from_slice(text.as_bytes());
        let index = self.root_block_count();
        self.session.commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index,
            body_type: "heading".into(),
            body_payload: payload,
        })?;
        self.reflow();
        Ok(())
    }

    pub fn insert_bullet_paragraph(&mut self, text: &str) -> Result<(), EditorError> {
        let body = format!("• {text}");
        let index = self.root_block_count();
        self.session.commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index,
            body_type: "paragraph".into(),
            body_payload: body.into_bytes(),
        })?;
        self.reflow();
        Ok(())
    }

    pub fn insert_simple_table(&mut self, rows: u32, cols: u32) -> Result<(), EditorError> {
        use qf_blocks::{BlockBody, TableBlock, TableCell, TableRow};
        use qf_kernel::encode_body;
        let rows = rows.clamp(1, 8);
        let cols = cols.clamp(1, 8);
        let mut table_rows = Vec::new();
        for _ in 0..rows {
            let mut cells = Vec::new();
            for _ in 0..cols {
                cells.push(TableCell {
                    id: BlockId::new(self.session.ids.as_mut()),
                    colspan: 1,
                    rowspan: 1,
                    blocks: vec![],
                });
            }
            table_rows.push(TableRow { cells });
        }
        let body = BlockBody::Table(TableBlock {
            rows: table_rows,
            column_count: cols,
        });
        let bb = encode_body(&body).map_err(|e| EditorError::Other(e.to_string()))?;
        let index = self.root_block_count();
        self.session.commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index,
            body_type: bb.type_name,
            body_payload: bb.payload,
        })?;
        self.reflow();
        Ok(())
    }

    pub fn insert_citation_paragraph(&mut self, locator: &str) -> Result<(), EditorError> {
        let text = format!("[cite: {locator}]");
        let index = self.root_block_count();
        self.session.commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index,
            body_type: "paragraph".into(),
            body_payload: text.into_bytes(),
        })?;
        self.reflow();
        Ok(())
    }

    fn root_block_count(&self) -> u32 {
        let doc = &self.session.state().document;
        doc.sections
            .get(&doc.root_section_id)
            .map(|s| s.blocks.len() as u32)
            .unwrap_or(0)
    }

    /// Cycle page margin: 0.5", 1", 1.5" (layout config, derived reflow).
    pub fn cycle_margins(&mut self) -> Result<String, EditorError> {
        let cur = fixed_to_inch(self.layout_config.margin);
        let next = if (cur - 0.5).abs() < 0.01 {
            1.0
        } else if (cur - 1.0).abs() < 0.01 {
            1.5
        } else {
            0.5
        };
        self.layout_config.margin = qf_layout::Fixed32::from_pt((next * 72.0) as i32);
        self.reflow();
        Ok(format!("{next}\" margins"))
    }

    /// Toggle US Letter portrait (612×792) vs landscape (792×612).
    pub fn toggle_orientation(&mut self) -> Result<String, EditorError> {
        let w = self.layout_config.page_width;
        let h = self.layout_config.page_height;
        self.layout_config.page_width = h;
        self.layout_config.page_height = w;
        self.reflow();
        let label = if self.layout_config.page_width.0 > self.layout_config.page_height.0 {
            "Landscape"
        } else {
            "Portrait"
        };
        Ok(label.into())
    }

    /// Cycle page size: Letter → A4-ish → Letter.
    pub fn cycle_page_size(&mut self) -> Result<String, EditorError> {
        let w = self.layout_config.page_width.0 / 64;
        let (nw, nh, name) = if w <= 620 {
            (595, 842, "A4")
        } else {
            (612, 792, "US Letter")
        };
        self.layout_config.page_width = qf_layout::Fixed32::from_pt(nw);
        self.layout_config.page_height = qf_layout::Fixed32::from_pt(nh);
        self.reflow();
        Ok(name.into())
    }
}

fn mark_kind(m: &qf_blocks::Mark) -> &'static str {
    match m {
        qf_blocks::Mark::Bold => "bold",
        qf_blocks::Mark::Italic => "italic",
        qf_blocks::Mark::Underline => "underline",
        qf_blocks::Mark::Strike => "strike",
        qf_blocks::Mark::Code => "code",
        qf_blocks::Mark::Link { .. } => "link",
        qf_blocks::Mark::Highlight { .. } => "highlight",
        qf_blocks::Mark::CommentRef { .. } => "comment",
        qf_blocks::Mark::CitationRef { .. } => "citation",
        qf_blocks::Mark::Unknown { .. } => "unknown",
    }
}

fn fixed_to_inch(f: qf_layout::Fixed32) -> f32 {
    f.0 as f32 / 64.0 / 72.0
}
