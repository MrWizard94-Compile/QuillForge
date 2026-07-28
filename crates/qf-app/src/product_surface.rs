//! Word-class product surface model (no Microsoft branding).
//!
//! Binding UX priority: feel like an upgraded desktop document editor
//! (page canvas, command bar, status) while remaining WPAI-branded.
//! See `docs/28-product-surface.md`.

use qf_core::{PRODUCT_NAME, STUDIO_SHORT, STUDIO_TAGLINE};
use serde::{Deserialize, Serialize};

/// Primary authoring view — default is print/page layout (Word-class default).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DocumentViewMode {
    /// Paginated page canvas (default).
    #[default]
    Page,
    /// Continuous draft without page chrome.
    Draft,
    /// Read-only focus mode.
    Focus,
}

/// Top-level command bar tabs (product name: Command bar — not trademarked "Ribbon").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandBarTab {
    File,
    Home,
    Insert,
    Layout,
    Review,
    View,
    Export,
}

impl CommandBarTab {
    pub fn all() -> &'static [CommandBarTab] {
        &[
            CommandBarTab::File,
            CommandBarTab::Home,
            CommandBarTab::Insert,
            CommandBarTab::Layout,
            CommandBarTab::Review,
            CommandBarTab::View,
            CommandBarTab::Export,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            CommandBarTab::File => "File",
            CommandBarTab::Home => "Home",
            CommandBarTab::Insert => "Insert",
            CommandBarTab::Layout => "Layout",
            CommandBarTab::Review => "Review",
            CommandBarTab::View => "View",
            CommandBarTab::Export => "Export",
        }
    }
}

/// Groups of actions under a tab (future GUI binds widgets to these ids).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandGroup {
    pub id: String,
    pub label: String,
    pub actions: Vec<CommandAction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandAction {
    pub id: String,
    pub label: String,
    /// Maps to input/session commands when wired.
    pub command_hint: String,
}

/// Status bar fields users expect on a Word-class editor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StatusBarModel {
    pub page_current: u32,
    pub page_total: u32,
    pub word_count: u32,
    pub language: String,
    pub zoom_percent: u32,
    pub offline: bool,
    pub encrypted: bool,
    /// Pending suggestions awaiting accept/reject (Review surface).
    pub pending_suggestions: u32,
}

impl StatusBarModel {
    pub fn summary_line(&self) -> String {
        let net = if self.offline { "Offline" } else { "Online" };
        let enc = if self.encrypted { " · Encrypted" } else { "" };
        let sug = if self.pending_suggestions > 0 {
            format!(" ·  {} suggestions", self.pending_suggestions)
        } else {
            String::new()
        };
        format!(
            "Page {} of {}  ·  {} words  ·  {}%  ·  {}  ·  {}{}{}",
            self.page_current.max(1),
            self.page_total.max(1),
            self.word_count,
            self.zoom_percent,
            self.language,
            net,
            enc,
            sug
        )
    }
}

/// Full product chrome snapshot for a window shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductSurface {
    pub window_title: String,
    pub view_mode: DocumentViewMode,
    pub active_tab: CommandBarTab,
    pub status: StatusBarModel,
    pub show_rulers: bool,
    pub show_status_bar: bool,
    pub document_display_name: String,
}

impl Default for ProductSurface {
    fn default() -> Self {
        Self::word_class_default("Untitled")
    }
}

impl ProductSurface {
    /// Default chrome: page view, Home tab, status visible — Word-class familiarity.
    pub fn word_class_default(document_name: &str) -> Self {
        Self {
            window_title: format!("{document_name} – {PRODUCT_NAME}"),
            view_mode: DocumentViewMode::Page,
            active_tab: CommandBarTab::Home,
            status: StatusBarModel {
                page_current: 1,
                page_total: 1,
                word_count: 0,
                language: "en-US".into(),
                zoom_percent: 100,
                offline: true,
                encrypted: false,
                pending_suggestions: 0,
            },
            show_rulers: true,
            show_status_bar: true,
            document_display_name: document_name.into(),
        }
    }

