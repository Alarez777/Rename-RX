use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::file::FileEntry;

/// All rename rules, in the order they are applied.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RuleSet {
    /// Replaces the whole base name.
    pub new_name: NewNameRule,
    /// Literal text replacement, optionally whole-word.
    pub replace: ReplaceRule,
    /// Regular-expression replacement.
    pub regex: RegexRule,
    /// Inserts text at a character position.
    pub insert: InsertRule,
    /// Keeps only one range of characters.
    pub keep: KeepRule,
    /// Removes one range of characters.
    pub remove: RemoveRule,
    /// Moves one range of characters elsewhere in the name.
    pub move_rule: MoveRule,
    /// Adds a sequential number.
    pub serialize: SerializeRule,
    /// Adds a fixed prefix.
    pub prefix: TextRule,
    /// Adds a fixed suffix.
    pub suffix: TextRule,
    /// Replaces the extension.
    pub extension: ExtensionRule,
    /// Changes the case of the base name.
    pub case: CaseRule,
    /// Changes the display order and serial-number order.
    pub sort: SortRule,
}

impl RuleSet {
    /// True when at least one rule can change a file name. Sorting only changes
    /// the order and is deliberately excluded.
    pub fn has_name_effects(&self) -> bool {
        self.new_name.enabled
            || self.replace.enabled
            || self.regex.enabled
            || self.insert.enabled
            || self.keep.enabled
            || self.remove.enabled
            || self.move_rule.enabled
            || self.serialize.enabled
            || self.prefix.enabled
            || self.suffix.enabled
            || self.extension.enabled
            || self.case.enabled
    }
}

/// Replaces the whole base name with a fixed value.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct NewNameRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// New base name; an empty value makes the resulting name invalid.
    pub value: String,
}

/// Replaces literal text in the base name.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ReplaceRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// Literal text to find.
    pub find: String,
    /// Replacement text.
    pub replace: String,
    /// Whether matching is case-sensitive.
    pub case_sensitive: bool,
    /// Whether only whole words match.
    pub whole_word: bool,
}

impl Default for ReplaceRule {
    fn default() -> Self {
        Self {
            enabled: false,
            find: String::new(),
            replace: String::new(),
            case_sensitive: true,
            whole_word: false,
        }
    }
}

/// Replaces text matched by a regular expression.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RegexRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// Regular expression pattern.
    pub pattern: String,
    /// Replacement text; capture groups use `$1`, `${name}`.
    pub replacement: String,
    /// Whether matching ignores case.
    pub case_insensitive: bool,
}

/// Inserts text into the base name.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct InsertRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// Text to insert.
    pub text: String,
    /// Grapheme position, counted from [`InsertAnchor`].
    pub position: u32,
    /// End of the base name that `position` is counted from.
    pub anchor: InsertAnchor,
}

/// Where inserted text goes relative to the counted position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertAnchor {
    /// Insert before the grapheme at `position`.
    #[default]
    Before,
    /// Insert after the grapheme at `position`, counting from the end.
    After,
}

/// Keeps one range of graphemes from the base name.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct KeepRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// Number of graphemes to keep.
    pub count: u32,
    /// End of the base name that `position` is counted from.
    pub origin: CharacterOrigin,
    /// Grapheme position where the kept range starts.
    pub position: u32,
}

impl Default for KeepRule {
    fn default() -> Self {
        Self {
            enabled: false,
            count: 1,
            origin: CharacterOrigin::Before,
            position: 0,
        }
    }
}

/// Removes one range of graphemes from the base name.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RemoveRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// Number of graphemes to remove.
    pub count: u32,
    /// End of the base name that `position` is counted from.
    pub origin: CharacterOrigin,
    /// Grapheme position where the removed range starts.
    pub position: u32,
}

impl Default for RemoveRule {
    fn default() -> Self {
        Self {
            enabled: false,
            // Zero characters would silently do nothing; the rule is meant to
            // be switched off instead.
            count: 1,
            origin: CharacterOrigin::Before,
            position: 0,
        }
    }
}

/// End of the base name that a position or count is measured from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterOrigin {
    /// Measure from the start of the base name.
    #[default]
    Before,
    /// Measure from the end of the base name.
    After,
}

