use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use wmacro_core_types::MacroCommand;
use eframe::egui;

// TODO: implement clipboard(copy and paste) instead of simulating typing through evdev.
// it will destroy user clipboard if they had copied.
// user's able to type any emoji and symbol known to world.
pub fn render(
    ui: &mut egui::Ui,
    _state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    text: &mut String,
    edit_idx: &Option<usize>,
) {
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Enter the text to type:")
            .color(palette.text_muted)
            .size(12.0),
    );
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Text:")
                .color(palette.text_primary)
                .size(13.0),
        );
        ui.add(
            egui::TextEdit::multiline(text)
                .desired_width(220.0)
                .desired_rows(3),
        );
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    render_buttons(ui, close, commit, edit_idx, text.as_str());
}

fn render_buttons(
    ui: &mut egui::Ui,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    edit_idx: &Option<usize>,
    text: &str,
) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if edit_idx.is_some() { "Save" } else { "Add" };
            let is_valid = !text.is_empty();

            if ui
                .add_enabled(
                    is_valid,
                    egui::Button::new(egui::RichText::new(label).strong())
                        .min_size(egui::vec2(80.0, 28.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *commit = Some(MacroCommand::TypeText(text.to_string()));
                *close = true;
            }

            if ui
                .add(
                    egui::Button::new(egui::RichText::new("Cancel"))
                        .min_size(egui::vec2(80.0, 28.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *close = true;
            }
        });
    });
}
