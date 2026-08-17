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
    Absolute { x: Coord, y: Coord },
    Current,
}

/// a coordinate value: a fixed number or a `$name` variable reference,
/// resolved at playback time; a missing variable reads as 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Coord {
    Const(i32),
    Var(String),
}

/// a runtime variable value: a whole number, a decimal, or text, coerced via
/// [`Value::as_i64`] / [`Value::as_f64`] / [`Value::as_text`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Number(i64),
    Float(f64),
    Text(String),
}

impl Value {
    /// the value as a whole number: decimals truncate toward zero, text
    /// parses with whitespace ignored; unparseable text reads as 0.
    pub fn as_i64(&self) -> i64 {
        self.as_i64_opt().unwrap_or(0)
    }

    /// the value as a whole number when it is one, or fully numeric text.
    pub fn as_i64_opt(&self) -> Option<i64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Float(f) => Some(*f as i64),
            Value::Text(s) => s.trim().parse().ok(),
        }
    }

    /// the value as a decimal; numeric text parses.
    pub fn as_f64(&self) -> f64 {
        self.as_f64_opt().unwrap_or(0.0)
    }

    pub fn as_f64_opt(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n as f64),
            Value::Float(f) => Some(*f),
            Value::Text(s) => s.trim().parse().ok(),
        }
    }

    /// the value as text; integral decimals render without a trailing `.0`.
    pub fn as_text(&self) -> String {
        match self {
            Value::Number(n) => n.to_string(),
            Value::Float(f) => {
                if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                    format!("{}", *f as i64)
                } else {
                    f.to_string()
                }
            }
            Value::Text(s) => s.clone(),
        }
    }
}

/// a value for variable commands: a literal (number or text) or a variable reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Operand {
    Var(String),
    Literal(Value),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
}

impl CompareOp {
    pub fn symbol(&self) -> &'static str {
        match self {
            CompareOp::Eq => "==",
            CompareOp::Ne => "!=",
            CompareOp::Lt => "<",
            CompareOp::Le => "<=",
            CompareOp::Gt => ">",
            CompareOp::Ge => ">=",
            CompareOp::Contains => "contains",
        }
    }
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
        matches!(
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MacroEvent {
    Delay(u64),
    MouseMove {
        x: Coord,
        y: Coord,
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

/// current macro file format version; bump when a command's serialized form changes.
// TODO: add a migration path for loading macros saved with older format versions.
pub const CURRENT_FORMAT_VERSION: u8 = 8;

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
            version: CURRENT_FORMAT_VERSION,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MacroCommand {
    Action(MacroEvent),
    IfPixelColor {
        x: Coord,
        y: Coord,
        r: u8,
        g: u8,
        b: u8,
        tolerance: u8,
    },
    IfImageFound {
        target_image_path: String,
        similarity_threshold: f32,
        move_cursor_if_found: bool,
        trigger_if_not_found: bool,
        region: Option<(i32, i32, i32, i32)>,
        store_x: Option<String>,
        store_y: Option<String>,
    },
    /// searches `region` (None = whole screen) for the largest connected
    /// region within `tolerance` (0-100, Euclidean RGB distance) of (r,g,b);
    /// a match stores the center via store_x/store_y and size via store_w/store_h.
    IfColorFound {
        region: Option<(i32, i32, i32, i32)>,
        r: u8,
        g: u8,
        b: u8,
        tolerance: u8,
        min_width: u32,
        min_height: u32,
        move_cursor_if_found: bool,
        store_x: Option<String>,
        store_y: Option<String>,
        store_w: Option<String>,
        store_h: Option<String>,
    },
    Else,
    EndIf,
    Loop {
        /// number of iterations; a `Var` resolves to its runtime value.
        count: Operand,
    },
    EndLoop,
    PlayMacro(String),
    Label(String),
    Goto(String),
    TypeText(String),
    OpenFile {
        path: String,
        args: String,
        run_as_admin: bool,
    },
    SetVariable {
        target: String,
        value: Operand,
    },
    /// evaluates a free-form expression into a variable: `$` vars, decimals,
    /// quoted text, `+ - * / %`, `.` text joins, comparisons, `cond ? a : b`,
    /// and functions `abs floor ceil round sqrt min max random`.
    Calculate {
        target: String,
        expression: String,
    },
    /// compares operands; a missing variable reads as 0. `==`/`!=`/`contains`
    /// compare text forms; ordering ops compare numerically when both sides
    /// are numbers, lexicographically otherwise.
    IfCompare {
        left: Operand,
        op: CompareOp,
        right: Operand,
    },
    /// sleeps `duration_ms`, which may be a `$` variable (e.g. `$cooldown_ms`).
    Delay {
        duration_ms: Operand,
    },
    /// sets the clipboard to the operand's text form (text/plain, UTF-8).
    SetClipboard {
        text: Operand,
    },
    /// reads the clipboard text into a variable; an empty or non-text clipboard reads as `""`.
    GetClipboard {
        target: String,
    },
    /// a note attached to the macro; never executed, only visible in the editor and
    /// kept in the command list so it moves and saves with its steps.
    Comment(String),
    // TODO: add BreakLoop action for dynamically escaping loops.
    // TODO: add ExitMacro action for halting execution early.
}
