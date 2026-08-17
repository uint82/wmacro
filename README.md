# Wmacro

[![Arch Linux AUR](https://img.shields.io/badge/Arch-AUR-1793D1?logo=arch-linux&logoColor=white)](https://aur.archlinux.org/packages/wmacro)
[![Release](https://img.shields.io/github/v/release/uint82/wmacro)](https://github.com/uint82/wmacro/releases)
[![License](https://img.shields.io/github/license/uint82/wmacro)](LICENSE)

<p align="center">
    <img src="assets/icon/wmacro.png" alt="Wmacro logo" width="200" />
</p>

Wmacro is a macro recorder and automation tool for Wayland. Record, edit, and replay mouse and keyboard events, then
humanize the playback so it feels like a real person at the desk. It runs natively on Wayland through the
xdg-desktop-portal ScreenCast capture path, so it works on Hyprland, KDE Plasma, GNOME, Wayfire, and other compositors.

---

## Table of Contents

* [Features](#features)
* [Requirements](#requirements)
* [Installation](#installation)
  * [Arch Linux (AUR)](#arch-linux-aur)
  * [Building from Source](#building-from-source)
* [Daemon setup](#daemon-setup)
* [Usage and hotkeys](#usage-and-hotkeys)
* [Screenshots](#screenshots)
* [Custom themes](#custom-themes)
* [Roadmap](#roadmap)
* [Troubleshooting](#troubleshooting)
* [Contributing](#contributing)
* [License](#license)

---

## Features

### Recording

- Mouse: movement, clicks, drag and drop, and scrolling
- Touchpad: movement, clicks, and drag and drop
- Keyboard: all keys
- Record, pause, and resume
- Play, pause, and resume
- Adjustable playback speed from `0.1x` to `10.0x`
- Adjustable repeat count and manual delay insertion
- Humanized playback with a hybrid synthetic path engine, path wobble, endpoint jitter, and curve adjustments

### Scripting and flow control

- Variables (`$name`) referenced by coordinates, durations, text, and expressions; missing variables read as 0
- Calculate expressions: decimal arithmetic (`+ - * / %`), text concatenation (`.`), comparisons, a ternary operator,
  string literals, and function calls (`abs`, `round`, `floor`, `ceil`, `min`, `max`, `clamp`)
- If Compare: branch on variable values with `=`, `!=`, `<`, `>`, `<=`, `>=`
- If Pixel Color: branch on a pixel color at a captured coordinate
- If Color Found: branch on the largest connected color region appearing within a region
- If Image Found: match a target image on screen and store its position into variables
- Loop / EndLoop and Else / EndIf blocks
- Goto and Label with safe index-0 handling
- Execute other macros from within a macro
- Set / Get Clipboard: read and write the Wayland and X11 selections, including the wlroots XWayland
  clipboard-mirror workaround so XWayland apps (most browsers) can paste
- Custom `.wmr` script files: a parseable text format with a full parser and serializer, saved and loaded by the GUI

### Editor

- Full GUI editor for creating and modifying macros
- Drag and drop command reordering, selection, copy, cut, paste, duplicate
- Undo and redo (Ctrl+Z / Ctrl+Y)
- Find and replace across command fields (case-insensitive)
- Inline value editing: click a command's detail text to edit it in place
- Live preview tooltips describing each command in full
- Block analysis: foldable If / Loop blocks with orphan-block detection
- Type text action for fast text input
- Open File action with arguments and optional PolicyKit elevation

### Appearance and customization

- 13 built-in themes (light and dark)
- Custom themes in `~/.config/wmacro/themes/`
- Customizable system hotkeys (record, play, abort, step, capture)

---

## Requirements

### Runtime

- A Wayland compositor with an xdg-desktop-portal ScreenCast implementation (Hyprland, KDE Plasma, GNOME, Wayfire,
  and wlroots-based compositors)
- The `uinput` kernel module loaded (`modprobe uinput`), with read and write access to `/dev/uinput`
- `xdg-desktop-portal` with the ScreenCast interface available (the primary capture path goes
  portal → PipeWire → DMABuf → GBM CPU readback)
- `slurp` and `grim` (optional fallback for region selection and capture when the portal path is unavailable)
- `pkexec` (PolicyKit) for the "Run as administrator" option on the Open File command
- A running `wmacro-daemon` with the user in the `wmacro` group

### Build

- Rust toolchain (`rust`, `cargo`)
- C compiler toolchain (`base-devel` on Arch)
- `pkg-config` with development headers for: `wayland-client`, `libxkbcommon`, `gbm`, `pipewire` / `libspa`,
  `dbus` (via ashpd), and `udev`

---

## Installation

### Arch Linux (AUR)

#### Stable release:

   ```bash
   paru -S wmacro
   ```
   or
   ```bash
   yay -S wmacro
   ```

#### Development version:

   ```bash
   paru -S wmacro-git
   ```
   or
   ```bash
   yay -S wmacro-git
   ```

The package installs the daemon as a systemd service, a `wmacro` user, a udev rule granting `/dev/uinput` access,
and a modules-load entry that loads the `uinput` kernel module at boot. After installation, complete the
[daemon setup](#daemon-setup).

### Building from Source

If you prefer to build manually or are on a different distribution, you can compile `wmacro` from source with `cargo`.

1. Clone the repository and build the project:

   ```bash
   git clone https://github.com/uint82/wmacro.git
   cd wmacro
   cargo build --release --workspace
   ```

2. Run the daemon (requires root privileges to access input devices):

   ```bash
   sudo ./target/release/wmacro-daemon
   ```

3. In a separate terminal, run the GUI:

   ```bash
   ./target/release/wmacro-gui
   ```

---

## Daemon setup

The daemon injects and reads input through `uinput` and `evdev`, and the GUI talks to it over a unix socket in
`/run/wmacro`. For AUR installs, the package already ships the sysusers config (daemon user and groups), the udev
rule, and the modules-load entry, so only two steps are needed:

1. Add your user to the `wmacro` group:

   ```bash
   sudo usermod -aG wmacro $USER
   ```

2. Log out and log back in (or reboot, which also loads the `uinput` module), then enable and start the daemon:

   ```bash
   sudo systemctl enable --now wmacro-daemon
   ```

If the daemon cannot open the input devices, it will log:

```
Ensure you are in the 'wmacro' group and the uinput module is loaded.
```

---

## Usage and hotkeys

Record with the record hotkey, play with the play hotkey. Everything is visible in the GUI: the command list, the
toolbox, and the status bar. System hotkeys are configurable in Settings > Hotkeys, with these defaults:

| Action | Default |
| :--- | :--- |
| Record / pause | `F7` |
| Abort recording | `F8` |
| Play / pause | `F9` |
| Abort playback | `F10` |
| Step through playback | `Shift + K` |
| Capture coordinate | `F2` |

Editor keybindings (active when the command list has focus):

| Action | Keys |
| :--- | :--- |
| Select all commands | `Ctrl+A` |
| Copy selected | `Ctrl+C` |
| Cut selected | `Ctrl+X` |
| Paste | `Ctrl+V` |
| Duplicate selection | `Ctrl+D` |
| Undo / redo | `Ctrl+Z` / `Ctrl+Shift+Z` / `Ctrl+Y` |
| Move selection / extend selection | `↑` / `Shift+↑` |
| Move rows up / down | `Alt+↑` / `Alt+↓` |
| Edit selected command | `Enter` |
| Delete selected | `Delete` / `Backspace` |
| Find in macro | `Ctrl+Shift+F` |
| Find and replace | `Ctrl+Shift+H` |
| Find: next / previous match | `Enter` / `Shift+Enter` |
| Close find bar | `Esc` |

Global and dialog shortcuts:

| Action | Keys |
| :--- | :--- |
| Toggle toolbox | `Ctrl+B` |
| Focus toolbox search | `Ctrl+F` |
| Confirm dialog input | `Enter` |
| Close dialog / dismiss alerts | `Esc` |

---

## Screenshots

| Gruvbox Dark | Gruvbox Light |
| :---: | :---: |
| ![Gruvbox Dark](assets/screenshots/gruvbox_dark.png) | ![Gruvbox Light](assets/screenshots/gruvbox_light.png) |

---

## Custom themes

To create a custom theme, place a `.json` file in `~/.config/wmacro/themes/` using the structure below. Colors must
be provided in 6-character hex format (e.g., `#FFFFFF`).

```json
{
  "name": "My Custom Theme",
  "is_dark": true,
  "bg_base": "#1e1e2e",
  "bg_surface": "#181825",
  "bg_element": "#313244",
  "bg_element_alt": "#45475a",
  "border": "#cba6f7",
  "text_primary": "#cdd6f4",
  "text_muted": "#a6adc8",
  "accent_primary": "#cba6f7",
  "accent_primary_fg": "#11111b",
  "accent_danger": "#f38ba8",
  "accent_danger_fg": "#11111b",
  "accent_success": "#a6e3a1",
  "accent_success_fg": "#11111b",
  "col_delay": "#f9e2af",
  "col_move": "#89b4fa",
  "col_click": "#f38ba8",
  "col_keyboard": "#cba6f7",
  "col_if": "#94e2d5",
  "col_else": "#94e2d5",
  "col_end_if": "#94e2d5",
  "col_loop": "#f5c2e7",
  "col_end_loop": "#f5c2e7",
  "col_label": "#b4befe",
  "col_goto": "#b4befe",
  "col_type_text": "#a6e3a1",
  "col_import_saved_macro": "#f38ba8",
  "col_var": "#e6b9a8",
  "col_calc": "#a8c1e6",
  "col_clipboard": "#8fd0b8"
}
```

---

## Roadmap

- [x] Variables, expressions, and advanced conditions (v0.3.0)
- [ ] Alternative scripting backends for macros (e.g., Python or an embedded Rust DSL)
- [ ] Additional IDE themes and layout customizations
- [ ] More Wayland compositor coverage

---

## Troubleshooting

| Problem | Fix |
| :--- | :--- |
| "Ensure you are in the 'wmacro' group and the uinput module is loaded." | Add yourself to the `wmacro` group, reboot (or run `modprobe uinput`), then restart the daemon |
| Screen capture returns nothing | Make sure `xdg-desktop-portal` and a ScreenCast implementation (e.g., `xdg-desktop-portal-hyprland`) are installed and running |
| XWayland apps paste stale clipboard text | A clipboard mirror fight with wlroots compositors can need one extra paste attempt; the daemon re-asserts the X11 selection automatically |
| GUI shows "backend unavailable" | The portal session may need one grant; check the daemon logs and your portal configuration |

---

## Contributing

Contributions are always welcome! If you have a feature request, bug report, or want to submit a pull request, please
check the [issues page](https://github.com/uint82/wmacro/issues) on GitHub.

When submitting PRs, please ensure your code builds cleanly and follows the existing Rust conventions.

## License

This project is licensed under the GPL-3.0-only License - see the [LICENSE](LICENSE) file for details.