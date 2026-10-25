//! Helpers shared by the optional desktop integrations.

use std::path::PathBuf;

/// The file the desktop should launch.
///
/// When running as an AppImage, `APPIMAGE` points at the outer file;
/// `current_exe` would point inside the temporary mount and disappear when the
/// app exits.
pub(crate) fn launcher() -> Option<PathBuf> {
    let candidate = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| std::env::current_exe().ok().filter(|path| path.is_file()))?;

    if candidate.is_absolute() {
        Some(candidate)
    } else {
        std::env::current_dir()
            .ok()
            .map(|directory| directory.join(candidate))
    }
}
