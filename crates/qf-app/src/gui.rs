//! Word-class native window shell (egui) — no Microsoft branding.
//!
//! Layout: command bar · print page canvas (layout tree) · status bar.
//! Page geometry matches `qf_layout` US Letter defaults (print fidelity).

use crate::product_surface::{CommandBarTab, DocumentViewMode};
use crate::shell_session::ShellSession;
use crate::ProductSurface;
use eframe::egui::{self, Color32, FontId, RichText, Sense, Vec2};
use qf_layout::{fixed_to_pt, PAGE_HEIGHT_PT, PAGE_WIDTH_PT};
use qf_theme::Theme;
use std::path::PathBuf;

/// Snapshot of one laid-out run for painting (screen ≈ layout coords).
struct PagePaintLine {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    text: String,
    font_size: f32,
    is_media: bool,
}

struct PagePaint {
    page_num: u32,
    lines: Vec<PagePaintLine>,
}

pub struct QuillShellApp {
    session: ShellSession,
    theme: Theme,
}

impl QuillShellApp {
    pub fn new(work_dir: PathBuf, doc_name: &str) -> Result<Self, String> {
        let session = ShellSession::open_or_create(&work_dir, doc_name).map_err(|e| e.to_string())?;
        Ok(Self {
            session,
            theme: Theme::document_workspace_light(),
        })
    }

    fn paper(&self) -> Color32 {
        self.theme
            .resolve(qf_core::ColorId(qf_theme::role::SURFACE_RAISED))
            .map(|r| Color32::from_rgb(r.r, r.g, r.b))
            .unwrap_or(Color32::WHITE)
    }

    fn rail(&self) -> Color32 {
        self.theme
            .resolve(qf_core::ColorId(qf_theme::role::SURFACE))
            .map(|r| Color32::from_rgb(r.r, r.g, r.b))
            .unwrap_or(Color32::from_rgb(0xf3, 0xf2, 0xf1))
    }

    fn ember(&self) -> Color32 {
        self.theme
            .resolve(qf_core::ColorId(qf_theme::role::ACCENT))
            .map(|r| Color32::from_rgb(r.r, r.g, r.b))
            .unwrap_or(Color32::from_rgb(0xff, 0x7a, 0x26))
    }

    fn ink(&self) -> Color32 {
        self.theme
            .resolve(qf_core::ColorId(qf_theme::role::TEXT))
            .map(|r| Color32::from_rgb(r.r, r.g, r.b))
            .unwrap_or(Color32::from_rgb(0x1a, 0x1a, 0x1a))
    }

    fn page_scale(&self) -> f32 {
        let zoom = self.session.surface.status.zoom_percent.max(50) as f32 / 100.0;
        // Fit letter width ~704px at 100% (similar to prior P1 canvas).
        1.15 * zoom
    }

    fn handle_action(&mut self, id: &str) {
        match id {
            "file.save" => {
                if let Err(e) = self.session.save() {
                    self.session.last_error = Some(e.to_string());
                }
            }
            "view.page" => self.session.set_view(DocumentViewMode::Page),
            "view.draft" => self.session.set_view(DocumentViewMode::Draft),
            "view.focus" => self.session.set_view(DocumentViewMode::Focus),
            "export.pdf" => match self.session.export_pdf() {
                Ok(path) => {
                    self.session.last_error = Some(format!("Exported {}", path.display()));
                }
                Err(e) => self.session.last_error = Some(e.to_string()),
            },
            "export.docx" => match self.session.export_docx() {
                Ok(path) => {
                    self.session.last_error = Some(format!("Exported {}", path.display()));
                }
                Err(e) => self.session.last_error = Some(e.to_string()),
            },
            "file.import_docx" => {
                // Word-class import: load sibling/default import path under work dir.
                let path = self
                    .session
                    .package_path
                    .parent()
                    .map(|p| p.join("import.docx"))
                    .unwrap_or_else(|| PathBuf::from("import.docx"));
                match self.session.import_docx(&path) {
                    Ok(n) => {
                        self.session.last_error =
                            Some(format!("Imported {n} blocks from {}", path.display()));
                    }
                    Err(e) => self.session.last_error = Some(e.to_string()),
                }
            }
            "export.md" | "export.html" => {
                if let Err(e) = self.session.apply_draft() {
                    self.session.last_error = Some(e.to_string());
                }
            }
            "review.suggest" => {
                self.session.review.visible = true;
                self.session.surface.activate_tab(CommandBarTab::Review);
                match self.session.propose_suggestion() {
                    Ok(_) => {
                        self.session.last_error =
                            Some("Suggestion recorded (pending accept)".into());
                    }
                    Err(e) => self.session.last_error = Some(e.to_string()),
                }
            }
            "review.accept" => {
                if let Err(e) = self.session.accept_selected() {
                    self.session.last_error = Some(e.to_string());
                } else {
                    self.session.last_error = Some("Suggestion accepted".into());
                }
            }
            "review.reject" => {
                if let Err(e) = self.session.reject_selected() {
                    self.session.last_error = Some(e.to_string());
                } else {
                    self.session.last_error = Some("Suggestion rejected".into());
                }
            }
            "review.comment" => {
                self.session.review.visible = true;
                self.session.surface.activate_tab(CommandBarTab::Review);
                self.session.last_error =
                    Some("Comments: use Suggestions for review notes (P3)".into());
            }
            "view.ruler" => {
                self.session.toggle_rulers();
                self.session.last_error = Some(if self.session.surface.show_rulers {
                    "Rulers shown".into()
                } else {
                    "Rulers hidden".into()
                });
            }
            "layout.header" | "layout.footer" => {
                self.session.chrome.panel_visible = true;
                self.session.surface.activate_tab(CommandBarTab::Layout);
            }
            "insert.image" => match self.session.insert_image_placeholder() {
                Ok(_) => {
                    self.session.last_error = Some("Image block inserted".into());
                }
                Err(e) => self.session.last_error = Some(e.to_string()),
            },
            "layout.margins" | "layout.orientation" | "layout.size" => {
                self.session.chrome.panel_visible = true;
                self.session.surface.activate_tab(CommandBarTab::Layout);
                self.session.last_error =
                    Some("Page setup: US Letter · 1\" margins (layout engine)".into());
            }
            _ => self.session.toggle_bold_hint(),
        }
    }

