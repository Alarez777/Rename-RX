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
    pub remove: RemoveRule,
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
            || self.remove.enabled
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RemoveRule {
    pub enabled: bool,
    pub count: u32,
    pub position: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SerializeRule {
    pub enabled: bool,
    pub start: i64,
    pub step: i64,
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
            padding: 3,
            position: SerializePosition::After,
            // A leading space preserves the original `name 001` look;
            // the space is part of the prefix rather than a separate separator.
            prefix: " ".to_owned(),
            suffix: String::new(),
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

        if self.rules.remove.enabled && self.rules.remove.count > 0 {
            stem = remove_chars(
                &stem,
                self.rules.remove.count as usize,
                self.rules.remove.position as usize,
            );
        }

        if self.rules.serialize.enabled {
            stem = serialize(
                &stem,
                self.rules.serialize.start + index as i64 * self.rules.serialize.step,
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
    let index = match anchor {
        InsertAnchor::Before => position,
        InsertAnchor::After => position.saturating_add(1),
    }
    .min(graphemes.len());

    let mut result = String::with_capacity(stem.len() + text.len());
    result.extend(graphemes[..index].iter().copied());
    result.push_str(text);
    result.extend(graphemes[index..].iter().copied());
    result
}

fn remove_chars(stem: &str, count: usize, position: usize) -> String {
    let graphemes: Vec<&str> = stem.graphemes(true).collect();
    let start = position.min(graphemes.len());
    let end = start.saturating_add(count).min(graphemes.len());

    let mut result = String::with_capacity(stem.len());
    result.extend(graphemes[..start].iter().copied());
    result.extend(graphemes[end..].iter().copied());
    result
}

fn serialize(
    stem: &str,
    number: i64,
    padding: u32,
    position: SerializePosition,
    prefix: &str,
    suffix: &str,
) -> String {
    let width = padding as usize;
    let digits = if number < 0 {
        format!("-{:0width$}", number.unsigned_abs())
    } else {
        format!("{number:0width$}")
    };

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
            "París Winter 2017 001.jpg"
        );
        assert_eq!(
            apply(&rules, "HEC-0942.jpg", 1),
            "París Winter 2017 002.jpg"
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

        assert_eq!(apply(&rules, "a.jpg", 0), "X Pic 001!.jpg");
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
        assert_eq!(insert_text("abc", "X", 0, InsertAnchor::After), "aXbc");
        assert_eq!(insert_text("abc", "X", 99, InsertAnchor::After), "abcX");

        let combined = "e\u{301}x";
        assert_eq!(
            insert_text(combined, "!", 1, InsertAnchor::Before),
            "e\u{301}!x"
        );
        assert_eq!(remove_chars("e\u{301}x", 1, 0), "x");
        assert_eq!(remove_chars("abcdef", 2, 1), "adef");
        assert_eq!(remove_chars("abcdef", 0, 3), "abcdef");
        assert_eq!(remove_chars("abc", 5, 1), "a");
        assert_eq!(remove_chars("abc", 2, 99), "abc");
    }

    #[test]
    fn remove_rule_starts_at_the_given_position() {
        let mut rules = RuleSet::default();
        rules.remove.enabled = true;
        rules.remove.count = 2;
        rules.remove.position = 1;

        assert_eq!(apply(&rules, "abcdef.txt", 0), "adef.txt");
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
    fn serialize_prefix_and_suffix_wrap_only_the_number() {
        let mut rules = RuleSet::default();
        rules.serialize.enabled = true;
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
