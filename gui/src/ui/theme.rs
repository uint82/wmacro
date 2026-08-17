//! loads, saves, and applies ui themes from json theme files.

use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFile {
    pub name: String,
    pub is_dark: bool,

    pub bg_base: String,
    pub bg_surface: String,
    pub bg_element: String,
    pub bg_element_alt: String,

    pub border: String,
    pub text_primary: String,
    pub text_muted: String,

    pub accent_primary: String,
    pub accent_primary_fg: String,
    pub accent_danger: String,
    pub accent_danger_fg: String,
    pub accent_success: String,
    pub accent_success_fg: String,

    pub col_delay: String,
    pub col_move: String,
    pub col_click: String,
    pub col_keyboard: String,
    pub col_if: String,

    pub col_else: String,
    pub col_end_if: String,
    pub col_loop: String,
    pub col_end_loop: String,
    pub col_label: String,
    pub col_goto: String,
    pub col_type_text: String,
    pub col_import_saved_macro: String,
    // new fields use serde defaults so existing theme files (built-in and user-provided) still deserialize.
    #[serde(default = "default_fallback_color")]
    pub col_var: String,
    #[serde(default = "default_fallback_color")]
    pub col_calc: String,
    #[serde(default = "default_fallback_color")]
    pub col_clipboard: String,
}

fn default_fallback_color() -> String {
    "#808080".to_string()
}

#[derive(Debug, Clone)]
pub struct ThemePalette {
    pub name: String,
    pub is_dark: bool,

    pub bg_base: Color32,
    pub bg_surface: Color32,
    pub bg_element: Color32,
    pub bg_element_alt: Color32,

    pub border: Color32,
    pub text_primary: Color32,
    pub text_muted: Color32,

    pub accent_primary: Color32,
    pub accent_primary_fg: Color32,
    pub accent_danger: Color32,
    pub accent_danger_fg: Color32,
    pub accent_success: Color32,
    pub accent_success_fg: Color32,

    pub col_delay: Color32,
    pub col_move: Color32,
    pub col_click: Color32,
    pub col_keyboard: Color32,
    pub col_if: Color32,
    pub col_else: Color32,
    pub col_end_if: Color32,
    pub col_loop: Color32,
    pub col_end_loop: Color32,
    pub col_label: Color32,
    pub col_goto: Color32,
    pub col_type_text: Color32,
    pub col_import_saved_macro: Color32,
    pub col_var: Color32,
    pub col_calc: Color32,
    pub col_clipboard: Color32,
}

/// TODO: expand this to support 3-digit hex (#FFF) and 8-digit hex with alpha (#FFFFFFFF) if needed.
fn hex_to_color(hex: &str) -> Color32 {
    let hex_clean = hex.trim_start_matches('#');

    if hex_clean.len() != 6 {
        log::warn!("Invalid hex color length '{}', falling back to black", hex);
        return Color32::BLACK;
    }

    let parse_channel = |start: usize, end: usize| -> Result<u8, std::num::ParseIntError> {
        u8::from_str_radix(&hex_clean[start..end], 16)
    };

    match (
        parse_channel(0, 2),
        parse_channel(2, 4),
        parse_channel(4, 6),
    ) {
        (Ok(r), Ok(g), Ok(b)) => Color32::from_rgb(r, g, b),
        _ => {
            log::warn!("Failed to parse hex color '{}', falling back to black", hex);
            Color32::BLACK
        }
    }
}

impl ThemeFile {
    pub fn to_palette(&self) -> ThemePalette {
        ThemePalette {
            name: self.name.clone(),
            is_dark: self.is_dark,
            bg_base: hex_to_color(&self.bg_base),
            bg_surface: hex_to_color(&self.bg_surface),
            bg_element: hex_to_color(&self.bg_element),
            bg_element_alt: hex_to_color(&self.bg_element_alt),
            border: hex_to_color(&self.border),
            accent_primary: hex_to_color(&self.accent_primary),
            accent_primary_fg: hex_to_color(&self.accent_primary_fg),
            accent_danger: hex_to_color(&self.accent_danger),
            accent_danger_fg: hex_to_color(&self.accent_danger_fg),
            accent_success: hex_to_color(&self.accent_success),
            accent_success_fg: hex_to_color(&self.accent_success_fg),
            text_primary: hex_to_color(&self.text_primary),
            text_muted: hex_to_color(&self.text_muted),
            col_delay: hex_to_color(&self.col_delay),
            col_move: hex_to_color(&self.col_move),
            col_click: hex_to_color(&self.col_click),
            col_keyboard: hex_to_color(&self.col_keyboard),
            col_if: hex_to_color(&self.col_if),
            col_else: hex_to_color(&self.col_else),
            col_end_if: hex_to_color(&self.col_end_if),
            col_loop: hex_to_color(&self.col_loop),
            col_end_loop: hex_to_color(&self.col_end_loop),
            col_label: hex_to_color(&self.col_label),
            col_goto: hex_to_color(&self.col_goto),
            col_type_text: hex_to_color(&self.col_type_text),
            col_import_saved_macro: hex_to_color(&self.col_import_saved_macro),
            col_var: hex_to_color(&self.col_var),
            col_calc: hex_to_color(&self.col_calc),
            col_clipboard: hex_to_color(&self.col_clipboard),
        }
    }
}

