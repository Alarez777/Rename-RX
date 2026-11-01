//! Thunar custom-action integration.
//!
//! Thunar stores custom actions in `$XDG_CONFIG_HOME/Thunar/uca.xml`. There is
//! no one-file-per-action mechanism, so the existing XML is preserved and a
//! single action block identified by `unique-id` is added, updated or removed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const UNIQUE_ID: &str = "rename-rx";
const ACTION_NAME: &str = "Rename with Rename RX";
const ACTION_NAME_ES: &str = "Renombrar con Rename RX";

pub(crate) struct ThunarState {
    /// True when the stored action launches this same executable.
    pub(crate) current: bool,
}

pub(crate) fn state() -> Option<ThunarState> {
    let path = uca_path()?;
    let xml = fs::read_to_string(path).ok()?;
    let blocks = action_blocks(&xml);
    if blocks.is_empty() {
        return None;
    }

    let current = crate::integration::launcher()
        .and_then(|launcher| raw_command(&launcher).ok())
        .is_some_and(|command| {
            blocks.len() == 1
                && action_command(blocks[0].text)
                    .map(xml_unescape)
                    .is_some_and(|stored| stored == command)
        });

    Some(ThunarState { current })
}

pub(crate) fn install() -> Result<PathBuf, String> {
    let launcher = crate::integration::launcher()
        .ok_or_else(|| "Could not locate the Rename RX executable.".to_owned())?;
    let path =
        uca_path().ok_or_else(|| "Could not determine the configuration directory.".to_owned())?;

    let existing = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("Could not read {}: {error}", path.display())),
    };

    let merged = merge_uca(&existing, &action_block(&launcher)?)?;
    write_atomic(&path, merged.as_bytes())?;
    Ok(path)
}

pub(crate) fn remove() -> Result<(), String> {
    let Some(path) = uca_path() else {
        return Ok(());
    };

    let existing = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Could not read {}: {error}", path.display())),
    };

    let ranges: Vec<(usize, usize)> = action_blocks(&existing)
        .iter()
        .map(|block| (block.start, block.end))
        .collect();
    if ranges.is_empty() {
        return Ok(());
    }

    let mut cleaned = existing;
    for (start, end) in ranges.iter().rev() {
        cleaned.replace_range(*start..*end, "");
    }

    write_atomic(&path, cleaned.as_bytes())
}

fn uca_path() -> Option<PathBuf> {
    Some(
        crate::integration::config_home()?
            .join("Thunar")
            .join("uca.xml"),
    )
}

fn action_block(launcher: &Path) -> Result<String, String> {
    let command = xml_escape(&raw_command(launcher)?);

    Ok(format!(
        "<action>\n\
         \t<icon>document-edit-symbolic</icon>\n\
         \t<name>{ACTION_NAME}</name>\n\
         \t<name xml:lang=\"es\">{ACTION_NAME_ES}</name>\n\
         \t<submenu></submenu>\n\
         \t<unique-id>{UNIQUE_ID}</unique-id>\n\
         \t<command>{command}</command>\n\
         \t<description>Open the selected files in Rename RX</description>\n\
         \t<description xml:lang=\"es\">Abrir los archivos seleccionados en Rename RX</description>\n\
         \t<range></range>\n\
         \t<patterns>*</patterns>\n\
         \t<startup-notify/>\n\
         \t<audio-files/>\n\
         \t<image-files/>\n\
         \t<other-files/>\n\
         \t<text-files/>\n\
         \t<video-files/>\n\
         </action>\n"
    ))
}

fn raw_command(launcher: &Path) -> Result<String, String> {
    let quoted = crate::integration::shell_quoted_path(launcher)?;
    Ok(format!("{quoted} %F"))
}

fn action_command(block: &str) -> Option<&str> {
    let start = block.find("<command>")? + "<command>".len();
    let rest = &block[start..];
    let end = rest.find("</command>")?;
    Some(&rest[..end])
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn merge_uca(existing: &str, block: &str) -> Result<String, String> {
    if existing.trim().is_empty() {
        return Ok(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n{block}</actions>\n"
        ));
    }

    let close = existing
        .rfind("</actions>")
        .ok_or_else(|| "The existing uca.xml has no </actions> element.".to_owned())?;
    if !existing[..close].contains("<actions") {
        return Err("The existing uca.xml is not a Thunar actions file.".to_owned());
    }

    let blocks = action_blocks(existing);
    let first_start = blocks.first().map(|block| block.start);
    let mut cleaned = existing.to_owned();

    for block in blocks.iter().rev() {
        cleaned.replace_range(block.start..block.end, "");
    }

    let insert_at = match first_start {
        Some(start) => start,
        None => cleaned
            .rfind("</actions>")
            .ok_or_else(|| "The existing uca.xml has no </actions> element.".to_owned())?,
    };

    let mut merged = String::with_capacity(cleaned.len() + block.len());
    merged.push_str(&cleaned[..insert_at]);
    if !merged.ends_with('\n') {
        merged.push('\n');
    }
    merged.push_str(block);
    merged.push_str(&cleaned[insert_at..]);
    Ok(merged)
}

