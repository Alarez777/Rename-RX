//! The rename tools that can be shown or hidden in the rule panel.

use std::collections::BTreeSet;

use rename_rx_core::RuleSet;
use serde::{Deserialize, Serialize};

/// Whether a tool is active in the current rule set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolState {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Tool {
    Case,
    Extension,
    Insert,
    Keep,
    Move,
    NewName,
    Prefix,
    Regex,
    Remove,
    Replace,
    Serialize,
    Sort,
    Suffix,
}

impl Tool {
    /// Every tool, sorted by name for the Tools dialog.
    pub(crate) const ALL: [Self; 13] = [
        Self::Case,
        Self::Extension,
        Self::Insert,
        Self::Keep,
        Self::Move,
        Self::NewName,
        Self::Prefix,
        Self::Regex,
        Self::Remove,
        Self::Replace,
        Self::Serialize,
        Self::Sort,
        Self::Suffix,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Case => "Case",
            Self::Extension => "Extension",
            Self::Insert => "Insert",
            Self::Keep => "Keep",
            Self::Move => "Move",
            Self::NewName => "New Name",
            Self::Prefix => "Prefix",
            Self::Regex => "Regular Expression",
            Self::Remove => "Remove",
            Self::Replace => "Replace",
            Self::Serialize => "Serialize",
            Self::Sort => "Sort",
            Self::Suffix => "Suffix",
        }
    }

    pub(crate) fn state(self, rules: &RuleSet) -> ToolState {
        let enabled = match self {
            Self::Case => rules.case.enabled,
            Self::Extension => rules.extension.enabled,
            Self::Insert => rules.insert.enabled,
            Self::Keep => rules.keep.enabled,
            Self::Move => rules.move_rule.enabled,
            Self::NewName => rules.new_name.enabled,
            Self::Prefix => rules.prefix.enabled,
            Self::Regex => rules.regex.enabled,
            Self::Remove => rules.remove.enabled,
            Self::Replace => rules.replace.enabled,
            Self::Serialize => rules.serialize.enabled,
            Self::Sort => rules.sort.enabled,
            Self::Suffix => rules.suffix.enabled,
        };

        if enabled {
            ToolState::Enabled
        } else {
            ToolState::Disabled
        }
    }

    pub(crate) fn set_state(self, rules: &mut RuleSet, state: ToolState) {
        let enabled = matches!(state, ToolState::Enabled);

        match self {
            Self::Case => rules.case.enabled = enabled,
            Self::Extension => rules.extension.enabled = enabled,
            Self::Insert => rules.insert.enabled = enabled,
            Self::Keep => rules.keep.enabled = enabled,
            Self::Move => rules.move_rule.enabled = enabled,
            Self::NewName => rules.new_name.enabled = enabled,
            Self::Prefix => rules.prefix.enabled = enabled,
            Self::Regex => rules.regex.enabled = enabled,
            Self::Remove => rules.remove.enabled = enabled,
            Self::Replace => rules.replace.enabled = enabled,
            Self::Serialize => rules.serialize.enabled = enabled,
            Self::Sort => rules.sort.enabled = enabled,
            Self::Suffix => rules.suffix.enabled = enabled,
        }
    }

    /// Whether a preset relies on this tool. Loading such a preset makes the
    /// tool visible again, even if the user hid it.
    pub(crate) fn is_used_by(self, rules: &RuleSet) -> bool {
        self.state(rules) == ToolState::Enabled
    }
}

/// Shows every tool that `rules` uses, returning whether `hidden` changed.
pub(crate) fn show_tools_used_by(hidden: &mut BTreeSet<Tool>, rules: &RuleSet) -> bool {
    let mut changed = false;

    for tool in Tool::ALL {
        if tool.is_used_by(rules) {
            changed |= hidden.remove(&tool);
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_are_alphabetical_and_uniquely_labelled() {
        let labels: Vec<&str> = Tool::ALL.iter().map(|tool| tool.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();

        assert_eq!(labels, sorted);

        let unique: std::collections::HashSet<&str> = labels.iter().copied().collect();
        assert_eq!(unique.len(), labels.len());
    }

    #[test]
    fn every_tool_maps_to_exactly_one_rule() {
        for tool in Tool::ALL {
            let mut rules = RuleSet::default();
            tool.set_state(&mut rules, ToolState::Enabled);

            for other in Tool::ALL {
                assert_eq!(
                    other.state(&rules) == ToolState::Enabled,
                    other == tool,
                    "{} affected {}",
                    tool.label(),
                    other.label()
                );
            }

            assert!(tool.is_used_by(&rules));
        }
    }

    #[test]
    fn disabled_rules_are_not_used_by_presets() {
        let rules = RuleSet::default();

        for tool in Tool::ALL {
            assert_eq!(tool.state(&rules), ToolState::Disabled);
            assert!(!tool.is_used_by(&rules));
        }
    }

    #[test]
    fn tools_used_by_a_preset_are_shown_again() {
        let mut hidden = BTreeSet::from([Tool::Replace, Tool::Sort, Tool::Suffix]);
        let mut rules = RuleSet::default();
        rules.replace.enabled = true;
        rules.sort.enabled = true;

        assert!(show_tools_used_by(&mut hidden, &rules));
        assert_eq!(hidden, BTreeSet::from([Tool::Suffix]));
        assert!(!show_tools_used_by(&mut hidden, &rules));
    }
}
