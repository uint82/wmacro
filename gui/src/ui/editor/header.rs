use crate::state::SharedState;
use crate::ui::components::badge;
use crate::ui::theme::ThemePalette;
use eframe::egui;

pub fn render_editor_header(ui: &mut egui::Ui, state: &SharedState, palette: &ThemePalette) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Macro Editor")
                .strong()
                .size(16.0)
                .color(palette.text_primary),
        );
        ui.add_space(8.0);

        let (name, event_count, total_ms) = {
            let s = state.lock().unwrap_or_else(|e| {
                log::error!("State mutex poisoned: {e}");
                e.into_inner()
            });
            let name = s.macro_state.macro_name.clone();
            let ms = s
                .macro_state
                .current_macro
                .as_ref()
                .map(|m| m.total_duration_ms())
                .unwrap_or(0);
            let count = s.macro_state.events_captured;
            (name, count, ms)
        };

        badge(ui, &name, palette.accent_primary, None);
        ui.add_space(8.0);

        ui.label(
            egui::RichText::new(format!("{} commands", event_count))
                .size(11.0)
                .color(palette.text_muted),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!("{:.2} s total", total_ms as f64 / 1000.0))
                .size(11.0)
                .color(palette.text_muted),
        );
    });

    ui.add_space(4.0);
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.cursor().top(),
        egui::Stroke::new(1.0_f32, palette.border),
    );
    ui.add_space(10.0);
}
