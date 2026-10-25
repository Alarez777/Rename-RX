//! GNOME desktop integration.
//!
//! Installing a `.desktop` file named after the window's app ID lets GNOME
//! Shell group every Rename RX window under one launcher and show its icon.
//! The entry and icon are written to the user's data directories; no root or
//! system-wide installation is involved.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const APP_ID: &str = "io.github.alarez777.RenameRX";
const DESKTOP_FILE_NAME: &str = "io.github.alarez777.RenameRX.desktop";
const ICON_FILE_NAME: &str = "io.github.alarez777.RenameRX.svg";
const ICON: &str = include_str!("../../../assets/rename-rx.svg");

pub struct DesktopState {
    /// True when the launcher and icon match this executable.
    pub current: bool,
}

pub fn state() -> Option<DesktopState> {
    let path = desktop_path()?;
    let contents = fs::read_to_string(path).ok()?;
    let icon_matches = icon_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .is_some_and(|installed| installed == ICON);

    let current = crate::integration::launcher()
        .and_then(|launcher| desktop_contents(&launcher).ok())
        .is_some_and(|expected| contents == expected)
        && icon_matches;

    Some(DesktopState { current })
}

pub fn install() -> Result<PathBuf, String> {
    let launcher = crate::integration::launcher()
        .ok_or_else(|| "Could not locate the Rename RX executable.".to_owned())?;
    let applications = applications_directory()
        .ok_or_else(|| "Could not determine the applications directory.".to_owned())?;
    let icons =
        icons_directory().ok_or_else(|| "Could not determine the icon directory.".to_owned())?;

    write_integration(&applications, &icons, &launcher)
}

pub fn remove() -> Result<(), String> {
    if let Some(path) = desktop_path() {
        remove_file(&path)?;
    }
    if let Some(path) = icon_path() {
        remove_file(&path)?;
    }
    Ok(())
}

/// An older launcher, usually created by an AppImage manager, that uses the
/// wrong startup class and would show up as a second menu entry.
pub fn legacy_launcher() -> Option<PathBuf> {
    let path = applications_directory()?.join("rename-rx.desktop");
    let contents = fs::read_to_string(&path).ok()?;
    legacy_contents_match(&contents).then_some(path)
}

fn legacy_contents_match(contents: &str) -> bool {
    contents.contains("Name=Rename RX")
        && (contents.contains("StartupWMClass=rename-rx") || contents.contains("Icon=rename-rx"))
}

pub fn remove_legacy_launcher() -> Result<(), String> {
    match legacy_launcher() {
        Some(path) => remove_file(&path),
        None => Ok(()),
    }
}

fn data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        })
}

fn applications_directory() -> Option<PathBuf> {
    Some(data_home()?.join("applications"))
}

fn icons_directory() -> Option<PathBuf> {
    Some(data_home()?.join("icons/hicolor/scalable/apps"))
}

fn desktop_path() -> Option<PathBuf> {
    Some(applications_directory()?.join(DESKTOP_FILE_NAME))
}

fn icon_path() -> Option<PathBuf> {
    Some(icons_directory()?.join(ICON_FILE_NAME))
}

fn write_integration(
    applications: &Path,
    icons: &Path,
    launcher: &Path,
) -> Result<PathBuf, String> {
    let contents = desktop_contents(launcher)?;

    fs::create_dir_all(applications)
        .map_err(|error| format!("Could not create {}: {error}", applications.display()))?;
    fs::create_dir_all(icons)
        .map_err(|error| format!("Could not create {}: {error}", icons.display()))?;

    let desktop = applications.join(DESKTOP_FILE_NAME);
    write_atomic(&desktop, contents.as_bytes())?;

    let icon = icons.join(ICON_FILE_NAME);
    write_atomic(&icon, ICON.as_bytes())?;

    Ok(desktop)
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("tmp");

    fs::write(&temporary, contents)
        .map_err(|error| format!("Could not write {}: {error}", temporary.display()))?;

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("Could not install {}: {error}", path.display()));
    }

    Ok(())
}

fn remove_file(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not remove {}: {error}", path.display())),
    }
}

fn desktop_contents(launcher: &Path) -> Result<String, String> {
    let exec = exec_line(launcher)?;

    Ok(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Rename RX\n\
         Comment=Batch file renamer for Linux\n\
         {exec}\n\
         Icon={APP_ID}\n\
         Terminal=false\n\
         Categories=Utility;FileTools;\n\
         StartupNotify=true\n\
         StartupWMClass={APP_ID}\n"
    ))
}

fn exec_line(launcher: &Path) -> Result<String, String> {
    let quoted = crate::integration::desktop_quoted_path(launcher)?;
    Ok(format!("Exec={quoted} %F"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_with_spaces() {
        assert_eq!(
            exec_line(Path::new("/tmp/Rename RX.AppImage")).unwrap(),
            "Exec=\"/tmp/Rename RX.AppImage\" %F"
        );
    }

    #[test]
    fn escapes_reserved_characters_and_percent_tokens() {
        assert_eq!(
            exec_line(Path::new("/tmp/a b/100%/x\"$`\\")).unwrap(),
            "Exec=\"/tmp/a b/100%%/x\\\"\\$\\`\\\\\" %F"
        );
    }

    #[test]
    fn rejects_line_breaks_in_the_path() {
        assert!(exec_line(Path::new("/tmp/bad\npath")).is_err());
    }

    #[test]
    fn desktop_file_matches_the_window_app_id() {
        let contents = desktop_contents(Path::new("/tmp/rename-rx")).unwrap();

        assert!(contents.contains("Name=Rename RX\n"));
        assert!(contents.contains("Exec=\"/tmp/rename-rx\" %F\n"));
        assert!(contents.contains(&format!("Icon={APP_ID}\n")));
        assert!(contents.contains(&format!("StartupWMClass={APP_ID}\n")));
        assert!(contents.contains("Type=Application\n"));
        assert!(contents.contains("StartupNotify=true\n"));
    }

    #[test]
    fn writes_the_desktop_file_and_icon() {
        let root = std::env::temp_dir().join(format!("rename-rx-desktop-{}", std::process::id()));
        let applications = root.join("applications");
        let icons = root.join("icons");
        let _ = fs::remove_dir_all(&root);

        let desktop = write_integration(&applications, &icons, Path::new("/tmp/rename-rx"))
            .expect("write integration");

        assert_eq!(desktop.file_name().unwrap(), DESKTOP_FILE_NAME);
        assert_eq!(
            fs::read_to_string(&desktop).unwrap(),
            desktop_contents(Path::new("/tmp/rename-rx")).unwrap()
        );
        assert_eq!(
            fs::read_to_string(icons.join(ICON_FILE_NAME)).unwrap(),
            ICON
        );
        assert!(!applications.join(ICON_FILE_NAME).exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recognizes_the_legacy_launcher_contents() {
        assert!(legacy_contents_match(
            "[Desktop Entry]\nName=Rename RX\nIcon=rename-rx\n"
        ));
        assert!(legacy_contents_match(
            "[Desktop Entry]\nName=Rename RX\nStartupWMClass=rename-rx\n"
        ));
        assert!(!legacy_contents_match(
            "[Desktop Entry]\nName=Other App\nIcon=rename-rx\n"
        ));
        assert!(!legacy_contents_match(
            "[Desktop Entry]\nName=Rename RX\nIcon=other-icon\n"
        ));
    }
}
