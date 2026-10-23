//! Adwaita-inspired colours, typography and widget styling for egui.

use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::egui::style::WidgetVisuals;
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, Style, TextStyle, Theme, Visuals, vec2,
};

const INTER_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
const INTER_BOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-Bold.ttf");

pub const HEADER_HEIGHT: f32 = 48.0;
pub const FOOTER_HEIGHT: f32 = 52.0;

/// Semibold family, used for titles and section headings.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Bold family, available for emphasis.
#[allow(dead_code)]
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

/// Install fonts and the light/dark styles once, at startup.
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);

    for theme in [Theme::Dark, Theme::Light] {
        ctx.set_style_of(theme, style_for(theme));
    }
}

/// Select the active theme. Both styles are already configured by [`install`].
pub fn set(ctx: &egui::Context, theme: Theme) {
    ctx.set_theme(theme);
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "inter".to_owned(),
        Arc::new(FontData::from_static(INTER_REGULAR)),
    );
    fonts.font_data.insert(
        "inter-semibold".to_owned(),
        Arc::new(FontData::from_static(INTER_SEMIBOLD)),
    );
    fonts.font_data.insert(
        "inter-bold".to_owned(),
        Arc::new(FontData::from_static(INTER_BOLD)),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());

    fonts.families.insert(
        FontFamily::Name("semibold".into()),
        vec!["inter-semibold".to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name("bold".into()),
        vec!["inter-bold".to_owned()],
    );

    ctx.set_fonts(fonts);
}

fn style_for(theme: Theme) -> Style {
    let palette = Palette::for_theme(theme);

    let mut style = Style {
        visuals: visuals(&palette),
        ..Default::default()
    };

    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.button_padding = vec2(12.0, 6.0);
    style.spacing.menu_margin = Margin::same(6);
    style.spacing.window_margin = Margin::same(16);
    style.spacing.interact_size = vec2(40.0, 28.0);
    style.spacing.icon_width = 18.0;
    style.spacing.icon_width_inner = 12.0;
    style.spacing.icon_spacing = 8.0;
    style.spacing.combo_width = 150.0;
    style.spacing.text_edit_width = 220.0;
    style.spacing.indent = 16.0;

    style.text_styles = BTreeMap::from([
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]);

    style
}

fn visuals(palette: &Palette) -> Visuals {
    let mut visuals = if palette.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.dark_mode = palette.dark;
    visuals.override_text_color = Some(palette.text);
    visuals.weak_text_color = Some(palette.secondary);
    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.surface;
    visuals.extreme_bg_color = palette.input;
    visuals.text_edit_bg_color = Some(palette.input);
    visuals.faint_bg_color = palette.stripe;
    visuals.code_bg_color = palette.header;
    visuals.hyperlink_color = palette.accent;
    visuals.warn_fg_color = palette.warning;
    visuals.error_fg_color = palette.danger;

    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.window_stroke = Stroke::new(1.0, palette.border);
    visuals.window_shadow = Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if palette.dark { 96 } else { 32 }),
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if palette.dark { 80 } else { 24 }),
    };

    visuals.selection.bg_fill = palette.selection;
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);

    visuals.widgets.noninteractive = widget(palette.surface, palette.text, palette.border);
    visuals.widgets.inactive = widget(palette.button, palette.text, Color32::TRANSPARENT);
    visuals.widgets.hovered = widget(palette.button_hover, palette.text, Color32::TRANSPARENT);
    visuals.widgets.active = widget(palette.button_active, palette.text, Color32::TRANSPARENT);
    visuals.widgets.open = widget(palette.button_hover, palette.text, Color32::TRANSPARENT);

    visuals.button_frame = true;
    visuals.striped = false;
    visuals.slider_trailing_fill = true;

    visuals
}

fn widget(fill: Color32, text: Color32, border: Color32) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0, border),
        corner_radius: CornerRadius::same(6),
        fg_stroke: Stroke::new(1.0, text),
        expansion: 0.0,
    }
}

/// Colours from the Adwaita palette.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub background: Color32,
    pub header: Color32,
    pub surface: Color32,
    pub button: Color32,
    pub button_hover: Color32,
    pub button_active: Color32,
    pub input: Color32,
    pub border: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub selection: Color32,
    pub stripe: Color32,
    pub danger: Color32,
    pub warning: Color32,
}

impl Palette {
    pub fn for_theme(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            background: Color32::from_rgb(0x24, 0x24, 0x24),
            header: Color32::from_rgb(0x30, 0x30, 0x30),
            surface: Color32::from_rgb(0x30, 0x30, 0x30),
            button: Color32::from_rgb(0x3d, 0x3d, 0x3d),
            button_hover: Color32::from_rgb(0x48, 0x48, 0x48),
            button_active: Color32::from_rgb(0x55, 0x55, 0x55),
            input: Color32::from_rgb(0x1e, 0x1e, 0x1e),
            border: Color32::from_rgb(0x45, 0x45, 0x45),
            text: Color32::from_rgb(0xff, 0xff, 0xff),
            secondary: Color32::from_rgb(0xb8, 0xb8, 0xb8),
            accent: Color32::from_rgb(0x35, 0x84, 0xe4),
            on_accent: Color32::WHITE,
            selection: Color32::from_rgba_unmultiplied(0x35, 0x84, 0xe4, 72),
            stripe: Color32::from_rgb(0x2b, 0x2b, 0x2b),
            danger: Color32::from_rgb(0xf6, 0x61, 0x51),
            warning: Color32::from_rgb(0xf5, 0xc2, 0x11),
        }
    }

    pub fn light() -> Self {
        Self {
            dark: false,
            background: Color32::from_rgb(0xfa, 0xfa, 0xfa),
            header: Color32::from_rgb(0xeb, 0xeb, 0xeb),
            surface: Color32::from_rgb(0xff, 0xff, 0xff),
            button: Color32::from_rgb(0xed, 0xed, 0xed),
            button_hover: Color32::from_rgb(0xe3, 0xe3, 0xe3),
            button_active: Color32::from_rgb(0xd9, 0xd9, 0xd9),
            input: Color32::WHITE,
            border: Color32::from_rgb(0xd4, 0xd4, 0xd4),
            text: Color32::from_rgb(0x2e, 0x34, 0x36),
            secondary: Color32::from_rgb(0x5e, 0x5c, 0x64),
            accent: Color32::from_rgb(0x35, 0x84, 0xe4),
            on_accent: Color32::WHITE,
            selection: Color32::from_rgba_unmultiplied(0x35, 0x84, 0xe4, 60),
            stripe: Color32::from_rgb(0xf3, 0xf3, 0xf3),
            danger: Color32::from_rgb(0xc0, 0x1c, 0x28),
            warning: Color32::from_rgb(0xb5, 0x83, 0x00),
        }
    }
}
