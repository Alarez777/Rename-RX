//! UI-independent core for Rename RX.
//!
//! The crate owns the file model, the rename rule pipeline, validation, and
//! the transactional rename implementation. It must not depend on egui.

mod file;
mod preview;
mod rename;
mod rules;

pub use file::{FileEntry, FileId, split_file_name};
pub use preview::{Issue, PreviewResult, Severity, ValidationReport, build_preview};
pub use rename::{RenameError, RenameOutcome, RenameStep, execute, plan_steps};
pub use rules::{
    CaseMode, CaseRule, CharacterOrigin, CompiledRules, ExtensionRule, InsertAnchor, InsertRule,
    KeepRule, MoveAnchor, MoveRule, NameParts, NewNameRule, RegexRule, RemoveRule, ReplaceRule,
    Rule, RuleKind, RuleSet, SerializePosition, SerializeRule, SortMethod, SortRule, TextRule,
};
