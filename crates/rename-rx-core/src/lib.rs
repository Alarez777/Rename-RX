//! UI-independent core for Rename RX.
//!
//! The crate owns the file model, the rename rule pipeline, validation, and
//! the transactional rename implementation. It must not depend on egui.

mod file;
mod preview;
mod rename;
mod rules;

pub use file::{FileEntry, FileId, split_file_name};
pub use preview::{
    Issue, PreviewResult, Severity, ValidationReport, build_preview, sorted_indices,
};
pub use rename::{RenameError, RenameOutcome, RenameStep, execute, plan_steps};
pub use rules::{
    CaseMode, CaseRule, CompiledRules, ExtensionRule, InsertAnchor, InsertRule, NameParts,
    NewNameRule, RegexRule, RemoveFrom, RemoveRule, ReplaceRule, RuleSet, SerializePosition,
    SerializeRule, SortMethod, SortRule, TextRule, apply_rules,
};

/// Human-readable application name.
pub const APP_NAME: &str = "Rename RX";

/// Short application description.
pub const APP_DESCRIPTION: &str = "Batch file renamer for Linux";