/// Moves one range of graphemes elsewhere in the base name.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MoveRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// Grapheme position of the range, counted from [`MoveAnchor`].
    pub start: u32,
    /// Number of graphemes to move.
    pub length: u32,
    /// End of the base name that `start` is counted from.
    pub anchor: MoveAnchor,
    /// End of the remaining name that `offset` is counted from.
    pub destination: CharacterOrigin,
    /// Grapheme offset for the insertion point in the remaining name.
    pub offset: u32,
}

impl Default for MoveRule {
    fn default() -> Self {
        Self {
            enabled: false,
            start: 0,
            length: 1,
            anchor: MoveAnchor::Start,
            destination: CharacterOrigin::Before,
            offset: 0,
        }
    }
}

/// End of the base name where the moved range starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveAnchor {
    /// Count `start` from the beginning.
    #[default]
    Start,
    /// Count `start` backwards from the end.
    End,
}

/// Adds a sequential number to the base name.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SerializeRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// Number assigned to the first file.
    pub start: u32,
    /// Increment between consecutive files.
    pub step: u32,
    /// Constant added to every number.
    pub offset: u32,
    /// Total width of the number, zero-padded.
    pub padding: u32,
    /// Where the number is placed.
    pub position: SerializePosition,
    /// Text before the number.
    pub prefix: String,
    /// Text after the number.
    pub suffix: String,
}

impl Default for SerializeRule {
    fn default() -> Self {
        Self {
            enabled: false,
            start: 1,
            step: 1,
            offset: 0,
            padding: 3,
            position: SerializePosition::Before,
            // A trailing space keeps the default `001 name` layout; the space
            // travels with the number rather than being a separate separator.
            prefix: String::new(),
            suffix: " ".to_owned(),
        }
    }
}

/// Where the serial number is placed relative to the base name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SerializePosition {
    /// Place the number after the base name.
    #[default]
    After,
    /// Place the number before the base name.
    Before,
}

/// Adds a fixed piece of text to the base name.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct TextRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// Text to add.
    pub text: String,
}

/// Replaces the extension.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ExtensionRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// New extension; a leading dot is ignored.
    pub value: String,
}

/// Changes the case of the base name.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CaseRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// Case conversion to apply.
    pub mode: CaseMode,
}

/// Text case applied by [`CaseRule`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseMode {
    /// Convert every character to lowercase.
    #[default]
    Lower,
    /// Capitalize the first letter of every word.
    Title,
    /// Convert every character to uppercase.
    Upper,
}

/// Changes the display order and the order used for serial numbers.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SortRule {
    /// Whether the rule runs.
    pub enabled: bool,
    /// Sort key.
    pub method: SortMethod,
    /// Whether the order is reversed.
    pub descending: bool,
}

/// Order used to list files and assign serial numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMethod {
    /// Keep the order in which the files were added.
    #[default]
    Original,
    /// Sort by file name, ignoring case.
    Name,
    /// Sort by creation time, then by name.
    Created,
    /// Sort by modification time, then by name.
    Modified,
}

/// The two parts a rename rule operates on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameParts {
    /// Base name without its extension.
    pub stem: String,
    /// Extension without the leading dot.
    pub extension: String,
}

impl NameParts {
    /// Join the stem and extension into a complete file name.
    pub fn file_name(&self) -> String {
        join_name(&self.stem, &self.extension)
    }
}

/// Rules compiled once for a whole preview pass.
#[derive(Debug)]
pub struct CompiledRules<'rules> {
    rules: &'rules RuleSet,
    regex: Option<Regex>,
    replace: Option<Regex>,
    /// Rule-level problems, such as an invalid regular expression.
    errors: Vec<String>,
}

impl<'rules> CompiledRules<'rules> {
    /// Compile the regular expressions and report rule-level problems.
    pub fn new(rules: &'rules RuleSet) -> Self {
        let mut errors = Vec::new();

        let regex = if rules.regex.enabled && !rules.regex.pattern.is_empty() {
            match RegexBuilder::new(&rules.regex.pattern)
                .case_insensitive(rules.regex.case_insensitive)
                .build()
            {
                Ok(regex) => Some(regex),
                Err(error) => {
                    errors.push(format!("Regular expression: {error}"));
                    None
                }
            }
        } else {
            None
        };

        let replace = if rules.replace.enabled && !rules.replace.find.is_empty() {
            let pattern = if rules.replace.whole_word {
                format!(r"\b{}\b", regex::escape(&rules.replace.find))
            } else {
                regex::escape(&rules.replace.find)
            };

            match RegexBuilder::new(&pattern)
                .case_insensitive(!rules.replace.case_sensitive)
                .build()
            {
                Ok(regex) => Some(regex),
                Err(error) => {
                    errors.push(format!("Replace rule: {error}"));
                    None
                }
            }
        } else {
            None
        };

        Self {
            rules,
            regex,
            replace,
            errors,
        }
    }

