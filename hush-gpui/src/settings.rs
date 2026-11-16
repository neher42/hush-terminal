//! User settings, stored as JSON in the same file (and format) as the web version.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    System,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::System, Appearance::Light, Appearance::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Appearance::System => "Match System",
            Appearance::Light => "Light",
            Appearance::Dark => "Dark",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| {
            serde_json::to_value(a)
                .ok()
                .as_ref()
                .and_then(Value::as_str)
                == Some(value)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FontChoice {
    #[serde(rename = "jetbrains-mono")]
    JetBrainsMono,
    #[serde(rename = "sf-mono")]
    SfMono,
    #[serde(rename = "menlo")]
    Menlo,
}

impl FontChoice {
    pub const ALL: [FontChoice; 3] = [
        FontChoice::JetBrainsMono,
        FontChoice::SfMono,
        FontChoice::Menlo,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FontChoice::JetBrainsMono => "JetBrains Mono",
            FontChoice::SfMono => "SF Mono",
            FontChoice::Menlo => "Menlo",
        }
    }

    /// The font family to ask the text system for.
    pub fn family(self) -> &'static str {
        self.label()
    }

    fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| {
            serde_json::to_value(f)
                .ok()
                .as_ref()
                .and_then(Value::as_str)
                == Some(value)
        })
    }
}

pub const DEFAULT_FONT_SIZE: f32 = 13.5;
const FONT_SIZE_RANGE: (f32, f32) = (9.0, 28.0);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub font_size: f32,
    pub font: FontChoice,
    pub appearance: Appearance,
    pub sidebar_visible: bool,
    pub show_git_status: bool,
    /// Absolute paths, in the order they show in the sidebar.
    pub pinned_folders: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: DEFAULT_FONT_SIZE,
            font: FontChoice::JetBrainsMono,
            appearance: Appearance::System,
            sidebar_visible: true,
            show_git_status: true,
            pinned_folders: Vec::new(),
        }
    }
}

impl Settings {
    /// Reads what it can: unknown or invalid values fall back to their defaults.
    pub fn from_json(json: &str) -> Self {
        let value: Value = serde_json::from_str(json).unwrap_or(Value::Null);
        let defaults = Settings::default();
        let get = |key: &str| value.get(key);
        let mut pinned: Vec<String> = Vec::new();
        for path in get("pinnedFolders")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(path) = path.as_str().filter(|p| p.starts_with('/')) {
                if !pinned.iter().any(|p| p == path) {
                    pinned.push(path.to_owned());
                }
            }
        }
        Settings {
            font_size: get("fontSize")
                .and_then(Value::as_f64)
                .map_or(defaults.font_size, |size| clamp_font_size(size as f32)),
            font: get("font")
                .and_then(Value::as_str)
                .and_then(FontChoice::parse)
                .unwrap_or(defaults.font),
            appearance: get("appearance")
                .and_then(Value::as_str)
                .and_then(Appearance::parse)
                .unwrap_or(defaults.appearance),
            sidebar_visible: get("sidebarVisible")
                .and_then(Value::as_bool)
                .unwrap_or(defaults.sidebar_visible),
            show_git_status: get("showGitStatus")
                .and_then(Value::as_bool)
                .unwrap_or(defaults.show_git_status),
            pinned_folders: pinned,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("settings serialize")
    }

    pub fn toggle_pin(&mut self, path: &str) {
        if let Some(i) = self.pinned_folders.iter().position(|p| p == path) {
            self.pinned_folders.remove(i);
        } else {
            self.pinned_folders.push(path.to_owned());
        }
    }

    /// Moves the pin at `index` up or down; out-of-range moves change nothing.
    pub fn move_pin(&mut self, index: usize, down: bool) {
        let target = if down {
            index + 1
        } else {
            index.wrapping_sub(1)
        };
        if index < self.pinned_folders.len() && target < self.pinned_folders.len() {
            self.pinned_folders.swap(index, target);
        }
    }
}

/// Half-point steps within the supported range.
pub fn clamp_font_size(size: f32) -> f32 {
    ((size * 2.0).round() / 2.0).clamp(FONT_SIZE_RANGE.0, FONT_SIZE_RANGE.1)
}

/// Where settings are read from and saved to. Every window shares the same file.
#[derive(Clone)]
pub struct SettingsFile(PathBuf);

impl SettingsFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn load(&self) -> Settings {
        match hush_core::settings::read(&self.0) {
            Ok(Some(json)) => Settings::from_json(&json),
            Ok(None) => Settings::default(),
            Err(err) => {
                eprintln!("Could not read settings, using defaults: {err}");
                Settings::default()
            }
        }
    }

    pub fn save(&self, settings: &Settings) {
        if let Err(err) = hush_core::settings::write(&self.0, &settings.to_json()) {
            eprintln!("Could not save settings: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_valid_values_and_replaces_the_rest() {
        let s = Settings::from_json(
            r#"{"fontSize": 40, "font": "comic", "appearance": "dark",
                "sidebarVisible": false, "pinnedFolders": ["/a", "rel", "/a", 3, "/b"]}"#,
        );
        assert_eq!(s.font_size, 28.0);
        assert_eq!(s.font, FontChoice::JetBrainsMono);
        assert_eq!(s.appearance, Appearance::Dark);
        assert!(!s.sidebar_visible);
        assert!(s.show_git_status);
        assert_eq!(s.pinned_folders, ["/a", "/b"]);
        assert_eq!(Settings::from_json("not json"), Settings::default());
    }

    #[test]
    fn round_trips_through_json() {
        let s = Settings {
            font: FontChoice::SfMono,
            appearance: Appearance::Light,
            pinned_folders: vec!["/x".into()],
            ..Settings::default()
        };
        assert_eq!(Settings::from_json(&s.to_json()), s);
        assert!(s.to_json().contains(r#""font": "sf-mono""#));
    }

    #[test]
    fn font_sizes_snap_to_half_points() {
        assert_eq!(clamp_font_size(13.3), 13.5);
        assert_eq!(clamp_font_size(2.0), 9.0);
    }

    #[test]
    fn pins_toggle_and_move() {
        let mut s = Settings::default();
        s.toggle_pin("/a");
        s.toggle_pin("/b");
        s.move_pin(0, true);
        assert_eq!(s.pinned_folders, ["/b", "/a"]);
        s.move_pin(0, false);
        s.move_pin(1, true);
        assert_eq!(s.pinned_folders, ["/b", "/a"]);
        s.toggle_pin("/b");
        assert_eq!(s.pinned_folders, ["/a"]);
    }
}
