use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local};
use eframe::egui::{
    self, Align, Align2, Button, Color32, ComboBox, CornerRadius, CursorIcon, FontFamily, FontId,
    Frame, Key, LayerId, Layout, Margin, Modifiers, Order, Rect, ResizeDirection, RichText, Sense,
    Shadow, Stroke, StrokeKind, TextEdit, Theme, Ui, ViewportCommand, pos2, vec2,
};
use egui_extras::{Column, TableBuilder};
use rename_rx_core::{
    CaseMode, CaseRule, ExtensionRule, FileEntry, FileId, InsertAnchor, InsertRule, NewNameRule,
    RegexRule, RemoveOrigin, RemoveRule, RenameOutcome, RenameStep, ReplaceRule, RuleSet,
    SerializePosition, SerializeRule, Severity, SortMethod, SortRule, TextRule, ValidationReport,
    build_preview, execute as execute_rename, plan_steps,
};

use crate::settings::{self, Settings, ThemePreference, WindowControlsStyle};
use crate::theme::{self, Palette, semibold};

const ROW_HEIGHT: f32 = 30.0;
const TABLE_HEADER_HEIGHT: f32 = 30.0;
const RULE_ROW_HEIGHT: f32 = 34.0;
const BEFORE_AFTER_WIDTH: f32 = 96.0;
const SPINBOX_WIDTH: f32 = 72.0;

#[derive(Clone, Copy)]
struct RuleColumns {
    name: f32,
    controls: f32,
    settings: f32,
    gap: f32,
}

#[derive(Default)]
struct PreviewCache {
    order: Vec<usize>,
    previews: Vec<String>,
    report: ValidationReport,
    dirty: bool,
}

struct NemoNotice {
    text: String,
    error: bool,
}

