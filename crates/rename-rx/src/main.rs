//! Rename RX: a native batch file renamer for Linux.

mod app;
mod desktop;
mod dolphin;
mod integration;
mod nautilus;
mod nemo;
mod presets;
mod settings;
mod theme;
mod thunar;
mod tools;

use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let initial_paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 760.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("Rename RX")
            .with_app_id("io.github.alarez777.RenameRX")
            // The Adwaita style draws its own headerbar. Native decorations are
            // kept for future automatic/native styles.
            .with_decorations(
                settings::Settings::load().window_controls.resolved()
                    != settings::WindowControlsStyle::Adwaita,
            ),
        ..Default::default()
    };

    eframe::run_native(
        "Rename RX",
        options,
        Box::new(move |cc| Ok(Box::new(app::RenameRxApp::new(cc, initial_paths)))),
    )
}
