use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareEvent {
    pub hardware_time: SystemTime,
    pub kind: HardwareEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HardwareEventKind {
    MouseMove,
    MouseDown(MacroButton),
    MouseUp(MacroButton),
    Scroll { dx: i32, dy: i32 },
    KeyDown(String, u16),
    KeyUp(String, u16),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonRequest {
    MoveTo {
        x: i32,
        y: i32,
    },
    Press {
        button: ClickButton,
    },
    Release {
        button: ClickButton,
    },
    Scroll {
        dx: i32,
        dy: i32,
    },
    Click {
        target_x: i32,
        target_y: i32,
        current_x: i32,
        current_y: i32,
        button: ClickButton,
        click_type: ClickType,
        hold_duration_ms: u64,
        move_cursor: bool,
    },
    KeyDown {
        key: String,
        code: u16,
    },
    KeyUp {
        key: String,
        code: u16,
    },
    StartRecording,
    StopRecording,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MousePosition {
    Absolute { x: i32, y: i32 },
    Current,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonResponse {
    HardwareInput(HardwareEvent),
    HotkeyTriggered(u16),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DaemonEvent {
    Recorded(HardwareEvent),
    Hotkey(HotkeyEvent),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClickButton {
    Left,
    Right,
    Middle,
}

impl ClickButton {
    pub fn label(&self) -> &'static str {
        match self {
            ClickButton::Left => "Left",
            ClickButton::Right => "Right",
            ClickButton::Middle => "Middle",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClickType {
    Single,
    Double,
}

impl ClickType {
    pub fn label(&self) -> &'static str {
        match self {
            ClickType::Single => "Single",
            ClickType::Double => "Double",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HotkeyEvent {
    pub code: u16,
    pub pressed: bool,
}

pub const BTN_LEFT_CODE: u16 = 0x110;
pub const BTN_RIGHT_CODE: u16 = 0x111;
pub const BTN_MIDDLE_CODE: u16 = 0x112;

pub const KEY_LEFTCTRL_CODE: u16 = 29;
pub const KEY_RIGHTCTRL_CODE: u16 = 97;
pub const KEY_LEFTSHIFT_CODE: u16 = 42;
pub const KEY_RIGHTSHIFT_CODE: u16 = 54;
pub const KEY_LEFTALT_CODE: u16 = 56;
pub const KEY_RIGHTALT_CODE: u16 = 100;
pub const KEY_LEFTMETA_CODE: u16 = 125;
pub const KEY_RIGHTMETA_CODE: u16 = 126;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub fn apply(&mut self, code: u16, pressed: bool) -> bool {
        match code {
            KEY_LEFTCTRL_CODE | KEY_RIGHTCTRL_CODE => {
                self.ctrl = pressed;
                true
            }
            KEY_LEFTSHIFT_CODE | KEY_RIGHTSHIFT_CODE => {
                self.shift = pressed;
                true
            }
            KEY_LEFTALT_CODE | KEY_RIGHTALT_CODE => {
                self.alt = pressed;
                true
            }
            KEY_LEFTMETA_CODE | KEY_RIGHTMETA_CODE => {
                self.meta = pressed;
                true
            }
            _ => false,
        }
    }

    pub fn is_modifier_code(code: u16) -> bool {
        return matches!(
            code,
            KEY_LEFTCTRL_CODE
                | KEY_RIGHTCTRL_CODE
                | KEY_LEFTSHIFT_CODE
                | KEY_RIGHTSHIFT_CODE
                | KEY_LEFTALT_CODE
                | KEY_RIGHTALT_CODE
                | KEY_LEFTMETA_CODE
                | KEY_RIGHTMETA_CODE
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotkey {
    pub code: u16,
    pub mods: Modifiers,
}

impl Hotkey {
    pub fn new(code: u16, mods: Modifiers) -> Self {
        Self { code, mods }
    }

    pub fn plain(code: u16) -> Self {
        Self {
            code,
            mods: Modifiers::default(),
        }
    }

    pub fn matches(&self, code: u16, mods: &Modifiers) -> bool {
        self.code == code && self.mods == *mods
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum MacroButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MacroEvent {
    Delay(u64),
    MouseMove {
        x: i32,
        y: i32,
    },
    Click {
        position: MousePosition,
        button: MacroButton,
        jitter: u32,
        hold_time_ms: u32,
    },
    MouseDown {
        position: MousePosition,
        button: MacroButton,
        jitter: u32,
    },
    MouseUp {
        position: MousePosition,
        button: MacroButton,
        jitter: u32,
    },
    Scroll {
        dx: i32,
        dy: i32,
    },
    KeyDown {
        key: String,
        code: u16,
    },
    KeyUp {
        key: String,
        code: u16,
    },
    KeyPress {
        key: String,
        code: u16,
        hold_time_ms: u32,
    },
}

impl MacroEvent {
    pub fn delay_us(&self) -> u64 {
        match self {
            MacroEvent::Delay(us) => *us,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SmartPathOptions {
    pub enabled: bool,
    pub path_wobble: f32,
    pub endpoint_jitter: f32,
    pub segment_delay_threshold_ms: u64,
    pub path_curve: f32,
    pub submovement_enabled: bool,
    pub short_move_threshold: f32,
    pub long_move_threshold: f32,
}

impl Default for SmartPathOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            path_wobble: 12.0,
            endpoint_jitter: 2.0,
            segment_delay_threshold_ms: 100,
            path_curve: 0.08,
            submovement_enabled: true,
            short_move_threshold: 80.0,
            long_move_threshold: 300.0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaybackOptions {
    pub smart_path: SmartPathOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub version: u8,
    pub commands: Vec<MacroCommand>,
}

impl Macro {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: 3,
            commands: Vec::new(),
        }
    }

    pub fn total_duration_us(&self) -> u64 {
        self.commands
            .iter()
            .map(|cmd| match cmd {
                MacroCommand::Action(ev) => ev.delay_us(),
                _ => 0,
            })
            .sum()
    }

    pub fn total_duration_ms(&self) -> u64 {
        self.total_duration_us() / 1000
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MacroCommand {
    Action(MacroEvent),
    IfPixelColor { x: i32, y: i32, r: u8, g: u8, b: u8, tolerance: u8 },
    Else,
    EndIf,
    Loop { count: u32 },
    EndLoop,
    PlayMacro(String),
    Label(String),
    Goto(String),
    TypeText(String),
    // TODO: add BreakLoop action for dynamically escaping loops.
    // TODO: add ExitMacro action for halting execution early.
}