    /// Rule-level problems found while compiling, such as an invalid regex.
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// Apply the pipeline to one file. `index` is the position of the file in
    /// the current (sorted) order and drives serialization.
    pub fn apply(&self, file: &FileEntry, index: usize) -> String {
        self.apply_parts(file, index).file_name()
    }

    /// Apply the pipeline and return the stem and extension separately.
    pub fn apply_parts(&self, file: &FileEntry, index: usize) -> NameParts {
        let (stem, extension) = file.name_parts();
        let mut stem = stem.to_owned();
        let mut extension = extension.to_owned();

        if self.rules.new_name.enabled {
            stem.clone_from(&self.rules.new_name.value);
        }

        if let Some(regex) = &self.replace {
            stem = regex
                .replace_all(&stem, self.rules.replace.replace.as_str())
                .into_owned();
        }

        if let Some(regex) = &self.regex {
            stem = regex
                .replace_all(&stem, self.rules.regex.replacement.as_str())
                .into_owned();
        }

        if self.rules.insert.enabled && !self.rules.insert.text.is_empty() {
            stem = insert_text(
                &stem,
                &self.rules.insert.text,
                usize_from(self.rules.insert.position),
                self.rules.insert.anchor,
            );
        }

        if self.rules.keep.enabled && self.rules.keep.count > 0 {
            stem = keep_text(
                &stem,
                usize_from(self.rules.keep.count),
                self.rules.keep.origin,
                usize_from(self.rules.keep.position),
            );
        }

        if self.rules.remove.enabled && self.rules.remove.count > 0 {
            stem = remove_chars(
                &stem,
                usize_from(self.rules.remove.count),
                self.rules.remove.origin,
                usize_from(self.rules.remove.position),
            );
        }

        if self.rules.move_rule.enabled && self.rules.move_rule.length > 0 {
            stem = move_text(
                &stem,
                usize_from(self.rules.move_rule.start),
                usize_from(self.rules.move_rule.length),
                self.rules.move_rule.anchor,
                self.rules.move_rule.destination,
                usize_from(self.rules.move_rule.offset),
            );
        }

        if self.rules.serialize.enabled {
            let number = u64::from(self.rules.serialize.start)
                + u64::try_from(index).unwrap_or(u64::MAX) * u64::from(self.rules.serialize.step)
                + u64::from(self.rules.serialize.offset);
            stem = serialize(
                &stem,
                number,
                self.rules.serialize.padding,
                self.rules.serialize.position,
                &self.rules.serialize.prefix,
                &self.rules.serialize.suffix,
            );
        }

        if self.rules.prefix.enabled {
            stem = format!("{}{stem}", self.rules.prefix.text);
        }

        if self.rules.suffix.enabled {
            stem.push_str(&self.rules.suffix.text);
        }

        if self.rules.extension.enabled {
            extension = normalize_extension(&self.rules.extension.value);
        }

        if self.rules.case.enabled {
            stem = apply_case(&stem, self.rules.case.mode);
        }

        NameParts { stem, extension }
    }
}

