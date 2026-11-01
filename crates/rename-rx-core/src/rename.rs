use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::file::FileEntry;

/// One source → target rename.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameStep {
    /// Current path of the file.
    pub from: PathBuf,
    /// Path the file should have after the rename.
    pub to: PathBuf,
}

/// A completed rename, used to build the undo operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameOutcome {
    /// Path before the rename.
    pub original: PathBuf,
    /// Path after the rename.
    pub renamed: PathBuf,
}

/// Failure while staging or finalizing a transactional rename.
#[derive(Debug)]
pub struct RenameError {
    message: String,
    source: Option<io::Error>,
    /// True when every change made before the failure was rolled back.
    rolled_back: bool,
}

impl RenameError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
            rolled_back: true,
        }
    }

    fn with_source(message: impl Into<String>, source: io::Error) -> Self {
        Self {
            message: message.into(),
            source: Some(source),
            rolled_back: false,
        }
    }

    /// True when every change made before the failure was rolled back.
    pub fn rolled_back(&self) -> bool {
        self.rolled_back
    }
}

impl fmt::Display for RenameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)?;
        if self.rolled_back {
            write!(formatter, " (all changes rolled back)")
        } else {
            write!(
                formatter,
                " (rollback incomplete; check the affected files)"
            )
        }
    }
}

impl Error for RenameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.source {
            Some(error) => Some(error),
            None => None,
        }
    }
}

/// Build the steps for the current previews. Files whose name did not change
/// are omitted.
pub fn plan_steps(files: &[FileEntry], previews: &[String], order: &[usize]) -> Vec<RenameStep> {
    let mut steps = Vec::new();

    for &index in order {
        let (Some(file), Some(preview)) = (files.get(index), previews.get(index)) else {
            continue;
        };
        let Some(parent) = file.path.parent() else {
            continue;
        };

        let to = parent.join(preview);
        if to != file.path {
            steps.push(RenameStep {
                from: file.path.clone(),
                to,
            });
        }
    }

    steps
}

/// Rename every source to its target using a staging directory inside the same
/// parent directory.
///
/// Staging makes swaps (`a ↔ b`) and cycles safe: every source is moved to a
/// unique temporary name first, and only then to its final name. If anything
/// fails, the completed work is rolled back.
///
/// # Errors
///
/// Returns [`RenameError`] when the plan is invalid, a source disappeared, a
/// target appeared after validation, or the filesystem rejects a rename. The
/// error reports whether the rollback completed.
pub fn execute(steps: &[RenameStep]) -> Result<Vec<RenameOutcome>, RenameError> {
    let steps: Vec<RenameStep> = steps
        .iter()
        .filter(|step| step.from != step.to)
        .cloned()
        .collect();

    if steps.is_empty() {
        return Ok(Vec::new());
    }

    validate_steps(&steps)?;
    let temporary_paths = temporary_paths(&steps)?;

    let mut staged: Vec<(usize, PathBuf)> = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        if let Err(error) = fs::rename(&step.from, &temporary_paths[index]) {
            let rolled_back = rollback_staged(&steps, &staged);
            let mut failure = RenameError::with_source(
                format!("could not stage {}: {error}", step.from.display()),
                error,
            );
            failure.rolled_back = rolled_back;
            return Err(failure);
        }
        staged.push((index, temporary_paths[index].clone()));
    }

    let mut finalized: Vec<usize> = Vec::with_capacity(steps.len());
    for (index, step) in steps.iter().enumerate() {
        // Everything in the batch was moved aside, so no target should exist.
        // Recheck to avoid replacing a file created after validation.
        if fs::symlink_metadata(&step.to).is_ok() {
            let rolled_back = rollback_finalized(&steps, &temporary_paths, &finalized)
                && rollback_staged(&steps, &staged);
            return Err(RenameError {
                message: format!("target appeared while renaming: {}", step.to.display()),
                source: None,
                rolled_back,
            });
        }

        if let Err(error) = fs::rename(&temporary_paths[index], &step.to) {
            let rolled_back = rollback_finalized(&steps, &temporary_paths, &finalized)
                && rollback_staged(&steps, &staged);
            let mut failure = RenameError::with_source(
                format!(
                    "could not rename {} to {}: {error}",
                    step.from.display(),
                    step.to.display()
                ),
                error,
            );
            failure.rolled_back = rolled_back;
            return Err(failure);
        }
        finalized.push(index);
    }

    Ok(steps
        .into_iter()
        .map(|step| RenameOutcome {
            original: step.from,
            renamed: step.to,
        })
        .collect())
}