impl PreviewCache {
    fn new() -> Self {
        Self {
            dirty: true,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuleSettings {
    Replace,
    Regex,
    Serialize,
}

#[derive(Clone, Copy)]
enum WindowControlIcon {
    Close,
    Minimize,
    Maximize(bool),
}

impl RuleSettings {
    fn title(self) -> &'static str {
        match self {
            Self::Replace => "Replace Options",
            Self::Regex => "Regular Expression Options",
            Self::Serialize => "Serialize Options",
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            Self::Replace => "Replace options",
            Self::Regex => "Regular expression options",
            Self::Serialize => "Serialize options",
        }
    }
}

pub struct RenameRxApp {
    settings: Settings,
    theme: Theme,
    system_dark: Option<bool>,
    watcher: Option<dark_light::Watcher>,
    last_theme_check: Instant,

    rules: RuleSet,
    files: Vec<FileEntry>,
    next_file_id: u64,
    selected: BTreeSet<u64>,
    selection_anchor: Option<usize>,
    preview: PreviewCache,
    status: String,
    add_files_requested: bool,
    rename_requested: bool,
    last_rename: Option<Vec<RenameOutcome>>,
    confirm_rename_open: bool,
    rename_error: Option<String>,

    settings_open: bool,
    about_open: bool,
    validation_open: bool,
    rule_settings: Option<RuleSettings>,
    nemo_notice: Option<NemoNotice>,
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
            rules: RuleSet::default(),
            files: Vec::new(),
            next_file_id: 0,
            selected: BTreeSet::new(),
            selection_anchor: None,
            preview: PreviewCache::new(),
            status: "Ready".to_owned(),
            add_files_requested: false,
            rename_requested: false,
            last_rename: None,
            confirm_rename_open: false,
            rename_error: None,
            settings_open: false,
            about_open: false,
            validation_open: false,
            rule_settings: None,
            nemo_notice: None,
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

        self.preview.dirty = true;
        self.status = add_status(added, duplicates, folders, failed);
    }

    fn remove_selected(&mut self) {
        let selected = std::mem::take(&mut self.selected);
        let before = self.files.len();
        self.files.retain(|file| !selected.contains(&file.id));
        let removed = before - self.files.len();

        self.selection_anchor = None;
        self.preview.dirty = true;
        self.last_rename = None;
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
        self.preview.dirty = true;
        self.last_rename = None;
        self.status = "File list cleared".to_owned();
    }

    fn select_row(&mut self, id: FileId, display_index: usize, modifiers: Modifiers) {
        if modifiers.shift
            && let Some(anchor) = self.selection_anchor
        {
            let (start, end) = if anchor <= display_index {
                (anchor, display_index)
            } else {
                (display_index, anchor)
            };

            let ids: Vec<FileId> = self
                .preview
                .order
                .get(start..=end)
                .unwrap_or_default()
                .iter()
                .filter_map(|&file_index| self.files.get(file_index).map(|file| file.id))
                .collect();

            if !modifiers.command && !modifiers.ctrl {
                self.selected.clear();
            }

            self.selected.extend(ids);
            return;
        }

        if modifiers.command || modifiers.ctrl {
            if !self.selected.remove(&id) {
                self.selected.insert(id);
            }
            self.selection_anchor = Some(display_index);
        } else {
            self.selected.clear();
            self.selected.insert(id);
            self.selection_anchor = Some(display_index);
        }
    }

    fn select_file_by_id(&mut self, id: FileId) {
        if let Some(position) = self
            .preview
            .order
            .iter()
            .position(|&index| self.files.get(index).is_some_and(|file| file.id == id))
        {
            self.selected.clear();
            self.selected.insert(id);
            self.selection_anchor = Some(position);
        }
    }

    fn recompute_preview(&mut self) {
        let result = build_preview(&self.files, &self.rules);
        self.preview.order = result.order;
        self.preview.previews = result.previews;
        self.preview.report = result.report;
        self.preview.dirty = false;
    }

    fn handle_rename_request(&mut self) {
        if self.files.is_empty() {
            return;
        }

        if self.preview.report.errors() > 0 {
            self.validation_open = true;
            return;
        }

        if self.preview.report.warnings() > 0 {
            self.confirm_rename_open = true;
            return;
        }

        self.perform_rename();
    }

    fn perform_rename(&mut self) {
        let steps = plan_steps(&self.files, &self.preview.previews, &self.preview.order);

        if steps.is_empty() {
            self.status = "Nothing to rename".to_owned();
            return;
        }

        match execute_rename(&steps) {
            Ok(outcomes) => {
                let renamed = outcomes.len();
                let failures = self.reload_from_outcomes(&outcomes);
                self.last_rename = Some(outcomes);
                self.status = if failures == 0 {
                    format!("{renamed} {} renamed", plural(renamed, "file", "files"))
                } else {
                    format!("{renamed} renamed; {failures} could not be reloaded")
                };
            }
            Err(error) => {
                self.status = "Rename failed".to_owned();
                self.rename_error = Some(error.to_string());
            }
        }
    }

    fn undo_last_rename(&mut self) {
        let Some(outcomes) = self.last_rename.take() else {
            return;
        };

        let steps: Vec<RenameStep> = outcomes
            .iter()
            .map(|outcome| RenameStep {
                from: outcome.renamed.clone(),
                to: outcome.original.clone(),
            })
            .collect();

        match execute_rename(&steps) {
            Ok(reverse) => {
                self.reload_from_outcomes(&reverse);
                self.status = format!(
                    "{} {} restored",
                    reverse.len(),
                    plural(reverse.len(), "file", "files")
                );
            }
            Err(error) => {
                self.last_rename = Some(outcomes);
                self.status = "Undo failed".to_owned();
                self.rename_error = Some(error.to_string());
            }
        }
    }

    /// Re-read the batch after a rename and return how many entries could not
    /// be reloaded.
    fn reload_from_outcomes(&mut self, outcomes: &[RenameOutcome]) -> usize {
        let renamed: HashMap<PathBuf, PathBuf> = outcomes
            .iter()
            .map(|outcome| (outcome.original.clone(), outcome.renamed.clone()))
            .collect();

        let old_files = std::mem::take(&mut self.files);
        let mut new_files = Vec::with_capacity(old_files.len());
        let mut failures = 0;

        for file in old_files {
            let path = renamed
                .get(&file.path)
                .cloned()
                .unwrap_or_else(|| file.path.clone());
            match FileEntry::read(file.id, path) {
                Ok(entry) => new_files.push(entry),
                Err(error) => {
                    failures += 1;
                    eprintln!(
                        "rename-rx: could not reload {}: {error}",
                        file.path.display()
                    );
                }
            }
        }

        self.files = new_files;
        self.selected.clear();
        self.selection_anchor = None;
        self.preview.dirty = true;
        failures
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.settings_open
            || self.about_open
            || self.validation_open
            || self.rule_settings.is_some()
            || self.confirm_rename_open
            || self.rename_error.is_some()
            || egui::Popup::is_any_open(ctx)
        {
            return;
        }

        let has_focus = ctx.egui_wants_keyboard_input();
        let (delete, select_all, escape, open, undo, rename) = ctx.input_mut(|input| {
            let delete = !has_focus
                && (input.consume_key(Modifiers::NONE, Key::Delete)
                    || input.consume_key(Modifiers::NONE, Key::Backspace));
            let select_all = !has_focus && input.consume_key(Modifiers::COMMAND, Key::A);
            let escape = !has_focus && input.consume_key(Modifiers::NONE, Key::Escape);
            let open = input.consume_key(Modifiers::COMMAND, Key::O);
            let undo = !has_focus && input.consume_key(Modifiers::COMMAND, Key::Z);
            let rename = !has_focus && input.consume_key(Modifiers::COMMAND, Key::Enter);

            (delete, select_all, escape, open, undo, rename)
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

        if !has_focus && undo {
            self.undo_last_rename();
        }

        if !has_focus && rename {
            self.rename_requested = true;
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
        let errors = self.preview.report.errors();
        let warnings = self.preview.report.warnings();
        let issue_count = errors + warnings;
        let can_undo = self.last_rename.is_some();

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
                let custom_chrome =
                    self.settings.window_controls.resolved() == WindowControlsStyle::Adwaita;
                let mut drag_left = panel_rect.left();
                let mut drag_right = panel_rect.right();

                ui.horizontal(|ui| {
                    let add_files = ui.add(
                        Button::new(RichText::new("Add Files").color(palette.on_accent))
                            .fill(palette.accent)
                            .corner_radius(6)
                            .min_size(vec2(104.0, 32.0)),
                    );
                    drag_left = add_files.rect.right() + 8.0;
                    if add_files
                        .on_hover_text("Add files to rename (Ctrl+O)")
                        .clicked()
                    {
                        self.add_files_requested = true;
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let window_style = self.settings.window_controls.resolved();
                        if window_style == WindowControlsStyle::Adwaita {
                            let maximized = ui
                                .ctx()
                                .input(|input| input.viewport().maximized.unwrap_or(false));

                            if window_control_button(ui, &palette, WindowControlIcon::Close) {
                                ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                            }
                            if window_control_button(
                                ui,
                                &palette,
                                WindowControlIcon::Maximize(maximized),
                            ) {
                                ui.ctx()
                                    .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                            }
                            if window_control_button(ui, &palette, WindowControlIcon::Minimize) {
                                ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
                            }
                        }

                        let menu = egui::containers::menu::MenuButton::from_button(
                            Button::new(RichText::new("≡").size(19.0)).frame(false),
                        )
                        .ui(ui, |ui| {
                            ui.set_min_width(210.0);

                            if ui
                                .add_enabled(can_undo, Button::new("Undo Rename"))
                                .on_hover_text("Ctrl+Z")
                                .on_disabled_hover_text("No rename to undo")
                                .clicked()
                            {
                                self.undo_last_rename();
                                ui.close();
                            }

                            ui.separator();

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

                        let validity_label = if issue_count == 0 {
                            "Validity".to_owned()
                        } else {
                            format!("Validity ({issue_count})")
                        };
                        let validity_color = if errors > 0 {
                            palette.danger
                        } else if warnings > 0 {
                            palette.warning
                        } else {
                            palette.text
                        };

                        let validity = ui.add_enabled(
                            has_files,
                            Button::new(RichText::new(validity_label).color(validity_color)),
                        );
                        if validity.clicked() {
                            self.validation_open = true;
                        }
                        drag_right = validity.rect.left() - 8.0;
                        if has_files {
                            validity.on_hover_text(format!(
                                "{errors} {}, {warnings} {}",
                                plural(errors, "error", "errors"),
                                plural(warnings, "warning", "warnings")
                            ));
                        } else {
                            validity.on_disabled_hover_text("Add files first");
                        }
                    });
                });

                if custom_chrome {
                    let drag_rect = Rect::from_min_max(
                        pos2(drag_left, panel_rect.top() + 6.0),
                        pos2(drag_right, panel_rect.bottom()),
                    );
                    let drag_response = ui.interact(
                        drag_rect,
                        egui::Id::unique("titlebar_drag"),
                        Sense::click_and_drag(),
                    );
                    if drag_response.drag_started() {
                        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                    }
                    if drag_response.double_clicked() {
                        let maximized = ui
                            .ctx()
                            .input(|input| input.viewport().maximized.unwrap_or(false));
                        ui.ctx()
                            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }
                }

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

    // ---------------------------------------------------------------- rules

    fn rules_panel(&mut self, ui: &mut Ui) -> bool {
        let palette = self.palette();
        let content_height = ui.ctx().content_rect().height();
        let default_height = (content_height * 0.56).clamp(220.0, 480.0);
        let max_height = (content_height * 0.72).max(240.0);
        let mut changed = false;
        let mut open_settings = self.rule_settings;

        egui::Panel::top("rules_panel")
            .resizable(true)
            .default_size(default_height)
            .min_size(110.0)
            .max_size(max_height)
            .show_separator_line(false)
            .frame(
                Frame::new()
                    .fill(palette.background)
                    .inner_margin(Margin::symmetric(12, 6)),
            )
            .show(ui, |ui| {
                let panel_rect = ui.clip_rect();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        style_controls(ui, &palette);
                        ui.spacing_mut().item_spacing.y = 4.0;

                        let regular_name_width = ui
                            .painter()
                            .layout_no_wrap(
                                "Regular Expression".to_owned(),
                                FontId::new(13.5, FontFamily::Proportional),
                                palette.text,
                            )
                            .size()
                            .x;
                        let semibold_name_width = ui
                            .painter()
                            .layout_no_wrap(
                                "Regular Expression".to_owned(),
                                FontId::new(13.5, semibold()),
                                palette.text,
                            )
                            .size()
                            .x;
                        let name_width = regular_name_width.max(semibold_name_width);
                        let gap = 8.0;
                        let name_column = (ui.spacing().icon_width
                            + ui.spacing().icon_spacing
                            + ui.spacing().item_spacing.x
                            + name_width
                            + 8.0)
                            .ceil();
                        let settings_column = 24.0;
                        let available = ui.available_width();
                        let controls_column =
                            (available - name_column - settings_column - 2.0 * gap).max(120.0);
                        let columns = RuleColumns {
                            name: name_column,
                            controls: controls_column,
                            settings: settings_column,
                            gap,
                        };

                        let RuleSet {
                            new_name,
                            replace,
                            regex,
                            insert,
                            remove,
                            serialize,
                            prefix,
                            suffix,
                            extension,
                            case,
                            sort,
                        } = &mut self.rules;

                        let NewNameRule { enabled, value } = new_name;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "New Name",
                            enabled,
                            None,
                            &mut open_settings,
                            |ui| text_edit_fill(ui, value, "New base name"),
                        );

                        let ReplaceRule {
                            enabled,
                            find,
                            replace,
                            case_sensitive: _,
                            whole_word: _,
                        } = replace;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "Replace",
                            enabled,
                            Some(RuleSettings::Replace),
                            &mut open_settings,
                            |ui| two_text_fields(ui, find, "Find", replace, "Replace"),
                        );

                        let RegexRule {
                            enabled,
                            pattern,
                            replacement,
                            case_insensitive: _,
                        } = regex;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "Regular Expression",
                            enabled,
                            Some(RuleSettings::Regex),
                            &mut open_settings,
                            |ui| {
                                two_text_fields(ui, pattern, "Pattern", replacement, "Replacement")
                            },
                        );

                        let InsertRule {
                            enabled,
                            text,
                            position,
                            anchor,
                        } = insert;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "Insert",
                            enabled,
                            None,
                            &mut open_settings,
                            |ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let mut row_changed =
                                        spinbox_u32(ui, position, 0..=999, SPINBOX_WIDTH);
                                    row_changed |= combo_insert_anchor(ui, anchor);
                                    let text_width = ui.available_width().max(80.0);
                                    row_changed |=
                                        text_edit_width(ui, text, text_width, "Text to insert");
                                    row_changed
                                })
                                .inner
                            },
                        );

                        let RemoveRule {
                            enabled,
                            count,
                            origin,
                            position,
                        } = remove;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "Remove",
                            enabled,
                            None,
                            &mut open_settings,
                            |ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let mut row_changed =
                                        spinbox_u32(ui, position, 0..=999, SPINBOX_WIDTH);
                                    row_changed |= combo_remove_origin(ui, origin);
                                    row_changed |= rtl_labeled_spinbox_u32(
                                        ui,
                                        "Characters",
                                        count,
                                        1..=999,
                                        SPINBOX_WIDTH,
                                    );
                                    row_changed
                                })
                                .inner
                            },
                        );

                        let SerializeRule {
                            enabled,
                            start,
                            step: _,
                            offset,
                            padding: _,
                            position,
                            prefix: serialize_prefix,
                            suffix: serialize_suffix,
                        } = serialize;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "Serialize",
                            enabled,
                            Some(RuleSettings::Serialize),
                            &mut open_settings,
                            |ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let mut row_changed =
                                        spinbox_u32(ui, offset, 0..=999_999, SPINBOX_WIDTH);
                                    row_changed |= combo_serialize_position(
                                        ui,
                                        position,
                                        serialize_prefix,
                                        serialize_suffix,
                                    );
                                    row_changed |= rtl_labeled_spinbox_u32(
                                        ui,
                                        "Start",
                                        start,
                                        0..=999_999,
                                        SPINBOX_WIDTH,
                                    );
                                    row_changed
                                })
                                .inner
                            },
                        );

                        let TextRule { enabled, text } = prefix;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "Prefix",
                            enabled,
                            None,
                            &mut open_settings,
                            |ui| text_edit_fill(ui, text, "Prefix"),
                        );

                        let TextRule { enabled, text } = suffix;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "Suffix",
                            enabled,
                            None,
                            &mut open_settings,
                            |ui| text_edit_fill(ui, text, "Suffix"),
                        );

                        let ExtensionRule { enabled, value } = extension;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "Extension",
                            enabled,
                            None,
                            &mut open_settings,
                            |ui| text_edit_fill(ui, value, "e.g. jpg"),
                        );

                        let CaseRule { enabled, mode } = case;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "Case",
                            enabled,
                            None,
                            &mut open_settings,
                            |ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    combo_case_mode(ui, mode)
                                })
                                .inner
                            },
                        );

                        let SortRule {
                            enabled,
                            method,
                            descending,
                        } = sort;
                        changed |= rule_row(
                            ui,
                            &palette,
                            columns,
                            "Sort",
                            enabled,
                            None,
                            &mut open_settings,
                            |ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let mut row_changed =
                                        ui.checkbox(descending, "Descending").changed();
                                    row_changed |= combo_sort_method(ui, method);
                                    row_changed
                                })
                                .inner
                            },
                        );
                    });

                ui.painter().hline(
                    panel_rect.x_range(),
                    panel_rect.bottom() - 0.5,
                    Stroke::new(1.0, palette.border),
                );
            });

        self.rule_settings = open_settings;
        changed
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
        let mut clicked: Option<(FileId, usize)> = None;

        // The table is for selection, not for selecting text. This also avoids
        // the I-beam cursor and text-selection mode stealing clicks from rows.
        ui.style_mut().interaction.selectable_labels = false;

        {
            let files = &self.files;
            let selected = &self.selected;
            let order = &self.preview.order;
            let previews = &self.preview.previews;

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
                    body.rows(ROW_HEIGHT, order.len(), |mut row| {
                        let display_index = row.index();
                        let Some(&file_index) = order.get(display_index) else {
                            return;
                        };
                        let Some(file) = files.get(file_index) else {
                            return;
                        };

                        let preview = previews
                            .get(file_index)
                            .map(String::as_str)
                            .unwrap_or(&file.original_name);
                        let is_selected = selected.contains(&file.id);

                        row.set_selected(is_selected);

                        row.col(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new((display_index + 1).to_string())
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
                                .truncate()
                                .selectable(false),
                            );
                        });

                        row.col(|ui| {
                            paint_type_cell(ui, &file.extension, &palette);
                        });

                        row.col(|ui| {
                            ui.add(
                                egui::Label::new(RichText::new(preview).color(palette.secondary))
                                    .truncate()
                                    .selectable(false),
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
                            clicked = Some((file.id, display_index));
                        }
                        response.on_hover_text(file.path.display().to_string());
                    });
                });
        }

        if let Some((id, display_index)) = clicked {
            self.select_row(id, display_index, modifiers);
        }
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
                RichText::new("Drop files here or use Add Files above.")
                    .size(14.0)
                    .color(palette.secondary),
            );
        });
    }

    // ---------------------------------------------------------------- footer

    fn footer(&mut self, ui: &mut Ui) {
        let palette = self.palette();
        let file_count = self.files.len();
        let selected_count = self.selected.len();
        let has_files = file_count > 0;
        let errors = self.preview.report.errors();
        let warnings = self.preview.report.warnings();
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
        let rename_hint = if errors > 0 {
            format!(
                "{errors} {} block renaming; open Validity to review them",
                plural(errors, "error", "errors")
            )
        } else if warnings > 0 {
            format!(
                "{warnings} {} to review before renaming",
                plural(warnings, "warning", "warnings")
            )
        } else {
            "Rename all files (Ctrl+Enter)".to_owned()
        };
        let mut rename_clicked = false;

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
                ui.add_enabled_ui(has_files, |ui| {
                    let response = ui.put(
                        rename_rect,
                        Button::new(RichText::new("Rename").color(palette.on_accent))
                            .fill(palette.accent)
                            .corner_radius(6),
                    );

                    if response.clicked() {
                        rename_clicked = true;
                    }

                    if has_files {
                        response.on_hover_text(rename_hint);
                    } else {
                        response.on_disabled_hover_text("Add files to rename");
                    }
                });

                ui.painter().text(
                    pos2(content_rect.right(), content_rect.center().y),
                    Align2::RIGHT_CENTER,
                    &self.status,
                    FontId::proportional(12.0),
                    palette.secondary,
                );
            });

        if rename_clicked {
            self.rename_requested = true;
        }
    }

    // ------------------------------------------------------------- dialogs

    fn settings_modal(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }

        let palette = self.palette();
        let mut open = self.settings_open;
        let mut preference = self.settings.theme;
        let mut window_controls = self.settings.window_controls;
        let nemo_state = crate::nemo::state();
        let mut nemo_notice = self.nemo_notice.take();

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

                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 230.0).max(180.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Appearance")
                                .font(FontId::new(13.0, semibold()))
                                .color(palette.secondary),
                        );
                        ui.add_space(6.0);

                        ui.scope(|ui| {
                            style_controls(ui, &palette);

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

                        ui.add_space(18.0);
                        ui.label(
                            RichText::new("Window")
                                .font(FontId::new(13.0, semibold()))
                                .color(palette.secondary),
                        );
                        ui.add_space(6.0);

                        ui.scope(|ui| {
                            style_controls(ui, &palette);
                            ComboBox::from_id_salt("window_controls_modal")
                                .selected_text(window_controls.label())
                                .width(220.0)
                                .show_ui(ui, |ui| {
                                    for style in WindowControlsStyle::ALL {
                                        ui.selectable_value(
                                            &mut window_controls,
                                            style,
                                            style.label(),
                                        );
                                    }
                                });
                        });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Automatic uses the platform default. \
                                 Windows and macOS styles will be added later.",
                            )
                            .size(12.0)
                            .color(palette.secondary),
                        );

                        ui.add_space(18.0);
                        ui.label(
                            RichText::new("File manager")
                                .font(FontId::new(13.0, semibold()))
                                .color(palette.secondary),
                        );
                        ui.add_space(6.0);

                        ui.scope(|ui| {
                            style_controls(ui, &palette);

                            match &nemo_state {
                                Some(state) if state.current => {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Nemo action installed.")
                                                .size(12.5)
                                                .color(palette.secondary),
                                        );
                                        if ui.button("Remove").clicked() {
                                            nemo_notice = Some(remove_nemo_notice());
                                        }
                                    });
                                }
                                Some(_) => {
                                    ui.horizontal(|ui| {
                                        if ui.button("Update Nemo Action").clicked() {
                                            nemo_notice = Some(install_nemo_notice());
                                        }
                                        if ui.button("Remove").clicked() {
                                            nemo_notice = Some(remove_nemo_notice());
                                        }
                                    });
                                }
                                None => {
                                    if ui.button("Add Nemo Action").clicked() {
                                        nemo_notice = Some(install_nemo_notice());
                                    }
                                }
                            }
                        });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Adds “Rename with Rename RX” to Nemo's right-click menu \
                                 for any file. The entry is shown in English or Spanish \
                                 according to the system language.",
                            )
                            .size(12.0)
                            .color(palette.secondary),
                        );

                        if let Some(notice) = &nemo_notice {
                            ui.add_space(4.0);
                            ui.label(RichText::new(&notice.text).size(12.0).color(
                                if notice.error {
                                    palette.danger
                                } else {
                                    palette.secondary
                                },
                            ));
                        }
                    });

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
        self.nemo_notice = if open { nemo_notice } else { None };

        if preference != self.settings.theme {
            self.settings.theme = preference;
            self.apply_theme_preference(ctx);
        }

        if window_controls != self.settings.window_controls {
            self.settings.window_controls = window_controls;
            self.settings.save();
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

                ui.add_space(16.0);
                ui.label(
                    RichText::new("Built with")
                        .font(FontId::new(12.0, semibold()))
                        .color(palette.secondary),
                );
                ui.add_space(4.0);
                egui::Grid::new("about_versions")
                    .num_columns(2)
                    .spacing(vec2(18.0, 2.0))
                    .show(ui, |ui| {
                        for (name, version) in [
                            ("Rust", env!("RENAME_RX_RUSTC_VERSION")),
                            ("egui", env!("RENAME_RX_EGUI_VERSION")),
                            ("eframe", env!("RENAME_RX_EFRAME_VERSION")),
                            ("egui_extras", env!("RENAME_RX_EGUI_EXTRAS_VERSION")),
                            ("winit", env!("RENAME_RX_WINIT_VERSION")),
                            ("glow", env!("RENAME_RX_GLOW_VERSION")),
                            ("regex", env!("RENAME_RX_REGEX_VERSION")),
                        ] {
                            ui.label(RichText::new(name).size(12.0).color(palette.secondary));
                            ui.label(RichText::new(version).size(12.0).color(palette.text));
                            ui.end_row();
                        }
                    });

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

    fn validation_modal(&mut self, ctx: &egui::Context) {
        if !self.validation_open {
            return;
        }

        let palette = self.palette();
        let report = &self.preview.report;
        let errors = report.errors();
        let warnings = report.warnings();
        let infos = report.infos();
        let mut open = self.validation_open;
        let mut selected: Option<FileId> = None;

        let modal = egui::Modal::new(egui::Id::unique("validation_modal"))
            .frame(modal_frame(&palette))
            .show(ctx, |ui| {
                ui.set_width(600.0);

                ui.label(
                    RichText::new("Validity")
                        .font(FontId::new(21.0, semibold()))
                        .color(palette.text),
                );
                ui.add_space(4.0);

                let summary = if report.issues.is_empty() {
                    "No issues found".to_owned()
                } else {
                    format!(
                        "{errors} {} · {warnings} {} · {infos} info",
                        plural(errors, "error", "errors"),
                        plural(warnings, "warning", "warnings")
                    )
                };
                let summary_color = if errors > 0 {
                    palette.danger
                } else if warnings > 0 {
                    palette.warning
                } else {
                    palette.secondary
                };
                ui.label(RichText::new(summary).size(13.0).color(summary_color));

                if report.issues.is_empty() {
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new("Everything looks good.")
                            .size(14.0)
                            .color(palette.secondary),
                    );
                    ui.add_space(24.0);
                } else {
                    ui.add_space(12.0);

                    egui::ScrollArea::vertical()
                        .max_height(330.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());

                            for issue in &report.issues {
                                ui.horizontal(|ui| {
                                    paint_severity_dot(ui, issue.severity, &palette);

                                    if let Some(file_id) = issue.file_id {
                                        if let Some(file) =
                                            self.files.iter().find(|file| file.id == file_id)
                                        {
                                            let response = ui.add(
                                                egui::Label::new(
                                                    RichText::new(&file.original_name)
                                                        .color(palette.accent),
                                                )
                                                .sense(Sense::click()),
                                            );
                                            if response.clicked() {
                                                selected = Some(file_id);
                                            }
                                        }
                                    } else {
                                        ui.label(
                                            RichText::new("Rule")
                                                .size(12.0)
                                                .color(palette.secondary),
                                        );
                                    }

                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&issue.message).color(palette.text),
                                        )
                                        .wrap(),
                                    );
                                });
                                ui.add_space(2.0);
                            }
                        });

                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Errors block renaming. Warnings can be reviewed and accepted.",
                        )
                        .size(12.0)
                        .color(palette.secondary),
                    );
                }

                ui.add_space(16.0);
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

        if let Some(file_id) = selected {
            self.select_file_by_id(file_id);
            open = false;
        }

        self.validation_open = open;
    }

    fn rule_settings_modal(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.rule_settings else {
            return;
        };

        let palette = self.palette();
        let mut open = true;
        let mut changed = false;

        let modal = egui::Modal::new(egui::Id::unique("rule_settings_modal"))
            .frame(modal_frame(&palette))
            .show(ctx, |ui| {
                ui.set_width(420.0);

                ui.label(
                    RichText::new(kind.title())
                        .font(FontId::new(20.0, semibold()))
                        .color(palette.text),
                );
                ui.add_space(6.0);

                let description = match kind {
                    RuleSettings::Replace => "These options apply to the Find and Replace fields.",
                    RuleSettings::Regex => "These options apply to the regular expression.",
                    RuleSettings::Serialize => {
                        "Step, padding and the text around the number. \
                         Start, After/Before and offset stay in the rule row."
                    }
                };
                ui.label(
                    RichText::new(description)
                        .size(12.5)
                        .color(palette.secondary),
                );

                ui.add_space(14.0);

                ui.scope(|ui| {
                    style_controls(ui, &palette);

                    match kind {
                        RuleSettings::Replace => {
                            changed |= ui
                                .checkbox(&mut self.rules.replace.case_sensitive, "Case sensitive")
                                .changed();
                            changed |= ui
                                .checkbox(&mut self.rules.replace.whole_word, "Whole word")
                                .changed();
                        }
                        RuleSettings::Regex => {
                            changed |= ui
                                .checkbox(&mut self.rules.regex.case_insensitive, "Ignore case")
                                .changed();
                        }
                        RuleSettings::Serialize => {
                            changed |= labeled_spinbox_u32(
                                ui,
                                "Step",
                                &mut self.rules.serialize.step,
                                1..=999,
                                SPINBOX_WIDTH,
                            );
                            ui.add_space(6.0);
                            changed |= labeled_spinbox_u32(
                                ui,
                                "Padding",
                                &mut self.rules.serialize.padding,
                                0..=12,
                                SPINBOX_WIDTH,
                            );

                            ui.add_space(12.0);
                            ui.label(RichText::new("Prefix").size(12.5).color(palette.secondary));
                            changed |=
                                text_edit_fill(ui, &mut self.rules.serialize.prefix, "Prefix");

                            ui.add_space(8.0);
                            ui.label(RichText::new("Suffix").size(12.5).color(palette.secondary));
                            changed |=
                                text_edit_fill(ui, &mut self.rules.serialize.suffix, "Suffix");

                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(
                                    "Padding is the total width of the number (3 → 001). \
                                     The default prefix is a space.",
                                )
                                .size(12.0)
                                .color(palette.secondary),
                            );
                        }
                    }
                });

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

        if !open {
            self.rule_settings = None;
        }

        if changed {
            self.preview.dirty = true;
            ctx.request_repaint();
        }
    }

    fn confirm_rename_modal(&mut self, ctx: &egui::Context) {
        if !self.confirm_rename_open {
            return;
        }

        let palette = self.palette();
        let warnings = self.preview.report.warnings();
        let mut open = self.confirm_rename_open;
        let mut confirmed = false;
        let mut review = false;

        let modal = egui::Modal::new(egui::Id::unique("confirm_rename_modal"))
            .frame(modal_frame(&palette))
            .show(ctx, |ui| {
                ui.set_width(460.0);

                ui.label(
                    RichText::new("Rename files?")
                        .font(FontId::new(21.0, semibold()))
                        .color(palette.text),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "Validity found {warnings} {} and no errors.",
                        plural(warnings, "warning", "warnings")
                    ))
                    .size(14.0)
                    .color(palette.warning),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "The names will be applied directly to the files on disk. \
                         You can undo this operation once afterwards.",
                    )
                    .size(13.0)
                    .color(palette.secondary),
                );

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(12.0);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            Button::new(RichText::new("Rename Anyway").color(palette.on_accent))
                                .fill(palette.accent)
                                .corner_radius(6)
                                .min_size(vec2(130.0, 32.0)),
                        )
                        .clicked()
                    {
                        confirmed = true;
                        open = false;
                    }

                    if ui.button("Cancel").clicked() {
                        open = false;
                    }

                    if ui.button("Review Warnings").clicked() {
                        review = true;
                        open = false;
                    }
                });
            });

        if modal.should_close() {
            open = false;
        }

        self.confirm_rename_open = open;

        if review {
            self.validation_open = true;
        }

        if confirmed {
            self.perform_rename();
        }
    }

    fn rename_error_modal(&mut self, ctx: &egui::Context) {
        let Some(message) = self.rename_error.clone() else {
            return;
        };

        let palette = self.palette();
        let mut open = true;

        let modal = egui::Modal::new(egui::Id::unique("rename_error_modal"))
            .frame(modal_frame(&palette))
            .show(ctx, |ui| {
                ui.set_width(540.0);

                ui.label(
                    RichText::new("Operation failed")
                        .font(FontId::new(21.0, semibold()))
                        .color(palette.danger),
                );
                ui.add_space(10.0);
                ui.add(
                    egui::Label::new(RichText::new(&message).size(13.5).color(palette.text)).wrap(),
                );
                ui.add_space(16.0);
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

        if !open {
            self.rename_error = None;
        }
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
        if self.settings.window_controls.resolved() == WindowControlsStyle::Adwaita {
            handle_window_resize(&ctx);
        }
        self.handle_shortcuts(&ctx);
        self.handle_dropped_files(&ctx);

        if self.preview.dirty {
            self.recompute_preview();
        }

        self.header(ui);

        if self.preview.dirty {
            self.recompute_preview();
        }

        self.footer(ui);

        let rules_changed = self.rules_panel(ui);
        if rules_changed {
            self.preview.dirty = true;
            self.selection_anchor = None;
        }
        if self.preview.dirty {
            self.recompute_preview();
        }

        self.central(ui);

        self.validation_modal(&ctx);
        self.rule_settings_modal(&ctx);
        self.confirm_rename_modal(&ctx);
        self.rename_error_modal(&ctx);
        self.settings_modal(&ctx);
        self.about_modal(&ctx);
        self.drag_overlay(&ctx);

        if self.rename_requested {
            self.rename_requested = false;
            self.handle_rename_request();
            ctx.request_repaint();
        }

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

#[allow(clippy::too_many_arguments)]
fn rule_row(
    ui: &mut Ui,
    palette: &Palette,
    columns: RuleColumns,
    name: &str,
    enabled: &mut bool,
    settings: Option<RuleSettings>,
    open_settings: &mut Option<RuleSettings>,
    controls: impl FnOnce(&mut Ui) -> bool,
) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = columns.gap;

        ui.allocate_ui_with_layout(
            vec2(columns.name, RULE_ROW_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_width(columns.name);

                changed |= ui
                    .checkbox(enabled, "")
                    .on_hover_text("Apply this rule")
                    .changed();

                let text = if *enabled {
                    RichText::new(name)
                        .size(13.5)
                        .color(palette.text)
                        .font(FontId::new(13.5, semibold()))
                } else {
                    RichText::new(name).size(13.5).color(palette.secondary)
                };
                ui.add(egui::Label::new(text).truncate());
            },
        );

        ui.allocate_ui_with_layout(
            vec2(columns.controls, RULE_ROW_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_width(columns.controls);
                changed |= controls(ui);
            },
        );

        ui.allocate_ui_with_layout(
            vec2(columns.settings, RULE_ROW_HEIGHT),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.set_width(columns.settings);

                if let Some(kind) = settings
                    && settings_button(ui, palette, kind)
                {
                    *open_settings = Some(kind);
                }
            },
        );
    });

    changed
}

