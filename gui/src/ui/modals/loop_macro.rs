use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use wmacro_core_types::MacroCommand;
use eframe::egui;

pub fn render(
    ui: &mut egui::Ui,
    _state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    count: &mut u32,
    edit_idx: &Option<usize>,
) {
    ui.label(
        egui::RichText::new("Number of iterations")
            .color(palette.text_muted)
            .size(11.0),
    );
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(count).speed(1).range(0..=10000));
        ui.label(
            egui::RichText::new("times")
                .color(palette.text_muted)
                .size(11.0),
        );
    });

    ui.add_space(16.0);
    render_buttons(ui, close, commit, edit_idx, *count);
}

fn render_buttons(
    ui: &mut egui::Ui,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    edit_idx: &Option<usize>,
    count: u32,
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
            *commit = Some(MacroCommand::Loop { count });
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
