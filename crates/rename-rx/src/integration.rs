//! Helpers shared by the optional desktop integrations.

use std::path::{Path, PathBuf};

/// The user's configuration directory, following the XDG base directory rules.
pub(crate) fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
}

/// The user's data directory, following the XDG base directory rules.
pub(crate) fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        })
}

/// The file the desktop should launch.
///
/// When running as an `AppImage`, `APPIMAGE` points at the outer file;
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

/// Quote an executable path for a shell-style command line (Nemo and Thunar
/// parse their command strings with a shell).
pub(crate) fn shell_quoted_path(path: &Path) -> Result<String, String> {
    let path = path_str(path)?;
    let escaped = path.replace('%', "%%");
    Ok(format!("'{}'", escaped.replace('\'', "'\\''")))
}

/// Quote an executable path for a Desktop Entry `Exec=` value (GNOME and KDE
/// parse these with the desktop-entry rules, not with a shell).
pub(crate) fn desktop_quoted_path(path: &Path) -> Result<String, String> {
    let path = path_str(path)?;
    let escaped = path.replace('%', "%%").replace('\\', "\\\\");
    let escaped = escaped
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    Ok(format!("\"{escaped}\""))
}

fn path_str(path: &Path) -> Result<&str, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "The application path is not valid UTF-8.".to_owned())?;

    if path.contains(['\n', '\r']) {
        return Err("The application path contains unsupported line breaks.".to_owned());
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_handles_spaces_quotes_and_percent() {
        assert_eq!(
            shell_quoted_path(Path::new("/tmp/andré's/100%/Rename RX")).unwrap(),
            "'/tmp/andré'\\''s/100%%/Rename RX'"
        );
    }

    #[test]
    fn desktop_quote_handles_reserved_characters() {
        assert_eq!(
            desktop_quoted_path(Path::new("/tmp/a b/100%/x\"$`\\")).unwrap(),
            "\"/tmp/a b/100%%/x\\\"\\$\\`\\\\\""
        );
    }

    #[test]
    fn rejects_line_breaks() {
        assert!(shell_quoted_path(Path::new("/tmp/bad\npath")).is_err());
        assert!(desktop_quoted_path(Path::new("/tmp/bad\rpath")).is_err());
    }
}
