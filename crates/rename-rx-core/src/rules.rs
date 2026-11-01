use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::file::FileEntry;

/// All rename rules, in the order they are applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RuleSetWire", into = "RuleSetWire")]
pub struct RuleSet {
    rules: Vec<Rule>,
    sort: SortRule,
}

impl Default for RuleSet {
    fn default() -> Self {
        Self {
            rules: RuleKind::ALL
                .iter()
                .copied()
                .map(Rule::default_for)
                .collect(),
            sort: SortRule::default(),
        }
    }
}

impl RuleSet {
    /// The configured rules, in execution order.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// The configured rules, in execution order, for editing.
    pub fn rules_mut(&mut self) -> &mut [Rule] {
        &mut self.rules
    }

    /// The rule that changes the display order and serial-number order.
    pub fn sort(&self) -> &SortRule {
        &self.sort
    }

    /// The ordering rule, for editing.
    pub fn sort_mut(&mut self) -> &mut SortRule {
        &mut self.sort
    }

    /// Inserts `rule` at `index`. An index past the end appends the rule.
    pub fn insert(&mut self, index: usize, rule: Rule) {
        let index = index.min(self.rules.len());
        self.rules.insert(index, rule);
    }

    /// Removes and returns the rule at `index`, if it exists.
    pub fn remove(&mut self, index: usize) -> Option<Rule> {
        if index >= self.rules.len() {
            return None;
        }

        Some(self.rules.remove(index))
    }

    /// True when at least one rule can change a file name. Sorting only changes
    /// the order and is deliberately excluded.
    pub fn has_name_effects(&self) -> bool {
        self.rules.iter().any(Rule::enabled)
    }

    /// Rebuild from serialized parts, adding any missing rule kinds.
    fn from_parts(mut rules: Vec<Rule>, sort: SortRule) -> Self {
        for kind in RuleKind::ALL {
            if rules.iter().any(|rule| rule.kind() == kind) {
                continue;
            }

            let index = rules
                .iter()
                .position(|rule| kind < rule.kind())
                .unwrap_or(rules.len());
            rules.insert(index, Rule::default_for(kind));
        }

        Self { rules, sort }
    }
}

/// The serialized form of [`RuleSet`], accepting the pre-list format too.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum RuleSetWire {
    Current(CurrentRuleSet),
    Legacy(Box<LegacyRuleSet>),
}

#[derive(Serialize, Deserialize)]
struct CurrentRuleSet {
    rules: Vec<Rule>,
    #[serde(default)]
    sort: SortRule,
}

/// Presets written before rules became an ordered list.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct LegacyRuleSet {
    new_name: NewNameRule,
    replace: ReplaceRule,
    regex: RegexRule,
    insert: InsertRule,
    keep: KeepRule,
    remove: RemoveRule,
    move_rule: MoveRule,
    serialize: SerializeRule,
    prefix: TextRule,
    suffix: TextRule,
    extension: ExtensionRule,
    case: CaseRule,
    sort: SortRule,
}

impl From<RuleSet> for RuleSetWire {
    fn from(value: RuleSet) -> Self {
        Self::Current(CurrentRuleSet {
            rules: value.rules,
            sort: value.sort,
        })
    }
}

impl From<RuleSetWire> for RuleSet {
    fn from(value: RuleSetWire) -> Self {
        match value {
            RuleSetWire::Current(current) => Self::from_parts(current.rules, current.sort),
            RuleSetWire::Legacy(legacy) => (*legacy).into(),
        }
    }
}

impl From<LegacyRuleSet> for RuleSet {
    fn from(value: LegacyRuleSet) -> Self {
        Self::from_parts(
            vec![
                Rule::NewName(value.new_name),
                Rule::Replace(value.replace),
                Rule::Regex(value.regex),
                Rule::Insert(value.insert),
                Rule::Keep(value.keep),
                Rule::Remove(value.remove),
                Rule::Move(value.move_rule),
                Rule::Serialize(value.serialize),
                Rule::Prefix(value.prefix),
                Rule::Suffix(value.suffix),
                Rule::Extension(value.extension),
                Rule::Case(value.case),
            ],
            value.sort,
        )
    }
}

