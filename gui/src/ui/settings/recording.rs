use super::save_settings;
use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;

pub fn render(ui: &mut egui::Ui, state: &SharedState, palette: &ThemePalette) {
    // TODO: add an option to record the focused window title for context logging.
    ui.label(
        egui::RichText::new("Choose which input types are captured during recording.")
            .color(palette.text_muted)
            .size(11.0),
    );
    ui.add_space(10.0);

    let (mut record_mouse, mut record_movements, mut record_keyboard) = {
        let Ok(s) = state.lock() else { return };
        (
            s.macro_state.record_mouse,
            s.macro_state.record_movements,
            s.macro_state.record_keyboard,
        )
    };

    let mut changed = false;

    egui::Grid::new("recording_grid")
        .num_columns(2)
        .spacing([16.0, 12.0])
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Record mouse")
                    .color(palette.text_primary)
                    .size(13.0),
            );
            if ui
                .add(egui::Checkbox::without_text(&mut record_mouse))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Record mouse clicks and scroll events")
                .changed()
            {
                changed = true;
                if !record_mouse {
                    record_movements = false;
                }
            }
            ui.end_row();

            ui.add_enabled_ui(record_mouse, |ui| {
                let color = if record_mouse {
                    palette.text_primary
                } else {
                    palette.text_muted
                };
                ui.label(
                    egui::RichText::new("Record movements also")
                        .color(color)
                        .size(13.0),
                );
            });

            ui.add_enabled_ui(record_mouse, |ui| {
                if ui
                    .add(egui::Checkbox::without_text(&mut record_movements))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Record mouse cursor movements between clicks")
                    .changed()
                {
                    changed = true;
                }
            });
            ui.end_row();

            ui.label(
                egui::RichText::new("Record keyboard")
                    .color(palette.text_primary)
                    .size(13.0),
            );
            if ui
                .add(egui::Checkbox::without_text(&mut record_keyboard))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Record keyboard key presses and releases")
                .changed()
            {
                changed = true;
            }
            ui.end_row();
        });

    if changed {
        if let Ok(mut s) = state.lock() {
            s.macro_state.record_mouse = record_mouse;
            s.macro_state.record_movements = record_movements;
            s.macro_state.record_keyboard = record_keyboard;
        }
        save_settings(state);
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(12.0);

    ui.label(
        egui::RichText::new("Recording Behavior")
            .color(palette.text_primary)
            .size(13.0),
    );
    ui.label(
        egui::RichText::new("When starting a recording with an existing macro:")
            .color(palette.text_muted)
            .size(11.0),
    );

    ui.add_space(6.0);

    let mut behavior = {
        let Ok(s) = state.lock() else { return };
        s.macro_state.record_hotkey_behavior.clone()
    };
    let mut behavior_changed = false;

    ui.horizontal(|ui| {
        behavior_changed |= ui
            .radio_value(
                &mut behavior,
                crate::state::RecordHotkeyBehavior::Append,
                "Append (Safe)",
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked();
        ui.add_space(12.0);
        behavior_changed |= ui
            .radio_value(
                &mut behavior,
                crate::state::RecordHotkeyBehavior::Overwrite,
                "Overwrite",
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked();
    });

    if behavior_changed {
        if let Ok(mut s) = state.lock() {
            s.macro_state.record_hotkey_behavior = behavior;
        }
        save_settings(state);
    }

    ui.add_space(16.0);

    egui::Frame::NONE.fill(palette.bg_element).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::same(10)).stroke(egui::Stroke::new(1.0_f32, palette.border))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            ui.label(egui::RichText::new("Info").color(palette.accent_primary)
                .size(11.0)
                .strong()
            );

            ui.add_space(4.0);

            ui.label(egui::RichText::new("These settings control which hardware events are captured\nduring recording. Disabling mouse will also disable movements.\nAlready recorded events are not affected.")
                .color(palette.text_muted)
                .size(10.0)
            );
    });
}