struct ActionBlock<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}

fn action_blocks(xml: &str) -> Vec<ActionBlock<'_>> {
    let mut blocks = Vec::new();
    let mut cursor = 0;

    while let Some(relative) = xml[cursor..].find("<action>") {
        let start = cursor + relative;
        let Some(close_relative) = xml[start..].find("</action>") else {
            break;
        };
        let end = start + close_relative + "</action>".len();
        let text = &xml[start..end];

        if action_unique_id(text) == Some(UNIQUE_ID) {
            blocks.push(ActionBlock { start, end, text });
        }

        cursor = end;
    }

    blocks
}

fn action_unique_id(block: &str) -> Option<&str> {
    let start = block.find("<unique-id>")? + "<unique-id>".len();
    let rest = &block[start..];
    let end = rest.find("</unique-id>")?;
    Some(rest[..end].trim())
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }

    let temporary = path.with_extension("xml.tmp");
    fs::write(&temporary, contents)
        .map_err(|error| format!("Could not write {}: {error}", temporary.display()))?;

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("Could not install {}: {error}", path.display()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_ACTION: &str = "\t<action>\n\t\t<name>Open terminal</name>\n\t\t<unique-id>user-action</unique-id>\n\t\t<command>exo-open --launch TerminalEmulator</command>\n\t\t<patterns>*</patterns>\n\t</action>\n";

    #[test]
    fn action_block_matches_all_files_and_is_localized() {
        let block = action_block(Path::new("/tmp/Rename RX.AppImage")).unwrap();

        assert!(block.contains("<unique-id>rename-rx</unique-id>"));
        assert!(block.contains("<name>Rename with Rename RX</name>"));
        assert!(block.contains("<name xml:lang=\"es\">Renombrar con Rename RX</name>"));
        assert!(block.contains("<command>'/tmp/Rename RX.AppImage' %F</command>"));
        assert!(block.contains("<patterns>*</patterns>"));
        assert!(block.contains("<other-files/>"));
        assert!(block.contains("<image-files/>"));
    }

    #[test]
    fn creates_a_new_actions_file() {
        let block = action_block(Path::new("/tmp/rename-rx")).unwrap();
        let merged = merge_uca("", &block).unwrap();

        assert!(merged.starts_with("<?xml version=\"1.0\""));
        assert!(merged.contains("<actions>"));
        assert_eq!(
            merged.matches("<unique-id>rename-rx</unique-id>").count(),
            1
        );
    }

    #[test]
    fn preserves_existing_actions() {
        let existing = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n{USER_ACTION}</actions>\n"
        );
        let block = action_block(Path::new("/tmp/rename-rx")).unwrap();
        let merged = merge_uca(&existing, &block).unwrap();

        assert!(merged.contains("<unique-id>user-action</unique-id>"));
        assert_eq!(
            merged.matches("<unique-id>rename-rx</unique-id>").count(),
            1
        );
    }

    #[test]
    fn replaces_a_stale_action_without_duplicates() {
        let stale = action_block(Path::new("/tmp/old-rename-rx")).unwrap();
        let existing = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n{USER_ACTION}{stale}</actions>\n"
        );
        let block = action_block(Path::new("/tmp/new-rename-rx")).unwrap();
        let merged = merge_uca(&existing, &block).unwrap();

        assert!(!merged.contains("/tmp/old-rename-rx"));
        assert!(merged.contains("/tmp/new-rename-rx"));
        assert_eq!(
            merged.matches("<unique-id>rename-rx</unique-id>").count(),
            1
        );
        assert!(merged.contains("<unique-id>user-action</unique-id>"));
    }

    #[test]
    fn removes_only_our_action() {
        let existing = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n{USER_ACTION}{}</actions>\n",
            action_block(Path::new("/tmp/rename-rx")).unwrap()
        );
        let ranges: Vec<(usize, usize)> = action_blocks(&existing)
            .iter()
            .map(|block| (block.start, block.end))
            .collect();
        assert_eq!(ranges.len(), 1);

        let mut cleaned = existing;
        for (start, end) in ranges.iter().rev() {
            cleaned.replace_range(*start..*end, "");
        }

        assert!(!cleaned.contains("rename-rx"));
        assert!(cleaned.contains("<unique-id>user-action</unique-id>"));
    }

    #[test]
    fn rejects_files_without_an_actions_root() {
        assert!(merge_uca("<action>oops</action>", "<action></action>").is_err());
    }

    #[test]
    fn escapes_xml_special_characters_in_the_command() {
        let command = xml_escape(&raw_command(Path::new("/tmp/a&b/<rename>")).unwrap());
        assert!(command.contains("&amp;"));
        assert!(command.contains("&lt;rename&gt;"));
    }

    #[test]
    fn stored_commands_survive_thunar_escaping() {
        let raw = raw_command(Path::new("/tmp/a&b/it's")).unwrap();
        let escaped = xml_escape(&raw);
        assert_eq!(xml_unescape(&escaped), raw);
        assert_eq!(xml_unescape(&escaped.replace('\'', "&apos;")), raw);
    }
}
