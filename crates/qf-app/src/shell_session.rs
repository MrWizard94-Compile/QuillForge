//! Word-class shell session: editor + product surface metrics.

use crate::product_surface::{CommandBarTab, DocumentViewMode, ProductSurface};
use crate::review::ReviewPanel;
use crate::{EditorController, EditorError};
use qf_core::SuggestionId;
use std::path::{Path, PathBuf};

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
}

impl ShellSession {
    pub fn open_or_create(work_dir: &Path, name: &str) -> Result<Self, EditorError> {
        std::fs::create_dir_all(work_dir).map_err(|e| EditorError::Other(e.to_string()))?;
        let journal_path = work_dir.join(format!("{name}.journal"));
        let package_path = work_dir.join(format!("{name}.qfdoc"));
        let mut editor = EditorController::create_default(&journal_path)?;
        editor.ensure_editable_paragraph()?;
        let draft = editor.focused_text().unwrap_or_default();
        let surface = ProductSurface::word_class_default(name);
        let mut session = Self {
            editor,
            surface,
            package_path,
            journal_path,
            draft,
            last_error: None,
            dirty: false,
            review: ReviewPanel::word_class_default(),
        };
        session.refresh_surface();
        Ok(session)
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
        if self.dirty {
            // keep name; mark title with *
            let base = self.surface.document_display_name.trim_end_matches('*').to_string();
            if !self.surface.document_display_name.ends_with('*') {
                self.surface.set_document_name(&format!("{base}*"));
            }
        }
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

    pub fn toggle_bold_hint(&mut self) {
        // Formatting marks land in P2; surface still exposes the action.
        self.surface.activate_tab(CommandBarTab::Home);
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
        self.apply_draft()?;
        let bytes = self.editor.export_pdf_bytes()?;
        let path = self.package_path.with_extension("pdf");
        std::fs::write(&path, bytes).map_err(|e| EditorError::Other(e.to_string()))?;
        Ok(path)
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
}