fn text_edit_fill(ui: &mut Ui, value: &mut String, hint: &str) -> bool {
    let width = ui.available_width().max(60.0);
    text_edit_width(ui, value, width, hint)
}

fn text_edit_width(ui: &mut Ui, value: &mut String, width: f32, hint: &str) -> bool {
    ui.add_sized(
        vec2(width, 28.0),
        TextEdit::singleline(value).hint_text(hint.to_owned()),
    )
    .changed()
}

fn two_text_fields(
    ui: &mut Ui,
    first: &mut String,
    first_hint: &str,
    second: &mut String,
    second_hint: &str,
) -> bool {
    let gap = ui.spacing().item_spacing.x;
    let width = ((ui.available_width() - gap) / 2.0).max(60.0);

    let mut changed = text_edit_width(ui, first, width, first_hint);
    changed |= text_edit_width(ui, second, width, second_hint);
    changed
}

fn labeled_spinbox_u32(
    ui: &mut Ui,
    label: &str,
    value: &mut u32,
    range: RangeInclusive<u32>,
    width: f32,
) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            RichText::new(label)
                .size(12.5)
                .color(ui.visuals().weak_text_color()),
        );
        changed |= spinbox_u32(ui, value, range, width);
    });

    changed
}

/// Add a spinbox first and its label second. In a right-to-left parent this
/// leaves the label on the left and the number on the right.
fn rtl_labeled_spinbox_u32(
    ui: &mut Ui,
    label: &str,
    value: &mut u32,
    range: RangeInclusive<u32>,
    width: f32,
) -> bool {
    let changed = spinbox_u32(ui, value, range, width);
    ui.label(
        RichText::new(label)
            .size(12.5)
            .color(ui.visuals().weak_text_color()),
    );
    changed
}