/// One configured rename rule.
///
/// Rules run in the order they appear in [`RuleSet`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tool", content = "config", rename_all = "snake_case")]
pub enum Rule {
    /// Replaces the whole base name.
    NewName(NewNameRule),
    /// Literal text replacement, optionally whole-word.
    Replace(ReplaceRule),
    /// Regular-expression replacement.
    Regex(RegexRule),
    /// Inserts text at a character position.
    Insert(InsertRule),
    /// Keeps only one range of characters.
    Keep(KeepRule),
    /// Removes one range of characters.
    Remove(RemoveRule),
    /// Moves one range of characters elsewhere in the name.
    Move(MoveRule),
    /// Adds a sequential number.
    Serialize(SerializeRule),
    /// Adds a fixed prefix.
    Prefix(TextRule),
    /// Adds a fixed suffix.
    Suffix(TextRule),
    /// Replaces the extension.
    Extension(ExtensionRule),
    /// Changes the case of the base name.
    Case(CaseRule),
}

/// The kind of a [`Rule`], without its settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuleKind {
    /// Replaces the whole base name.
    NewName,
    /// Literal text replacement.
    Replace,
    /// Regular-expression replacement.
    Regex,
    /// Inserts text.
    Insert,
    /// Keeps one range of characters.
    Keep,
    /// Removes one range of characters.
    Remove,
    /// Moves one range of characters.
    Move,
    /// Adds a sequential number.
    Serialize,
    /// Adds a fixed prefix.
    Prefix,
    /// Adds a fixed suffix.
    Suffix,
    /// Replaces the extension.
    Extension,
    /// Changes the case.
    Case,
}

impl RuleKind {
    /// Every kind, in the default execution order.
    pub const ALL: [Self; 12] = [
        Self::NewName,
        Self::Replace,
        Self::Regex,
        Self::Insert,
        Self::Keep,
        Self::Remove,
        Self::Move,
        Self::Serialize,
        Self::Prefix,
        Self::Suffix,
        Self::Extension,
        Self::Case,
    ];
}

impl Rule {
    /// The rule's kind, without its settings.
    pub const fn kind(&self) -> RuleKind {
        match self {
            Self::NewName(_) => RuleKind::NewName,
            Self::Replace(_) => RuleKind::Replace,
            Self::Regex(_) => RuleKind::Regex,
            Self::Insert(_) => RuleKind::Insert,
            Self::Keep(_) => RuleKind::Keep,
            Self::Remove(_) => RuleKind::Remove,
            Self::Move(_) => RuleKind::Move,
            Self::Serialize(_) => RuleKind::Serialize,
            Self::Prefix(_) => RuleKind::Prefix,
            Self::Suffix(_) => RuleKind::Suffix,
            Self::Extension(_) => RuleKind::Extension,
            Self::Case(_) => RuleKind::Case,
        }
    }

    /// Whether the rule runs during a preview or rename.
    pub const fn enabled(&self) -> bool {
        match self {
            Self::NewName(rule) => rule.enabled,
            Self::Replace(rule) => rule.enabled,
            Self::Regex(rule) => rule.enabled,
            Self::Insert(rule) => rule.enabled,
            Self::Keep(rule) => rule.enabled,
            Self::Remove(rule) => rule.enabled,
            Self::Move(rule) => rule.enabled,
            Self::Serialize(rule) => rule.enabled,
            Self::Prefix(rule) | Self::Suffix(rule) => rule.enabled,
            Self::Extension(rule) => rule.enabled,
            Self::Case(rule) => rule.enabled,
        }
    }

    /// Turns the rule on.
    pub fn enable(&mut self) {
        *self.enabled_mut() = true;
    }

    /// Turns the rule off.
    pub fn disable(&mut self) {
        *self.enabled_mut() = false;
    }

    /// A new rule of `kind` with default settings, switched off.
    pub fn default_for(kind: RuleKind) -> Self {
        match kind {
            RuleKind::NewName => Self::NewName(NewNameRule::default()),
            RuleKind::Replace => Self::Replace(ReplaceRule::default()),
            RuleKind::Regex => Self::Regex(RegexRule::default()),
            RuleKind::Insert => Self::Insert(InsertRule::default()),
            RuleKind::Keep => Self::Keep(KeepRule::default()),
            RuleKind::Remove => Self::Remove(RemoveRule::default()),
            RuleKind::Move => Self::Move(MoveRule::default()),
            RuleKind::Serialize => Self::Serialize(SerializeRule::default()),
            RuleKind::Prefix => Self::Prefix(TextRule::default()),
            RuleKind::Suffix => Self::Suffix(TextRule::default()),
            RuleKind::Extension => Self::Extension(ExtensionRule::default()),
            RuleKind::Case => Self::Case(CaseRule::default()),
        }
    }

