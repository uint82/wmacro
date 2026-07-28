use super::theme::ThemePalette;
use core_types::MousePosition;
use core_types::{MacroButton, MacroEvent};
use eframe::egui;

pub fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32, fixed_width: Option<f32>) {
    let fill_color = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 40);

    if let Some(w) = fixed_width {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 20.0), egui::Sense::hover());

        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(4), fill_color);

        let mut badge_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        ));
        badge_ui.label(egui::RichText::new(text).color(color).size(10.0).strong());
    } else {
        egui::Frame::NONE
            .fill(fill_color)
            .corner_radius(egui::CornerRadius::same(4))
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).color(color).size(10.0).strong());
            });
    }
}

pub fn tool_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    color: egui::Color32,
    palette: &ThemePalette,
) -> bool {
    let resp = egui::Frame::NONE
        .fill(palette.bg_element)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon).color(color).size(15.0));
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(label)
                        .color(palette.text_primary)
                        .size(13.0),
                );
            });
        });
    let r = resp.response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    if r.hovered() {
        ui.painter().rect_filled(
            resp.response.rect,
            egui::CornerRadius::same(6),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        );
    }
    r.clicked()
}

pub fn status_chip(
    ui: &mut egui::Ui,
    recording: bool,
    playing: bool,
    record_paused: bool,
    play_paused: bool,
    palette: &ThemePalette,
) {
    let (dot, label, color) = if recording {
        if record_paused {
            (
                egui_phosphor::regular::PAUSE,
                "REC PAUSED",
                palette.accent_primary,
            )
        } else {
            (
                egui_phosphor::regular::RECORD,
                "RECORDING",
                palette.accent_danger,
            )
        }
    } else if playing {
        if play_paused {
            (
                egui_phosphor::regular::PAUSE,
                "PLAY PAUSED",
                palette.accent_primary,
            )
        } else {
            (
                egui_phosphor::regular::PLAY,
                "PLAYING",
                palette.accent_success,
            )
        }
    } else {
        (egui_phosphor::regular::CIRCLE, "IDLE", palette.text_muted)
    };

    egui::Frame::NONE
        .fill(egui::Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            40,
        ))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(8, 3))
        .stroke(egui::Stroke::new(1.0_f32, color))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(dot).color(color).size(11.0));
                ui.label(egui::RichText::new(label).color(color).size(10.0).strong());
            });
        });
}

pub fn event_display_info(
    ev: &MacroEvent,
    palette: &ThemePalette,
) -> (&'static str, &'static str, egui::Color32, String) {
    match ev {
        MacroEvent::Delay(us) => (
            egui_phosphor::regular::TIMER,
            "DELAY",
            palette.col_delay,
            format!("{} ms", us / 1000),
        ),
        MacroEvent::MouseMove { x, y } => (
            egui_phosphor::regular::CURSOR,
            "MOVE",
            palette.col_move,
            format!("x = {}   y = {}", x, y),
        ),
        MacroEvent::Click {
            position,
            button,
            jitter: _jitter,
            hold_time_ms,
        } => {
            let btn = match button {
                MacroButton::Left => "Left",
                MacroButton::Right => "Right",
                MacroButton::Middle => "Middle",
            };
            let pos_str = match position {
                MousePosition::Absolute { x, y } => format!("x = {}    y = {}", x, y),
                MousePosition::Current => "Current Position".to_string(),
            };
            (
                egui_phosphor::regular::MOUSE,
                "CLICK",
                palette.col_click,
                format!(
                    "{}    {}   ({}ms hold)",
                    btn, pos_str, hold_time_ms
                ),
            )
        }
        MacroEvent::MouseDown {
            position,
            button,
            jitter: _jitter,
        } => {
            let btn = match button {
                MacroButton::Left => "Left",
                MacroButton::Right => "Right",
                MacroButton::Middle => "Middle",
            };
            let pos_str = match position {
                MousePosition::Absolute { x, y } => format!("x = {}    y = {}", x, y),
                MousePosition::Current => "Current Position".to_string(),
            };
            (
                egui_phosphor::regular::MOUSE,
                "MOUSE DOWN",
                palette.col_click,
                format!("{}    {}", btn, pos_str),
            )
        }
        MacroEvent::MouseUp {
            position,
            button,
            jitter: _jitter,
        } => {
            let btn = match button {
                MacroButton::Left => "Left",
                MacroButton::Right => "Right",
                MacroButton::Middle => "Middle",
            };
            let pos_str = match position {
                MousePosition::Absolute { x, y } => format!("x = {}    y = {}", x, y),
                MousePosition::Current => "Current Position".to_string(),
            };
            (
                egui_phosphor::regular::MOUSE,
                "MOUSE UP",
                palette.col_click,
                format!("{}    {}", btn, pos_str),
            )
        }
        MacroEvent::Scroll { dx, dy } => (
            egui_phosphor::regular::ARROWS_VERTICAL,
            "SCROLL",
            palette.col_move,
            format!("dx = {}   dy = {}", dx, dy),
        ),
        MacroEvent::KeyDown { key, .. } => (
            egui_phosphor::regular::KEYBOARD,
            "KEY DOWN",
            palette.col_keyboard,
            format!("{}", key),
        ),
        MacroEvent::KeyUp { key, .. } => (
            egui_phosphor::regular::KEYBOARD,
            "KEY UP",
            palette.col_keyboard,
            format!("{}", key),
        ),
        MacroEvent::KeyPress {
            key, hold_time_ms, ..
        } => (
            egui_phosphor::regular::KEYBOARD,
            "KEY PRESS",
            palette.col_keyboard,
            format!("{}   ({}ms hold)", key, hold_time_ms),
        ),
    }
}