fn spinbox_u32(ui: &mut Ui, value: &mut u32, range: RangeInclusive<u32>, width: f32) -> bool {
    let mut changed = false;
    let (min, max) = (*range.start(), *range.end());

    ui.allocate_ui_with_layout(
        vec2(width + 16.0, 28.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_width(width + 16.0);
            ui.spacing_mut().item_spacing.x = 0.0;

            changed |= ui
                .add_sized(
                    vec2(width, 28.0),
                    egui::DragValue::new(value).range(range).speed(1.0),
                )
                .changed();

            ui.allocate_ui_with_layout(vec2(16.0, 28.0), Layout::top_down(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                if spin_arrow(ui, true) {
                    *value = (*value + 1).min(max);
                    changed = true;
                }
                if spin_arrow(ui, false) {
                    *value = (*value).saturating_sub(1).max(min);
                    changed = true;
                }
            });
        },
    );

    changed
}

fn spin_arrow(ui: &mut Ui, up: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(16.0, 14.0), Sense::click());

    if response.hovered() || response.is_pointer_button_down_on() {
        let visuals = ui.style().interact(&response);
        ui.painter()
            .rect_filled(rect, CornerRadius::same(3), visuals.weak_bg_fill);
    }

    let color = ui.style().interact(&response).fg_stroke.color;
    let center = rect.center();
    let points = if up {
        vec![
            pos2(center.x, center.y - 3.0),
            pos2(center.x - 4.0, center.y + 2.0),
            pos2(center.x + 4.0, center.y + 2.0),
        ]
    } else {
        vec![
            pos2(center.x, center.y + 3.0),
            pos2(center.x - 4.0, center.y - 2.0),
            pos2(center.x + 4.0, center.y - 2.0),
        ]
    };

    ui.painter()
        .add(egui::Shape::convex_polygon(points, color, Stroke::NONE));

    response.clicked()
}