    fn enabled_mut(&mut self) -> &mut bool {
        match self {
            Self::NewName(rule) => &mut rule.enabled,
            Self::Replace(rule) => &mut rule.enabled,
            Self::Regex(rule) => &mut rule.enabled,
            Self::Insert(rule) => &mut rule.enabled,
            Self::Keep(rule) => &mut rule.enabled,
            Self::Remove(rule) => &mut rule.enabled,
            Self::Move(rule) => &mut rule.enabled,
            Self::Serialize(rule) => &mut rule.enabled,
            Self::Prefix(rule) | Self::Suffix(rule) => &mut rule.enabled,
            Self::Extension(rule) => &mut rule.enabled,
            Self::Case(rule) => &mut rule.enabled,
        }
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

/// One rule paired with its compiled regular expression, when it has one.
#[derive(Debug)]
struct CompiledRule<'rules> {
    rule: &'rules Rule,
    regex: Option<Regex>,
}

/// Rules compiled once for a whole preview pass.
#[derive(Debug)]
pub struct CompiledRules<'rules> {
    rules: Vec<CompiledRule<'rules>>,
    /// Rule-level problems, such as an invalid regular expression.
    errors: Vec<String>,
}

impl<'rules> CompiledRules<'rules> {
    /// Compile the regular expressions and report rule-level problems.
    pub fn new(rules: &'rules RuleSet) -> Self {
        let mut errors = Vec::new();
        let mut compiled = Vec::with_capacity(rules.rules().len());

        for rule in rules.rules() {
            let regex = match rule {
                Rule::Regex(rule) if rule.enabled && !rule.pattern.is_empty() => {
                    match RegexBuilder::new(&rule.pattern)
                        .case_insensitive(rule.case_insensitive)
                        .build()
                    {
                        Ok(regex) => Some(regex),
                        Err(error) => {
                            errors.push(format!("Regular expression: {error}"));
                            None
                        }
                    }
                }
                Rule::Replace(rule) if rule.enabled && !rule.find.is_empty() => {
                    let pattern = if rule.whole_word {
                        format!(r"\b{}\b", regex::escape(&rule.find))
                    } else {
                        regex::escape(&rule.find)
                    };

                    match RegexBuilder::new(&pattern)
                        .case_insensitive(!rule.case_sensitive)
                        .build()
                    {
                        Ok(regex) => Some(regex),
                        Err(error) => {
                            errors.push(format!("Replace rule: {error}"));
                            None
                        }
                    }
                }
                _ => None,
            };

            compiled.push(CompiledRule { rule, regex });
        }

        Self {
            rules: compiled,
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

        for compiled in &self.rules {
            match compiled.rule {
                Rule::NewName(rule) => {
                    if rule.enabled {
                        stem.clone_from(&rule.value);
                    }
                }
                Rule::Replace(rule) => {
                    if let Some(regex) = &compiled.regex {
                        stem = regex.replace_all(&stem, rule.replace.as_str()).into_owned();
                    }
                }
                Rule::Regex(rule) => {
                    if let Some(regex) = &compiled.regex {
                        stem = regex
                            .replace_all(&stem, rule.replacement.as_str())
                            .into_owned();
                    }
                }
                Rule::Insert(rule) => {
                    if rule.enabled && !rule.text.is_empty() {
                        stem =
                            insert_text(&stem, &rule.text, usize_from(rule.position), rule.anchor);
                    }
                }
                Rule::Keep(rule) => {
                    if rule.enabled && rule.count > 0 {
                        stem = keep_text(
                            &stem,
                            usize_from(rule.count),
                            rule.origin,
                            usize_from(rule.position),
                        );
                    }
                }
                Rule::Remove(rule) => {
                    if rule.enabled && rule.count > 0 {
                        stem = remove_chars(
                            &stem,
                            usize_from(rule.count),
                            rule.origin,
                            usize_from(rule.position),
                        );
                    }
                }
                Rule::Move(rule) => {
                    if rule.enabled && rule.length > 0 {
                        stem = move_text(
                            &stem,
                            usize_from(rule.start),
                            usize_from(rule.length),
                            rule.anchor,
                            rule.destination,
                            usize_from(rule.offset),
                        );
                    }
                }
                Rule::Serialize(rule) => {
                    if rule.enabled {
                        let number = u64::from(rule.start)
                            + u64::try_from(index).unwrap_or(u64::MAX) * u64::from(rule.step)
                            + u64::from(rule.offset);
                        stem = serialize(
                            &stem,
                            number,
                            rule.padding,
                            rule.position,
                            &rule.prefix,
                            &rule.suffix,
                        );
                    }
                }
                Rule::Prefix(rule) => {
                    if rule.enabled {
                        stem = format!("{}{stem}", rule.text);
                    }
                }
                Rule::Suffix(rule) => {
                    if rule.enabled {
                        stem.push_str(&rule.text);
                    }
                }
                Rule::Extension(rule) => {
                    if rule.enabled {
                        extension = normalize_extension(&rule.value);
                    }
                }
                Rule::Case(rule) => {
                    if rule.enabled {
                        stem = apply_case(&stem, rule.mode);
                    }
                }
            }
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

    fn rule_mut<'a, T>(
        rules: &'a mut RuleSet,
        extract: impl FnMut(&'a mut Rule) -> Option<&'a mut T>,
    ) -> &'a mut T {
        rules
            .rules_mut()
            .iter_mut()
            .find_map(extract)
            .expect("the default rule set contains every kind")
    }

    fn new_name(rules: &mut RuleSet) -> &mut NewNameRule {
        rule_mut(rules, |rule| match rule {
            Rule::NewName(rule) => Some(rule),
            _ => None,
        })
    }

    fn replace(rules: &mut RuleSet) -> &mut ReplaceRule {
        rule_mut(rules, |rule| match rule {
            Rule::Replace(rule) => Some(rule),
            _ => None,
        })
    }

    fn regex(rules: &mut RuleSet) -> &mut RegexRule {
        rule_mut(rules, |rule| match rule {
            Rule::Regex(rule) => Some(rule),
            _ => None,
        })
    }

    fn keep(rules: &mut RuleSet) -> &mut KeepRule {
        rule_mut(rules, |rule| match rule {
            Rule::Keep(rule) => Some(rule),
            _ => None,
        })
    }

    fn remove(rules: &mut RuleSet) -> &mut RemoveRule {
        rule_mut(rules, |rule| match rule {
            Rule::Remove(rule) => Some(rule),
            _ => None,
        })
    }

    fn move_rule(rules: &mut RuleSet) -> &mut MoveRule {
        rule_mut(rules, |rule| match rule {
            Rule::Move(rule) => Some(rule),
            _ => None,
        })
    }

    fn serialize(rules: &mut RuleSet) -> &mut SerializeRule {
        rule_mut(rules, |rule| match rule {
            Rule::Serialize(rule) => Some(rule),
            _ => None,
        })
    }

    fn prefix(rules: &mut RuleSet) -> &mut TextRule {
        rule_mut(rules, |rule| match rule {
            Rule::Prefix(rule) => Some(rule),
            _ => None,
        })
    }

    fn suffix(rules: &mut RuleSet) -> &mut TextRule {
        rule_mut(rules, |rule| match rule {
            Rule::Suffix(rule) => Some(rule),
            _ => None,
        })
    }

    fn extension(rules: &mut RuleSet) -> &mut ExtensionRule {
        rule_mut(rules, |rule| match rule {
            Rule::Extension(rule) => Some(rule),
            _ => None,
        })
    }

    fn case(rules: &mut RuleSet) -> &mut CaseRule {
        rule_mut(rules, |rule| match rule {
            Rule::Case(rule) => Some(rule),
            _ => None,
        })
    }

    #[test]
    fn new_name_and_serialize() {
        let mut rules = RuleSet::default();
        new_name(&mut rules).enabled = true;
        new_name(&mut rules).value = "París Winter 2017".to_owned();
        serialize(&mut rules).enabled = true;

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
        new_name(&mut rules).enabled = true;
        new_name(&mut rules).value = "Pic".to_owned();
        serialize(&mut rules).enabled = true;
        prefix(&mut rules).enabled = true;
        prefix(&mut rules).text = "X ".to_owned();
        suffix(&mut rules).enabled = true;
        suffix(&mut rules).text = "!".to_owned();

        assert_eq!(apply(&rules, "a.jpg", 0), "X 001 Pic!.jpg");
    }

    #[test]
    fn replace_supports_case_and_words() {
        let mut rules = RuleSet::default();
        replace(&mut rules).enabled = true;
        replace(&mut rules).find = "cat".to_owned();
        replace(&mut rules).replace = "dog".to_owned();
        replace(&mut rules).case_sensitive = false;

        assert_eq!(apply(&rules, "Cat catalog.txt", 0), "dog dogalog.txt");

        replace(&mut rules).whole_word = true;
        assert_eq!(apply(&rules, "Cat catalog.txt", 0), "dog catalog.txt");
    }

    #[test]
    fn regex_supports_capture_groups() {
        let mut rules = RuleSet::default();
        regex(&mut rules).enabled = true;
        regex(&mut rules).pattern = r"(\d+)".to_owned();
        regex(&mut rules).replacement = "[$1]".to_owned();

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
        keep(&mut rules).enabled = true;
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
        new_name(&mut rules).enabled = true;
        new_name(&mut rules).value = "abcdef".to_owned();
        keep(&mut rules).enabled = true;
        keep(&mut rules).count = 3;
        keep(&mut rules).origin = CharacterOrigin::Before;
        keep(&mut rules).position = 1;
        remove(&mut rules).enabled = true;
        remove(&mut rules).count = 1;
        remove(&mut rules).origin = CharacterOrigin::Before;
        remove(&mut rules).position = 0;

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
        move_rule(&mut rules).enabled = true;
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
        new_name(&mut rules).enabled = true;
        new_name(&mut rules).value = "abcd".to_owned();
        remove(&mut rules).enabled = true;
        remove(&mut rules).count = 1;
        remove(&mut rules).origin = CharacterOrigin::Before;
        remove(&mut rules).position = 0;
        move_rule(&mut rules).enabled = true;
        move_rule(&mut rules).start = 0;
        move_rule(&mut rules).length = 1;
        move_rule(&mut rules).anchor = MoveAnchor::Start;
        move_rule(&mut rules).destination = CharacterOrigin::After;
        move_rule(&mut rules).offset = 0;

        assert_eq!(apply(&rules, "original.txt", 0), "cdb.txt");
    }

    #[test]
    fn remove_defaults_to_one_character() {
        let mut rules = RuleSet::default();
        assert_eq!(remove(&mut rules).count, 1);

        remove(&mut rules).enabled = true;
        assert_eq!(apply(&rules, "abcdef.txt", 0), "bcdef.txt");
    }

    #[test]
    fn remove_rule_uses_origin_and_position() {
        let mut rules = RuleSet::default();
        remove(&mut rules).enabled = true;
        remove(&mut rules).count = 2;
        remove(&mut rules).position = 1;

        assert_eq!(apply(&rules, "abcdef.txt", 0), "adef.txt");

        remove(&mut rules).origin = CharacterOrigin::After;
        remove(&mut rules).position = 0;
        assert_eq!(apply(&rules, "abcdef.txt", 0), "abcd.txt");
    }

    #[test]
    fn extension_and_case() {
        let mut rules = RuleSet::default();
        extension(&mut rules).enabled = true;
        extension(&mut rules).value = ".PNG".to_owned();
        assert_eq!(apply(&rules, "photo.jpg", 0), "photo.PNG");

        extension(&mut rules).value.clear();
        assert_eq!(apply(&rules, "photo.jpg", 0), "photo");

        extension(&mut rules).enabled = false;
        case(&mut rules).enabled = true;
        case(&mut rules).mode = CaseMode::Title;
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
        serialize(&mut rules).enabled = true;
        serialize(&mut rules).padding = 4;
        serialize(&mut rules).start = 10;
        serialize(&mut rules).step = 5;
        serialize(&mut rules).position = SerializePosition::Before;
        serialize(&mut rules).prefix.clear();
        serialize(&mut rules).suffix = "-".to_owned();

        assert_eq!(apply(&rules, "a.txt", 0), "0010-a.txt");
        assert_eq!(apply(&rules, "a.txt", 2), "0020-a.txt");
    }

    #[test]
    fn serialize_offset_is_added_to_the_number() {
        let mut rules = RuleSet::default();
        serialize(&mut rules).enabled = true;
        serialize(&mut rules).start = 1;
        serialize(&mut rules).offset = 2;

        assert_eq!(apply(&rules, "photo.jpg", 0), "003 photo.jpg");
        assert_eq!(apply(&rules, "photo.jpg", 1), "004 photo.jpg");
    }

    #[test]
    fn serialize_prefix_and_suffix_wrap_only_the_number() {
        let mut rules = RuleSet::default();
        serialize(&mut rules).enabled = true;
        serialize(&mut rules).position = SerializePosition::After;
        serialize(&mut rules).prefix = " [".to_owned();
        serialize(&mut rules).suffix = "]".to_owned();

        assert_eq!(apply(&rules, "photo.jpg", 0), "photo [001].jpg");

        serialize(&mut rules).position = SerializePosition::Before;
        serialize(&mut rules).prefix.clear();
        serialize(&mut rules).suffix = " ".to_owned();
        assert_eq!(apply(&rules, "photo.jpg", 0), "001 photo.jpg");
    }

    #[test]
    fn invalid_regex_is_reported_not_applied() {
        let mut rules = RuleSet::default();
        regex(&mut rules).enabled = true;
        regex(&mut rules).pattern = "(".to_owned();

        let compiled = CompiledRules::new(&rules);
        assert_eq!(compiled.errors.len(), 1);
        assert_eq!(compiled.apply(&entry("a.txt"), 0), "a.txt");
    }

    #[test]
    fn default_rules_have_every_kind_in_order() {
        let rules = RuleSet::default();
        let kinds: Vec<RuleKind> = rules.rules().iter().map(Rule::kind).collect();

        assert_eq!(kinds, RuleKind::ALL);
        assert!(rules.rules().iter().all(|rule| !rule.enabled()));
        assert!(!rules.sort().enabled);
        assert!(!rules.has_name_effects());
    }

    #[test]
    fn missing_rule_kinds_are_added_when_rebuilding() {
        let rules = RuleSet::from_parts(
            vec![Rule::Replace(ReplaceRule::default())],
            SortRule::default(),
        );
        let kinds: Vec<RuleKind> = rules.rules().iter().map(Rule::kind).collect();

        assert_eq!(kinds, RuleKind::ALL);
    }

    #[test]
    fn repeated_rules_run_in_list_order() {
        let mut rules = RuleSet::default();
        new_name(&mut rules).enabled = true;
        new_name(&mut rules).value = "aa".to_owned();

        let index = rules
            .rules()
            .iter()
            .position(|rule| rule.kind() == RuleKind::Replace)
            .expect("the default rule set contains Replace");
        let a_to_b = ReplaceRule {
            enabled: true,
            find: "a".to_owned(),
            replace: "b".to_owned(),
            ..ReplaceRule::default()
        };
        let b_to_c = ReplaceRule {
            enabled: true,
            find: "b".to_owned(),
            replace: "c".to_owned(),
            ..ReplaceRule::default()
        };

        rules.insert(index, Rule::Replace(a_to_b.clone()));
        rules.insert(index + 1, Rule::Replace(b_to_c.clone()));
        assert_eq!(apply(&rules, "x.txt", 0), "cc.txt");

        let mut reversed = RuleSet::default();
        new_name(&mut reversed).enabled = true;
        new_name(&mut reversed).value = "aa".to_owned();
        reversed.insert(index, Rule::Replace(b_to_c));
        reversed.insert(index + 1, Rule::Replace(a_to_b));
        assert_eq!(apply(&reversed, "x.txt", 0), "bb.txt");
    }

    #[test]
    fn insert_appends_past_the_end_and_remove_is_bounds_checked() {
        let mut rules = RuleSet::default();
        let original_len = rules.rules().len();

        rules.insert(usize::MAX, Rule::default_for(RuleKind::Replace));
        assert_eq!(rules.rules().len(), original_len + 1);
        assert!(rules.remove(original_len + 1).is_none());
        assert!(rules.remove(original_len).is_some());
        assert_eq!(rules.rules().len(), original_len);
    }
}
