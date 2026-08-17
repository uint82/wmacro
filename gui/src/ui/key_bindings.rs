//! canonical list of fixed ui keybindings (single source of truth); system hotkeys (Record, Play, Capture) are user-configurable and live in settings.

/// canonical list of fixed UI keybindings (single source of truth); system hotkeys (Record, Play, Capture) are user-configurable and live in settings.
pub struct KeyBinding {
    pub group: &'static str,
    pub action: &'static str,
    pub keys: &'static str,
}

pub const GROUPS: [&str; 3] = ["Editor", "Global", "Dialogs & Pickers"];

pub static BINDINGS: &[KeyBinding] = &[
    // editor: active when the command list has focus
    KeyBinding {
        group: "Editor",
        action: "Select all commands",
        keys: "Ctrl+A",
    },
    KeyBinding {
        group: "Editor",
        action: "Copy selected",
        keys: "Ctrl+C",
    },
    KeyBinding {
        group: "Editor",
        action: "Cut selected",
        keys: "Ctrl+X",
    },
    KeyBinding {
        group: "Editor",
        action: "Paste",
        keys: "Ctrl+V",
    },
    KeyBinding {
        group: "Editor",
        action: "Duplicate selection",
        keys: "Ctrl+D",
    },
    KeyBinding {
        group: "Editor",
        action: "Undo",
        keys: "Ctrl+Z",
    },
    KeyBinding {
        group: "Editor",
        action: "Redo",
        keys: "Ctrl+Shift+Z / Ctrl+Y",
    },
    KeyBinding {
        group: "Editor",
        action: "Move selection / extend selection",
        keys: "↑ / Shift+↑",
    },
    KeyBinding {
        group: "Editor",
        action: "Move rows up / down",
        keys: "Alt+↑ / Alt+↓",
    },
    KeyBinding {
        group: "Editor",
        action: "Edit selected command",
        keys: "Enter",
    },
    KeyBinding {
        group: "Editor",
        action: "Delete selected",
        keys: "Delete / Backspace",
    },
    KeyBinding {
        group: "Editor",
        action: "Find in macro",
        keys: "Ctrl+Shift+F",
    },
    KeyBinding {
        group: "Editor",
        action: "Find & replace",
        keys: "Ctrl+Shift+H",
    },
    KeyBinding {
        group: "Editor",
        action: "Find: next / previous match",
        keys: "Enter / Shift+Enter",
    },
    KeyBinding {
        group: "Editor",
        action: "Close find bar",
        keys: "Esc",
    },
    // global: active app-wide
    KeyBinding {
        group: "Global",
        action: "Toggle toolbox",
        keys: "Ctrl+B",
    },
    KeyBinding {
        group: "Global",
        action: "Focus toolbox search",
        keys: "Ctrl+F",
    },
    KeyBinding {
        group: "Global",
        action: "Close settings / dismiss alerts",
        keys: "Esc",
    },
    // dialogs & pickers.
    KeyBinding {
        group: "Dialogs & Pickers",
        action: "Confirm dialog input",
        keys: "Enter",
    },
    KeyBinding {
        group: "Dialogs & Pickers",
        action: "Close dialog",
        keys: "Esc",
    },
    KeyBinding {
        group: "Dialogs & Pickers",
        action: "Cancel / confirm coordinate picker",
        keys: "Esc / Enter",
    },
];
