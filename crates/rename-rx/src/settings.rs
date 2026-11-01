use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::tools::Tool;

/// The user's theme preference, persisted between launches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub(crate) const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::System => "Follow system",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub(crate) fn resolve(self, system_dark: Option<bool>) -> egui::Theme {
        match self {
            Self::Dark => egui::Theme::Dark,
            Self::Light => egui::Theme::Light,
            Self::System => match system_dark {
                Some(false) => egui::Theme::Light,
                // The original app is dark by default; keep that when the
                // desktop does not expose a preference.
                Some(true) | None => egui::Theme::Dark,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum WindowControlsStyle {
    #[default]
    Automatic,
    Adwaita,
}

impl WindowControlsStyle {
    pub(crate) const ALL: [Self; 2] = [Self::Automatic, Self::Adwaita];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::Adwaita => "GNOME (Adwaita)",
        }
    }

    /// Resolve the platform-independent preference to the implementation used
    /// today. Windows and macOS styles will be added later.
    pub(crate) fn resolved(self) -> Self {
        match self {
            Self::Automatic | Self::Adwaita => Self::Adwaita,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub(crate) theme: ThemePreference,
    pub(crate) window_controls: WindowControlsStyle,
    /// Tools hidden from the rule panel. Storing only hidden tools keeps new
    /// tools visible when they are added in a later version.
    pub(crate) hidden_tools: BTreeSet<Tool>,
}

impl Settings {
    pub(crate) fn path() -> Option<PathBuf> {
        Some(
            crate::integration::config_home()?
                .join("rename-rx")
                .join("settings.json"),
        )
    }

    pub(crate) fn load() -> Self {
        let Some(path) = Self::path() else {
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

    pub(crate) fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };

        if let Some(parent) = path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            log::warn!("could not create {}: {error}", parent.display());
            return;
        }

        let Ok(contents) = serde_json::to_string_pretty(self) else {
            log::warn!("could not encode settings");
            return;
        };

        // Write through a temporary file so a crash cannot leave a truncated
        // settings file behind.
        let temporary = path.with_extension("json.tmp");
        if let Err(error) = fs::write(&temporary, contents) {
            log::warn!("could not write {}: {error}", temporary.display());
            return;
        }

        if let Err(error) = fs::rename(&temporary, &path) {
            log::warn!("could not save {}: {error}", path.display());
            let _ = fs::remove_file(&temporary);
        }
    }
}

/// `Some(true)` means dark, `Some(false)` means light, and `None` means the
/// desktop did not report a preference.
pub(crate) fn detect_system_dark() -> Option<bool> {
    match dark_light::detect() {
        Ok(dark_light::Mode::Dark) => Some(true),
        Ok(dark_light::Mode::Light) => Some(false),
        Ok(dark_light::Mode::Unspecified) | Err(_) => None,
    }
}

pub(crate) fn watch_system_theme() -> Option<dark_light::Watcher> {
    dark_light::subscribe().ok()
}

pub(crate) fn mode_to_dark(mode: dark_light::Mode) -> Option<bool> {
    match mode {
        dark_light::Mode::Dark => Some(true),
        dark_light::Mode::Light => Some(false),
        dark_light::Mode::Unspecified => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_without_hidden_tools_show_every_tool() {
        let settings: Settings =
            serde_json::from_str(r#"{"theme":"light","window_controls":"adwaita"}"#).unwrap();

        assert!(settings.hidden_tools.is_empty());
    }

    #[test]
    fn hidden_tools_round_trip() {
        let settings = Settings {
            hidden_tools: BTreeSet::from([Tool::Replace, Tool::Serialize]),
            ..Settings::default()
        };

        let json = serde_json::to_string(&settings).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();

        assert!(json.contains("\"hidden_tools\""));
        assert_eq!(restored.hidden_tools, settings.hidden_tools);
    }
}