fn usize_from(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn join_name(stem: &str, extension: &str) -> String {
    if extension.is_empty() {
        stem.to_owned()
    } else {
        format!("{stem}.{extension}")
    }
}

fn normalize_extension(value: &str) -> String {
    value.trim().trim_start_matches('.').to_owned()
}

fn insert_text(stem: &str, text: &str, position: usize, anchor: InsertAnchor) -> String {
    let graphemes: Vec<&str> = stem.graphemes(true).collect();
    let len = graphemes.len();
    // `Before` counts from the start of the name; `After` counts from the end,
    // where 0 is the very end and 5 is five graphemes before it.
    let index = match anchor {
        InsertAnchor::Before => position.min(len),
        InsertAnchor::After => len.saturating_sub(position.min(len)),
    };

    let mut result = String::with_capacity(stem.len() + text.len());
    result.extend(graphemes[..index].iter().copied());
    result.push_str(text);
    result.extend(graphemes[index..].iter().copied());
    result
}

fn keep_text(stem: &str, count: usize, origin: CharacterOrigin, position: usize) -> String {
    let graphemes: Vec<&str> = stem.graphemes(true).collect();
    let len = graphemes.len();

    let (start, end) = match origin {
        CharacterOrigin::Before => {
            let start = position.min(len);
            (start, start.saturating_add(count).min(len))
        }
        CharacterOrigin::After => {
            let end = len.saturating_sub(position.min(len));
            (end.saturating_sub(count), end)
        }
    };

    graphemes[start..end].concat()
}

fn move_text(
    stem: &str,
    start: usize,
    length: usize,
    anchor: MoveAnchor,
    destination: CharacterOrigin,
    offset: usize,
) -> String {
    let graphemes: Vec<&str> = stem.graphemes(true).collect();
    let len = graphemes.len();

    let (slice_start, slice_end) = match anchor {
        MoveAnchor::Start => {
            let start = start.min(len);
            (start, start.saturating_add(length).min(len))
        }
        MoveAnchor::End => {
            let end = len.saturating_sub(start.min(len));
            (end.saturating_sub(length), end)
        }
    };

    if slice_start == slice_end {
        return stem.to_owned();
    }

    let moved: String = graphemes[slice_start..slice_end].concat();
    let remaining: Vec<&str> = graphemes[..slice_start]
        .iter()
        .chain(graphemes[slice_end..].iter())
        .copied()
        .collect();
    let insert_at = match destination {
        CharacterOrigin::Before => offset.min(remaining.len()),
        CharacterOrigin::After => remaining.len().saturating_sub(offset.min(remaining.len())),
    };

    let mut result = String::with_capacity(stem.len());
    result.extend(remaining[..insert_at].iter().copied());
    result.push_str(&moved);
    result.extend(remaining[insert_at..].iter().copied());
    result
}

fn remove_chars(stem: &str, count: usize, origin: CharacterOrigin, position: usize) -> String {
    let graphemes: Vec<&str> = stem.graphemes(true).collect();
    let len = graphemes.len();

    let (start, end) = match origin {
        CharacterOrigin::Before => {
            let start = position.min(len);
            (start, start.saturating_add(count).min(len))
        }
        CharacterOrigin::After => {
            let end = len.saturating_sub(position.min(len));
            (end.saturating_sub(count), end)
        }
    };

    let mut result = String::with_capacity(stem.len());
    result.extend(graphemes[..start].iter().copied());
    result.extend(graphemes[end..].iter().copied());
    result
}

fn serialize(
    stem: &str,
    number: u64,
    padding: u32,
    position: SerializePosition,
    prefix: &str,
    suffix: &str,
) -> String {
    let width = usize_from(padding);
    let digits = format!("{number:0width$}");
    let token = format!("{prefix}{digits}{suffix}");

    if stem.is_empty() {
        return token;
    }

    match position {
        SerializePosition::After => format!("{stem}{token}"),
        SerializePosition::Before => format!("{token}{stem}"),
    }
}

fn apply_case(stem: &str, mode: CaseMode) -> String {
    match mode {
        CaseMode::Lower => stem.to_lowercase(),
        CaseMode::Upper => stem.to_uppercase(),
        CaseMode::Title => {
            let mut result = String::with_capacity(stem.len());
            let mut capitalize_next = true;

            for character in stem.chars() {
                if character.is_alphanumeric() {
                    if capitalize_next {
                        result.extend(character.to_uppercase());
                        capitalize_next = false;
                    } else {
                        result.extend(character.to_lowercase());
                    }
                } else {
                    result.push(character);
                    // Apostrophes stay inside a word: "don't" -> "Don't".
                    if character != '\'' && character != '’' {
                        capitalize_next = true;
                    }
                }
            }

            result
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn entry(name: &str) -> FileEntry {
        FileEntry {
            id: 0,
            path: PathBuf::from("/tmp").join(name),
            original_name: name.to_owned(),
            created: None,
            modified: None,
            size: 0,
            utf8_name: true,
        }
    }

    fn apply(rules: &RuleSet, name: &str, index: usize) -> String {
        CompiledRules::new(rules).apply(&entry(name), index)
    }

    #[test]
    fn new_name_and_serialize() {
        let mut rules = RuleSet::default();
        rules.new_name.enabled = true;
        rules.new_name.value = "París Winter 2017".to_owned();
        rules.serialize.enabled = true;

        assert_eq!(
            apply(&rules, "HEC-0941.jpg", 0),
            "001 París Winter 2017.jpg"
        );
        assert_eq!(
            apply(&rules, "HEC-0942.jpg", 1),
            "002 París Winter 2017.jpg"
        );
    }

    #[test]
    fn rule_order_matches_the_interface() {
        let mut rules = RuleSet::default();
        rules.new_name.enabled = true;
        rules.new_name.value = "Pic".to_owned();
        rules.serialize.enabled = true;
        rules.prefix.enabled = true;
        rules.prefix.text = "X ".to_owned();
        rules.suffix.enabled = true;
        rules.suffix.text = "!".to_owned();

        assert_eq!(apply(&rules, "a.jpg", 0), "X 001 Pic!.jpg");
    }

    #[test]
    fn replace_supports_case_and_words() {
        let mut rules = RuleSet::default();
        rules.replace.enabled = true;
        rules.replace.find = "cat".to_owned();
        rules.replace.replace = "dog".to_owned();
        rules.replace.case_sensitive = false;

        assert_eq!(apply(&rules, "Cat catalog.txt", 0), "dog dogalog.txt");

        rules.replace.whole_word = true;
        assert_eq!(apply(&rules, "Cat catalog.txt", 0), "dog catalog.txt");
    }

    #[test]
    fn regex_supports_capture_groups() {
        let mut rules = RuleSet::default();
        rules.regex.enabled = true;
        rules.regex.pattern = r"(\d+)".to_owned();
        rules.regex.replacement = "[$1]".to_owned();

        assert_eq!(apply(&rules, "IMG 1234.jpg", 0), "IMG [1234].jpg");
    }

    #[test]
    fn insert_and_remove_count_graphemes() {
        assert_eq!(insert_text("abc", "X", 0, InsertAnchor::Before), "Xabc");
        assert_eq!(insert_text("abc", "X", 1, InsertAnchor::Before), "aXbc");
        assert_eq!(insert_text("abc", "X", 99, InsertAnchor::Before), "abcX");
        assert_eq!(insert_text("abc", "X", 0, InsertAnchor::After), "abcX");
        assert_eq!(insert_text("abc", "X", 1, InsertAnchor::After), "abXc");
        assert_eq!(insert_text("abc", "X", 3, InsertAnchor::After), "Xabc");
        assert_eq!(insert_text("abc", "X", 99, InsertAnchor::After), "Xabc");

        let combined = "e\u{301}x";
        assert_eq!(
            insert_text(combined, "!", 1, InsertAnchor::Before),
            "e\u{301}!x"
        );
        assert_eq!(
            insert_text(combined, "!", 1, InsertAnchor::After),
            "e\u{301}!x"
        );
        assert_eq!(
            remove_chars("e\u{301}x", 1, CharacterOrigin::Before, 0),
            "x"
        );
        assert_eq!(
            remove_chars("abcdef", 2, CharacterOrigin::Before, 1),
            "adef"
        );
        assert_eq!(
            remove_chars("abcdef", 0, CharacterOrigin::Before, 3),
            "abcdef"
        );
        assert_eq!(remove_chars("abc", 5, CharacterOrigin::Before, 1), "a");
        assert_eq!(remove_chars("abc", 2, CharacterOrigin::Before, 99), "abc");
        assert_eq!(remove_chars("abcdef", 2, CharacterOrigin::After, 0), "abcd");
        assert_eq!(remove_chars("abcdef", 2, CharacterOrigin::After, 1), "abcf");
        assert_eq!(remove_chars("abc", 2, CharacterOrigin::After, 99), "abc");
    }

    #[test]
    fn keep_defaults_to_one_character_from_the_start() {
        let rule = KeepRule::default();
        assert!(!rule.enabled);
        assert_eq!(rule.count, 1);
        assert_eq!(rule.origin, CharacterOrigin::Before);
        assert_eq!(rule.position, 0);

        let mut rules = RuleSet::default();
        rules.keep.enabled = true;
        assert!(rules.has_name_effects());
    }

    #[test]
    fn keep_selects_characters_from_start_or_end() {
        assert_eq!(keep_text("abcdef", 2, CharacterOrigin::Before, 0), "ab");
        assert_eq!(keep_text("abcdef", 2, CharacterOrigin::Before, 2), "cd");
        assert_eq!(keep_text("abcdef", 2, CharacterOrigin::After, 0), "ef");
        assert_eq!(keep_text("abcdef", 2, CharacterOrigin::After, 2), "cd");
        assert_eq!(
            keep_text("abcdef", 99, CharacterOrigin::Before, 0),
            "abcdef"
        );
        assert_eq!(keep_text("abcdef", 2, CharacterOrigin::Before, 99), "");
        assert_eq!(keep_text("abcdef", 2, CharacterOrigin::After, 99), "");

        let combined = "e\u{301}x";
        assert_eq!(
            keep_text(combined, 1, CharacterOrigin::Before, 0),
            "e\u{301}"
        );
        assert_eq!(keep_text(combined, 1, CharacterOrigin::After, 0), "x");
    }

    #[test]
    fn keep_runs_before_remove() {
        let mut rules = RuleSet::default();
        rules.new_name.enabled = true;
        rules.new_name.value = "abcdef".to_owned();
        rules.keep.enabled = true;
        rules.keep.count = 3;
        rules.keep.origin = CharacterOrigin::Before;
        rules.keep.position = 1;
        rules.remove.enabled = true;
        rules.remove.count = 1;
        rules.remove.origin = CharacterOrigin::Before;
        rules.remove.position = 0;

        assert_eq!(apply(&rules, "original.txt", 0), "cd.txt");
    }

    #[test]
    fn move_defaults_to_one_character() {
        let rule = MoveRule::default();
        assert!(!rule.enabled);
        assert_eq!(rule.start, 0);
        assert_eq!(rule.length, 1);
        assert_eq!(rule.anchor, MoveAnchor::Start);
        assert_eq!(rule.destination, CharacterOrigin::Before);
        assert_eq!(rule.offset, 0);

        let mut rules = RuleSet::default();
        rules.move_rule.enabled = true;
        assert!(rules.has_name_effects());
    }

    #[test]
    fn move_relocates_a_range_from_start_or_end() {
        assert_eq!(
            move_text(
                "abcdef",
                0,
                1,
                MoveAnchor::Start,
                CharacterOrigin::Before,
                0
            ),
            "abcdef"
        );
        assert_eq!(
            move_text(
                "abcdef",
                1,
                1,
                MoveAnchor::Start,
                CharacterOrigin::Before,
                0
            ),
            "bacdef"
        );
        assert_eq!(
            move_text(
                "abcdef",
                0,
                2,
                MoveAnchor::Start,
                CharacterOrigin::Before,
                2
            ),
            "cdabef"
        );
        assert_eq!(
            move_text("abcdef", 0, 1, MoveAnchor::Start, CharacterOrigin::After, 0),
            "bcdefa"
        );
        assert_eq!(
            move_text("abcdef", 0, 1, MoveAnchor::Start, CharacterOrigin::After, 1),
            "bcdeaf"
        );
        assert_eq!(
            move_text("abcdef", 0, 2, MoveAnchor::End, CharacterOrigin::Before, 0),
            "efabcd"
        );
        assert_eq!(
            move_text("abcdef", 2, 1, MoveAnchor::End, CharacterOrigin::Before, 0),
            "dabcef"
        );
        assert_eq!(
            move_text(
                "e\u{301}x",
                0,
                1,
                MoveAnchor::End,
                CharacterOrigin::Before,
                0
            ),
            "xe\u{301}"
        );
        assert_eq!(
            move_text(
                "abcdef",
                99,
                1,
                MoveAnchor::Start,
                CharacterOrigin::Before,
                0
            ),
            "abcdef"
        );
    }

    #[test]
    fn move_runs_after_remove() {
        let mut rules = RuleSet::default();
        rules.new_name.enabled = true;
        rules.new_name.value = "abcd".to_owned();
        rules.remove.enabled = true;
        rules.remove.count = 1;
        rules.remove.origin = CharacterOrigin::Before;
        rules.remove.position = 0;
        rules.move_rule.enabled = true;
        rules.move_rule.start = 0;
        rules.move_rule.length = 1;
        rules.move_rule.anchor = MoveAnchor::Start;
        rules.move_rule.destination = CharacterOrigin::After;
        rules.move_rule.offset = 0;

        assert_eq!(apply(&rules, "original.txt", 0), "cdb.txt");
    }

    #[test]
    fn remove_defaults_to_one_character() {
        let mut rules = RuleSet::default();
        assert_eq!(rules.remove.count, 1);

        rules.remove.enabled = true;
        assert_eq!(apply(&rules, "abcdef.txt", 0), "bcdef.txt");
    }

    #[test]
    fn remove_rule_uses_origin_and_position() {
        let mut rules = RuleSet::default();
        rules.remove.enabled = true;
        rules.remove.count = 2;
        rules.remove.position = 1;

        assert_eq!(apply(&rules, "abcdef.txt", 0), "adef.txt");

        rules.remove.origin = CharacterOrigin::After;
        rules.remove.position = 0;
        assert_eq!(apply(&rules, "abcdef.txt", 0), "abcd.txt");
    }

    #[test]
    fn extension_and_case() {
        let mut rules = RuleSet::default();
        rules.extension.enabled = true;
        rules.extension.value = ".PNG".to_owned();
        assert_eq!(apply(&rules, "photo.jpg", 0), "photo.PNG");

        rules.extension.value.clear();
        assert_eq!(apply(&rules, "photo.jpg", 0), "photo");

        rules.extension.enabled = false;
        rules.case.enabled = true;
        rules.case.mode = CaseMode::Title;
        assert_eq!(
            apply(&rules, "HEC-0941 vacation.jpg", 0),
            "Hec-0941 Vacation.jpg"
        );
    }

    #[test]
    fn serialize_defaults_to_prefixing_one_by_one() {
        let rule = SerializeRule::default();
        assert_eq!(rule.start, 1);
        assert_eq!(rule.step, 1);
        assert_eq!(rule.offset, 0);
        assert_eq!(rule.padding, 3);
        assert_eq!(rule.position, SerializePosition::Before);
        assert!(rule.prefix.is_empty());
        assert_eq!(rule.suffix, " ");
    }

    #[test]
    fn serialize_supports_position_padding_and_step() {
        let mut rules = RuleSet::default();
        rules.serialize.enabled = true;
        rules.serialize.padding = 4;
        rules.serialize.start = 10;
        rules.serialize.step = 5;
        rules.serialize.position = SerializePosition::Before;
        rules.serialize.prefix.clear();
        rules.serialize.suffix = "-".to_owned();

        assert_eq!(apply(&rules, "a.txt", 0), "0010-a.txt");
        assert_eq!(apply(&rules, "a.txt", 2), "0020-a.txt");
    }

    #[test]
    fn serialize_offset_is_added_to_the_number() {
        let mut rules = RuleSet::default();
        rules.serialize.enabled = true;
        rules.serialize.start = 1;
        rules.serialize.offset = 2;

        assert_eq!(apply(&rules, "photo.jpg", 0), "003 photo.jpg");
        assert_eq!(apply(&rules, "photo.jpg", 1), "004 photo.jpg");
    }

    #[test]
    fn serialize_prefix_and_suffix_wrap_only_the_number() {
        let mut rules = RuleSet::default();
        rules.serialize.enabled = true;
        rules.serialize.position = SerializePosition::After;
        rules.serialize.prefix = " [".to_owned();
        rules.serialize.suffix = "]".to_owned();

        assert_eq!(apply(&rules, "photo.jpg", 0), "photo [001].jpg");

        rules.serialize.position = SerializePosition::Before;
        rules.serialize.prefix.clear();
        rules.serialize.suffix = " ".to_owned();
        assert_eq!(apply(&rules, "photo.jpg", 0), "001 photo.jpg");
    }

    #[test]
    fn invalid_regex_is_reported_not_applied() {
        let mut rules = RuleSet::default();
        rules.regex.enabled = true;
        rules.regex.pattern = "(".to_owned();

        let compiled = CompiledRules::new(&rules);
        assert_eq!(compiled.errors.len(), 1);
        assert_eq!(compiled.apply(&entry("a.txt"), 0), "a.txt");
    }
}
