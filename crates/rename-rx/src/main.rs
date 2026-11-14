//! Rename RX: a native batch file renamer for Linux.

mod app;
mod desktop;
mod dolphin;
mod instance;
mod integration;
mod nautilus;
mod nemo;
mod presets;
mod settings;
mod theme;
mod thunar;
mod tools;

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let (new_instance, initial_paths) = parse_args(std::env::args_os().skip(1));
    let server = if new_instance {
        None
    } else {
        match instance::acquire(&initial_paths) {
            instance::Startup::Primary(server) => Some(server),
            instance::Startup::Forwarded => return Ok(()),
            instance::Startup::Standalone => None,
        }
    };

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
        Box::new(move |cc| Ok(Box::new(app::RenameRxApp::new(cc, initial_paths, server)))),
    )
}

/// Split the command line into the internal new-window flag and file paths.
fn parse_args(args: impl IntoIterator<Item = OsString>) -> (bool, Vec<PathBuf>) {
    let mut new_instance = false;
    let mut paths_only = false;
    let mut paths = Vec::new();

    for arg in args {
        if !paths_only {
            if arg.as_os_str() == OsStr::new("--new-instance") {
                new_instance = true;
                continue;
            }
            if arg.as_os_str() == OsStr::new("--") {
                paths_only = true;
                continue;
            }
        }

        paths.push(PathBuf::from(arg));
    }

    (new_instance, paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_paths_and_the_new_instance_flag() {
        let args = ["--new-instance", "a.txt", "--", "--new-instance"].map(OsString::from);

        let (new_instance, paths) = parse_args(args);

        assert!(new_instance);
        assert_eq!(
            paths,
            vec![PathBuf::from("a.txt"), PathBuf::from("--new-instance")]
        );
    }

    #[test]
    fn keeps_regular_paths_unchanged() {
        let args = ["/tmp/a.txt", "/tmp/b.txt"].map(OsString::from);

        let (new_instance, paths) = parse_args(args);

        assert!(!new_instance);
        assert_eq!(
            paths,
            vec![PathBuf::from("/tmp/a.txt"), PathBuf::from("/tmp/b.txt")]
        );
    }
}
