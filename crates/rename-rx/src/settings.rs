use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The user's theme preference, persisted between launches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Follow system",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn resolve(self, system_dark: Option<bool>) -> egui::Theme {
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemePreference,
}

impl Settings {
    pub fn path() -> Option<PathBuf> {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

        Some(config_home.join("rename-rx").join("settings.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
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

    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };

        if let Some(parent) = path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            eprintln!("rename-rx: could not create {}: {error}", parent.display());
            return;
        }

        let Ok(contents) = serde_json::to_string_pretty(self) else {
            return;
        };

        // Write through a temporary file so a crash cannot leave a truncated
        // settings file behind.
        let temporary = path.with_extension("json.tmp");
        if let Err(error) = fs::write(&temporary, contents) {
            eprintln!(
                "rename-rx: could not write {}: {error}",
                temporary.display()
            );
            return;
        }

        if let Err(error) = fs::rename(&temporary, &path) {
            eprintln!("rename-rx: could not save {}: {error}", path.display());
            let _ = fs::remove_file(&temporary);
        }
    }
}

/// `Some(true)` means dark, `Some(false)` means light, and `None` means the
/// desktop did not report a preference.
pub fn detect_system_dark() -> Option<bool> {
    match dark_light::detect() {
        Ok(dark_light::Mode::Dark) => Some(true),
        Ok(dark_light::Mode::Light) => Some(false),
        Ok(dark_light::Mode::Unspecified) | Err(_) => None,
    }
}

pub fn watch_system_theme() -> Option<dark_light::Watcher> {
    dark_light::subscribe().ok()
}

pub fn mode_to_dark(mode: dark_light::Mode) -> Option<bool> {
    match mode {
        dark_light::Mode::Dark => Some(true),
        dark_light::Mode::Light => Some(false),
        dark_light::Mode::Unspecified => None,
    }
}
