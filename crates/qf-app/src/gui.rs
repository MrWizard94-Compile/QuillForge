//! Word-class native window shell (egui) — no Microsoft branding.
//!
//! Layout: command bar tabs · paper page with editor · status bar.

use crate::product_surface::{CommandBarTab, DocumentViewMode};
use crate::shell_session::ShellSession;
use crate::ProductSurface;
use eframe::egui::{self, Color32, FontId, RichText, Sense, Vec2};
use qf_theme::Theme;
use std::path::PathBuf;

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
        let c = self
            .theme
            .resolve(qf_core::ColorId(qf_theme::role::SURFACE_RAISED))
            .map(|r| Color32::from_rgb(r.r, r.g, r.b))
            .unwrap_or(Color32::WHITE);
        c
    }

    fn rail(&self) -> Color32 {
        let c = self
            .theme
            .resolve(qf_core::ColorId(qf_theme::role::SURFACE))
            .map(|r| Color32::from_rgb(r.r, r.g, r.b))
            .unwrap_or(Color32::from_rgb(0xf3, 0xf2, 0xf1));
        c
    }

    fn ember(&self) -> Color32 {
        let c = self
            .theme
            .resolve(qf_core::ColorId(qf_theme::role::ACCENT))
            .map(|r| Color32::from_rgb(r.r, r.g, r.b))
            .unwrap_or(Color32::from_rgb(0xff, 0x7a, 0x26));
        c
    }

    fn ink(&self) -> Color32 {
        let c = self
            .theme
            .resolve(qf_core::ColorId(qf_theme::role::TEXT))
            .map(|r| Color32::from_rgb(r.r, r.g, r.b))
            .unwrap_or(Color32::from_rgb(0x1a, 0x1a, 0x1a));
        c
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
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Save").clicked() {
                            if let Err(e) = self.session.save() {
                                self.session.last_error = Some(e.to_string());
                            }
                        }
                    });
                });
                // Active groups
                ui.horizontal_wrapped(|ui| {
                    for g in self.session.surface.active_groups() {
                        ui.group(|ui| {
                            ui.label(RichText::new(&g.label).small().color(ink));
                            ui.horizontal(|ui| {
                                for a in &g.actions {
                                    if ui.small_button(&a.label).clicked() {
                                        match a.id.as_str() {
                                            "file.save" => {
                                                if let Err(e) = self.session.save() {
                                                    self.session.last_error = Some(e.to_string());
                                                }
                                            }
                                            "view.page" => {
                                                self.session.set_view(DocumentViewMode::Page)
                                            }
                                            "view.draft" => {
                                                self.session.set_view(DocumentViewMode::Draft)
                                            }
                                            "view.focus" => {
                                                self.session.set_view(DocumentViewMode::Focus)
                                            }
                                            "export.pdf" | "export.docx" | "export.md"
                                            | "export.html" => {
                                                // Export via save path sibling; full dialogs in P2.
                                                if let Err(e) = self.session.apply_draft() {
                                                    self.session.last_error = Some(e.to_string());
                                                }
                                            }
                                            _ => self.session.toggle_bold_hint(),
                                        }
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
                        ui.label(RichText::new(self.session.status_line()).small().color(ink));
                        if let Some(err) = &self.session.last_error {
                            ui.colored_label(Color32::RED, err);
                        }
                    });
                });
        }

        // Page canvas
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(rail).inner_margin(16.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let page_w = 612.0 * 1.15; // US Letter-ish scale
                    let page_h = 792.0 * 0.85;
                    let (rect, _resp) =
                        ui.allocate_exact_size(Vec2::new(page_w, page_h), Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, paper);
                    ui.painter().rect_stroke(
                        rect,
                        2.0,
                        egui::Stroke::new(1.0, Color32::from_gray(180)),
                    );

                    // Editor over paper
                    let mut child = ui.child_ui(
                        rect.shrink2(Vec2::new(48.0, 48.0)),
                        egui::Layout::top_down(egui::Align::Min),
                    );
                    child.style_mut().visuals.override_text_color = Some(ink);
                    let te = egui::TextEdit::multiline(&mut self.session.draft)
                        .font(FontId::proportional(14.0))
                        .desired_width(f32::INFINITY)
                        .frame(false);
                    let response = child.add_sized(child.available_size(), te);
                    if response.changed() {
                        if let Err(e) = self.session.apply_draft() {
                            self.session.last_error = Some(e.to_string());
                        } else {
                            self.session.dirty = true;
                            self.session.refresh_surface();
                        }
                    }
                });
            });

        // Keep window title in sync
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
