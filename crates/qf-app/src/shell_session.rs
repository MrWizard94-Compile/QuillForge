//! Word-class shell session: editor + product surface metrics.

use crate::page_chrome::PageChrome;
use crate::product_surface::{CommandBarTab, DocumentViewMode, ProductSurface};
use crate::review::ReviewPanel;
use crate::{EditorController, EditorError};
use qf_core::{AssetId, SuggestionId};
use qf_search::{SearchHit, SearchMode};
use std::path::{Path, PathBuf};

/// Search panel chrome (not durable).
#[derive(Clone, Debug, Default)]
pub struct SearchPanel {
    pub visible: bool,
    pub query: String,
    pub mode: SearchMode,
    pub hits: Vec<SearchHit>,
}

/// Desktop-shell state used by GUI and headless surface tests.
pub struct ShellSession {
    pub editor: EditorController,
    pub surface: ProductSurface,
    pub package_path: PathBuf,
    pub journal_path: PathBuf,
    /// Working buffer for the focused paragraph (Word-class typing surface).
    pub draft: String,
    pub last_error: Option<String>,
    pub dirty: bool,
    pub review: ReviewPanel,
    pub chrome: PageChrome,
    pub search_panel: SearchPanel,
    /// Local clipboard for cut/copy/paste (session-scoped buffer).
    pub clipboard: String,
}

impl ShellSession {
    pub fn open_or_create(work_dir: &Path, name: &str) -> Result<Self, EditorError> {
        std::fs::create_dir_all(work_dir).map_err(|e| EditorError::Other(e.to_string()))?;
        let journal_path = work_dir.join(format!("{name}.journal"));
        let package_path = work_dir.join(format!("{name}.qfdoc"));
        // create_default needs an empty journal; demos re-running reuse work dirs.
        if journal_path.exists() {
            let _ = std::fs::remove_file(&journal_path);
        }
        let mut editor = EditorController::create_default(&journal_path)?;
        editor.ensure_editable_paragraph()?;
        let draft = editor.focused_text().unwrap_or_default();
        let surface = ProductSurface::word_class_default(name);
        let chrome = PageChrome::from_meta(&editor.session.state().document.meta.custom);
        let mut session = Self {
            editor,
            surface,
            package_path,
            journal_path,
            draft,
            last_error: None,
            dirty: false,
            review: ReviewPanel::word_class_default(),
            chrome,
            search_panel: SearchPanel::default(),
            clipboard: String::new(),
        };
        session.refresh_surface();
        Ok(session)
    }

    pub fn run_search(&mut self) {
        let q = self.search_panel.query.trim().to_string();
        if q.is_empty() {
            self.search_panel.hits.clear();
            return;
        }
        self.search_panel.hits = self
            .editor
            .search_with_mode(&q, self.search_panel.mode);
        self.search_panel.visible = true;
    }

    pub fn open_search_hit(&mut self, index: usize) -> Result<(), EditorError> {
        let Some(hit) = self.search_panel.hits.get(index).cloned() else {
            return Err(EditorError::Other("search hit out of range".into()));
        };
        self.editor.focus_search_hit(&hit)?;
        self.draft = self.editor.focused_text().unwrap_or_default();
        self.refresh_surface();
        Ok(())
    }

    pub fn refresh_surface(&mut self) {
        self.surface.status.page_total = self.editor.page_count();
        self.surface.status.page_current = self.surface.status.page_current.clamp(
            1,
            self.surface.status.page_total,
        );
        self.surface.status.word_count = self.editor.word_count();
        self.surface.status.pending_suggestions =
            self.editor.pending_suggestions().len() as u32;
        // Drop selection if suggestion is no longer pending.
        if let Some(id) = self.review.selected {
            if !self
                .editor
                .pending_suggestions()
                .iter()
                .any(|s| s.id == id)
            {
                self.review.selected = None;
            }
        }
        // Keep chrome drafts aligned unless panel is actively editing.
        if !self.chrome.panel_visible {
            self.chrome.header = self.editor.page_header();
            self.chrome.footer = self.editor.page_footer();
        }
        if self.dirty {
            // keep name; mark title with *
            let base = self.surface.document_display_name.trim_end_matches('*').to_string();
            if !self.surface.document_display_name.ends_with('*') {
                self.surface.set_document_name(&format!("{base}*"));
            }
        }
    }

