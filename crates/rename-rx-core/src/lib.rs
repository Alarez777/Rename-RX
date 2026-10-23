//! UI-independent core for Rename RX.
//!
//! The crate owns the file model, the rename rule pipeline, validation, and
//! the transactional rename implementation. It must not depend on egui.

/// Human-readable application name.
pub const APP_NAME: &str = "Rename RX";

/// Short application description.
pub const APP_DESCRIPTION: &str = "Batch file renamer for Linux";
