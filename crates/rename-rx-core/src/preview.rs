use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::time::SystemTime;

use crate::file::{FileEntry, FileId, split_file_name};
use crate::rules::{CompiledRules, NameParts, RuleSet, SortMethod, SortRule};

/// How serious a validation finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational finding that does not block renaming.
    Info,
    /// Finding that the user should review before renaming.
    Warning,
    /// Finding that blocks renaming.
    Error,
}

/// One validation finding. Rule-level findings (such as an invalid regular
/// expression) have no file id.
#[derive(Clone, Debug)]
pub struct Issue {
    /// File the finding belongs to, or `None` for a rule-level finding.
    pub file_id: Option<FileId>,
    /// How serious the finding is.
    pub severity: Severity,
    /// Human-readable description shown in the validity dialog.
    pub message: String,
}

/// Every finding produced by one preview pass.
#[derive(Clone, Debug, Default)]
pub struct ValidationReport {
    /// Findings, most severe first.
    pub issues: Vec<Issue>,
}

impl ValidationReport {
    /// Number of [`Severity::Error`] findings.
    pub fn errors(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == Severity::Error)
            .count()
    }

    /// Number of [`Severity::Warning`] findings.
    pub fn warnings(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == Severity::Warning)
            .count()
    }

    /// Number of [`Severity::Info`] findings.
    pub fn infos(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == Severity::Info)
            .count()
    }
}

/// Previews and validation for one state of the rule set.
#[derive(Clone, Debug, Default)]
pub struct PreviewResult {
    /// Display order as indices into the input slice.
    pub order: Vec<usize>,
    /// Resulting file name for each input file, indexed like the input slice.
    pub previews: Vec<String>,
    /// Findings produced while validating the previews.
    pub report: ValidationReport,
}

/// Compute the display order, every preview name, and the validation report.
pub fn build_preview(files: &[FileEntry], rules: &RuleSet) -> PreviewResult {
    let order = sorted_indices(files, rules.sort());
    let compiled = CompiledRules::new(rules);
    let mut previews = vec![String::new(); files.len()];
    let mut parts = vec![NameParts::default(); files.len()];

    for (position, &index) in order.iter().enumerate() {
        let result = compiled.apply_parts(&files[index], position);
        previews[index] = result.file_name();
        parts[index] = result;
    }

    let mut report = validate(files, &order, &previews, &parts, rules, compiled.errors());
    // Errors first, then warnings, then informational findings.
    report
        .issues
        .sort_by_key(|issue| std::cmp::Reverse(issue.severity));

    PreviewResult {
        order,
        previews,
        report,
    }
}

/// Sort indices without touching the file list itself.
fn sorted_indices(files: &[FileEntry], sort: &SortRule) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..files.len()).collect();

    if !sort.enabled || sort.method == SortMethod::Original {
        return indices;
    }

    match sort.method {
        SortMethod::Original => {}
        SortMethod::Name => {
            indices.sort_by_cached_key(|&index| files[index].original_name.to_lowercase());
        }
        SortMethod::Created => {
            indices.sort_by(|&a, &b| {
                compare_optional_time(files[a].display_time(), files[b].display_time()).then_with(
                    || {
                        files[a]
                            .original_name
                            .to_lowercase()
                            .cmp(&files[b].original_name.to_lowercase())
                    },
                )
            });
        }
        SortMethod::Modified => {
            indices.sort_by(|&a, &b| {
                compare_optional_time(files[a].modified, files[b].modified).then_with(|| {
                    files[a]
                        .original_name
                        .to_lowercase()
                        .cmp(&files[b].original_name.to_lowercase())
                })
            });
        }
    }

    if sort.descending {
        indices.reverse();
    }

    indices
}

