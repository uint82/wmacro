# Wmacro


<p align="center">
    <img src="assets/icon/wmacro.png" alt="Wmacro logo" width="200" />
</p>


Wmacro is a macro recorder app for Arch Linux + Hyprland. Users can record, edit, and replay mouse/keyboard events.
It features a GUI with 13 built-in themes, shortcuts, and custom .wmr text files to save the macro.

## Installation

Currently, `wmacro` can be installed on Arch Linux using the provided `PKGBUILD`.

1. Clone the repository:
   ```bash
   git clone https://github.com/uint82/wmacro.git
   cd wmacro/scripts/packaging/arch/wmacro-git
   ```

2. Build and install the package:
   ```bash
   makepkg -si
   ```

3. Add your user to the `wmacro` group to allow the GUI to communicate with the system daemon:
   ```bash
   sudo usermod -aG wmacro $USER
   ```

4. Log out and log back in (or reboot) for the group changes to take effect.

5. Enable and start the system daemon:
   ```bash
   sudo systemctl enable --now wmacro-daemon
   ```

### Building from Source

If you prefer to build manually or are on a different distribution, you can compile `wmacro` from source using `cargo`. Make sure you have the Rust toolchain and dependencies installed (e.g., `wayland`, `libxkbcommon`).

1. Clone the repository and build the project:
   ```bash
   git clone https://github.com/uint82/wmacro.git
   cd wmacro
   cargo build
   ```

2. Run the daemon (requires root privileges to access input devices):
   ```bash
   sudo ./target/debug/daemon
   ```

3. In a separate terminal, run the GUI:
   ```bash
   cargo run --bin gui
   ```

## Screenshots

| Gruvbox Dark | Gruvbox Light  |
| :---: | :---: |
| ![Gruvbox Dark](assets/screenshots/gruvbox_dark.png) | ![Gruvbox Light](assets/screenshots/gruvbox_light.png) |

## Features

### Record and playback

- Mouse: movement, clicks, and scrolling
- Touchpad: movement, clicks, and drag and drop
- Keyboard: All keys
- Record/pause/resume
- Play/pause/resume
- Adjustable playback speed from `0.1x` to `10.0x`
- Adjustable repeat, humanize playback using hybrid synthetic path engine
- Smart Mouse Randomization with adjustable path wobble and endpoint jitter
- Humanized submovements and curve adjustments for mouse paths
- Customizable global hotkeys (default):
    - `F7` record/pause
    - `F8` abort recording
    - `F9` play/pause
    - `F10` abort playback
    - `Shift + k` step-by-step
    - `F2` Capture Coordinate

### Editor and Flow Control
- Full GUI editor for creating and modifying macros
- Drag and drop command reordering
- Type text action for fast text input
- Logic and Flow Control:
    - If Pixel Color condition
    - Loop and EndLoop
    - Else and EndIf
    - Goto and Label
- Execute other macros from within a macro
- Custom `.wmr` files for saving and loading macros

### Appearance
- 13 built-in themes
- Modern interface with customizable settings
- User can add their own themes in `.config/wmacro/themes/`

#### Creating Custom Themes

To create a custom theme, place a `.json` file in `~/.config/wmacro/themes/` using the structure below. Colors must be provided in 6-character hex format (e.g., `#FFFFFF`).

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
  "col_import_saved_macro": "#f38ba8"
}
```

## Roadmap
- [ ] Expand flow control with advanced variables and conditions
- [ ] Additional IDE themes and layout customizations
- [ ] Add more wayland compositor support

## Contributing
Contributions are always welcome! If you have a feature request, bug report, or want to submit a pull request, please check the [issues page](https://github.com/uint82/wmacro/issues) on GitHub.

When submitting PRs, please ensure your code builds cleanly and follows the existing Rust conventions.

## License
This project is licensed under the GPL-3.0 License - see the [LICENSE](LICENSE) file for details.