    fn paint_layout_panel(&mut self, ui: &mut egui::Ui, ink: Color32, ember: Color32) {
        ui.heading(RichText::new("Page chrome").color(ember).size(16.0));
        ui.label(
            RichText::new("Headers & footers are journaled document meta")
                .small()
                .color(Color32::from_gray(100)),
        );
        ui.separator();
        ui.label(RichText::new("Header").color(ink));
        ui.add(
            egui::TextEdit::singleline(&mut self.session.chrome.header).desired_width(240.0),
        );
        ui.label(RichText::new("Footer").color(ink));
        ui.add(
            egui::TextEdit::singleline(&mut self.session.chrome.footer).desired_width(240.0),
        );
        ui.horizontal(|ui| {
            if ui.button("Apply").clicked() {
                if let Err(e) = self.session.apply_page_chrome() {
                    self.session.last_error = Some(e.to_string());
                } else {
                    self.session.last_error = Some("Header/footer saved".into());
                }
            }
            if ui
                .selectable_label(self.session.surface.show_rulers, "Rulers")
                .clicked()
            {
                self.session.toggle_rulers();
            }
            if ui.small_button("Close").clicked() {
                self.session.chrome.panel_visible = false;
            }
        });
        ui.separator();
        ui.label(
            RichText::new(format!(
                "Images in document: {}",
                self.session.editor.image_block_count()
            ))
            .small()
            .color(ink),
        );
        if ui.button("Insert image").clicked() {
            self.handle_action("insert.image");
        }
    }