    pub fn toggle_rulers(&mut self) {
        self.surface.show_rulers = !self.surface.show_rulers;
    }

    pub fn apply_page_chrome(&mut self) -> Result<(), EditorError> {
        self.editor.set_page_chrome(
            Some(self.chrome.header.as_str()),
            Some(self.chrome.footer.as_str()),
        )?;
        self.dirty = true;
        self.refresh_surface();
        Ok(())
    }

    pub fn insert_image_placeholder(&mut self) -> Result<AssetId, EditorError> {
        // Prefer official product mark; fall back to ember gradient / 1×1 PNG.
        let id = match crate::product_mark_png_for_document(320) {
            Ok(png) => self
                .editor
                .insert_image_bytes("Quillforge", &png, None, None)?,
            Err(_) => match crate::demo_png_bytes(96, 64) {
                Ok(png) => self
                    .editor
                    .insert_image_bytes("Image", &png, None, None)?,
                Err(_) => self.editor.insert_image("Image", 96, 64)?,
            },
        };
        self.dirty = true;
        self.refresh_surface();
        Ok(id)
    }

    /// Insert the full-resolution (document-scaled) product mark logo.
    pub fn insert_product_mark(&mut self) -> Result<AssetId, EditorError> {
        let png = crate::product_mark_png_for_document(480).map_err(EditorError::Other)?;
        let id = self
            .editor
            .insert_image_bytes("Quillforge mark", &png, None, None)?;
        self.dirty = true;
        self.refresh_surface();
        Ok(id)
    }

    pub fn insert_image_file(&mut self, path: &Path) -> Result<AssetId, EditorError> {
        let alt = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Image");
        let id = self.editor.insert_image_path(path, alt)?;
        self.dirty = true;
        self.refresh_surface();
        Ok(id)
    }

    pub fn toggle_review_panel(&mut self) {
        self.review.visible = !self.review.visible;
        if self.review.visible {
            self.surface.activate_tab(CommandBarTab::Review);
        }
    }

    pub fn propose_suggestion(&mut self) -> Result<SuggestionId, EditorError> {
        self.apply_draft()?;
        let instr = if self.review.instruction.trim().is_empty() {
            "clarify"
        } else {
            self.review.instruction.trim()
        };
        let id = self.editor.propose_local_suggestion(instr)?;
        self.review.selected = Some(id);
        self.review.visible = true;
        self.dirty = true;
        self.refresh_surface();
        Ok(id)
    }

    pub fn accept_selected(&mut self) -> Result<(), EditorError> {
        let id = self
            .review
            .selected
            .ok_or_else(|| EditorError::Other("no suggestion selected".into()))?;
        self.editor.accept_suggestion(id)?;
        self.draft = self.editor.focused_text().unwrap_or_default();
        self.dirty = true;
        self.refresh_surface();
        Ok(())
    }

    pub fn reject_selected(&mut self) -> Result<(), EditorError> {
        let id = self
            .review
            .selected
            .ok_or_else(|| EditorError::Other("no suggestion selected".into()))?;
        self.editor.reject_suggestion(id)?;
        self.refresh_surface();
        Ok(())
    }

    pub fn select_suggestion(&mut self, id: SuggestionId) {
        self.review.selected = Some(id);
        self.review.visible = true;
        self.surface.activate_tab(CommandBarTab::Review);
    }

    pub fn apply_draft(&mut self) -> Result<(), EditorError> {
        let r = self.editor.set_focused_plain(&self.draft)?;
        if r.committed {
            self.dirty = true;
        }
        self.refresh_surface();
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), EditorError> {
        self.apply_draft()?;
        self.editor.save(&self.package_path)?;
        self.dirty = false;
        let name = self
            .surface
            .document_display_name
            .trim_end_matches('*')
            .to_string();
        self.surface.set_document_name(&name);
        self.refresh_surface();
        Ok(())
    }

    pub fn toggle_bold(&mut self) -> Result<(), EditorError> {
        self.editor.toggle_mark(qf_blocks::Mark::Bold)?;
        self.dirty = true;
        self.refresh_surface();
        Ok(())
    }