impl ThemePalette {
    pub fn to_egui_visuals(&self) -> egui::Visuals {
        let mut visuals = if self.is_dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        visuals.panel_fill = self.bg_surface;
        visuals.window_fill = self.bg_surface;
        visuals.window_stroke = egui::Stroke::new(1.0_f32, self.border);
        visuals.faint_bg_color = self.bg_element;
        visuals.extreme_bg_color = self.bg_base;

        visuals.widgets.noninteractive.bg_fill = self.bg_element;
        visuals.widgets.inactive.bg_fill = self.bg_element_alt;
        visuals.widgets.hovered.bg_fill = self.bg_element;
        visuals.widgets.active.bg_fill = self.accent_primary;

        visuals.widgets.noninteractive.weak_bg_fill = self.bg_element;
        visuals.widgets.inactive.weak_bg_fill = self.bg_element_alt;
        visuals.widgets.hovered.weak_bg_fill = self.bg_element;
        visuals.widgets.active.weak_bg_fill = self.accent_primary;
        visuals.widgets.open.weak_bg_fill = self.bg_element;

        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, self.border);
        visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.active.bg_stroke = egui::Stroke::NONE;

        visuals.widgets.noninteractive.fg_stroke.color = self.text_muted;
        visuals.widgets.inactive.fg_stroke.color = self.text_primary;
        visuals.widgets.hovered.fg_stroke.color = self.text_primary;
        visuals.widgets.active.fg_stroke.color = self.accent_primary_fg;

        visuals.selection.bg_fill = self.accent_primary;
        visuals.selection.stroke.color = self.accent_primary_fg;

        visuals
    }
}

#[derive(Debug)]
pub struct ThemeManager {
    pub available_themes: Vec<ThemePalette>,
}

impl ThemeManager {
    pub fn new() -> Self {
        let mut manager = Self {
            available_themes: Vec::new(),
        };

        manager.load_built_in_themes();
        manager.load_user_themes_from_disk();

        manager
    }

    pub fn default_theme() -> ThemePalette {
        static DEFAULT_THEME: OnceLock<ThemePalette> = OnceLock::new();
        DEFAULT_THEME
            .get_or_init(|| {
                let json_str = include_str!("../../themes/gruvbox_dark.json");
                serde_json::from_str::<ThemeFile>(json_str)
                    .expect("error: gruvbox_dark.json is missing or invalid")
                    .to_palette()
            })
            .clone()
    }

    pub fn get_theme(&self, name: &str) -> ThemePalette {
        self.available_themes
            .iter()
            .find(|t| t.name == name)
            .cloned()
            .unwrap_or_else(|| {
                log::warn!("Theme '{}' not found, falling back to default theme", name);
                Self::default_theme()
            })
    }

    fn insert_theme(&mut self, palette: ThemePalette) {
        // a user theme with the same name silently replaces the built-in, which is how custom palettes can take over default names.
        if let Some(existing) = self
            .available_themes
            .iter_mut()
            .find(|t| t.name == palette.name)
        {
            *existing = palette;
        } else {
            self.available_themes.push(palette);
        }
    }

    fn load_built_in_themes(&mut self) {
        // embedded JSON keeps the binary self-contained; a broken built-in is a packaging bug, hence the loud error level.
        let built_ins = [
            include_str!("../../themes/ayu_dark.json"),
            include_str!("../../themes/catppuccin_mocha.json"),
            include_str!("../../themes/catppuccin_latte.json"),
            include_str!("../../themes/dracula.json"),
            include_str!("../../themes/github_dark.json"),
            include_str!("../../themes/github_light.json"),
            include_str!("../../themes/gruvbox_dark.json"),
            include_str!("../../themes/gruvbox_light.json"),
            include_str!("../../themes/monokai_pro.json"),
            include_str!("../../themes/night_owl.json"),
            include_str!("../../themes/nord.json"),
            include_str!("../../themes/one_dark.json"),
            include_str!("../../themes/rose_pine.json"),
        ];

        for json_str in built_ins {
            match serde_json::from_str::<ThemeFile>(json_str) {
                Ok(theme_file) => self.insert_theme(theme_file.to_palette()),
                Err(e) => log::error!("Critical error: failed to parse built-in theme: {}", e),
            }
        }
    }

    fn load_user_themes_from_disk(&mut self) {
        // TODO: watch the themes directory for changes so new themes show up without an app restart.
        let Some(proj_dirs) = directories::ProjectDirs::from("", "", "wmacro") else {
            log::warn!("Could not determine project directories for user themes");
            return;
        };

        let themes_dir = proj_dirs.config_dir().join("themes");
        if let Err(e) = fs::create_dir_all(&themes_dir) {
            log::error!("Failed to create user themes directory: {}", e);
            return;
        }

        let entries = match fs::read_dir(&themes_dir) {
            Ok(entries) => entries,
            Err(e) => {
                log::error!("Failed to read themes directory: {}", e);
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            match fs::read_to_string(&path) {
                Ok(json_str) => match serde_json::from_str::<ThemeFile>(&json_str) {
                    Ok(theme_file) => self.insert_theme(theme_file.to_palette()),
                    Err(e) => log::warn!("Failed to parse user theme at {}: {}", path.display(), e),
                },
                Err(e) => log::warn!("Failed to read theme file at {}: {}", path.display(), e),
            }
        }
    }
}

pub fn topbar_frame(p: &ThemePalette) -> egui::Frame {
    egui::Frame::NONE
        .fill(p.bg_surface)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .stroke(egui::Stroke::new(1.0_f32, p.border))
}

pub fn sidebar_frame(p: &ThemePalette) -> egui::Frame {
    egui::Frame::NONE
        .fill(p.bg_surface)
        .inner_margin(egui::Margin::symmetric(10, 10))
}

pub fn editor_bg_frame(p: &ThemePalette) -> egui::Frame {
    egui::Frame::NONE
        .fill(p.bg_base)
        .inner_margin(egui::Margin::same(16))
}

pub fn modal_frame(p: &ThemePalette) -> egui::Frame {
    egui::Frame::NONE
        .fill(p.bg_surface)
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(20))
        .stroke(egui::Stroke::new(1.0_f32, p.border))
}