fn settings_button(ui: &mut Ui, palette: &Palette, kind: RuleSettings) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());

    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), palette.button_hover);
    } else if response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), palette.button_active);
    }

    let color = if response.hovered() {
        palette.accent
    } else {
        palette.secondary
    };
    paint_gear(ui.painter(), rect.center(), color);

    let clicked = response.clicked();
    response.on_hover_text(kind.tooltip());
    clicked
}

fn handle_window_resize(ctx: &egui::Context) {
    let Some(position) = ctx.input(|input| input.pointer.hover_pos()) else {
        return;
    };

    let rect = ctx.content_rect();
    let edge = 6.0;
    let left = position.x <= rect.left() + edge;
    let right = position.x >= rect.right() - edge;
    let top = position.y <= rect.top() + edge;
    let bottom = position.y >= rect.bottom() - edge;

    let direction = match (left, right, top, bottom) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    };

    let Some(direction) = direction else {
        return;
    };

    ctx.set_cursor_icon(match direction {
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
    });

    if ctx.input(|input| input.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}

fn window_control_button(ui: &mut Ui, palette: &Palette, icon: WindowControlIcon) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());

    if response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(7), palette.button_active);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(7), palette.button_hover);
    }

    let color = if response.hovered() || response.is_pointer_button_down_on() {
        palette.text
    } else {
        palette.secondary
    };
    let center = rect.center();
    let painter = ui.painter();
    let stroke = Stroke::new(1.6, color);

    match icon {
        WindowControlIcon::Close => {
            painter.line_segment(
                [
                    pos2(center.x - 4.0, center.y - 4.0),
                    pos2(center.x + 4.0, center.y + 4.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x - 4.0, center.y + 4.0),
                    pos2(center.x + 4.0, center.y - 4.0),
                ],
                stroke,
            );
        }
        WindowControlIcon::Minimize => {
            painter.line_segment(
                [
                    pos2(center.x - 6.0, center.y + 3.0),
                    pos2(center.x + 6.0, center.y + 3.0),
                ],
                stroke,
            );
        }
        WindowControlIcon::Maximize(maximized) => {
            if maximized {
                painter.rect_stroke(
                    Rect::from_center_size(center + vec2(-1.5, 1.5), vec2(9.0, 9.0)),
                    CornerRadius::same(1),
                    stroke,
                    StrokeKind::Inside,
                );
                painter.rect_stroke(
                    Rect::from_center_size(center + vec2(1.5, -1.5), vec2(9.0, 9.0)),
                    CornerRadius::same(1),
                    stroke,
                    StrokeKind::Inside,
                );
            } else {
                painter.rect_stroke(
                    Rect::from_center_size(center, vec2(10.0, 10.0)),
                    CornerRadius::same(1),
                    stroke,
                    StrokeKind::Inside,
                );
            }
        }
    }

    let clicked = response.clicked();
    response.on_hover_text(match icon {
        WindowControlIcon::Close => "Close",
        WindowControlIcon::Minimize => "Minimize",
        WindowControlIcon::Maximize(false) => "Maximize",
        WindowControlIcon::Maximize(true) => "Restore",
    });
    clicked
}

