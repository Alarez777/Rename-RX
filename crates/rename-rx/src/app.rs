use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local};
use eframe::egui::{
    self, Align, Align2, Button, Color32, CornerRadius, FontId, Frame, Key, LayerId, Layout,
    Margin, Modifiers, Order, Rect, RichText, Sense, Shadow, Stroke, StrokeKind, Theme, Ui, pos2,
    vec2,
};
use egui_extras::{Column, TableBuilder};
use rename_rx_core::FileEntry;

use crate::settings::{self, Settings, ThemePreference};
use crate::theme::{self, Palette, semibold};

const ROW_HEIGHT: f32 = 30.0;
const TABLE_HEADER_HEIGHT: f32 = 30.0;

pub struct RenameRxApp {
    settings: Settings,
    theme: Theme,
    system_dark: Option<bool>,
    watcher: Option<dark_light::Watcher>,
    last_theme_check: Instant,

    files: Vec<FileEntry>,
    next_file_id: u64,
    selected: BTreeSet<u64>,
    selection_anchor: Option<usize>,
    status: String,
    add_files_requested: bool,

    settings_open: bool,
    about_open: bool,
}

impl RenameRxApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_paths: Vec<PathBuf>) -> Self {
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

        let mut app = Self {
            settings,
            theme,
            system_dark,
            watcher,
            last_theme_check: Instant::now(),
            files: Vec::new(),
            next_file_id: 0,
            selected: BTreeSet::new(),
            selection_anchor: None,
            status: "Ready".to_owned(),
            add_files_requested: false,
            settings_open: false,
            about_open: false,
        };

