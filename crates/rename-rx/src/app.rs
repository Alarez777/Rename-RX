use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Button, CornerRadius, FontId, Frame, Layout, Margin, Rect, RichText,
    Shadow, Stroke, Theme, Ui, pos2, vec2,
};

use crate::settings::{self, Settings, ThemePreference};
use crate::theme::{self, Palette, semibold};

pub struct RenameRxApp {
    settings: Settings,
    theme: Theme,
    system_dark: Option<bool>,
    watcher: Option<dark_light::Watcher>,
    last_theme_check: Instant,
    settings_open: bool,
    about_open: bool,
}

impl RenameRxApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);

        let settings = Settings::load();
        let system_dark = settings::detect_system_dark();
        let theme = settings.theme.resolve(system_dark);
        theme::set(&cc.egui_ctx, theme);

        let watcher = if settings.theme == ThemePreference::System {
            settings::watch_system_theme()
        } else {
            None
        };

        Self {
            settings,
            theme,
            system_dark,
            watcher,
            last_theme_check: Instant::now(),
            settings_open: false,
            about_open: false,
        }
    }

    fn palette(&self) -> Palette {
        Palette::for_theme(self.theme)
    }

    fn poll_system_theme(&mut self, ctx: &egui::Context) {
        if self.settings.theme != ThemePreference::System {
            self.watcher = None;
            return;
        }

        let mut changed = false;

        if let Some(watcher) = &self.watcher {
            while let Ok(mode) = watcher.try_recv() {
                if let Some(dark) = settings::mode_to_dark(mode)
                    && self.system_dark != Some(dark)
                {
                    self.system_dark = Some(dark);
                    changed = true;
                }
            }
        } else if self.last_theme_check.elapsed() >= Duration::from_secs(3) {
            self.last_theme_check = Instant::now();
            let dark = settings::detect_system_dark();
            if dark != self.system_dark {
                self.system_dark = dark;
                changed = true;
            }
        }

        if changed {
            let theme = self.settings.theme.resolve(self.system_dark);
            if theme != self.theme {
                self.theme = theme;
                theme::set(ctx, theme);
            }
        }

        // Poll the portal watcher, or retry detection when the portal is absent.
        ctx.request_repaint_after(Duration::from_secs(2));
    }

    fn apply_theme_preference(&mut self, ctx: &egui::Context) {
        if self.settings.theme == ThemePreference::System {
            self.system_dark = settings::detect_system_dark();
            self.watcher = settings::watch_system_theme();
            self.last_theme_check = Instant::now();
        } else {
            self.watcher = None;
        }

        self.theme = self.settings.theme.resolve(self.system_dark);
        theme::set(ctx, self.theme);
        self.settings.save();
    }

    fn header(&mut self, ui: &mut Ui) {
        let palette = self.palette();

        egui::Panel::top("headerbar")
            .exact_size(theme::HEADER_HEIGHT)
            .show_separator_line(false)
            .frame(
                Frame::new()
                    .fill(palette.header)
                    .inner_margin(Margin::symmetric(12, 7)),
            )
            .show(ui, |ui| {
                let panel_rect = ui.clip_rect();

                ui.horizontal(|ui| {
                    let add_files = ui.add_enabled(
                        false,
                        Button::new(RichText::new("Add Files").color(palette.on_accent))
                            .fill(palette.accent)
                            .corner_radius(6)
                            .min_size(vec2(104.0, 32.0)),
                    );
                    add_files
                        .on_disabled_hover_text("File selection arrives in the next milestone");

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let menu = egui::containers::menu::MenuButton::from_button(
                            Button::new(RichText::new("≡").size(19.0)).frame(false),
                        )
                        .ui(ui, |ui| {
                            ui.set_min_width(190.0);

                            if ui.button("Settings").clicked() {
                                self.settings_open = true;
                                ui.close();
                            }

                            ui.separator();

                            if ui.button("About Rename RX").clicked() {
                                self.about_open = true;
                                ui.close();
                            }
                        });
                        menu.0.on_hover_text("Main menu");

                        let validity = ui.add_enabled(false, Button::new("Validity"));
                        validity.on_disabled_hover_text("Available once files are added");
                    });
                });

                ui.painter().text(
                    panel_rect.center(),
                    Align2::CENTER_CENTER,
                    "Rename RX",
                    FontId::new(15.0, semibold()),
                    palette.text,
                );
                ui.painter().hline(
                    panel_rect.x_range(),
                    panel_rect.bottom() - 0.5,
                    Stroke::new(1.0, palette.border),
                );
            });
    }

    fn footer(&mut self, ui: &mut Ui) {
        let palette = self.palette();

        egui::Panel::bottom("footer")
            .exact_size(theme::FOOTER_HEIGHT)
            .show_separator_line(false)
            .frame(
                Frame::new()
                    .fill(palette.header)
                    .inner_margin(Margin::symmetric(16, 10)),
            )
            .show(ui, |ui| {
                let panel_rect = ui.clip_rect();
                let content_rect = ui.max_rect();

                ui.painter().hline(
                    panel_rect.x_range(),
                    panel_rect.top() + 0.5,
                    Stroke::new(1.0, palette.border),
                );

                ui.painter().text(
                    pos2(content_rect.left(), content_rect.center().y),
                    Align2::LEFT_CENTER,
                    "0 files — 0 selected",
                    FontId::proportional(13.0),
                    palette.secondary,
                );

                let rename_rect = Rect::from_center_size(content_rect.center(), vec2(168.0, 32.0));
                ui.add_enabled_ui(false, |ui| {
                    let response = ui.put(
                        rename_rect,
                        Button::new(RichText::new("Rename").color(palette.on_accent))
                            .fill(palette.accent)
                            .corner_radius(6),
                    );
                    response.on_disabled_hover_text("Add files to rename");
                });

                ui.painter().text(
                    pos2(content_rect.right(), content_rect.center().y),
                    Align2::RIGHT_CENTER,
                    "Ready",
                    FontId::proportional(12.0),
                    palette.secondary,
                );
            });
    }

    fn empty_state(&self, ui: &mut Ui) {
        let palette = self.palette();
        let top_space = ((ui.available_height() - 260.0) * 0.42).max(32.0);

        ui.add_space(top_space);
        ui.vertical_centered(|ui| {
            paint_empty_icon(ui, &palette);
            ui.add_space(20.0);

            ui.label(
                RichText::new("No files added")
                    .font(FontId::new(21.0, semibold()))
                    .color(palette.text),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Add files to start renaming.")
                    .size(14.0)
                    .color(palette.secondary),
            );
            ui.add_space(18.0);

            ui.add_enabled(
                false,
                Button::new(RichText::new("Add Files").color(palette.on_accent))
                    .fill(palette.accent)
                    .corner_radius(6)
                    .min_size(vec2(120.0, 34.0)),
            )
            .on_disabled_hover_text("File selection arrives in the next milestone");
        });
    }

    fn settings_modal(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }

        let palette = self.palette();
        let mut open = self.settings_open;
        let mut preference = self.settings.theme;

        let modal = egui::Modal::new(egui::Id::unique("settings_modal"))
            .frame(modal_frame(&palette))
            .show(ctx, |ui| {
                ui.set_width(380.0);

                ui.label(
                    RichText::new("Settings")
                        .font(FontId::new(21.0, semibold()))
                        .color(palette.text),
                );
                ui.add_space(18.0);

                ui.label(
                    RichText::new("Appearance")
                        .font(FontId::new(13.0, semibold()))
                        .color(palette.secondary),
                );
                ui.add_space(6.0);

                ui.scope(|ui| {
                    // Make radio/checkbox outlines visible: the global button style
                    // uses a filled background and no border.
                    let widgets = &mut ui.visuals_mut().widgets;
                    widgets.inactive.bg_fill = palette.background;
                    widgets.inactive.bg_stroke = Stroke::new(1.0, palette.border);
                    widgets.hovered.bg_fill = palette.background;
                    widgets.hovered.bg_stroke = Stroke::new(1.0, palette.accent);
                    widgets.active.bg_fill = palette.background;
                    widgets.active.bg_stroke = Stroke::new(1.0, palette.accent);

                    for option in ThemePreference::ALL {
                        ui.radio_value(&mut preference, option, option.label());
                    }
                });

                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "System follows the desktop's light or dark preference. \
                         If the desktop does not report one, Rename RX uses dark.",
                    )
                    .size(12.0)
                    .color(palette.secondary),
                );

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(12.0);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            Button::new(RichText::new("Done").color(palette.on_accent))
                                .fill(palette.accent)
                                .corner_radius(6)
                                .min_size(vec2(88.0, 32.0)),
                        )
                        .clicked()
                    {
                        open = false;
                    }
                });
            });

        if modal.should_close() {
            open = false;
        }

        self.settings_open = open;

        if preference != self.settings.theme {
            self.settings.theme = preference;
            self.apply_theme_preference(ctx);
        }
    }

    fn about_modal(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }

        let palette = self.palette();
        let mut open = self.about_open;

        let modal = egui::Modal::new(egui::Id::unique("about_modal"))
            .frame(modal_frame(&palette))
            .show(ctx, |ui| {
                ui.set_width(400.0);

                ui.label(
                    RichText::new("Rename RX")
                        .font(FontId::new(24.0, semibold()))
                        .color(palette.text),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                        .size(13.0)
                        .color(palette.secondary),
                );

                ui.add_space(16.0);
                ui.label(
                    RichText::new(
                        "A native batch file renamer for Linux, written in Rust \
                         with egui. Inspired by Rename X for macOS.",
                    )
                    .size(14.0)
                    .color(palette.text),
                );

                ui.add_space(12.0);
                ui.hyperlink_to(
                    "github.com/Alarez777/rename-rx",
                    "https://github.com/Alarez777/rename-rx",
                );

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(12.0);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            Button::new(RichText::new("Close").color(palette.on_accent))
                                .fill(palette.accent)
                                .corner_radius(6)
                                .min_size(vec2(88.0, 32.0)),
                        )
                        .clicked()
                    {
                        open = false;
                    }
                });
            });

        if modal.should_close() {
            open = false;
        }

        self.about_open = open;
    }
}

