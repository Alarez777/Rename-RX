//! Dolphin file-manager integration.
//!
//! Dolphin reads KDE service menus from `$XDG_DATA_HOME/kio/servicemenus`.
//! A service menu is a desktop entry with `Type=Service`; user-level files must
//! be executable and KDE's service cache has to be refreshed afterwards.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SERVICE_MENU_NAME: &str = "rename-rx.desktop";

pub struct DolphinState {
    /// True when the service menu matches this executable and is executable.
    pub current: bool,
}

pub fn state() -> Option<DolphinState> {
    let path = service_menu_path()?;
    let contents = fs::read_to_string(&path).ok()?;
    let executable = is_executable(&path);

    let current = crate::integration::launcher()
        .and_then(|launcher| service_menu_contents(&launcher).ok())
        .is_some_and(|expected| contents == expected)
        && executable;

    Some(DolphinState { current })
}

pub fn install() -> Result<PathBuf, String> {
    let launcher = crate::integration::launcher()
        .ok_or_else(|| "Could not locate the Rename RX executable.".to_owned())?;
    let directory = service_menu_directory()
        .ok_or_else(|| "Could not determine the user data directory.".to_owned())?;

    let path = write_service_menu(&directory, &launcher)?;
    refresh_kde_cache();
    Ok(path)
}

pub fn remove() -> Result<(), String> {
    let Some(path) = service_menu_path() else {
        return Ok(());
    };

    match fs::remove_file(&path) {
        Ok(()) => {
            refresh_kde_cache();
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not remove {}: {error}", path.display())),
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

fn service_menu_directory() -> Option<PathBuf> {
    Some(data_home()?.join("kio").join("servicemenus"))
}

fn service_menu_path() -> Option<PathBuf> {
    Some(service_menu_directory()?.join(SERVICE_MENU_NAME))
}

fn write_service_menu(directory: &Path, launcher: &Path) -> Result<PathBuf, String> {
    let contents = service_menu_contents(launcher)?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;

    let path = directory.join(SERVICE_MENU_NAME);
    let temporary = path.with_extension("desktop.tmp");

    fs::write(&temporary, contents)
        .map_err(|error| format!("Could not write {}: {error}", temporary.display()))?;
    mark_executable(&temporary)?;

    if let Err(error) = fs::rename(&temporary, &path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("Could not install {}: {error}", path.display()));
    }

    Ok(path)
}

fn service_menu_contents(launcher: &Path) -> Result<String, String> {
    let exec = crate::integration::desktop_quoted_path(launcher)?;

    Ok(format!(
        "[Desktop Entry]\n\
         Type=Service\n\
         ServiceTypes=KonqPopupMenu/Plugin\n\
         MimeType=application/octet-stream;\n\
         Actions=RenameRx;\n\
         X-KDE-Priority=TopLevel\n\
         X-KDE-Protocols=file\n\
         \n\
         [Desktop Action RenameRx]\n\
         Name=Rename with Rename RX\n\
         Name[es]=Renombrar con Rename RX\n\
         Icon=document-edit-symbolic\n\
         Exec={exec} %F\n"
    ))
}

fn refresh_kde_cache() {
    for binary in ["kbuildsycoca6", "kbuildsycoca5"] {
        let Ok(mut child) = Command::new(binary)
            .arg("--noincremental")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };

        let _ = child.wait();
        break;
    }
}

#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("Could not make {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
fn mark_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_menu_is_localized_and_matches_any_file() {
        let contents = service_menu_contents(Path::new("/tmp/Rename RX.AppImage")).unwrap();

        assert!(contents.contains("Type=Service\n"));
        assert!(contents.contains("MimeType=application/octet-stream;\n"));
        assert!(contents.contains("X-KDE-Priority=TopLevel\n"));
        assert!(contents.contains("X-KDE-Protocols=file\n"));
        assert!(contents.contains("Name=Rename with Rename RX\n"));
        assert!(contents.contains("Name[es]=Renombrar con Rename RX\n"));
        assert!(contents.contains("Exec=\"/tmp/Rename RX.AppImage\" %F\n"));
    }

    #[test]
    fn writes_an_executable_service_menu() {
        let directory =
            std::env::temp_dir().join(format!("rename-rx-dolphin-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);

        let path = write_service_menu(&directory, Path::new("/tmp/rename-rx")).unwrap();

        assert_eq!(path.file_name().unwrap(), SERVICE_MENU_NAME);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            service_menu_contents(Path::new("/tmp/rename-rx")).unwrap()
        );
        assert!(is_executable(&path));
        assert!(!directory.join("rename-rx.desktop.tmp").exists());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_line_breaks_in_the_path() {
        assert!(service_menu_contents(Path::new("/tmp/bad\npath")).is_err());
    }
}
