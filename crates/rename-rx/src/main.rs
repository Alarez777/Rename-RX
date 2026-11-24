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

    let style = settings::Settings::load().window_controls.resolved();
    let window_surface =
        if style == settings::WindowControlsStyle::Adwaita && transparency_supported() {
            app::WindowSurface::Transparent
        } else {
            app::WindowSurface::Opaque
        };
    log::debug!("window surface: {window_surface:?}");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 760.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("Rename RX")
            .with_app_id("io.github.alarez777.RenameRX")
            // The Adwaita style draws its own headerbar. Native decorations are
            // kept for future automatic/native styles.
            .with_decorations(style != settings::WindowControlsStyle::Adwaita)
            // The custom headerbar rounds its own top and bottom corners; the
            // window surface stays transparent so they show the desktop.
            .with_transparent(window_surface == app::WindowSurface::Transparent),
        ..Default::default()
    };

    eframe::run_native(
        "Rename RX",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::RenameRxApp::new(
                cc,
                initial_paths,
                server,
                window_surface,
            )))
        }),
    )
}

/// Native transparency needs a compositing window manager on X11. Wayland
/// always composites, unless the X11 backend was forced explicitly.
fn transparency_supported() -> bool {
    let forced_x11 = std::env::var("WINIT_UNIX_BACKEND").is_ok_and(|backend| backend == "x11");
    if app::is_wayland_session() && !forced_x11 {
        return true;
    }

    x11_compositor_running()
}

/// Check for an X11 compositing manager through its `_NET_WM_CM_S0` selection.
fn x11_compositor_running() -> bool {
    use x11rb::protocol::xproto::ConnectionExt;

    let Ok((connection, _screen)) = x11rb::connect(None) else {
        return false;
    };
    let atom = connection
        .intern_atom(false, b"_NET_WM_CM_S0")
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|reply| reply.atom);
    let Some(atom) = atom else {
        return false;
    };

    connection
        .get_selection_owner(atom)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .is_some_and(|reply| reply.owner != x11rb::NONE)
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