fn paint_gear(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    painter.circle_stroke(center, 5.0, Stroke::new(1.4, color));
    painter.circle_filled(center, 1.8, color);

    for index in 0..8 {
        let angle = std::f32::consts::TAU * index as f32 / 8.0;
        let (sin, cos) = angle.sin_cos();
        painter.line_segment(
            [
                pos2(center.x + cos * 5.0, center.y + sin * 5.0),
                pos2(center.x + cos * 7.5, center.y + sin * 7.5),
            ],
            Stroke::new(1.5, color),
        );
    }
}

fn combo_insert_anchor(ui: &mut Ui, anchor: &mut InsertAnchor) -> bool {
    let mut changed = false;
    ComboBox::from_id_salt("insert_anchor")
        .selected_text(match *anchor {
            InsertAnchor::Before => "Before",
            InsertAnchor::After => "After",
        })
        .width(BEFORE_AFTER_WIDTH)
        .show_ui(ui, |ui| {
            changed |= ui
                .selectable_value(anchor, InsertAnchor::Before, "Before")
                .changed();
            changed |= ui
                .selectable_value(anchor, InsertAnchor::After, "After")
                .changed();
        });
    changed
}

fn combo_remove_origin(ui: &mut Ui, origin: &mut RemoveOrigin) -> bool {
    let mut changed = false;
    ComboBox::from_id_salt("remove_origin")
        .selected_text(match *origin {
            RemoveOrigin::Before => "Before",
            RemoveOrigin::After => "After",
        })
        .width(BEFORE_AFTER_WIDTH)
        .show_ui(ui, |ui| {
            changed |= ui
                .selectable_value(origin, RemoveOrigin::Before, "Before")
                .changed();
            changed |= ui
                .selectable_value(origin, RemoveOrigin::After, "After")
                .changed();
        });
    changed
}