fn validate_steps(steps: &[RenameStep]) -> Result<(), RenameError> {
    let sources: HashSet<&Path> = steps.iter().map(|step| step.from.as_path()).collect();
    let mut targets: HashSet<&Path> = HashSet::new();
    let mut seen_sources: HashSet<&Path> = HashSet::new();

    for step in steps {
        if !seen_sources.insert(step.from.as_path()) {
            return Err(problem(format!(
                "duplicate source in the rename plan: {}",
                step.from.display()
            )));
        }

        if step.from.parent() != step.to.parent() {
            return Err(problem(
                "cross-directory renames are not supported".to_owned(),
            ));
        }

        if fs::symlink_metadata(&step.from).is_err() {
            return Err(problem(format!(
                "source no longer exists: {}",
                step.from.display()
            )));
        }

        if !targets.insert(step.to.as_path()) {
            return Err(problem(format!(
                "duplicate target in the rename plan: {}",
                step.to.display()
            )));
        }

        if !sources.contains(step.to.as_path()) && fs::symlink_metadata(&step.to).is_ok() {
            return Err(problem(format!(
                "target already exists: {}",
                step.to.display()
            )));
        }
    }

    Ok(())
}

fn problem(message: String) -> RenameError {
    RenameError::new(message)
}

fn temporary_paths(steps: &[RenameStep]) -> Result<Vec<PathBuf>, RenameError> {
    let mut used: HashSet<PathBuf> = HashSet::new();
    used.extend(steps.iter().map(|step| step.from.clone()));
    used.extend(steps.iter().map(|step| step.to.clone()));

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let mut counter = 0u64;
    let mut paths = Vec::with_capacity(steps.len());

    for step in steps {
        let parent = step.from.parent().ok_or_else(|| {
            problem(format!(
                "cannot find the parent directory of {}",
                step.from.display()
            ))
        })?;

        let candidate = loop {
            let candidate = parent.join(format!(
                ".rename-rx-{}-{stamp}-{counter}.tmp",
                std::process::id()
            ));
            counter += 1;

            if !used.contains(&candidate) && fs::symlink_metadata(&candidate).is_err() {
                break candidate;
            }

            if counter > 100_000 {
                return Err(problem(
                    "could not allocate a temporary file name".to_owned(),
                ));
            }
        };

        used.insert(candidate.clone());
        paths.push(candidate);
    }

    Ok(paths)
}

fn rollback_staged(steps: &[RenameStep], staged: &[(usize, PathBuf)]) -> bool {
    let mut complete = true;

    for (index, temporary) in staged.iter().rev() {
        if fs::rename(temporary, &steps[*index].from).is_err() {
            complete = false;
        }
    }

    complete
}

fn rollback_finalized(
    steps: &[RenameStep],
    temporary_paths: &[PathBuf],
    finalized: &[usize],
) -> bool {
    let mut complete = true;

    for index in finalized.iter().rev() {
        if fs::rename(&steps[*index].to, &temporary_paths[*index]).is_err() {
            complete = false;
        }
    }

    complete
}

#[cfg(test)]
mod tests {
    use super::*;

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
                .join(format!("rename-rx-rename-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn write(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.path.join(name);
            fs::write(&path, contents).unwrap();
            path
        }

        fn path(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn renames_to_a_new_name() {
        let temp = TempDir::new();
        let source = temp.write("a.txt", "A");
        let target = temp.path("b.txt");

        let outcomes = execute(&[RenameStep {
            from: source.clone(),
            to: target.clone(),
        }])
        .unwrap();

        assert_eq!(outcomes.len(), 1);
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&target).unwrap(), "A");
    }