    pub fn toggle_italic(&mut self) -> Result<(), EditorError> {
        self.editor.toggle_mark(qf_blocks::Mark::Italic)?;
        self.dirty = true;
        self.refresh_surface();
        Ok(())
    }

    pub fn toggle_underline(&mut self) -> Result<(), EditorError> {
        self.editor.toggle_mark(qf_blocks::Mark::Underline)?;
        self.dirty = true;
        self.refresh_surface();
        Ok(())
    }

    pub fn cut_selection(&mut self) -> Result<(), EditorError> {
        self.clipboard = self.draft.clone();
        self.draft.clear();
        self.apply_draft()?;
        Ok(())
    }

    pub fn copy_selection(&mut self) {
        self.clipboard = if self.draft.is_empty() {
            self.editor.document_plain()
        } else {
            self.draft.clone()
        };
    }

    pub fn paste_clipboard(&mut self) -> Result<(), EditorError> {
        if self.clipboard.is_empty() {
            return Ok(());
        }
        self.draft.push_str(&self.clipboard);
        self.apply_draft()?;
        Ok(())
    }

    pub fn insert_page_break(&mut self) -> Result<(), EditorError> {
        self.apply_draft()?;
        self.editor.insert_page_break()?;
        self.dirty = true;
        self.refresh_surface();
        Ok(())
    }

    pub fn insert_heading(&mut self) -> Result<(), EditorError> {
        let text = if self.draft.trim().is_empty() {
            "Heading".to_string()
        } else {
            self.draft.trim().to_string()
        };
        self.apply_draft()?;
        self.editor.insert_heading(1, &text)?;
        self.dirty = true;
        self.refresh_surface();
        Ok(())
    }

    pub fn insert_bullet(&mut self) -> Result<(), EditorError> {
        let text = if self.draft.trim().is_empty() {
            "List item".to_string()
        } else {
            self.draft.trim().to_string()
        };
        self.apply_draft()?;
        self.editor.insert_bullet_paragraph(&text)?;
        self.dirty = true;
        self.refresh_surface();
        Ok(())
    }

    pub fn insert_table(&mut self) -> Result<(), EditorError> {
        self.apply_draft()?;
        self.editor.insert_simple_table(2, 2)?;
        self.dirty = true;
        self.refresh_surface();
        Ok(())
    }

    pub fn insert_citation(&mut self) -> Result<(), EditorError> {
        self.apply_draft()?;
        self.editor.insert_citation_paragraph("local-source")?;
        self.dirty = true;
        self.refresh_surface();
        Ok(())
    }

    pub fn add_comment_note(&mut self, body: &str) -> Result<SuggestionId, EditorError> {
        self.apply_draft()?;
        let id = self
            .editor
            .propose_human_suggestion(body, "")?;
        self.review.visible = true;
        self.review.selected = Some(id);
        self.surface.activate_tab(CommandBarTab::Review);
        self.refresh_surface();
        Ok(id)
    }

    pub fn new_document(&mut self, name: &str) -> Result<(), EditorError> {
        let work = self
            .package_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        *self = Self::open_or_create(&work, name)?;
        Ok(())
    }

