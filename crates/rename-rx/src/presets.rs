//! Persistent user presets: a named snapshot of the complete rule set.

use std::fs;
use std::path::{Path, PathBuf};

use rename_rx_core::RuleSet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub rules: RuleSet,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PresetStore {
    pub presets: Vec<Preset>,
}

impl PresetStore {
    pub fn load() -> Self {
        let Some(path) = path() else {
            return Self::default();
        };

        let Ok(contents) = fs::read_to_string(&path) else {
            return Self::default();
        };

        serde_json::from_str(&contents).unwrap_or_else(|error| {
            eprintln!("rename-rx: could not parse {}: {error}", path.display());
            Self::default()
        })
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = path() else {
            return Err("Could not determine the configuration directory.".to_owned());
        };

        write_to(&path, self)
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.presets
            .iter()
            .position(|preset| preset.name.eq_ignore_ascii_case(name))
    }
}

fn path() -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

    Some(config_home.join("rename-rx").join("presets.json"))
}

fn write_to(path: &Path, store: &PresetStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }

    let contents = serde_json::to_string_pretty(store)
        .map_err(|error| format!("Could not encode presets: {error}"))?;
    let temporary = path.with_extension("json.tmp");

    fs::write(&temporary, contents)
        .map_err(|error| format!("Could not write {}: {error}", temporary.display()))?;

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("Could not save {}: {error}", path.display()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rename_rx_core::{CharacterOrigin, MoveAnchor, SerializePosition};

    fn sample_rules() -> RuleSet {
        let mut rules = RuleSet::default();
        rules.keep.enabled = true;
        rules.keep.count = 3;
        rules.keep.origin = CharacterOrigin::After;
        rules.move_rule.enabled = true;
        rules.move_rule.start = 1;
        rules.move_rule.length = 2;
        rules.move_rule.anchor = MoveAnchor::End;
        rules.serialize.enabled = true;
        rules.serialize.padding = 4;
        rules.serialize.position = SerializePosition::Before;
        rules
    }

    #[test]
    fn store_round_trips_all_rules() {
        let store = PresetStore {
            presets: vec![
                Preset {
                    name: "Photos".to_owned(),
                    rules: sample_rules(),
                },
                Preset {
                    name: "Default".to_owned(),
                    rules: RuleSet::default(),
                },
            ],
        };

        let json = serde_json::to_string(&store).unwrap();
        let restored: PresetStore = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.presets, store.presets);
    }

    #[test]
    fn empty_json_and_missing_rule_fields_use_defaults() {
        let store: PresetStore = serde_json::from_str("{}").unwrap();
        assert!(store.presets.is_empty());

        let store: PresetStore =
            serde_json::from_str(r#"{"presets":[{"name":"Old","rules":{}}]}"#).unwrap();
        assert_eq!(store.presets.len(), 1);
        assert_eq!(store.presets[0].rules, RuleSet::default());
    }

    #[test]
    fn names_are_matched_case_insensitively() {
        let store = PresetStore {
            presets: vec![Preset {
                name: "Photos".to_owned(),
                rules: RuleSet::default(),
            }],
        };

        assert_eq!(store.find("photos"), Some(0));
        assert_eq!(store.find("PHOTOS"), Some(0));
        assert_eq!(store.find("Other"), None);
    }

    #[test]
    fn writes_presets_to_disk() {
        let directory =
            std::env::temp_dir().join(format!("rename-rx-presets-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let path = directory.join("presets.json");
        let store = PresetStore {
            presets: vec![Preset {
                name: "Photos".to_owned(),
                rules: sample_rules(),
            }],
        };

        write_to(&path, &store).unwrap();

        let restored: PresetStore =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(restored.presets, store.presets);
        assert!(!directory.join("presets.json.tmp").exists());

        fs::remove_dir_all(directory).unwrap();
    }
}
