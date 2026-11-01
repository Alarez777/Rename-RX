//! Nemo file-manager integration.
//!
//! Rename RX can drop a `.nemo_action` file into the user's Nemo actions
//! directory so that selected files can be opened from the context menu. The
//! action is user-level, works for any non-directory file, and names itself in
//! English or Spanish depending on the desktop locale.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ACTION_FILE_NAME: &str = "rename-rx.nemo_action";

pub(crate) struct NemoActionState {
    /// True when the action on disk launches this same executable.
    pub(crate) current: bool,
}

pub(crate) fn state() -> Option<NemoActionState> {
    let path = action_path()?;
    let contents = fs::read_to_string(path).ok()?;
    let current = crate::integration::launcher()
        .and_then(|launcher| exec_line(&launcher).ok())
        .is_some_and(|expected| {
            contents
                .lines()
                .any(|line| line.trim_end() == expected.as_str())
        });

    Some(NemoActionState { current })
}

pub(crate) fn install() -> Result<PathBuf, String> {
    let directory = actions_directory()
        .ok_or_else(|| "Could not determine the user data directory.".to_owned())?;
    let launcher = crate::integration::launcher()
        .ok_or_else(|| "Could not locate the Rename RX executable.".to_owned())?;

    write_action(&directory, &launcher)
}

pub(crate) fn remove() -> Result<(), String> {
    let Some(path) = action_path() else {
        return Ok(());
    };

    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not remove {}: {error}", path.display())),
    }
}

fn actions_directory() -> Option<PathBuf> {
    Some(
        crate::integration::data_home()?
            .join("nemo")
            .join("actions"),
    )
}

fn action_path() -> Option<PathBuf> {
    Some(actions_directory()?.join(ACTION_FILE_NAME))
}

fn write_action(directory: &Path, launcher: &Path) -> Result<PathBuf, String> {
    let contents = action_contents(launcher)?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;

    let path = directory.join(ACTION_FILE_NAME);
    let temporary = path.with_extension("nemo_action.tmp");

    fs::write(&temporary, contents)
        .map_err(|error| format!("Could not write {}: {error}", temporary.display()))?;

    if let Err(error) = fs::rename(&temporary, &path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("Could not install {}: {error}", path.display()));
    }

    Ok(path)
}

fn action_contents(launcher: &Path) -> Result<String, String> {
    let exec = exec_line(launcher)?;

    Ok(format!(
        "[Nemo Action]\n\
         Active=true\n\
         Name=Rename with Rename RX\n\
         Name[es]=Renombrar con Rename RX\n\
         Comment=Open the selected files in Rename RX\n\
         Comment[es]=Abrir los archivos seleccionados en Rename RX\n\
         {exec}\n\
         Icon-Name=document-edit-symbolic\n\
         Selection=notnone\n\
         Extensions=nodirs;\n\
         Quote=double\n"
    ))
}

fn exec_line(launcher: &Path) -> Result<String, String> {
    let quoted = crate::integration::shell_quoted_path(launcher)?;
    Ok(format!("Exec={quoted} %F"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_with_spaces() {
        assert_eq!(
            exec_line(Path::new("/tmp/Rename RX.AppImage")).unwrap(),
            "Exec='/tmp/Rename RX.AppImage' %F"
        );
    }

    #[test]
    fn escapes_single_quotes_and_percent_tokens() {
        assert_eq!(
            exec_line(Path::new("/tmp/andré's/100%/rename-rx")).unwrap(),
            "Exec='/tmp/andré'\\''s/100%%/rename-rx' %F"
        );
    }

    #[test]
    fn rejects_line_breaks_in_the_path() {
        assert!(exec_line(Path::new("/tmp/bad\npath")).is_err());
    }

    #[test]
    fn action_file_is_localized_and_matches_any_file() {
        let contents = action_contents(Path::new("/tmp/rename-rx")).unwrap();

        assert!(contents.contains("Name=Rename with Rename RX\n"));
        assert!(contents.contains("Name[es]=Renombrar con Rename RX\n"));
        assert!(contents.contains("Comment[es]=Abrir los archivos seleccionados en Rename RX\n"));
        assert!(contents.contains("Exec='/tmp/rename-rx' %F\n"));
        assert!(contents.contains("Selection=notnone\n"));
        assert!(contents.contains("Extensions=nodirs;\n"));
        assert!(contents.contains("Quote=double\n"));
    }

    #[test]
    fn writes_the_action_file() {
        let directory = std::env::temp_dir().join(format!("rename-rx-nemo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);

        let path = write_action(&directory, Path::new("/tmp/rename-rx")).unwrap();
        let written = fs::read_to_string(&path).unwrap();

        assert_eq!(path.file_name().unwrap(), ACTION_FILE_NAME);
        assert!(written.contains("Name[es]=Renombrar con Rename RX"));
        assert!(!directory.join("rename-rx.nemo_action.tmp").exists());

        fs::remove_dir_all(directory).unwrap();
    }
}