        app.add_paths(initial_paths);
        app
    }

    fn palette(&self) -> Palette {
        Palette::for_theme(self.theme)
    }

    // ---------------------------------------------------------------- theme

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

    // ---------------------------------------------------------------- files

    fn pick_files(&mut self) {
        let Some(paths) = rfd::FileDialog::new().set_title("Add Files").pick_files() else {
            return;
        };

        self.add_paths(paths);
    }

    fn add_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let mut seen: HashSet<PathBuf> = self.files.iter().map(|file| file.path.clone()).collect();
        let mut added = 0usize;
        let mut duplicates = 0usize;
        let mut folders = 0usize;
        let mut failed = 0usize;

        for path in paths {
            let absolute = std::path::absolute(&path).unwrap_or(path);

            if std::fs::symlink_metadata(&absolute)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false)
            {
                folders += 1;
                continue;
            }

            if !seen.insert(absolute.clone()) {
                duplicates += 1;
                continue;
            }

            match FileEntry::read(self.next_file_id, absolute.clone()) {
                Ok(file) => {
                    self.next_file_id += 1;
                    self.files.push(file);
                    added += 1;
                }
                Err(error) => {
                    failed += 1;
                    eprintln!("rename-rx: could not read {}: {error}", absolute.display());
                }
            }
        }

        if added > 0 {
            self.selected.clear();
            self.selection_anchor = None;
        }

        self.status = add_status(added, duplicates, folders, failed);
    }

    fn remove_selected(&mut self) {
        let selected = std::mem::take(&mut self.selected);
        let before = self.files.len();
        self.files.retain(|file| !selected.contains(&file.id));
        let removed = before - self.files.len();

        self.selection_anchor = None;
        if removed > 0 {
            self.status = format!("{removed} {} removed", plural(removed, "file", "files"));
        }
    }

    fn clear_files(&mut self) {
        if self.files.is_empty() {
            return;
        }

        self.files.clear();
        self.selected.clear();
        self.selection_anchor = None;
        self.status = "File list cleared".to_owned();
    }

    fn select_row(&mut self, id: u64, index: usize, modifiers: Modifiers) {
        if modifiers.shift
            && let Some(anchor) = self.selection_anchor
        {
            let (start, end) = if anchor <= index {
                (anchor, index)
            } else {
                (index, anchor)
            };

            if !modifiers.command && !modifiers.ctrl {
                self.selected.clear();
            }

            for file in self.files.iter().take(end + 1).skip(start) {
                self.selected.insert(file.id);
            }
            return;
        }

        if modifiers.command || modifiers.ctrl {
            if !self.selected.remove(&id) {
                self.selected.insert(id);
            }
            self.selection_anchor = Some(index);
        } else {
            self.selected.clear();
            self.selected.insert(id);
            self.selection_anchor = Some(index);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.settings_open || self.about_open || egui::Popup::is_any_open(ctx) {
            return;
        }

        let has_focus = ctx.memory(|memory| memory.focused().is_some());
        let (delete, select_all, escape, open) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::Delete)
                    || input.consume_key(Modifiers::NONE, Key::Backspace),
                input.consume_key(Modifiers::COMMAND, Key::A),
                input.consume_key(Modifiers::NONE, Key::Escape),
                input.consume_key(Modifiers::COMMAND, Key::O),
            )
        });

        if open {
            self.add_files_requested = true;
        }

        if escape {
            self.selected.clear();
            self.selection_anchor = None;
        }

        if !has_focus && delete && !self.selected.is_empty() {
            self.remove_selected();
        }

        if !has_focus && select_all && !self.files.is_empty() {
            self.selected = self.files.iter().map(|file| file.id).collect();
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_owned())
                .collect()
        });

        if !dropped.is_empty() {
            self.add_paths(dropped);
        }
    }

    // ---------------------------------------------------------------- header

    fn header(&mut self, ui: &mut Ui) {
        let palette = self.palette();
        let has_files = !self.files.is_empty();
        let has_selection = !self.selected.is_empty();

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
                    let add_files = ui.add(
                        Button::new(RichText::new("Add Files").color(palette.on_accent))
                            .fill(palette.accent)
                            .corner_radius(6)
                            .min_size(vec2(104.0, 32.0)),
                    );
                    if add_files
                        .on_hover_text("Add files to rename (Ctrl+O)")
                        .clicked()
                    {
                        self.add_files_requested = true;
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let menu = egui::containers::menu::MenuButton::from_button(
                            Button::new(RichText::new("≡").size(19.0)).frame(false),
                        )
                        .ui(ui, |ui| {
                            ui.set_min_width(210.0);

                            if ui
                                .add_enabled(has_selection, Button::new("Remove Selected"))
                                .on_disabled_hover_text("Select files first")
                                .clicked()
                            {
                                self.remove_selected();
                                ui.close();
                            }

                            if ui
                                .add_enabled(has_files, Button::new("Clear List"))
                                .clicked()
                            {
                                self.clear_files();
                                ui.close();
                            }

                            ui.separator();

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
                        validity.on_disabled_hover_text(if has_files {
                            "Available with the rule engine"
                        } else {
                            "Add files first"
                        });
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

    // ---------------------------------------------------------------- table

    fn central(&mut self, ui: &mut Ui) {
        let has_files = !self.files.is_empty();

        egui::CentralPanel::default()
            .frame(Frame::new().fill(self.palette().background))
            .show(ui, |ui| {
                if has_files {
                    self.file_table(ui);
                } else {
                    self.empty_state(ui);
                }
            });
    }

    fn file_table(&mut self, ui: &mut Ui) {
        let palette = self.palette();
        let modifiers = ui.input(|input| input.modifiers);
        let mut clicked: Option<(u64, usize)> = None;

        {
            let files = &self.files;
            let selected = &self.selected;

            TableBuilder::new(ui)
                .striped(false)
                .resizable(false)
                .sense(Sense::click())
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::exact(42.0))
                .column(Column::initial(280.0).at_least(150.0).clip(true))
                .column(Column::exact(78.0))
                .column(Column::remainder().at_least(160.0).clip(true))
                .column(Column::exact(104.0))
                .header(TABLE_HEADER_HEIGHT, |mut header| {
                    header.col(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            header_label(ui, "#", &palette);
                        });
                    });
                    header.col(|ui| {
                        header_label(ui, "Original File Name", &palette);
                    });
                    header.col(|ui| {
                        header_label(ui, "Type", &palette);
                    });
                    header.col(|ui| {
                        header_label(ui, "Preview File Name", &palette);
                    });
                    header.col(|ui| {
                        header_label(ui, "Created", &palette);
                    });
                })
                .body(|body| {
                    body.rows(ROW_HEIGHT, files.len(), |mut row| {
                        let index = row.index();
                        let file = &files[index];
                        let is_selected = selected.contains(&file.id);

                        row.set_selected(is_selected);

                        row.col(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new((index + 1).to_string())
                                        .size(12.0)
                                        .color(palette.secondary),
                                );
                            });
                        });

                        row.col(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&file.original_name).color(palette.text),
                                )
                                .truncate(),
                            );
                        });

                        row.col(|ui| {
                            paint_type_cell(ui, &file.extension, &palette);
                        });

                        row.col(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&file.original_name).color(palette.secondary),
                                )
                                .truncate(),
                            );
                        });

                        row.col(|ui| {
                            ui.label(
                                RichText::new(format_date(file.display_time()))
                                    .size(12.5)
                                    .color(palette.secondary),
                            );
                        });

                        let response = row.response();
                        if response.clicked() {
                            clicked = Some((file.id, index));
                        }
                        response.on_hover_text(file.path.display().to_string());
                    });
                });
        }

        if let Some((id, index)) = clicked {
            self.select_row(id, index, modifiers);
        }
    }

    fn empty_state(&mut self, ui: &mut Ui) {
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

            let add_files = ui.add(
                Button::new(RichText::new("Add Files").color(palette.on_accent))
                    .fill(palette.accent)
                    .corner_radius(6)
                    .min_size(vec2(120.0, 34.0)),
            );
            if add_files
                .on_hover_text("Add files to rename (Ctrl+O)")
                .clicked()
            {
                self.add_files_requested = true;
            }
        });
    }

    // ---------------------------------------------------------------- footer

    fn footer(&mut self, ui: &mut Ui) {
        let palette = self.palette();
        let file_count = self.files.len();
        let selected_count = self.selected.len();
        let count_text = if file_count == 0 {
            "0 files — No files selected".to_owned()
        } else if selected_count == 0 {
            format!(
                "{file_count} {} — No files selected",
                plural(file_count, "file", "files")
            )
        } else {
            format!(
                "{file_count} {} — {selected_count} selected",
                plural(file_count, "file", "files")
            )
        };

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
                    &count_text,
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
                    response.on_disabled_hover_text("The rename engine arrives in Phase 3");
                });

                ui.painter().text(
                    pos2(content_rect.right(), content_rect.center().y),
                    Align2::RIGHT_CENTER,
                    &self.status,
                    FontId::proportional(12.0),
                    palette.secondary,
                );
            });
    }

    // ------------------------------------------------------------- dialogs

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

    fn drag_overlay(&self, ctx: &egui::Context) {
        let hovering = ctx.input(|input| !input.raw.hovered_files.is_empty());
        if !hovering {
            return;
        }

        let palette = self.palette();
        let rect = ctx.content_rect().shrink(10.0);
        let painter = ctx.layer_painter(LayerId::new(
            Order::Foreground,
            egui::Id::unique("drop_overlay"),
        ));

        painter.rect_filled(rect, CornerRadius::same(12), Color32::from_black_alpha(150));
        painter.rect_stroke(
            rect,
            CornerRadius::same(12),
            Stroke::new(2.0, palette.accent),
            StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Drop files to add them",
            FontId::new(20.0, semibold()),
            Color32::WHITE,
        );
    }
}

