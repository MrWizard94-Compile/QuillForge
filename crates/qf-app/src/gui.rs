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

/// Snapshot of one laid-out text run for painting (screen ≈ layout coords).
struct PagePaintLine {
    x: f32,
    y: f32,
    text: String,
    font_size: f32,
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
            "export.docx" | "export.md" | "export.html" => {
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
            _ => self.session.toggle_bold_hint(),
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
                            x: fixed_to_pt(frame.bounds.x) * scale,
                            y: fixed_to_pt(frame.bounds.y) * scale,
                            text,
                            font_size: font_size.max(10.0),
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
            for page in &pages {
                let is_current = page.page_num == current;
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(page_w, page_h), Sense::click());
                ui.painter().rect_filled(rect, 2.0, paper);
                let stroke = if is_current {
                    egui::Stroke::new(2.0, Color32::from_rgb(0xff, 0x7a, 0x26))
                } else {
                    egui::Stroke::new(1.0, Color32::from_gray(180))
                };
                ui.painter().rect_stroke(rect, 2.0, stroke);

                for line in &page.lines {
                    let pos = rect.min + Vec2::new(line.x, line.y);
                    ui.painter().text(
                        pos,
                        egui::Align2::LEFT_TOP,
                        &line.text,
                        FontId::proportional(line.font_size),
                        ink,
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
