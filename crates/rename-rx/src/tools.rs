//! The rename tools that can be shown or hidden in the rule panel.

use std::collections::BTreeSet;

use rename_rx_core::{RuleKind, RuleSet};
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

    /// The rename-rule kind behind this tool, or `None` for sorting.
    pub(crate) const fn rule_kind(self) -> Option<RuleKind> {
        match self {
            Self::Case => Some(RuleKind::Case),
            Self::Extension => Some(RuleKind::Extension),
            Self::Insert => Some(RuleKind::Insert),
            Self::Keep => Some(RuleKind::Keep),
            Self::Move => Some(RuleKind::Move),
            Self::NewName => Some(RuleKind::NewName),
            Self::Prefix => Some(RuleKind::Prefix),
            Self::Regex => Some(RuleKind::Regex),
            Self::Remove => Some(RuleKind::Remove),
            Self::Replace => Some(RuleKind::Replace),
            Self::Serialize => Some(RuleKind::Serialize),
            Self::Sort => None,
            Self::Suffix => Some(RuleKind::Suffix),
        }
    }

    /// Whether the rule panel offers a `+` button for this tool.
    pub(crate) const fn can_duplicate(self) -> bool {
        matches!(
            self,
            Self::Replace
                | Self::Regex
                | Self::Insert
                | Self::Keep
                | Self::Remove
                | Self::Move
                | Self::Serialize
                | Self::Prefix
                | Self::Suffix
                | Self::Case
        )
    }

    pub(crate) fn state(self, rules: &RuleSet) -> ToolState {
        let enabled = match self.rule_kind() {
            Some(kind) => rules
                .rules()
                .iter()
                .any(|rule| rule.kind() == kind && rule.enabled()),
            None => rules.sort().enabled,
        };

        if enabled {
            ToolState::Enabled
        } else {
            ToolState::Disabled
        }
    }

    pub(crate) fn set_state(self, rules: &mut RuleSet, state: ToolState) {
        match self.rule_kind() {
            Some(kind) => {
                for rule in rules
                    .rules_mut()
                    .iter_mut()
                    .filter(|rule| rule.kind() == kind)
                {
                    match state {
                        ToolState::Enabled => rule.enable(),
                        ToolState::Disabled => rule.disable(),
                    }
                }
            }
            None => rules.sort_mut().enabled = matches!(state, ToolState::Enabled),
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
    use rename_rx_core::Rule;

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
        Tool::Replace.set_state(&mut rules, ToolState::Enabled);
        Tool::Sort.set_state(&mut rules, ToolState::Enabled);

        assert!(show_tools_used_by(&mut hidden, &rules));
        assert_eq!(hidden, BTreeSet::from([Tool::Suffix]));
        assert!(!show_tools_used_by(&mut hidden, &rules));
    }

    #[test]
    fn hiding_a_tool_disables_every_duplicate() {
        let mut rules = RuleSet::default();
        let kind = Tool::Replace.rule_kind().expect("Replace is a rule");
        let mut first = Rule::default_for(kind);
        first.enable();
        let mut second = Rule::default_for(kind);
        second.enable();
        rules.insert(1, first);
        rules.insert(2, second);

        assert_eq!(Tool::Replace.state(&rules), ToolState::Enabled);

        Tool::Replace.set_state(&mut rules, ToolState::Disabled);

        assert_eq!(Tool::Replace.state(&rules), ToolState::Disabled);
        assert!(
            rules
                .rules()
                .iter()
                .filter(|rule| rule.kind() == kind)
                .all(|rule| !rule.enabled())
        );
    }

    #[test]
    fn only_transform_rules_that_can_repeat_are_duplicable() {
        for tool in Tool::ALL {
            let expected = matches!(
                tool,
                Tool::Replace
                    | Tool::Regex
                    | Tool::Insert
                    | Tool::Keep
                    | Tool::Remove
                    | Tool::Move
                    | Tool::Serialize
                    | Tool::Prefix
                    | Tool::Suffix
                    | Tool::Case
            );

            assert_eq!(tool.can_duplicate(), expected, "{}", tool.label());
        }
    }
}
