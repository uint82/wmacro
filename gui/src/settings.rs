use crate::state::{MacroRepeatMode, RecordHotkeyBehavior};
use wmacro_core_types::{Hotkey, Modifiers};
use log::error;
use serde::{Deserialize, Serialize};

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

    pub record_mouse: bool,
    pub record_movements: bool,
    pub record_keyboard: bool,
}

// uncomment if a future release adds a new boolean setting.
// so settings.json can still be deserilized without the new bool field
//
// #[serde(default = "default_true")]
//
// fn default_true() -> bool {
//     true
// }

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
    Some(Hotkey::new(37, Modifiers { shift: true, ..Default::default() }))
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
        if let Some(parent) = path.parent() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                error!(
                    "wmacro: failed to create settings directory {}: {err}",
                    parent.display()
                );
                return;
            }
        }
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(err) = std::fs::write(&path, text) {
                    error!("wmacro: failed to write settings to {}: {err}", path.display());
                }
            }
            Err(err) => error!("wmacro: failed to serialize settings: {err}"),
        }
    }
}