    #[test]
    fn swaps_two_files() {
        let temp = TempDir::new();
        let a = temp.write("a.txt", "A");
        let b = temp.write("b.txt", "B");

        execute(&[
            RenameStep {
                from: a.clone(),
                to: b.clone(),
            },
            RenameStep {
                from: b.clone(),
                to: a.clone(),
            },
        ])
        .unwrap();

        assert_eq!(fs::read_to_string(&a).unwrap(), "B");
        assert_eq!(fs::read_to_string(&b).unwrap(), "A");
    }

    #[test]
    fn handles_a_three_file_cycle() {
        let temp = TempDir::new();
        let a = temp.write("a.txt", "A");
        let b = temp.write("b.txt", "B");
        let c = temp.write("c.txt", "C");

        execute(&[
            RenameStep {
                from: a.clone(),
                to: b.clone(),
            },
            RenameStep {
                from: b.clone(),
                to: c.clone(),
            },
            RenameStep {
                from: c.clone(),
                to: a.clone(),
            },
        ])
        .unwrap();

        assert_eq!(fs::read_to_string(&a).unwrap(), "C");
        assert_eq!(fs::read_to_string(&b).unwrap(), "A");
        assert_eq!(fs::read_to_string(&c).unwrap(), "B");
    }

    #[test]
    fn rejects_existing_targets_before_touching_anything() {
        let temp = TempDir::new();
        let a = temp.write("a.txt", "A");
        let b = temp.write("b.txt", "B");

        let error = execute(&[RenameStep {
            from: a.clone(),
            to: b.clone(),
        }])
        .unwrap_err();

        assert!(error.rolled_back());
        assert_eq!(fs::read_to_string(&a).unwrap(), "A");
        assert_eq!(fs::read_to_string(&b).unwrap(), "B");
    }

    #[test]
    fn rolls_back_when_a_target_name_is_impossible() {
        let temp = TempDir::new();
        let a = temp.write("a.txt", "A");
        let b = temp.write("b.txt", "B");
        let impossible = temp.path(&"x".repeat(300));

        let error = execute(&[
            RenameStep {
                from: a.clone(),
                to: impossible,
            },
            RenameStep {
                from: b.clone(),
                to: temp.path("c.txt"),
            },
        ])
        .unwrap_err();

        assert!(error.rolled_back(), "{error}");
        assert_eq!(fs::read_to_string(&a).unwrap(), "A");
        assert_eq!(fs::read_to_string(&b).unwrap(), "B");
        assert!(!temp.path("c.txt").exists());

        let leftovers: Vec<_> = fs::read_dir(&temp.path)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .filter(|name| name.to_string_lossy().starts_with(".rename-rx-"))
            .collect();
        assert!(leftovers.is_empty(), "temporary files left: {leftovers:?}");
    }

    #[test]
    fn error_messages_are_lowercase_and_report_the_rollback() {
        let temp = TempDir::new();
        let source = temp.write("a.txt", "A");
        let target = temp.write("b.txt", "B");

        let error = execute(&[RenameStep {
            from: source,
            to: target,
        }])
        .unwrap_err();

        let message = error.to_string();
        assert!(message.starts_with("target already exists"), "{message}");
        assert!(message.ends_with("(all changes rolled back)"), "{message}");
        assert!(!message.ends_with('.'), "{message}");
        assert!(error.rolled_back());
        assert!(error.source().is_none());
    }

    #[test]
    fn skips_noop_steps() {
        let temp = TempDir::new();
        let a = temp.write("a.txt", "A");
        let outcomes = execute(&[RenameStep {
            from: a.clone(),
            to: a.clone(),
        }])
        .unwrap();

        assert!(outcomes.is_empty());
        assert!(a.exists());
    }
}
