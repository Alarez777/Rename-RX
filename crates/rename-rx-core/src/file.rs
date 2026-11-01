use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

/// Stable identifier for a file in the current batch.
pub type FileId = u64;

/// Split a file name into the stem used by the rename rules and the extension
/// (without the dot).
///
/// A leading dot does not start an extension for a dotfile, so `.bashrc` has no
/// extension. Only the last dot separates the extension, so `archive.tar.gz`
/// has the stem `archive.tar` and the extension `gz`.
pub fn split_file_name(name: &str) -> (&str, &str) {
    if name == "." || name == ".." {
        return (name, "");
    }

    match name.rfind('.') {
        Some(index) if index > 0 => (&name[..index], &name[index + 1..]),
        _ => (name, ""),
    }
}

/// A file selected for renaming.
///
/// The original name is kept separate from `stem` and `extension` so the rule
/// pipeline can rebuild the name without losing the extension. Paths are not
/// canonicalized: renaming a symlink must rename the link, not its target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    /// Identifier that is unique within the current batch.
    pub id: FileId,
    /// Path exactly as it was added, without canonicalization.
    pub path: PathBuf,
    /// File name as read from the path, converted lossily when it is not UTF-8.
    pub original_name: String,
    /// Creation time, when the filesystem exposes one.
    pub created: Option<SystemTime>,
    /// Last modification time, when the filesystem exposes one.
    pub modified: Option<SystemTime>,
    /// File size in bytes.
    pub size: u64,
    /// False when the OS filename is not valid UTF-8. Such files are shown
    /// lossily and will be skipped by renaming.
    pub utf8_name: bool,
}

impl FileEntry {
    /// Read a file from disk.
    ///
    /// `id` only needs to be unique within the current batch.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error when the path cannot be inspected or
    /// when it does not exist.
    pub fn read(id: FileId, path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let metadata = std::fs::symlink_metadata(&path)?;

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned);

        let (original_name, utf8_name) = match file_name {
            Some(name) => (name, true),
            None => (
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                false,
            ),
        };

        Ok(Self {
            id,
            path,
            original_name,
            created: metadata.created().ok(),
            modified: metadata.modified().ok(),
            size: metadata.len(),
            utf8_name,
        })
    }

    /// Base name and extension, derived from [`original_name`].
    ///
    /// [`original_name`]: FileEntry::original_name
    pub fn name_parts(&self) -> (&str, &str) {
        split_file_name(&self.original_name)
    }

    /// Base name without its final extension.
    pub fn stem(&self) -> &str {
        self.name_parts().0
    }

    /// Final extension without the leading dot.
    pub fn extension(&self) -> &str {
        self.name_parts().1
    }

    /// The date shown in the file table, falling back to the modification
    /// time when the filesystem does not expose a creation time.
    pub fn display_time(&self) -> Option<SystemTime> {
        self.created.or(self.modified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_regular_names() {
        assert_eq!(split_file_name("photo.jpg"), ("photo", "jpg"));
        assert_eq!(split_file_name("archive.tar.gz"), ("archive.tar", "gz"));
        assert_eq!(split_file_name("no-extension"), ("no-extension", ""));
        assert_eq!(split_file_name(""), ("", ""));
    }

    #[test]
    fn handles_dotfiles_and_trailing_dots() {
        assert_eq!(split_file_name(".bashrc"), (".bashrc", ""));
        assert_eq!(split_file_name(".config.json"), (".config", "json"));
        assert_eq!(split_file_name("report."), ("report", ""));
        assert_eq!(split_file_name(".."), ("..", ""));
    }

    #[test]
    fn reads_a_real_file() {
        let path = std::env::temp_dir().join(format!(
            "rename-rx-core-{}-{:?}.txt",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&path, b"hello").unwrap();

        let entry = FileEntry::read(7, &path).unwrap();
        assert_eq!(entry.id, 7);
        assert_eq!(
            entry.original_name,
            path.file_name().unwrap().to_string_lossy()
        );
        assert_eq!(entry.extension(), "txt");
        assert_eq!(entry.size, 5);
        assert!(entry.utf8_name);

        let _ = std::fs::remove_file(path);
    }
}
