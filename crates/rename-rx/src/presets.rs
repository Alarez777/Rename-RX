//! Persistent user presets: a named snapshot of the complete rule set.

use std::fs;
use std::path::{Path, PathBuf};

use rename_rx_core::RuleSet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Preset {
    pub(crate) name: String,
    pub(crate) rules: RuleSet,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct PresetStore {
    pub(crate) presets: Vec<Preset>,
}

impl PresetStore {
    pub(crate) fn load() -> Self {
        let Some(path) = path() else {
            return Self::default();
        };

        let Ok(contents) = fs::read_to_string(&path) else {
            return Self::default();
        };

        serde_json::from_str(&contents).unwrap_or_else(|error| {
            log::warn!("could not parse {}: {error}", path.display());
            Self::default()
        })
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        let Some(path) = path() else {
            return Err("Could not determine the configuration directory.".to_owned());
        };

        write_to(&path, self)
    }

    pub(crate) fn find(&self, name: &str) -> Option<usize> {
        self.presets
            .iter()
            .position(|preset| preset.name.eq_ignore_ascii_case(name))
    }
}

fn path() -> Option<PathBuf> {
    Some(
        crate::integration::config_home()?
            .join("rename-rx")
            .join("presets.json"),
    )
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
    use rename_rx_core::{
        CharacterOrigin, KeepRule, MoveAnchor, MoveRule, ReplaceRule, Rule, RuleKind,
        SerializePosition, SerializeRule, SortMethod,
    };

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

    fn keep(rules: &mut RuleSet) -> &mut KeepRule {
        rule_mut(rules, |rule| match rule {
            Rule::Keep(rule) => Some(rule),
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

    fn sample_rules() -> RuleSet {
        let mut rules = RuleSet::default();
        keep(&mut rules).enabled = true;
        keep(&mut rules).count = 3;
        keep(&mut rules).origin = CharacterOrigin::After;
        move_rule(&mut rules).enabled = true;
        move_rule(&mut rules).start = 1;
        move_rule(&mut rules).length = 2;
        move_rule(&mut rules).anchor = MoveAnchor::End;
        serialize(&mut rules).enabled = true;
        serialize(&mut rules).padding = 4;
        serialize(&mut rules).position = SerializePosition::Before;

        // Duplicates are part of the persisted state.
        let extra = ReplaceRule {
            enabled: true,
            find: "a".to_owned(),
            replace: "b".to_owned(),
            ..ReplaceRule::default()
        };
        rules.insert(2, Rule::Replace(extra));
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
    fn legacy_presets_load_into_ordered_rules() {
        let json = r#"{
            "presets": [{
                "name": "Old",
                "rules": {
                    "replace": {"enabled": true, "find": "a", "replace": "b"},
                    "sort": {"enabled": true, "method": "name"}
                }
            }]
        }"#;
        let store: PresetStore = serde_json::from_str(json).unwrap();
        let rules = &store.presets[0].rules;

        assert_eq!(rules.rules().len(), RuleKind::ALL.len());
        assert_eq!(rules.rules()[1].kind(), RuleKind::Replace);
        let Rule::Replace(replace) = &rules.rules()[1] else {
            panic!("the legacy Replace rule keeps its position");
        };
        assert!(replace.enabled);
        assert_eq!(replace.find, "a");
        assert_eq!(replace.replace, "b");
        assert!(rules.sort().enabled);
        assert_eq!(rules.sort().method, SortMethod::Name);
    }

    #[test]
    fn new_presets_store_the_ordered_rule_list() {
        let json = serde_json::to_string(&sample_rules()).unwrap();

        assert!(json.contains("\"rules\":["));
        assert!(json.contains("\"tool\":\"replace\""));
        assert!(json.contains("\"sort\":"));
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
