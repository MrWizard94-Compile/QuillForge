//! Word-class shell session: editor + product surface metrics.

use crate::product_surface::{CommandBarTab, DocumentViewMode, ProductSurface};
use crate::{EditorController, EditorError};
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
        if self.dirty {
            // keep name; mark title with *
            let base = self.surface.document_display_name.trim_end_matches('*').to_string();
            if !self.surface.document_display_name.ends_with('*') {
                self.surface.set_document_name(&format!("{base}*"));
            }
        }
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
}
