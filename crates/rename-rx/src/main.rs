mod app;
mod nemo;
mod settings;
mod theme;

use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result {
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
