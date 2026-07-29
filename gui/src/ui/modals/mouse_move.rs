use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use wmacro_core_types::{MacroCommand, MacroEvent};
use eframe::egui;

pub fn render(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    x: &mut i32,
    y: &mut i32,
    edit_idx: &Option<usize>,
) {
    render_grid(ui, state, palette, x, y);

    ui.add_space(16.0);
    render_buttons(ui, close, commit, edit_idx, *x, *y);
}

fn render_grid(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    x: &mut i32,
    y: &mut i32,
) {
    egui::Grid::new("mouse_move_grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("X").color(palette.text_muted));
            ui.add(egui::DragValue::new(x).speed(1));
            ui.end_row();

            ui.label(egui::RichText::new("Y").color(palette.text_muted));
            ui.add(egui::DragValue::new(y).speed(1));
            ui.end_row();

            ui.label(egui::RichText::new("Live Cursor").color(palette.text_muted));

            let (cx, cy, capture_hk) = {
                let s = state.lock().unwrap_or_else(|e| {
                    log::error!("State mutex poisoned: {e}");
                    e.into_inner()
                });
                (s.cursor_x, s.cursor_y, s.macro_state.capture_hotkey)
            };

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("X: {}  Y: {}", cx, cy))
                        .monospace()
                        .color(palette.text_primary)
                        .size(12.0),
                );
                ui.add_space(8.0);
                let hotkey_str = crate::ui::key_names::hotkey_display_name_opt(capture_hk, "Unbound");
                ui.label(
                    egui::RichText::new(format!("({} to capture)", hotkey_str))
                        .color(palette.text_muted)
                        .size(10.0),
                );
            });
            ui.end_row();
        });
}

fn render_buttons(
    ui: &mut egui::Ui,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    edit_idx: &Option<usize>,
    x: i32,
    y: i32,
) {
    ui.horizontal(|ui| {
        let btn_label = if edit_idx.is_some() { "Save" } else { "Add" };

        if ui
            .add(
                egui::Button::new(egui::RichText::new(btn_label).strong())
                    .min_size(egui::vec2(80.0, 28.0)),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            *commit = Some(MacroCommand::Action(MacroEvent::MouseMove { x, y }));
            *close = true;
        }

        ui.add_space(8.0);

        if ui
            .add(egui::Button::new("Cancel").min_size(egui::vec2(80.0, 28.0)))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            *close = true;
        }
    });
}
