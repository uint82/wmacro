//! user settings (theme, hotkeys, playback options) with load/save persistence.

use crate::state::{MacroRepeatMode, RecordHotkeyBehavior};
use crate::ui::toolbox::ToolId;
use log::error;
use serde::{Deserialize, Serialize};
use wmacro_core_types::{Hotkey, Modifiers};

pub const DEFAULT_THEME_NAME: &str = "Gruvbox Dark";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme_name: String,
    pub record_hotkey: Option<Hotkey>,
    pub abort_record_hotkey: Option<Hotkey>,
    pub play_hotkey: Option<Hotkey>,
    pub abort_play_hotkey: Option<Hotkey>,
    pub step_play_hotkey: Option<Hotkey>,
    pub capture_hotkey: Option<Hotkey>,
    pub speed_multiplier: f32,
    pub repeat_mode: MacroRepeatMode,
    pub repeat_count: u32,

    pub playback_options: wmacro_core_types::PlaybackOptions,

    pub record_hotkey_behavior: RecordHotkeyBehavior,

    // serde defaults keep these `true` so settings.json files written before
    // the field existed stay valid instead of discarding the user's whole
    // config (a missing field without a default fails the entire parse).
    #[serde(default = "default_true")]
    pub record_mouse: bool,
    #[serde(default = "default_true")]
    pub record_movements: bool,
    #[serde(default = "default_true")]
    pub record_keyboard: bool,

    // most recently used toolbox commands, most recent first.
    #[serde(default)]
    pub toolbox_recents: Vec<ToolId>,

    #[serde(default = "default_true")]
    pub show_toolbox: bool,

    #[serde(default)]
    pub recents_collapsed: bool,
}

/// default for opt-in booleans: absent in old settings.json means enabled,
/// matching the `Default` impl rather than bool's `false`.
fn default_true() -> bool {
    true
}

pub(crate) fn default_record_hotkey() -> Option<Hotkey> {
    Some(Hotkey::plain(65))
}

pub(crate) fn default_abort_record_hotkey() -> Option<Hotkey> {
    Some(Hotkey::plain(66))
}

pub(crate) fn default_play_hotkey() -> Option<Hotkey> {
    Some(Hotkey::plain(67))
}

pub(crate) fn default_abort_play_hotkey() -> Option<Hotkey> {
    Some(Hotkey::plain(68))
}

pub(crate) fn default_step_play_hotkey() -> Option<Hotkey> {
    Some(Hotkey::new(
        37,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    ))
}

pub(crate) fn default_capture_hotkey() -> Option<Hotkey> {
    Some(Hotkey::plain(60))
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_name: DEFAULT_THEME_NAME.to_string(),
            record_hotkey: default_record_hotkey(),
            abort_record_hotkey: default_abort_record_hotkey(),
            play_hotkey: default_play_hotkey(),
            abort_play_hotkey: default_abort_play_hotkey(),
            step_play_hotkey: default_step_play_hotkey(),
            capture_hotkey: default_capture_hotkey(),
            speed_multiplier: 1.0,
            repeat_mode: MacroRepeatMode::Once,
            repeat_count: 1,
            playback_options: wmacro_core_types::PlaybackOptions::default(),
            record_hotkey_behavior: RecordHotkeyBehavior::default(),
            record_mouse: true,
            record_movements: true,
            record_keyboard: true,
            toolbox_recents: Vec::new(),
            show_toolbox: true,
            recents_collapsed: false,
        }
    }
}

impl Settings {
    fn path() -> Option<std::path::PathBuf> {
        directories::ProjectDirs::from("", "", "wmacro")
            .map(|d| d.config_dir().join("settings.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str(&text) {
            Ok(settings) => settings,
            Err(err) => {
                error!(
                    "wmacro: failed to parse settings at {}, falling back to defaults: {err}",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        // TODO: write to a temp file and rename, so a crash mid-write cannot corrupt the settings file.
        if let Some(parent) = path.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            error!(
                "wmacro: failed to create settings directory {}: {err}",
                parent.display()
            );
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(err) = std::fs::write(&path, text) {
                    error!(
                        "wmacro: failed to write settings to {}: {err}",
                        path.display()
                    );
                }
            }
            Err(err) => error!("wmacro: failed to serialize settings: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_boolean_fields_default_to_enabled() {
        // a pre-show_toolbox / pre-record_* settings file must still load and
        // pick up `true` for the absent booleans instead of failing wholesale.
        let legacy = serde_json::json!({
            "theme_name": "Nord",
            "record_hotkey": null,
            "abort_record_hotkey": null,
            "play_hotkey": null,
            "abort_play_hotkey": null,
            "step_play_hotkey": null,
            "capture_hotkey": null,
            "speed_multiplier": 1.5,
            "repeat_mode": "Once",
            "repeat_count": 3,
            "playback_options": { "smart_path": {} },
            "record_hotkey_behavior": "Append",
        });
        let settings: Settings =
            serde_json::from_value(legacy).expect("legacy settings must deserialize");
        assert_eq!(settings.theme_name, "Nord");
        assert_eq!(settings.speed_multiplier, 1.5);
        assert!(settings.record_mouse);
        assert!(settings.record_movements);
        assert!(settings.record_keyboard);
        assert!(settings.show_toolbox);
        assert!(!settings.recents_collapsed);
    }

    #[test]
    fn explicit_false_is_preserved() {
        let mut value = serde_json::to_value(Settings::default()).unwrap();
        value["show_toolbox"] = serde_json::Value::Bool(false);
        let settings: Settings = serde_json::from_value(value).expect("round trip");
        assert!(!settings.show_toolbox);
    }
}
