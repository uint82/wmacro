use wmacro_core_types::Hotkey;

pub const COMMON_KEYS: &[(u16, &str); 113] = &[
    (1, "Esc"), (59, "F1"), (60, "F2"), (61, "F3"), (62, "F4"), (63, "F5"),
    (64, "F6"), (65, "F7"), (66, "F8"), (67, "F9"), (68, "F10"), (87, "F11"),
    (88, "F12"), (2, "1"), (3, "2"), (4, "3"), (5, "4"), (6, "5"), (7, "6"),
    (8, "7"), (9, "8"), (10, "9"), (11, "0"), (30, "A"), (48, "B"), (46, "C"),
    (32, "D"), (18, "E"), (33, "F"), (34, "G"), (35, "H"), (23, "I"), (36, "J"),
    (37, "K"), (38, "L"), (50, "M"), (49, "N"), (24, "O"), (25, "P"), (16, "Q"),
    (19, "R"), (31, "S"), (20, "T"), (22, "U"), (47, "V"), (17, "W"), (45, "X"),
    (21, "Y"), (44, "Z"), (12, "-"), (13, "="), (26, "["), (27, "]"), (39, ";"),
    (40, "'"), (41, "`"), (43, "\\"), (51, ","), (52, "."), (53, "/"),
    (86, "ISO<>"), (14, "Backspace"), (15, "Tab"), (28, "Enter"), (57, "Space"),
    (58, "CapsLock"), (69, "NumLock"), (70, "ScrollLock"), (29, "LCtrl"),
    (97, "RCtrl"), (42, "LShift"), (54, "RShift"), (56, "LAlt"), (100, "RAlt"),
    (125, "Super"), (126, "RSuper"), (127, "Menu"), (102, "Home"), (103, "↑"),
    (104, "PageUp"), (105, "←"), (106, "→"), (107, "End"), (108, "↓"),
    (109, "PageDown"), (110, "Insert"), (111, "Delete"), (71, "Num7"),
    (72, "Num8"), (73, "Num9"), (74, "Num-"), (75, "Num4"), (76, "Num5"),
    (77, "Num6"), (78, "Num+"), (79, "Num1"), (80, "Num2"), (81, "Num3"),
    (82, "Num0"), (83, "Num."), (96, "NumEnter"), (98, "Num/"), (55, "Num*"),
    (113, "Mute"), (114, "VolDown"), (115, "VolUp"), (163, "NextSong"),
    (164, "Play/Pause"), (165, "PrevSong"), (158, "BrowserBack"),
    (159, "BrowserForward"), (99, "SysRq"), (119, "Pause"),
];

pub fn key_code_display_name(code: u16) -> String {
    COMMON_KEYS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| format!("Key {}", code))
}

pub fn hotkey_display_name(hk: &Hotkey) -> String {
    let mut name = String::with_capacity(16);

    if hk.mods.ctrl {
        name.push_str("Ctrl+");
    }
    if hk.mods.shift {
        name.push_str("Shift+");
    }
    if hk.mods.alt {
        name.push_str("Alt+");
    }
    if hk.mods.meta {
        name.push_str("Super+");
    }

    name.push_str(&key_code_display_name(hk.code));
    name
}

pub fn hotkey_display_name_opt(hk: Option<Hotkey>, none_text: &str) -> String {
    hk.map_or_else(|| none_text.to_string(), |hk| hotkey_display_name(&hk))
}