    pub fn set_document_name(&mut self, name: &str) {
        self.document_display_name = name.into();
        self.window_title = format!("{name} – {PRODUCT_NAME}");
    }

    pub fn activate_tab(&mut self, tab: CommandBarTab) {
        self.active_tab = tab;
    }

    pub fn set_view_mode(&mut self, mode: DocumentViewMode) {
        self.view_mode = mode;
    }

    /// Groups shown for the active tab (static catalog for shell binding).
    pub fn active_groups(&self) -> Vec<CommandGroup> {
        groups_for_tab(self.active_tab)
    }

    /// One-line product positioning (no Microsoft marks).
    pub fn category_line() -> String {
        format!(
            "{PRODUCT_NAME} by {STUDIO_SHORT} — Word-class document editor · {STUDIO_TAGLINE}"
        )
    }

    /// Ensure we never emit forbidden trademark strings in chrome labels.
    pub fn assert_no_microsoft_marks(s: &str) -> bool {
        let lower = s.to_ascii_lowercase();
        !lower.contains("microsoft")
            && !lower.contains("office 365")
            && !lower.contains("onedrive")
            && !contains_word_trademark(&lower)
    }
}

fn contains_word_trademark(lower: &str) -> bool {
    lower.contains("ms word")
        || lower.contains("microsoft word")
        || lower.contains("word®")
}

fn groups_for_tab(tab: CommandBarTab) -> Vec<CommandGroup> {
    match tab {
        CommandBarTab::File => vec![CommandGroup {
            id: "file.main".into(),
            label: "File".into(),
            actions: vec![
                action("file.new", "New", "CreateDocument"),
                action("file.open", "Open", "OpenPackage"),
                action("file.import_docx", "Import DOCX", "ImportDocx"),
                action("file.save", "Save", "SavePackage"),
                action("file.save_as", "Save As", "SavePackageAs"),
                action("file.export", "Export", "ExportDialog"),
            ],
        }],
        CommandBarTab::Home => vec![
            CommandGroup {
                id: "home.clipboard".into(),
                label: "Clipboard".into(),
                actions: vec![
                    action("edit.cut", "Cut", "Cut"),
                    action("edit.copy", "Copy", "Copy"),
                    action("edit.paste", "Paste", "Paste"),
                ],
            },
            CommandGroup {
                id: "home.font".into(),
                label: "Font".into(),
                actions: vec![
                    action("mark.bold", "Bold", "ToggleBold"),
                    action("mark.italic", "Italic", "ToggleItalic"),
                    action("mark.underline", "Underline", "ToggleUnderline"),
                ],
            },
            CommandGroup {
                id: "home.paragraph".into(),
                label: "Paragraph".into(),
                actions: vec![
                    action("block.heading", "Heading", "InsertHeading"),
                    action("block.bullet", "Bullets", "InsertList"),
                ],
            },
        ],
        CommandBarTab::Insert => vec![CommandGroup {
            id: "insert.blocks".into(),
            label: "Insert".into(),
            actions: vec![
                action("insert.table", "Table", "InsertTable"),
                action("insert.image", "Image", "InsertImage"),
                action("insert.page_break", "Page Break", "InsertPageBreak"),
                action("insert.citation", "Citation", "InsertCitation"),
            ],
        }],
        CommandBarTab::Layout => vec![
            CommandGroup {
                id: "layout.page".into(),
                label: "Page Setup".into(),
                actions: vec![
                    action("layout.margins", "Margins", "PageMargins"),
                    action("layout.orientation", "Orientation", "PageOrientation"),
                    action("layout.size", "Size", "PageSize"),
                ],
            },
            CommandGroup {
                id: "layout.chrome".into(),
                label: "Headers & Footers".into(),
                actions: vec![
                    action("layout.header", "Header", "EditHeader"),
                    action("layout.footer", "Footer", "EditFooter"),
                ],
            },
        ],
        CommandBarTab::Review => vec![CommandGroup {
            id: "review.track".into(),
            label: "Review".into(),
            actions: vec![
                action("review.suggest", "Suggestions", "ToggleSuggestions"),
                action("review.comment", "Comment", "AddComment"),
                action("review.accept", "Accept", "AcceptSuggestion"),
                action("review.reject", "Reject", "RejectSuggestion"),
            ],
        }],
        CommandBarTab::View => vec![CommandGroup {
            id: "view.modes".into(),
            label: "Views".into(),
            actions: vec![
                action("view.page", "Page", "SetViewPage"),
                action("view.draft", "Draft", "SetViewDraft"),
                action("view.focus", "Focus", "SetViewFocus"),
                action("view.ruler", "Ruler", "ToggleRuler"),
            ],
        }],
        CommandBarTab::Export => vec![CommandGroup {
            id: "export.formats".into(),
            label: "Export".into(),
            actions: vec![
                action("export.pdf", "PDF", "ExportPdf"),
                action("export.docx", "DOCX", "ExportDocx"),
                action("export.md", "Markdown", "ExportMarkdown"),
                action("export.html", "HTML", "ExportHtml"),
            ],
        }],
    }
}