    fn paint_ruler_horizontal(&self, ui: &mut egui::Ui, page_w: f32, scale: f32) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(page_w, 18.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, 0.0, Color32::from_rgb(0xe8, 0xe6, 0xe4));
        ui.painter()
            .rect_stroke(rect, 0.0, egui::Stroke::new(1.0, Color32::from_gray(160)));
        // Inch ticks (72 pt).
        let mut inch = 0;
        while {
            let x = inch as f32 * 72.0 * scale;
            x <= page_w
        } {
            let x = rect.left() + inch as f32 * 72.0 * scale;
            ui.painter().line_segment(
                [egui::pos2(x, rect.top() + 4.0), egui::pos2(x, rect.bottom() - 2.0)],
                egui::Stroke::new(1.0, Color32::from_gray(90)),
            );
            ui.painter().text(
                egui::pos2(x + 2.0, rect.top() + 2.0),
                egui::Align2::LEFT_TOP,
                format!("{inch}\""),
                FontId::proportional(9.0),
                Color32::from_gray(80),
            );
            inch += 1;
        }
    }

    fn paint_ruler_vertical(&self, painter: &egui::Painter, page_rect: egui::Rect, scale: f32) {
        let ruler_w = 18.0;
        let rect = egui::Rect::from_min_size(
            page_rect.left_top() - Vec2::new(ruler_w + 2.0, 0.0),
            Vec2::new(ruler_w, page_rect.height()),
        );
        painter.rect_filled(rect, 0.0, Color32::from_rgb(0xe8, 0xe6, 0xe4));
        painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, Color32::from_gray(160)));
        let mut inch = 0;
        while {
            let y = inch as f32 * 72.0 * scale;
            y <= page_rect.height()
        } {
            let y = rect.top() + inch as f32 * 72.0 * scale;
            painter.line_segment(
                [egui::pos2(rect.left() + 2.0, y), egui::pos2(rect.right() - 2.0, y)],
                egui::Stroke::new(1.0, Color32::from_gray(90)),
            );
            inch += 1;
        }
    }

    fn paint_review_panel(&mut self, ui: &mut egui::Ui, ink: Color32, ember: Color32) {
        ui.heading(RichText::new("Suggestions").color(ember).size(16.0));
        ui.label(
            RichText::new("Accept applies via journal · Reject is status-only")
                .small()
                .color(Color32::from_gray(100)),
        );
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Instruction:").color(ink));
            ui.add(
                egui::TextEdit::singleline(&mut self.session.review.instruction)
                    .desired_width(140.0),
            );
            if ui.button("Propose").clicked() {
                self.handle_action("review.suggest");
            }
        });
        ui.separator();
        let rows = self.session.editor.pending_suggestion_rows();
        if rows.is_empty() {
            ui.label(RichText::new("No pending suggestions").italics().color(ink));
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for row in &rows {
                    let selected = self.session.review.selected == Some(row.id);
                    let label = format!(
                        "{} · {} · {} ops",
                        row.note, row.source_label, row.op_count
                    );
                    if ui.selectable_label(selected, RichText::new(label).color(ink)).clicked() {
                        self.session.select_suggestion(row.id);
                    }
                }
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Accept").clicked() {
                self.handle_action("review.accept");
            }
            if ui.button("Reject").clicked() {
                self.handle_action("review.reject");
            }
            if ui.small_button("Close").clicked() {
                self.session.review.visible = false;
            }
        });
    }

    fn paint_print_pages(&mut self, ui: &mut egui::Ui, paper: Color32, ink: Color32) {
        let scale = self.page_scale();
        let page_w = PAGE_WIDTH_PT * scale;
        let page_h = PAGE_HEIGHT_PT * scale;
        let current = self.session.surface.status.page_current;
        let show_rulers = self.session.surface.show_rulers;
        let header = self.session.editor.page_header();
        let footer = self.session.editor.page_footer();
        // Snapshot page content so we can mutate session on click without borrow fights.
        let pages: Vec<PagePaint> = self
            .session
            .editor
            .layout_tree
            .pages
            .iter()
            .map(|page| {
                let lines: Vec<PagePaintLine> = page
                    .frames
                    .iter()
                    .filter_map(|frame| {
                        let x = fixed_to_pt(frame.bounds.x) * scale;
                        let y = fixed_to_pt(frame.bounds.y) * scale;
                        let w = fixed_to_pt(frame.bounds.w) * scale;
                        let h = fixed_to_pt(frame.bounds.h).max(12.0) * scale / 1.0;
                        let is_media = matches!(frame.kind, qf_layout::FragmentKind::Media);
                        if is_media {
                            let alt = self
                                .session
                                .editor
                                .session
                                .state()
                                .document
                                .blocks
                                .get(&frame.block_id)
                                .and_then(|b| match &b.body {
                                    qf_blocks::BlockBody::Image(img) => Some(img.alt.clone()),
                                    _ => None,
                                })
                                .unwrap_or_else(|| "Image".into());
                            return Some(PagePaintLine {
                                x,
                                y,
                                w,
                                h: h.max(40.0 * scale / 1.15),
                                text: format!("[Image: {alt}]"),
                                font_size: 11.0,
                                is_media: true,
                            });
                        }
                        let text = self
                            .session
                            .editor
                            .session
                            .state()
                            .document
                            .project_text(frame.block_id)?;
                        if text.is_empty() {
                            return None;
                        }
                        let font_size = match &frame.kind {
                            qf_layout::FragmentKind::Heading { level } => {
                                (18.0 - f32::from((*level).min(3))) * scale / 1.15
                            }
                            _ => 12.0 * scale / 1.15,
                        };
                        Some(PagePaintLine {
                            x,
                            y,
                            w,
                            h,
                            text,
                            font_size: font_size.max(10.0),
                            is_media: false,
                        })
                    })
                    .collect();
                PagePaint {
                    page_num: page.index + 1,
                    lines,
                }
            })
            .collect();
        let empty_pages = pages.is_empty();
        let mut clicked_page: Option<u32> = None;

        ui.vertical_centered(|ui| {
            if show_rulers {
                // Corner + horizontal ruler aligned to page width (with vertical gutter).
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    self.paint_ruler_horizontal(ui, page_w, scale);
                });
            }
            for page in &pages {
                let is_current = page.page_num == current;
                ui.horizontal(|ui| {
                    if show_rulers {
                        ui.add_space(20.0);
                    }
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(page_w, page_h), Sense::click());
                    if show_rulers {
                        self.paint_ruler_vertical(ui.painter(), rect, scale);
                    }
                    ui.painter().rect_filled(rect, 2.0, paper);
                    let stroke = if is_current {
                        egui::Stroke::new(2.0, Color32::from_rgb(0xff, 0x7a, 0x26))
                    } else {
                        egui::Stroke::new(1.0, Color32::from_gray(180))
                    };
                    ui.painter().rect_stroke(rect, 2.0, stroke);

                    // Header band
                    if !header.is_empty() {
                        ui.painter().text(
                            rect.left_top() + Vec2::new(48.0 * scale / 1.15, 28.0 * scale / 1.15),
                            egui::Align2::LEFT_TOP,
                            &header,
                            FontId::proportional(10.0 * scale / 1.15),
                            Color32::from_gray(100),
                        );
                    }

                    for line in &page.lines {
                        let pos = rect.min + Vec2::new(line.x, line.y);
                        if line.is_media {
                            let media_rect = egui::Rect::from_min_size(
                                pos,
                                Vec2::new(line.w.max(80.0), line.h.max(40.0)),
                            );
                            ui.painter().rect_filled(
                                media_rect,
                                2.0,
                                Color32::from_rgb(0xf0, 0xee, 0xec),
                            );
                            ui.painter().rect_stroke(
                                media_rect,
                                2.0,
                                egui::Stroke::new(1.0, Color32::from_gray(150)),
                            );
                            ui.painter().text(
                                media_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &line.text,
                                FontId::proportional(line.font_size),
                                ink,
                            );
                        } else {
                            ui.painter().text(
                                pos,
                                egui::Align2::LEFT_TOP,
                                &line.text,
                                FontId::proportional(line.font_size),
                                ink,
                            );
                        }
                    }

                    // Footer band
                    if !footer.is_empty() {
                        ui.painter().text(
                            rect.left_bottom()
                                + Vec2::new(48.0 * scale / 1.15, -36.0 * scale / 1.15),
                            egui::Align2::LEFT_BOTTOM,
                            &footer,
                            FontId::proportional(10.0 * scale / 1.15),
                            Color32::from_gray(100),
                        );
                    }

                    ui.painter().text(
                        rect.right_bottom() - Vec2::new(24.0, 20.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("{}", page.page_num),
                        FontId::proportional(11.0),
                        Color32::from_gray(120),
                    );

                    if resp.clicked() {
                        clicked_page = Some(page.page_num);
                    }
                });

                ui.add_space(16.0);
            }

            if empty_pages {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(page_w, page_h), Sense::hover());
                ui.painter().rect_filled(rect, 2.0, paper);
            }
        });

        if let Some(p) = clicked_page {
            self.session.goto_page(p);
        }
    }
}

