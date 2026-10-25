use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::file::FileEntry;

/// All rename rules, in the order they are applied.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RuleSet {
    pub new_name: NewNameRule,
    pub replace: ReplaceRule,
    pub regex: RegexRule,
    pub insert: InsertRule,
    pub keep: KeepRule,
    pub remove: RemoveRule,
    pub move_rule: MoveRule,
    pub serialize: SerializeRule,
    pub prefix: TextRule,
    pub suffix: TextRule,
    pub extension: ExtensionRule,
    pub case: CaseRule,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct NewNameRule {
    pub enabled: bool,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ReplaceRule {
    pub enabled: bool,
    pub find: String,
    pub replace: String,
    pub case_sensitive: bool,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RegexRule {
    pub enabled: bool,
    pub pattern: String,
    pub replacement: String,
    pub case_insensitive: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct InsertRule {
    pub enabled: bool,
    pub text: String,
    pub position: u32,
    pub anchor: InsertAnchor,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertAnchor {
    #[default]
    Before,
    After,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct KeepRule {
    pub enabled: bool,
    pub count: u32,
    pub origin: CharacterOrigin,
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RemoveRule {
    pub enabled: bool,
    pub count: u32,
    pub origin: CharacterOrigin,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterOrigin {
    #[default]
    Before,
    After,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MoveRule {
    pub enabled: bool,
    pub start: u32,
    pub length: u32,
    pub anchor: MoveAnchor,
    pub destination: CharacterOrigin,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveAnchor {
    #[default]
    Start,
    End,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SerializeRule {
    pub enabled: bool,
    pub start: u32,
    pub step: u32,
    pub offset: u32,
    pub padding: u32,
    pub position: SerializePosition,
    pub prefix: String,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SerializePosition {
    #[default]
    After,
    Before,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct TextRule {
    pub enabled: bool,
    pub text: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ExtensionRule {
    pub enabled: bool,
    pub value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CaseRule {
    pub enabled: bool,
    pub mode: CaseMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseMode {
    #[default]
    Lower,
    Title,
    Upper,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SortRule {
    pub enabled: bool,
    pub method: SortMethod,
    pub descending: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMethod {
    #[default]
    Original,
    Name,
    Created,
    Modified,
}

/// The two parts a rename rule operates on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameParts {
    pub stem: String,
    pub extension: String,
}

impl NameParts {
    pub fn file_name(&self) -> String {
        join_name(&self.stem, &self.extension)
    }
}

/// Rules compiled once for a whole preview pass.
pub struct CompiledRules<'a> {
    rules: &'a RuleSet,
    regex: Option<Regex>,
    replace: Option<Regex>,
    /// Rule-level problems, such as an invalid regular expression.
    pub errors: Vec<String>,
}

impl<'a> CompiledRules<'a> {
    pub fn new(rules: &'a RuleSet) -> Self {
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

    /// Apply the pipeline to one file. `index` is the position of the file in
    /// the current (sorted) order and drives serialization.
    pub fn apply(&self, file: &FileEntry, index: usize) -> String {
        self.apply_parts(file, index).file_name()
    }

    /// Apply the pipeline and return the stem and extension separately.
    pub fn apply_parts(&self, file: &FileEntry, index: usize) -> NameParts {
        let mut stem = file.stem.clone();
        let mut extension = file.extension.clone();

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
                self.rules.insert.position as usize,
                self.rules.insert.anchor,
            );
        }

        if self.rules.keep.enabled && self.rules.keep.count > 0 {
            stem = keep_text(
                &stem,
                self.rules.keep.count as usize,
                self.rules.keep.origin,
                self.rules.keep.position as usize,
            );
        }

        if self.rules.remove.enabled && self.rules.remove.count > 0 {
            stem = remove_chars(
                &stem,
                self.rules.remove.count as usize,
                self.rules.remove.origin,
                self.rules.remove.position as usize,
            );
        }

        if self.rules.move_rule.enabled && self.rules.move_rule.length > 0 {
            stem = move_text(
                &stem,
                self.rules.move_rule.start as usize,
                self.rules.move_rule.length as usize,
                self.rules.move_rule.anchor,
                self.rules.move_rule.destination,
                self.rules.move_rule.offset as usize,
            );
        }

        if self.rules.serialize.enabled {
            let number = u64::from(self.rules.serialize.start)
                + index as u64 * u64::from(self.rules.serialize.step)
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

/// Apply every rule except sorting to a file name. Kept as a convenience for
/// callers that only need a single result (tests, future single-file previews).
pub fn apply_rules(file: &FileEntry, rules: &RuleSet, index: usize) -> String {
    CompiledRules::new(rules).apply(file, index)
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
    let width = padding as usize;
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
    use crate::file::split_file_name;

    fn entry(name: &str) -> FileEntry {
        let (stem, extension) = split_file_name(name);
        FileEntry {
            id: 0,
            path: PathBuf::from("/tmp").join(name),
            original_name: name.to_owned(),
            stem: stem.to_owned(),
            extension: extension.to_owned(),
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
