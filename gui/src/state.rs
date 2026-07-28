use crate::settings::{
    default_abort_play_hotkey, default_abort_record_hotkey, default_capture_hotkey,
    default_step_play_hotkey, Settings, DEFAULT_THEME_NAME,
};
use core_types::{Hotkey, Macro};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum DelayUnit {
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
}

impl DelayUnit {
    pub fn label(&self) -> &'static str {
        match self {
            DelayUnit::Milliseconds => "ms",
            DelayUnit::Seconds => "s",
            DelayUnit::Minutes => "m",
            DelayUnit::Hours => "h",
        }
    }

    pub fn to_ms(&self, value: f64) -> u64 {
        let ms = match self {
            DelayUnit::Milliseconds => value,
            DelayUnit::Seconds => value * 1_000.0,
            DelayUnit::Minutes => value * 60_000.0,
            DelayUnit::Hours => value * 3_600_000.0,
        };
        ms.max(1.0) as u64
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RecordHotkeyBehavior {
    Overwrite,
    Append,
}

impl Default for RecordHotkeyBehavior {
    fn default() -> Self {
        Self::Append
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MacroRepeatMode {
    Once,
    Count(u32),
    Infinite,
}

#[derive(Debug)]
pub struct MacroState {
    pub recording: bool,
    pub record_paused: bool,
    pub playing: bool,
    pub play_paused: bool,

    pub current_macro: Option<Macro>,
    pub macro_name: String,

    pub events_captured: usize,
    pub events_played: usize,
    pub current_step: usize,
    pub current_loop: u32,

    pub speed_multiplier: f32,
    pub repeat_mode: MacroRepeatMode,
    pub repeat_count: u32,
    pub playback_options: core_types::PlaybackOptions,

    pub record_hotkey: Option<Hotkey>,
    pub abort_record_hotkey: Option<Hotkey>,
    pub play_hotkey: Option<Hotkey>,
    pub abort_play_hotkey: Option<Hotkey>,
    pub step_play_hotkey: Option<Hotkey>,
    pub capture_hotkey: Option<Hotkey>,

    pub binding_record: bool,
    pub binding_abort_record: bool,
    pub binding_play: bool,
    pub binding_abort_play: bool,
    pub binding_step_play: bool,
    pub binding_capture: bool,

    pub play_kill: Option<Arc<std::sync::atomic::AtomicBool>>,
    pub play_paused_flag: Option<Arc<std::sync::atomic::AtomicBool>>,
    pub play_step_flag: Option<Arc<std::sync::atomic::AtomicBool>>,
    pub record_paused_flag: Option<Arc<std::sync::atomic::AtomicBool>>,

    pub recording_start: Option<std::time::SystemTime>,
    pub record_hotkey_behavior: RecordHotkeyBehavior,
    pub record_mouse: bool,
    pub record_movements: bool,
    pub record_keyboard: bool,
}

impl Default for MacroState {
    fn default() -> Self {
        Self {
            recording: false,
            record_paused: false,
            playing: false,
            play_paused: false,
            current_macro: None,
            macro_name: String::from("untitled"),
            events_captured: 0,
            events_played: 0,
            current_step: 0,
            current_loop: 0,
            speed_multiplier: 1.0,
            repeat_mode: MacroRepeatMode::Once,
            repeat_count: 1,
            playback_options: core_types::PlaybackOptions::default(),
            record_hotkey: Some(Hotkey::plain(65)),
            abort_record_hotkey: default_abort_record_hotkey(),
            play_hotkey: Some(Hotkey::plain(66)),
            abort_play_hotkey: default_abort_play_hotkey(),
            step_play_hotkey: default_step_play_hotkey(),
            capture_hotkey: default_capture_hotkey(),
            binding_record: false,
            binding_abort_record: false,
            binding_play: false,
            binding_abort_play: false,
            binding_step_play: false,
            binding_capture: false,
            play_kill: None,
            play_paused_flag: None,
            play_step_flag: None,
            record_paused_flag: None,
            recording_start: None,
            record_hotkey_behavior: RecordHotkeyBehavior::default(),
            record_mouse: true,
            record_movements: true,
            record_keyboard: true,
        }
    }
}

#[derive(Debug)]
pub struct AppState {
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub active_capture: Option<(i32, i32)>,

    pub status_msg: String,

    pub macro_state: MacroState,

    pub theme_name: String,
    pub theme_manager: crate::ui::theme::ThemeManager,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            cursor_x: 0,
            cursor_y: 0,
            active_capture: None,
            status_msg: String::from("Ready"),
            macro_state: MacroState::default(),
            theme_name: DEFAULT_THEME_NAME.to_string(),
            theme_manager: crate::ui::theme::ThemeManager::new(),
        }
    }
}

impl AppState {
    pub fn with_settings(settings: Settings) -> Self {
        let mut app_state = Self::default();
        app_state.macro_state.record_hotkey = settings.record_hotkey;
        app_state.macro_state.abort_record_hotkey = settings.abort_record_hotkey;
        app_state.macro_state.play_hotkey = settings.play_hotkey;
        app_state.macro_state.abort_play_hotkey = settings.abort_play_hotkey;
        app_state.macro_state.step_play_hotkey = settings.step_play_hotkey;
        app_state.macro_state.capture_hotkey = settings.capture_hotkey;
        app_state.macro_state.speed_multiplier = settings.speed_multiplier;
        app_state.macro_state.repeat_mode = settings.repeat_mode;
        app_state.macro_state.repeat_count = settings.repeat_count;
        app_state.macro_state.playback_options = settings.playback_options;
        app_state.macro_state.record_hotkey_behavior = settings.record_hotkey_behavior;
        app_state.macro_state.record_mouse = settings.record_mouse;
        app_state.macro_state.record_movements = settings.record_movements;
        app_state.macro_state.record_keyboard = settings.record_keyboard;
        app_state.theme_name = settings.theme_name;
        app_state
    }

    pub fn to_settings(&self) -> Settings {
        Settings {
            record_hotkey: self.macro_state.record_hotkey,
            abort_record_hotkey: self.macro_state.abort_record_hotkey,
            play_hotkey: self.macro_state.play_hotkey,
            abort_play_hotkey: self.macro_state.abort_play_hotkey,
            step_play_hotkey: self.macro_state.step_play_hotkey,
            capture_hotkey: self.macro_state.capture_hotkey,
            speed_multiplier: self.macro_state.speed_multiplier,
            repeat_mode: self.macro_state.repeat_mode.clone(),
            repeat_count: self.macro_state.repeat_count,
            playback_options: self.macro_state.playback_options.clone(),
            record_hotkey_behavior: self.macro_state.record_hotkey_behavior.clone(),
            theme_name: self.theme_name.clone(),
            record_mouse: self.macro_state.record_mouse,
            record_movements: self.macro_state.record_movements,
            record_keyboard: self.macro_state.record_keyboard,
        }
    }
}

pub type SharedState = Arc<Mutex<AppState>>;

pub fn new_shared_state() -> SharedState {
    Arc::new(Mutex::new(AppState::with_settings(Settings::load())))
}