fn combo_serialize_position(
    ui: &mut Ui,
    position: &mut SerializePosition,
    prefix: &mut String,
    suffix: &mut String,
) -> bool {
    let old_position = *position;
    let mut changed = false;
    ComboBox::from_id_salt("serialize_position")
        .selected_text(match *position {
            SerializePosition::After => "After",
            SerializePosition::Before => "Before",
        })
        .width(BEFORE_AFTER_WIDTH)
        .show_ui(ui, |ui| {
            changed |= ui
                .selectable_value(position, SerializePosition::After, "After")
                .changed();
            changed |= ui
                .selectable_value(position, SerializePosition::Before, "Before")
                .changed();
        });

    if old_position != *position {
        let after_default = prefix.as_str() == " " && suffix.is_empty();
        let before_default = prefix.is_empty() && suffix.as_str() == " ";

        match (old_position, *position) {
            (SerializePosition::After, SerializePosition::Before) if after_default => {
                prefix.clear();
                suffix.clear();
                suffix.push(' ');
            }
            (SerializePosition::Before, SerializePosition::After) if before_default => {
                prefix.clear();
                prefix.push(' ');
                suffix.clear();
            }
            _ => {}
        }
    }

    changed
}

fn combo_case_mode(ui: &mut Ui, mode: &mut CaseMode) -> bool {
    let mut changed = false;
    ComboBox::from_id_salt("case_mode")
        .selected_text(match *mode {
            CaseMode::Lower => "lowercase",
            CaseMode::Title => "Title Case",
            CaseMode::Upper => "UPPERCASE",
        })
        .width(110.0)
        .show_ui(ui, |ui| {
            changed |= ui
                .selectable_value(mode, CaseMode::Lower, "lowercase")
                .changed();
            changed |= ui
                .selectable_value(mode, CaseMode::Title, "Title Case")
                .changed();
            changed |= ui
                .selectable_value(mode, CaseMode::Upper, "UPPERCASE")
                .changed();
        });
    changed
}