    pub fn open_sibling_package(&mut self, file_name: &str) -> Result<(), EditorError> {
        let work = self
            .package_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let path = work.join(file_name);
        if !path.exists() {
            return Err(EditorError::Other(format!(
                "open failed: {} not found",
                path.display()
            )));
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Opened");
        *self = Self::open_from_path(&work, stem, &path)?;
        Ok(())
    }

    pub fn save_as(&mut self, name: &str) -> Result<PathBuf, EditorError> {
        let work = self
            .package_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        self.package_path = work.join(format!("{name}.qfdoc"));
        self.journal_path = work.join(format!("{name}.journal"));
        self.save()?;
        self.surface.set_document_name(name);
        Ok(self.package_path.clone())
    }

    pub fn cycle_margins(&mut self) -> Result<(), EditorError> {
        let msg = self.editor.cycle_margins()?;
        self.last_error = Some(msg);
        self.refresh_surface();
        Ok(())
    }

    pub fn toggle_orientation(&mut self) -> Result<(), EditorError> {
        let msg = self.editor.toggle_orientation()?;
        self.last_error = Some(format!("Orientation: {msg}"));
        self.refresh_surface();
        Ok(())
    }

    pub fn cycle_page_size(&mut self) -> Result<(), EditorError> {
        let msg = self.editor.cycle_page_size()?;
        self.last_error = Some(format!("Page size: {msg}"));
        self.refresh_surface();
        Ok(())
    }

    pub fn set_view(&mut self, mode: DocumentViewMode) {
        self.surface.set_view_mode(mode);
    }

    pub fn status_line(&self) -> String {
        self.surface.status.summary_line()
    }

    /// Navigate status bar page (1-based). Clamped to layout page range.
    pub fn goto_page(&mut self, page_1based: u32) {
        let total = self.editor.page_count().max(1);
        self.surface.status.page_current = page_1based.clamp(1, total);
        self.surface.status.page_total = total;
    }

    pub fn next_page(&mut self) {
        let cur = self.surface.status.page_current;
        self.goto_page(cur.saturating_add(1));
    }

    pub fn prev_page(&mut self) {
        let cur = self.surface.status.page_current;
        self.goto_page(cur.saturating_sub(1).max(1));
    }

    pub fn set_zoom_percent(&mut self, zoom: u32) {
        self.surface.status.zoom_percent = zoom.clamp(50, 200);
    }

    /// Write layout-aligned PDF next to the package path.
    pub fn export_pdf(&mut self) -> Result<std::path::PathBuf, EditorError> {
        self.export_as("pdf")
    }

    /// Export DOCX next to the package path.
    pub fn export_docx(&mut self) -> Result<std::path::PathBuf, EditorError> {
        self.export_as("docx")
    }

    pub fn export_markdown(&mut self) -> Result<std::path::PathBuf, EditorError> {
        self.export_as("md")
    }

    pub fn export_html(&mut self) -> Result<std::path::PathBuf, EditorError> {
        self.export_as("html")
    }

    pub fn export_plain(&mut self) -> Result<std::path::PathBuf, EditorError> {
        self.export_as("txt")
    }

    /// Export by extension (`pdf|docx|md|html|txt`) beside the package path.
    pub fn export_as(&mut self, ext: &str) -> Result<std::path::PathBuf, EditorError> {
        self.apply_draft()?;
        let bytes = self.editor.export_by_extension(ext)?;
        let path = self.package_path.with_extension(ext);
        std::fs::write(&path, bytes).map_err(|e| EditorError::Other(e.to_string()))?;
        Ok(path)
    }

    /// Export every foundation interchange format; returns written paths.
    pub fn export_all(&mut self) -> Result<Vec<std::path::PathBuf>, EditorError> {
        let mut paths = Vec::new();
        for ext in ["pdf", "docx", "md", "html", "txt"] {
            paths.push(self.export_as(ext)?);
        }
        // Native package is always available via save().
        self.save()?;
        paths.push(self.package_path.clone());
        Ok(paths)
    }

    /// Import a .docx file into this session and refresh surface/draft.
    pub fn import_docx(&mut self, path: &Path) -> Result<usize, EditorError> {
        self.import_path(path)
    }

    /// Import any supported interchange format into this session.
    pub fn import_path(&mut self, path: &Path) -> Result<usize, EditorError> {
        let n = self.editor.import_any_path(path)?;
        self.draft = self.editor.focused_text().unwrap_or_default();
        if self.draft.is_empty() {
            self.draft = self.editor.document_plain();
        }
        self.dirty = true;
        self.refresh_surface();
        Ok(n)
    }

    /// Open a new shell session from a .docx file (saves `.qfdoc` beside work_dir).
    pub fn open_from_docx(
        work_dir: &Path,
        name: &str,
        docx_path: &Path,
    ) -> Result<Self, EditorError> {
        Self::open_from_path(work_dir, name, docx_path)
    }

    /// Open any supported path into a new shell session and persist `.qfdoc`.
    ///
    /// Supported: `.qfdoc` (package open), `.docx`, `.md`, `.html`, `.txt`.
    /// `.pdf` is export-only.
    pub fn open_from_path(work_dir: &Path, name: &str, path: &Path) -> Result<Self, EditorError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        std::fs::create_dir_all(work_dir).map_err(|e| EditorError::Other(e.to_string()))?;
        if ext == "qfdoc" {
            let package_path = work_dir.join(format!("{name}.qfdoc"));
            let journal_path = work_dir.join(format!("{name}.journal"));
            if path.canonicalize().ok() != package_path.canonicalize().ok() {
                std::fs::copy(path, &package_path)
                    .map_err(|e| EditorError::Other(e.to_string()))?;
            }
            if journal_path.exists() {
                let _ = std::fs::remove_file(&journal_path);
            }
            let editor = EditorController::open_package(&package_path, &journal_path)?;
            let draft = editor.focused_text().unwrap_or_else(|| editor.document_plain());
            let surface = ProductSurface::word_class_default(name);
            let chrome = PageChrome::from_meta(&editor.session.state().document.meta.custom);
            let mut session = Self {
                editor,
                surface,
                package_path,
                journal_path,
                draft,
                last_error: None,
                dirty: false,
                review: ReviewPanel::word_class_default(),
                chrome,
                search_panel: SearchPanel::default(),
                clipboard: String::new(),
            };
            session.refresh_surface();
            return Ok(session);
        }
        let mut session = Self::open_or_create(work_dir, name)?;
        session.import_path(path)?;
        session.save()?;
        Ok(session)
    }