impl eframe::App for RenameRxApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_system_theme(&ctx);

        self.header(ui);
        self.footer(ui);

        egui::CentralPanel::default()
            .frame(Frame::new().fill(self.palette().background))
            .show(ui, |ui| {
                self.empty_state(ui);
            });

        self.settings_modal(&ctx);
        self.about_modal(&ctx);
    }
}

fn modal_frame(palette: &Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .inner_margin(Margin::same(24))
        .corner_radius(CornerRadius::same(12))
        .stroke(Stroke::new(1.0, palette.border))
        .shadow(Shadow {
            offset: [0, 8],
            blur: 32,
            spread: 0,
            color: egui::Color32::from_black_alpha(if palette.dark { 110 } else { 36 }),
        })
}

fn paint_empty_icon(ui: &mut Ui, palette: &Palette) {
    let size = vec2(88.0, 104.0);
    let (outer_rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let icon_rect = Rect::from_center_size(outer_rect.center(), size);
    let painter = ui.painter();

    painter.rect_filled(icon_rect, CornerRadius::same(14), palette.surface);
    painter.rect_stroke(
        icon_rect,
        CornerRadius::same(14),
        Stroke::new(1.5, palette.border),
        egui::StrokeKind::Inside,
    );

    // Folded top-right corner.
    let fold = 26.0;
    let right_top = icon_rect.right_top();
    let fold_points = vec![
        pos2(right_top.x - fold, right_top.y),
        pos2(right_top.x, right_top.y),
        pos2(right_top.x, right_top.y + fold),
    ];
    painter.add(egui::Shape::convex_polygon(
        fold_points,
        palette.header,
        Stroke::NONE,
    ));
    painter.line_segment(
        [
            pos2(right_top.x - fold, right_top.y),
            pos2(right_top.x - fold, right_top.y + fold),
        ],
        Stroke::new(1.5, palette.border),
    );
    painter.line_segment(
        [
            pos2(right_top.x - fold, right_top.y + fold),
            pos2(right_top.x, right_top.y + fold),
        ],
        Stroke::new(1.5, palette.border),
    );

    // Text lines.
    let left = icon_rect.left() + 18.0;
    let right = icon_rect.right() - 18.0;
    for (index, width) in [1.0_f32, 1.0, 0.6].into_iter().enumerate() {
        let y = icon_rect.top() + 50.0 + index as f32 * 13.0;
        painter.line_segment(
            [pos2(left, y), pos2(left + (right - left) * width, y)],
            Stroke::new(3.0, palette.border),
        );
    }

    // Add badge.
    let badge_center = pos2(icon_rect.right() - 4.0, icon_rect.bottom() - 4.0);
    painter.circle_filled(badge_center, 18.0, palette.accent);
    painter.line_segment(
        [badge_center - vec2(8.0, 0.0), badge_center + vec2(8.0, 0.0)],
        Stroke::new(2.5, palette.on_accent),
    );
    painter.line_segment(
        [badge_center - vec2(0.0, 8.0), badge_center + vec2(0.0, 8.0)],
        Stroke::new(2.5, palette.on_accent),
    );
}