impl eframe::App for QuillShellApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let rail = self.rail();
        let paper = self.paper();
        let ember = self.ember();
        let ink = self.ink();

        // Command bar
        egui::TopBottomPanel::top("command_bar")
            .frame(egui::Frame::none().fill(rail).inner_margin(6.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Quillforge")
                            .strong()
                            .color(ember)
                            .size(16.0),
                    );
                    ui.separator();
                    for tab in CommandBarTab::all() {
                        let selected = self.session.surface.active_tab == *tab;
                        let label = tab.label();
                        if ui
                            .selectable_label(selected, RichText::new(label).color(ink))
                            .clicked()
                        {
                            self.session.surface.activate_tab(*tab);
                            if *tab == CommandBarTab::Review {
                                self.session.review.visible = true;
                            }
                            if *tab == CommandBarTab::Layout {
                                self.session.chrome.panel_visible = true;
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Export PDF").clicked() {
                            self.handle_action("export.pdf");
                        }
                        if ui.button("Save").clicked() {
                            self.handle_action("file.save");
                        }
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    for g in self.session.surface.active_groups() {
                        ui.group(|ui| {
                            ui.label(RichText::new(&g.label).small().color(ink));
                            ui.horizontal(|ui| {
                                for a in &g.actions {
                                    if ui.small_button(&a.label).clicked() {
                                        self.handle_action(&a.id);
                                    }
                                }
                            });
                        });
                    }
                });
            });

        // Status bar
        if self.session.surface.show_status_bar {
            egui::TopBottomPanel::bottom("status")
                .frame(egui::Frame::none().fill(rail).inner_margin(4.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.small_button("◀").clicked() {
                            self.session.prev_page();
                        }
                        ui.label(
                            RichText::new(self.session.status_line())
                                .small()
                                .color(ink),
                        );
                        if ui.small_button("▶").clicked() {
                            self.session.next_page();
                        }
                        ui.separator();
                        if ui.small_button("−").clicked() {
                            let z = self.session.surface.status.zoom_percent;
                            self.session.set_zoom_percent(z.saturating_sub(10));
                        }
                        ui.label(
                            RichText::new(format!(
                                "{}%",
                                self.session.surface.status.zoom_percent
                            ))
                            .small(),
                        );
                        if ui.small_button("+").clicked() {
                            let z = self.session.surface.status.zoom_percent;
                            self.session.set_zoom_percent(z.saturating_add(10));
                        }
                        ui.separator();
                        if ui.small_button("Review").clicked() {
                            self.session.toggle_review_panel();
                        }
                        if let Some(err) = &self.session.last_error {
                            ui.colored_label(Color32::from_rgb(0x1a, 0x6b, 0x3c), err);
                        }
                    });
                });
        }

        // Review side panel (Word-class Suggestions surface)
        if self.session.review.visible
            || self.session.surface.active_tab == CommandBarTab::Review
        {
            self.session.review.visible = true;
            egui::SidePanel::right("review_panel")
                .default_width(280.0)
                .frame(egui::Frame::none().fill(rail).inner_margin(10.0))
                .show(ctx, |ui| {
                    self.paint_review_panel(ui, ink, ember);
                });
        } else if self.session.chrome.panel_visible
            || self.session.surface.active_tab == CommandBarTab::Layout
        {
            self.session.chrome.panel_visible = true;
            egui::SidePanel::right("layout_panel")
                .default_width(280.0)
                .frame(egui::Frame::none().fill(rail).inner_margin(10.0))
                .show(ctx, |ui| {
                    self.paint_layout_panel(ui, ink, ember);
                });
        }

        // Central: page canvas + typing strip
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(rail).inner_margin(8.0))
            .show(ctx, |ui| {
                let view = self.session.surface.view_mode;
                match view {
                    DocumentViewMode::Draft | DocumentViewMode::Focus => {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(if view == DocumentViewMode::Focus {
                                    "Focus view — typing surface"
                                } else {
                                    "Draft view — continuous typing"
                                })
                                .small()
                                .color(Color32::from_gray(100)),
                            );
                            let te = egui::TextEdit::multiline(&mut self.session.draft)
                                .font(FontId::proportional(14.0))
                                .desired_width(f32::INFINITY)
                                .desired_rows(24);
                            let response = ui.add_sized(
                                Vec2::new(ui.available_width(), ui.available_height().max(200.0)),
                                te,
                            );
                            if response.changed() {
                                if let Err(e) = self.session.apply_draft() {
                                    self.session.last_error = Some(e.to_string());
                                } else {
                                    self.session.dirty = true;
                                    self.session.refresh_surface();
                                }
                            }
                        });
                    }
                    DocumentViewMode::Page => {
                        // Typing strip (journal path) above scrollable print pages.
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Edit:").strong().color(ink));
                            let te = egui::TextEdit::multiline(&mut self.session.draft)
                                .font(FontId::proportional(13.0))
                                .desired_width(ui.available_width() - 8.0)
                                .desired_rows(3);
                            let response = ui.add(te);
                            if response.changed() {
                                if let Err(e) = self.session.apply_draft() {
                                    self.session.last_error = Some(e.to_string());
                                } else {
                                    self.session.dirty = true;
                                    self.session.refresh_surface();
                                }
                            }
                        });
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                self.paint_print_pages(ui, paper, ink);
                            });
                    }
                }
            });

        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            self.session.surface.window_title.clone(),
        ));
    }
}

/// Launch the Word-class shell (blocks until closed).
pub fn run_gui(work_dir: PathBuf, doc_name: &str) -> Result<(), String> {
    let app = QuillShellApp::new(work_dir, doc_name)?;
    let title = app.session.surface.window_title.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 800.0])
            .with_title(title),
        ..Default::default()
    };
    eframe::run_native(
        "Quillforge",
        options,
        Box::new(|_cc| Box::new(app)),
    )
    .map_err(|e| e.to_string())?;
    let _ = ProductSurface::category_line();
    Ok(())
}