fn compare_optional_time(a: Option<SystemTime>, b: Option<SystemTime>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn validate(
    files: &[FileEntry],
    order: &[usize],
    previews: &[String],
    parts: &[NameParts],
    rules: &RuleSet,
    rule_errors: &[String],
) -> ValidationReport {
    let mut report = ValidationReport::default();

    for error in rule_errors {
        report.issues.push(Issue {
            file_id: None,
            severity: Severity::Error,
            message: error.clone(),
        });
    }

    let moving_paths: HashSet<&Path> = files
        .iter()
        .enumerate()
        .filter(|(index, file)| previews[*index] != file.original_name)
        .map(|(_, file)| file.path.as_path())
        .collect();
    // Deterministic maps keep duplicate findings in a stable order.
    let mut exact_names: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut folded_names: BTreeMap<String, Vec<usize>> = BTreeMap::new();

    for &index in order {
        let file = &files[index];
        let preview = previews[index].as_str();

        validate_file(
            &mut report,
            file,
            preview,
            &parts[index],
            rules,
            rule_errors,
            &moving_paths,
        );

        exact_names.entry(preview).or_default().push(index);
        folded_names
            .entry(preview.to_lowercase())
            .or_default()
            .push(index);
    }

    report_duplicate_targets(&mut report, files, previews, &exact_names, &folded_names);
    report
}

fn validate_file(
    report: &mut ValidationReport,
    file: &FileEntry,
    preview: &str,
    parts: &NameParts,
    rules: &RuleSet,
    rule_errors: &[String],
    moving_paths: &HashSet<&Path>,
) {
    if !file.utf8_name {
        error(
            report,
            file.id,
            "File name is not valid UTF-8; it is not supported yet",
        );
    }

    if std::fs::symlink_metadata(&file.path).is_err() {
        error(report, file.id, "Source file no longer exists");
    }

    if preview.is_empty() {
        error(report, file.id, "Resulting name is empty");
    } else if parts.stem.trim().is_empty() {
        error(report, file.id, "Resulting name has no base name");
    }

    if preview == "." || preview == ".." {
        error(report, file.id, "Resulting name is reserved");
    }

    if preview.contains('/') {
        error(report, file.id, "Name contains a path separator (/)");
    }

    if preview.chars().any(char::is_control) {
        error(report, file.id, "Name contains control characters");
    }

    if preview.contains(['<', '>', ':', '"', '\\', '|', '?', '*']) {
        warning(
            report,
            file.id,
            "Name contains characters that are not portable to Windows",
        );
    }

    if preview.len() > 255 {
        error(report, file.id, "Name is longer than 255 bytes");
    }

    if preview.trim() != preview {
        warning(report, file.id, "Name has leading or trailing spaces");
    }

    if preview.ends_with('.') {
        warning(report, file.id, "Name ends with a dot");
    }

    if is_reserved_windows_name(preview) {
        warning(report, file.id, "Name is reserved on Windows");
    }

    validate_target(report, file, preview, moving_paths);

    if rule_errors.is_empty() && rules.has_name_effects() && preview == file.original_name {
        push(
            report,
            Some(file.id),
            Severity::Info,
            "Name is unchanged".to_owned(),
        );
    }
}

fn validate_target(
    report: &mut ValidationReport,
    file: &FileEntry,
    preview: &str,
    moving_paths: &HashSet<&Path>,
) {
    let Some(parent) = file.path.parent() else {
        return;
    };

    let target = parent.join(preview);

    if target.as_os_str().len() > 4096 {
        error(report, file.id, "Full path is longer than 4096 bytes");
    }

    if target != file.path
        && std::fs::symlink_metadata(&target).is_ok()
        && !moving_paths.contains(target.as_path())
    {
        push(
            report,
            Some(file.id),
            Severity::Error,
            format!("Target already exists: {preview}"),
        );
    }

    if parent
        .metadata()
        .is_ok_and(|metadata| metadata.permissions().readonly())
    {
        error(report, file.id, "Directory is not writable");
    }
}

fn report_duplicate_targets(
    report: &mut ValidationReport,
    files: &[FileEntry],
    previews: &[String],
    exact_names: &BTreeMap<&str, Vec<usize>>,
    folded_names: &BTreeMap<String, Vec<usize>>,
) {
    for (name, indices) in exact_names {
        if indices.len() <= 1 {
            continue;
        }

        for &index in indices {
            push(
                report,
                Some(files[index].id),
                Severity::Error,
                format!("Duplicate target name: {name}"),
            );
        }
    }

    for indices in folded_names.values() {
        if indices.len() <= 1 {
            continue;
        }

        let distinct: HashSet<&str> = indices
            .iter()
            .map(|&index| previews[index].as_str())
            .collect();

        // Exact duplicates already have an error; only warn when the names
        // differ solely by case and would collide on a case-insensitive disk.
        if distinct.len() > 1 {
            for &index in indices {
                warning(
                    report,
                    files[index].id,
                    "Target collides with another name on a case-insensitive filesystem",
                );
            }
        }
    }
}

fn error(report: &mut ValidationReport, file_id: FileId, message: &str) {
    push(report, Some(file_id), Severity::Error, message.to_owned());
}

fn warning(report: &mut ValidationReport, file_id: FileId, message: &str) {
    push(report, Some(file_id), Severity::Warning, message.to_owned());
}

fn push(
    report: &mut ValidationReport,
    file_id: Option<FileId>,
    severity: Severity,
    message: String,
) {
    report.issues.push(Issue {
        file_id,
        severity,
        message,
    });
}

fn is_reserved_windows_name(name: &str) -> bool {
    let (stem, _) = split_file_name(name);
    let stem = stem.trim_end_matches([' ', '.']).to_ascii_uppercase();

    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{
        ExtensionRule, NewNameRule, RegexRule, ReplaceRule, Rule, SerializeRule, TextRule,
    };
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("rename-rx-preview-{}-{nanos}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn add(&self, id: FileId, name: &str) -> FileEntry {
            let path = self.path.join(name);
            std::fs::write(&path, b"x").unwrap();
            FileEntry::read(id, path).unwrap()
        }

        fn touch(&self, name: &str) {
            std::fs::write(self.path.join(name), b"x").unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn fake_entry(id: FileId, name: &str) -> FileEntry {
        FileEntry {
            id,
            path: PathBuf::from("/tmp").join(name),
            original_name: name.to_owned(),
            created: None,
            modified: None,
            size: 0,
            utf8_name: true,
        }
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

    fn extension(rules: &mut RuleSet) -> &mut ExtensionRule {
        rule_mut(rules, |rule| match rule {
            Rule::Extension(rule) => Some(rule),
            _ => None,
        })
    }

    #[test]
    fn sorts_and_serializes_in_display_order() {
        let files = vec![
            fake_entry(0, "C.txt"),
            fake_entry(1, "A.txt"),
            fake_entry(2, "B.txt"),
        ];
        let mut rules = RuleSet::default();
        rules.sort_mut().enabled = true;
        rules.sort_mut().method = SortMethod::Name;
        serialize(&mut rules).enabled = true;

        let result = build_preview(&files, &rules);
        assert_eq!(result.order, vec![1, 2, 0]);
        assert_eq!(result.previews[1], "001 A.txt");
        assert_eq!(result.previews[2], "002 B.txt");
        assert_eq!(result.previews[0], "003 C.txt");
    }

    #[test]
    fn descending_sort_reverses_the_serial_numbers() {
        let files = vec![fake_entry(0, "A.txt"), fake_entry(1, "B.txt")];
        let mut rules = RuleSet::default();
        rules.sort_mut().enabled = true;
        rules.sort_mut().method = SortMethod::Name;
        rules.sort_mut().descending = true;
        serialize(&mut rules).enabled = true;

        let result = build_preview(&files, &rules);
        assert_eq!(result.order, vec![1, 0]);
        assert_eq!(result.previews[1], "001 B.txt");
        assert_eq!(result.previews[0], "002 A.txt");
    }

    #[test]
    fn reports_missing_sources() {
        let result = build_preview(&[fake_entry(0, "missing.txt")], &RuleSet::default());
        assert_eq!(result.report.errors(), 1);
        assert!(result.report.issues[0].message.contains("no longer exists"));
    }

    #[test]
    fn reports_duplicate_targets() {
        let temp = TempDir::new();
        let files = vec![temp.add(0, "a.txt"), temp.add(1, "b.txt")];
        let mut rules = RuleSet::default();
        new_name(&mut rules).enabled = true;
        new_name(&mut rules).value = "same".to_owned();

        let result = build_preview(&files, &rules);
        assert_eq!(result.report.errors(), 2);
        assert!(
            result
                .report
                .issues
                .iter()
                .any(|issue| issue.message.contains("Duplicate target name"))
        );
    }

    #[test]
    fn reports_existing_targets() {
        let temp = TempDir::new();
        let file = temp.add(0, "a.txt");
        temp.touch("taken.txt");

        let mut rules = RuleSet::default();
        new_name(&mut rules).enabled = true;
        new_name(&mut rules).value = "taken".to_owned();

        let result = build_preview(&[file], &rules);
        assert_eq!(result.report.errors(), 1);
        assert!(
            result
                .report
                .issues
                .iter()
                .any(|issue| issue.message.contains("already exists"))
        );
    }

    #[test]
    fn rejects_invalid_and_empty_names() {
        let temp = TempDir::new();
        let file = temp.add(0, "a.txt");

        let mut rules = RuleSet::default();
        new_name(&mut rules).enabled = true;
        new_name(&mut rules).value = "bad/name".to_owned();
        let result = build_preview(std::slice::from_ref(&file), &rules);
        assert!(result.report.errors() >= 1);
        assert!(
            result
                .report
                .issues
                .iter()
                .any(|issue| issue.message.contains("path separator"))
        );

        new_name(&mut rules).value.clear();
        let result = build_preview(std::slice::from_ref(&file), &rules);
        assert!(
            result
                .report
                .issues
                .iter()
                .any(|issue| issue.message.contains("no base name"))
        );

        extension(&mut rules).enabled = true;
        extension(&mut rules).value.clear();
        let result = build_preview(std::slice::from_ref(&file), &rules);
        assert!(
            result
                .report
                .issues
                .iter()
                .any(|issue| issue.message.contains("empty"))
        );
    }

    #[test]
    fn warns_about_case_insensitive_collisions() {
        let temp = TempDir::new();
        let files = vec![temp.add(0, "A.txt"), temp.add(1, "a.txt")];

        let result = build_preview(&files, &RuleSet::default());
        assert_eq!(result.report.errors(), 0);
        assert_eq!(result.report.warnings(), 2);
        assert!(
            result
                .report
                .issues
                .iter()
                .all(|issue| issue.message.contains("case-insensitive"))
        );
    }

    #[test]
    fn rejects_targets_occupied_by_an_unchanged_batch_file() {
        let temp = TempDir::new();
        let files = vec![temp.add(0, "a.txt"), temp.add(1, "b.txt")];

        let mut rules = RuleSet::default();
        replace(&mut rules).enabled = true;
        replace(&mut rules).find = "a".to_owned();
        replace(&mut rules).replace = "b".to_owned();

        let result = build_preview(&files, &rules);
        assert_eq!(result.previews[0], "b.txt");
        assert_eq!(result.previews[1], "b.txt");
        assert!(
            result
                .report
                .issues
                .iter()
                .any(|issue| issue.message.contains("already exists"))
        );
    }

    #[test]
    fn invalid_regex_is_a_rule_error() {
        let mut rules = RuleSet::default();
        regex(&mut rules).enabled = true;
        regex(&mut rules).pattern = "(".to_owned();

        let result = build_preview(&[], &rules);
        assert_eq!(result.report.errors(), 1);
        assert!(result.report.issues[0].file_id.is_none());
    }

    #[test]
    fn unchanged_names_are_only_reported_when_a_rule_is_enabled() {
        let temp = TempDir::new();
        let file = temp.add(0, "a.txt");

        let result = build_preview(std::slice::from_ref(&file), &RuleSet::default());
        assert_eq!(result.report.infos(), 0);

        let mut rules = RuleSet::default();
        prefix(&mut rules).enabled = true;
        let result = build_preview(std::slice::from_ref(&file), &rules);
        assert_eq!(result.report.infos(), 1);
    }
}