fn action(id: &str, label: &str, hint: &str) -> CommandAction {
    CommandAction {
        id: id.into(),
        label: label.into(),
        command_hint: hint.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_page_view_home_tab() {
        let s = ProductSurface::word_class_default("Thesis");
        assert_eq!(s.view_mode, DocumentViewMode::Page);
        assert_eq!(s.active_tab, CommandBarTab::Home);
        assert!(s.window_title.contains("Thesis"));
        assert!(s.window_title.contains("Quillforge"));
        assert!(s.show_status_bar);
        assert!(s.show_rulers);
    }

    #[test]
    fn status_line_word_class_shape() {
        let mut s = ProductSurface::default();
        s.status.page_current = 2;
        s.status.page_total = 10;
        s.status.word_count = 1200;
        let line = s.status.summary_line();
        assert!(line.contains("Page 2 of 10"));
        assert!(line.contains("1200 words"));
        assert!(ProductSurface::assert_no_microsoft_marks(&line));
    }

    #[test]
    fn command_bar_tabs_cover_word_class_workflow() {
        let labels: Vec<_> = CommandBarTab::all().iter().map(|t| t.label()).collect();
        assert!(labels.contains(&"Home"));
        assert!(labels.contains(&"Review"));
        assert!(labels.contains(&"Export"));
        for t in CommandBarTab::all() {
            assert!(!groups_for_tab(*t).is_empty());
            for g in groups_for_tab(*t) {
                assert!(ProductSurface::assert_no_microsoft_marks(&g.label));
                for a in &g.actions {
                    assert!(ProductSurface::assert_no_microsoft_marks(&a.label));
                }
            }
        }
    }

    #[test]
    fn category_line_is_wpai_not_microsoft() {
        let line = ProductSurface::category_line();
        assert!(line.contains("Quillforge"));
        assert!(line.contains("WPAI"));
        assert!(line.contains("Word-class"));
        assert!(ProductSurface::assert_no_microsoft_marks(&line));
        assert!(!line.to_ascii_lowercase().contains("microsoft"));
    }

    #[test]
    fn tab_switch_updates_groups() {
        let mut s = ProductSurface::default();
        s.activate_tab(CommandBarTab::Export);
        let groups = s.active_groups();
        let ids: Vec<&str> = groups
            .iter()
            .flat_map(|g| g.actions.iter().map(|a| a.id.as_str()))
            .collect();
        assert!(ids.contains(&"export.pdf"));
        assert!(ids.contains(&"export.docx"));
    }
}
