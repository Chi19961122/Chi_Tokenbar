//! Minimal Atoll-like shell: compact island + expandable limits list.
//! Experimental — mock data only.

use crate::mock::{demo_snapshot, Limit, Provider, Snapshot, Status};
use eframe::egui::{self, Align, Color32, CornerRadius, Frame, Layout, RichText, Sense, Stroke, Vec2};

pub struct AtollEguiApp {
    snap: Snapshot,
    expanded: bool,
    open_id: Option<&'static str>,
}

impl Default for AtollEguiApp {
    fn default() -> Self {
        Self {
            snap: demo_snapshot(),
            expanded: false,
            open_id: None,
        }
    }
}

impl eframe::App for AtollEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(18, 18, 22);
        visuals.window_fill = Color32::from_rgb(18, 18, 22);
        visuals.override_text_color = Some(Color32::from_rgb(230, 230, 235));
        ctx.set_visuals(visuals);

        let desired = if self.expanded {
            Vec2::new(380.0, 420.0)
        } else {
            Vec2::new(340.0, 56.0)
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(desired));

        egui::CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(Color32::from_rgb(18, 18, 22))
                    .inner_margin(8.0),
            )
            .show(ctx, |ui| {
                if self.expanded {
                    self.ui_panel(ui);
                } else {
                    self.ui_island(ui);
                }
            });
    }
}

impl AtollEguiApp {
    fn ui_island(&mut self, ui: &mut egui::Ui) {
        let claude = worst_for(&self.snap, Provider::Claude);
        let codex = worst_for(&self.snap, Provider::Codex);

        Frame::new()
            .fill(Color32::from_rgb(28, 28, 34))
            .corner_radius(CornerRadius::same(20))
            .stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60)))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    pill(ui, "C", claude);
                    pill(ui, "X", codex);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("egui exp")
                                .small()
                                .color(Color32::from_rgb(120, 120, 140)),
                        );
                        if ui
                            .add(egui::Button::new("v").frame(false))
                            .on_hover_text("Expand panel")
                            .clicked()
                        {
                            self.expanded = true;
                        }
                    });
                });
            });
    }

    fn ui_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Atoll").strong());
            ui.label(
                RichText::new("- egui experiment")
                    .small()
                    .color(Color32::from_rgb(140, 140, 160)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Island").clicked() {
                    self.expanded = false;
                    self.open_id = None;
                }
            });
        });
        ui.label(
            RichText::new(&self.snap.updated_label)
                .small()
                .color(Color32::from_rgb(130, 130, 150)),
        );
        ui.add_space(6.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            for lim in &self.snap.limits {
                let open = self.open_id == Some(lim.id);
                let resp = Frame::new()
                    .fill(Color32::from_rgb(28, 28, 34))
                    .corner_radius(CornerRadius::same(10))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(45, 45, 55)))
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                            ui.painter()
                                .circle_filled(rect.center(), 5.0, lim.status.color());
                            ui.label(RichText::new(lim.label).strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("{:.0}%", 100.0 - lim.util))
                                        .color(lim.status.color())
                                        .strong(),
                                );
                                ui.label(
                                    RichText::new("left")
                                        .small()
                                        .color(Color32::from_rgb(120, 120, 140)),
                                );
                            });
                        });
                        let bar = egui::ProgressBar::new((lim.util / 100.0) as f32)
                            .desired_width(ui.available_width())
                            .fill(lim.status.color());
                        ui.add(bar);
                        if open {
                            ui.add_space(4.0);
                            ui.label(format!("Provider: {}", lim.provider.short()));
                            ui.label(format!("Status: {}", status_label(lim.status)));
                            ui.label(format!("Runway: {}", lim.runway_label));
                            ui.label(
                                RichText::new("Live APIs / log scan not wired in this build.")
                                    .small()
                                    .color(Color32::from_rgb(150, 140, 100)),
                            );
                        }
                    })
                    .response
                    .interact(Sense::click());

                if resp.clicked() {
                    self.open_id = if open { None } else { Some(lim.id) };
                }
                ui.add_space(6.0);
            }

            ui.separator();
            ui.label(
                RichText::new(
                    "Local experiment branch exp/egui-shell.\n\
No WebView2. Demo data only.\n\
Main product remains Tauri unless you choose to rewrite.",
                )
                .small()
                .color(Color32::from_rgb(110, 110, 130)),
            );
        });
    }
}

fn pill(ui: &mut egui::Ui, tag: &str, lim: Option<&Limit>) {
    let (util, color) = match lim {
        Some(l) => (100.0 - l.util, l.status.color()),
        None => (0.0, Color32::GRAY),
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(tag)
                .small()
                .color(Color32::from_rgb(160, 160, 180)),
        );
        ui.label(RichText::new(format!("{util:.0}%")).color(color).strong());
    });
}

fn worst_for(snap: &Snapshot, p: Provider) -> Option<&Limit> {
    snap.limits
        .iter()
        .filter(|l| l.provider == p)
        .max_by(|a, b| {
            a.util
                .partial_cmp(&b.util)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn status_label(s: Status) -> &'static str {
    match s {
        Status::Ok => "ok",
        Status::Near => "near limit",
        Status::Locked => "locked",
        Status::Failed => "failed",
    }
}
