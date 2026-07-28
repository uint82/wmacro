use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;

pub fn render(ui: &mut egui::Ui, state: &SharedState, palette: &ThemePalette, close: &mut bool) {
    ui.label(
        egui::RichText::new(
            "This will overwrite your current macro.\nAll unsaved events will be lost!",
        )
        .color(palette.text_primary)
        .size(13.0),
    );
    ui.add_space(16.0);

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("Yes, Overwrite")
                        .color(palette.accent_danger_fg)
                        .strong(),
                )
                .fill(palette.accent_danger)
                .min_size(egui::vec2(100.0, 28.0)),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            let name = {
                let s = state.lock().unwrap_or_else(|e| {
                    log::error!("State mutex poisoned: {e}");
                    e.into_inner()
                });
                s.macro_state.macro_name.clone()
            };
            
            crate::macro_engine::recorder::start_recording(state, name, false);
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
