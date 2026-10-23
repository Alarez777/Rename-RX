mod app;
mod settings;
mod theme;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 760.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("Rename RX")
            .with_app_id("io.github.alarez777.RenameRX"),
        ..Default::default()
    };

    eframe::run_native(
        "Rename RX",
        options,
        Box::new(|cc| Ok(Box::new(app::RenameRxApp::new(cc)))),
    )
}