impl eframe::App for RenameRxApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        self.poll_system_theme(&ctx);
        self.handle_shortcuts(&ctx);
        self.handle_dropped_files(&ctx);

        self.header(ui);
        self.footer(ui);
        self.central(ui);

        self.settings_modal(&ctx);
        self.about_modal(&ctx);
        self.drag_overlay(&ctx);

        if self.add_files_requested {
            self.add_files_requested = false;
            self.pick_files();
            ctx.request_repaint();
        }
    }
}

fn add_status(added: usize, duplicates: usize, folders: usize, failed: usize) -> String {
    let mut parts = Vec::new();

    if added > 0 {
        parts.push(format!("{added} {} added", plural(added, "file", "files")));
    }
    if duplicates > 0 {
        parts.push(format!(
            "{duplicates} {} skipped",
            plural(duplicates, "duplicate", "duplicates")
        ));
    }
    if folders > 0 {
        parts.push(format!(
            "{folders} {} ignored",
            plural(folders, "folder", "folders")
        ));
    }
    if failed > 0 {
        parts.push(format!("{failed} could not be read"));
    }

    if parts.is_empty() {
        "No files added".to_owned()
    } else {
        parts.join(" · ")
    }
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

fn format_date(time: Option<SystemTime>) -> String {
    time.map(|time| {
        let datetime: DateTime<Local> = time.into();
        datetime.format("%d/%m/%Y").to_string()
    })
    .unwrap_or_else(|| "—".to_owned())
}

fn header_label(ui: &mut Ui, text: &str, palette: &Palette) {
    ui.label(
        RichText::new(text)
            .font(FontId::new(12.5, semibold()))
            .color(palette.secondary),
    );
}

fn paint_type_cell(ui: &mut Ui, extension: &str, palette: &Palette) {
    let (icon_rect, _) = ui.allocate_exact_size(vec2(15.0, 19.0), Sense::hover());
    let icon_rect = icon_rect.shrink2(vec2(1.0, 1.0));
    let painter = ui.painter();
    painter.rect_filled(icon_rect, CornerRadius::same(3), palette.stripe);
    painter.rect_stroke(
        icon_rect,
        CornerRadius::same(3),
        Stroke::new(1.0, palette.border),
        StrokeKind::Inside,
    );

    let fold = 5.0;
    painter.line_segment(
        [
            pos2(icon_rect.right() - fold, icon_rect.top()),
            pos2(icon_rect.right() - fold, icon_rect.top() + fold),
        ],
        Stroke::new(1.0, palette.border),
    );
    painter.line_segment(
        [
            pos2(icon_rect.right() - fold, icon_rect.top() + fold),
            pos2(icon_rect.right(), icon_rect.top() + fold),
        ],
        Stroke::new(1.0, palette.border),
    );

    let label = if extension.is_empty() {
        "file".to_owned()
    } else {
        extension.to_lowercase()
    };
    ui.label(RichText::new(label).size(12.0).color(palette.secondary));
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
            color: Color32::from_black_alpha(if palette.dark { 110 } else { 36 }),
        })
}

fn paint_empty_icon(ui: &mut Ui, palette: &Palette) {
    let size = vec2(88.0, 104.0);
    let (outer_rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let icon_rect = Rect::from_center_size(outer_rect.center(), size);
    let painter = ui.painter();

    painter.rect_filled(icon_rect, CornerRadius::same(14), palette.surface);
    painter.rect_stroke(
        icon_rect,
        CornerRadius::same(14),
        Stroke::new(1.5, palette.border),
        StrokeKind::Inside,
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