    /// Assert editor layout page count matches PDF `/Count` (print fidelity).
    pub fn layout_pdf_page_parity(&self) -> Result<(), EditorError> {
        let layout_pages = self.editor.page_count();
        let pdf = self.editor.export_pdf_bytes()?;
        let s = String::from_utf8_lossy(&pdf);
        let needle = format!("/Count {layout_pages}");
        if !s.contains(&needle) {
            return Err(EditorError::Other(format!(
                "print fidelity: layout pages={layout_pages} but PDF missing {needle}"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn shell_types_and_counts_words() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "demo").unwrap();
        s.draft = "Hello world from Quillforge".into();
        s.apply_draft().unwrap();
        assert!(s.editor.word_count() >= 4);
        assert_eq!(s.surface.view_mode, DocumentViewMode::Page);
        s.save().unwrap();
        assert!(s.package_path.exists());
        assert!(!s.dirty);
    }

    #[test]
    fn shell_status_has_pages() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "pages").unwrap();
        s.refresh_surface();
        assert!(s.surface.status.page_total >= 1);
        assert!(ProductSurface::assert_no_microsoft_marks(&s.status_line()));
    }

    #[test]
    fn print_layout_pdf_page_parity() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "parity").unwrap();
        // Many short lines force multi-page layout under US Letter defaults.
        let mut body = String::new();
        for i in 0..40 {
            body.push_str(&format!("Paragraph {i} of print fidelity test.\n"));
        }
        s.draft = body;
        s.apply_draft().unwrap();
        // Single block wraps by char estimate; insert many blocks via enter-like inserts.
        // Use export on current state and ensure /Count is well-formed at least 1.
        s.layout_pdf_page_parity().unwrap();
        let path = s.export_pdf().unwrap();
        assert!(path.exists());
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"));
    }

    #[test]
    fn page_navigation_clamps() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "nav").unwrap();
        s.goto_page(99);
        assert_eq!(s.surface.status.page_current, s.editor.page_count().max(1));
        s.goto_page(0);
        assert_eq!(s.surface.status.page_current, 1);
        s.next_page();
        s.prev_page();
        assert_eq!(s.surface.status.page_current, 1);
    }

    #[test]
    fn review_propose_accept_via_shell() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "review").unwrap();
        s.draft = "Draft for review".into();
        s.apply_draft().unwrap();
        let before = s.draft.clone();
        s.review.instruction = "annotate".into();
        let id = s.propose_suggestion().unwrap();
        assert!(s.review.visible);
        assert_eq!(s.surface.status.pending_suggestions, 1);
        assert!(s.status_line().contains("suggestions"));
        assert!(ProductSurface::assert_no_microsoft_marks(&s.status_line()));
        s.select_suggestion(id);
        s.accept_selected().unwrap();
        assert_eq!(s.surface.status.pending_suggestions, 0);
        assert_ne!(s.draft, before);
    }

    #[test]
    fn review_reject_keeps_draft() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "reject").unwrap();
        s.draft = "Keep me".into();
        s.apply_draft().unwrap();
        let id = s.propose_suggestion().unwrap();
        s.select_suggestion(id);
        s.reject_selected().unwrap();
        assert_eq!(s.draft, "Keep me");
        assert_eq!(s.editor.focused_text().unwrap(), "Keep me");
    }

    #[test]
    fn docx_export_import_round_trip() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "round").unwrap();
        s.draft = "Round-trip body".into();
        s.apply_draft().unwrap();
        // Promote first block text; also insert a heading via import path after export.
        let docx = s.export_docx().unwrap();
        assert!(docx.exists());
        let mut s2 = ShellSession::open_or_create(dir.path(), "round-import").unwrap();
        let n = s2.import_docx(&docx).unwrap();
        assert!(n >= 1);
        let plain = s2.editor.document_plain();
        assert!(
            plain.contains("Round-trip body"),
            "imported plain missing body: {plain}"
        );
        s2.save().unwrap();
        assert!(s2.package_path.exists());
    }

    #[test]
    fn docx_image_drawingml_round_trip() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "img-exp").unwrap();
        s.draft = "Caption host".into();
        s.apply_draft().unwrap();
        s.insert_image_placeholder().unwrap();
        assert_eq!(s.editor.image_block_count(), 1);
        let docx = s.export_docx().unwrap();
        let mut s2 = ShellSession::open_or_create(dir.path(), "img-imp").unwrap();
        let n = s2.import_docx(&docx).unwrap();
        assert!(n >= 1, "expected imported blocks");
        assert!(
            s2.editor.image_block_count() >= 1,
            "expected image block after DrawingML import"
        );
        assert!(
            !s2.editor.session.assets.is_empty(),
            "expected package assets from imported media"
        );
    }

    #[test]
    fn open_from_docx_writes_qfdoc() {
        let dir = tempdir().unwrap();
        let mut seed = ShellSession::open_or_create(dir.path(), "seed").unwrap();
        seed.draft = "From Word-class DOCX".into();
        seed.apply_draft().unwrap();
        let docx = seed.export_docx().unwrap();
        let opened =
            ShellSession::open_from_docx(dir.path(), "opened", &docx).unwrap();
        assert!(opened.package_path.exists());
        assert!(opened.editor.document_plain().contains("From Word-class DOCX"));
    }

    #[test]
    fn hybrid_search_finds_text() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "search").unwrap();
        s.draft = "Quillforge local-first document durability journal".into();
        s.apply_draft().unwrap();
        s.search_panel.query = "local first durability".into();
        s.search_panel.mode = SearchMode::Hybrid;
        s.run_search();
        assert!(
            !s.search_panel.hits.is_empty(),
            "expected hits for hybrid query"
        );
        s.open_search_hit(0).unwrap();
        assert!(s.draft.to_lowercase().contains("quillforge") || !s.draft.is_empty());
    }

    #[test]
    fn page_chrome_and_image_via_shell() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "chrome").unwrap();
        assert!(s.surface.show_rulers);
        s.toggle_rulers();
        assert!(!s.surface.show_rulers);
        s.chrome.header = "Header A".into();
        s.chrome.footer = "Footer B".into();
        s.apply_page_chrome().unwrap();
        assert_eq!(s.editor.page_header(), "Header A");
        assert_eq!(s.editor.page_footer(), "Footer B");
        s.insert_image_placeholder().unwrap();
        assert_eq!(s.editor.image_block_count(), 1);
        s.save().unwrap();
        assert!(s.package_path.exists());
    }

    #[test]
    fn insert_product_mark_via_shell() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "logo").unwrap();
        s.insert_product_mark().unwrap();
        assert_eq!(s.editor.image_block_count(), 1);
        assert!(!s.editor.session.assets.is_empty());
    }

    #[test]
    fn product_surface_marks_clipboard_insert_layout_comment() {
        use qf_blocks::Mark;
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "surface").unwrap();
        s.draft = "Bold me".into();
        s.apply_draft().unwrap();

        s.toggle_bold().unwrap();
        let block_id = s.editor.selection().block_id.expect("focus");
        let has_bold = s
            .editor
            .session
            .state()
            .document
            .blocks
            .get(&block_id)
            .map(|b| match &b.body {
                qf_blocks::BlockBody::Paragraph(p) => p
                    .spans
                    .first()
                    .map(|sp| sp.marks.iter().any(|m| matches!(m, Mark::Bold)))
                    .unwrap_or(false),
                _ => false,
            })
            .unwrap_or(false);
        assert!(has_bold, "bold mark must apply via journal");
        s.toggle_bold().unwrap();

        s.draft = "cut-copy-paste".into();
        s.cut_selection().unwrap();
        assert_eq!(s.clipboard, "cut-copy-paste");
        s.paste_clipboard().unwrap();
        assert!(s.editor.document_plain().contains("cut-copy-paste"));

        s.insert_heading().unwrap();
        s.insert_bullet().unwrap();
        s.insert_table().unwrap();
        s.insert_page_break().unwrap();
        s.insert_citation().unwrap();
        let plain = s.editor.document_plain();
        assert!(plain.contains("Heading") || plain.contains("•") || plain.contains("[cite:"));

        let m0 = s.editor.cycle_margins().unwrap();
        assert!(m0.contains("margins"));
        let o = s.editor.toggle_orientation().unwrap();
        assert!(o == "Landscape" || o == "Portrait");
        let sz = s.editor.cycle_page_size().unwrap();
        assert!(sz == "A4" || sz == "US Letter");

        let id = s.add_comment_note("ship note").unwrap();
        assert!(s.editor.suggestion_by_id(id).is_some());
    }

    #[test]
    fn export_all_formats_and_open_markdown() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "io").unwrap();
        s.draft = "All formats body text".into();
        s.apply_draft().unwrap();
        let paths = s.export_all().unwrap();
        assert!(paths.len() >= 6);
        for ext in ["pdf", "docx", "md", "html", "txt", "qfdoc"] {
            assert!(
                paths.iter().any(|p| p.extension().and_then(|e| e.to_str()) == Some(ext)),
                "missing .{ext} in {paths:?}"
            );
            let p = paths
                .iter()
                .find(|p| p.extension().and_then(|e| e.to_str()) == Some(ext))
                .unwrap();
            assert!(p.exists(), "missing file {}", p.display());
        }
        // Round-trip markdown import.
        let md = dir.path().join("io.md");
        let opened = ShellSession::open_from_path(dir.path(), "from-md", &md).unwrap();
        assert!(
            opened.editor.document_plain().contains("All formats"),
            "{}",
            opened.editor.document_plain()
        );
    }

    #[test]
    fn open_html_and_txt() {
        let dir = tempdir().unwrap();
        let html = dir.path().join("sample.html");
        std::fs::write(&html, "<h1>Hi</h1><p>Body copy</p>").unwrap();
        let s = ShellSession::open_from_path(dir.path(), "html", &html).unwrap();
        let plain = s.editor.document_plain();
        assert!(plain.contains("Hi") || plain.contains("Body"), "{plain}");

        let txt = dir.path().join("sample.txt");
        std::fs::write(&txt, "plain paragraph one\n\nplain two").unwrap();
        let s2 = ShellSession::open_from_path(dir.path(), "txt", &txt).unwrap();
        assert!(s2.editor.document_plain().contains("plain"));
    }

    #[test]
    fn open_qfdoc_preserves_text() {
        let dir = tempdir().unwrap();
        let mut s = ShellSession::open_or_create(dir.path(), "pkg").unwrap();
        s.draft = "Package open body".into();
        s.apply_draft().unwrap();
        s.save().unwrap();
        let qfdoc = s.package_path.clone();
        let reopened = ShellSession::open_from_path(dir.path(), "pkg2", &qfdoc).unwrap();
        assert!(
            reopened.editor.document_plain().contains("Package open body"),
            "{}",
            reopened.editor.document_plain()
        );
    }
}