fn combo_sort_method(ui: &mut Ui, method: &mut SortMethod) -> bool {
    let mut changed = false;
    ComboBox::from_id_salt("sort_method")
        .selected_text(match *method {
            SortMethod::Original => "Original order",
            SortMethod::Name => "Name",
            SortMethod::Created => "Created",
            SortMethod::Modified => "Modified",
        })
        .width(120.0)
        .show_ui(ui, |ui| {
            changed |= ui
                .selectable_value(method, SortMethod::Original, "Original order")
                .changed();
            changed |= ui
                .selectable_value(method, SortMethod::Name, "Name")
                .changed();
            changed |= ui
                .selectable_value(method, SortMethod::Created, "Created")
                .changed();
            changed |= ui
                .selectable_value(method, SortMethod::Modified, "Modified")
                .changed();
        });
    changed
}

fn style_controls(ui: &mut Ui, palette: &Palette) {
    let widgets = &mut ui.visuals_mut().widgets;
    widgets.inactive.bg_fill = palette.background;
    widgets.inactive.bg_stroke = Stroke::new(1.0, palette.border);
    widgets.hovered.bg_fill = palette.background;
    widgets.hovered.bg_stroke = Stroke::new(1.0, palette.accent);
    widgets.active.bg_fill = palette.background;
    widgets.active.bg_stroke = Stroke::new(1.0, palette.accent);
}

fn paint_severity_dot(ui: &mut Ui, severity: Severity, palette: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(12.0, 18.0), Sense::hover());
    let color = match severity {
        Severity::Error => palette.danger,
        Severity::Warning => palette.warning,
        Severity::Info => palette.secondary,
    };
    ui.painter().circle_filled(rect.center(), 4.0, color);
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

fn install_nemo_notice() -> NemoNotice {
    match crate::nemo::install() {
        Ok(_) => NemoNotice {
            text: "Added to Nemo's context menu.".to_owned(),
            error: false,
        },
        Err(error) => NemoNotice {
            text: format!("Could not add the Nemo action: {error}"),
            error: true,
        },
    }
}

fn remove_nemo_notice() -> NemoNotice {
    match crate::nemo::remove() {
        Ok(()) => NemoNotice {
            text: "Removed the Nemo action.".to_owned(),
            error: false,
        },
        Err(error) => NemoNotice {
            text: format!("Could not remove the Nemo action: {error}"),
            error: true,
        },
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
