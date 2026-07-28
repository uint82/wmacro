use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use core_types::{MacroCommand, MacroCommand::Label};
use eframe::egui;

pub fn render(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    target: &mut String,
    edit_idx: &mut Option<usize>,
) {
    ui.label(egui::RichText::new("Where to jump (label name):").color(palette.text_muted));

    let available_labels = get_available_labels(state);

    if available_labels.is_empty() {
        ui.label(
            egui::RichText::new("No labels found in this macro.").color(palette.accent_danger),
        );
    } else {
        if target.is_empty() {
            *target = available_labels[0].clone();
        }

        egui::ComboBox::from_id_salt("goto_label_combo")
            .selected_text(if target.is_empty() {
                "Select a label..."
            } else {
                target.as_str()
            })
            .width(200.0)
            .show_ui(ui, |ui| {
                for label in &available_labels {
                    ui.selectable_value(target, label.clone(), label);
                }
            });
    }

    ui.add_space(16.0);
    render_buttons(ui, close, commit, target, edit_idx, &available_labels);
}

fn get_available_labels(state: &SharedState) -> Vec<String> {
    let mut labels = Vec::new();
    let s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    
    if let Some(m) = s.macro_state.current_macro.as_ref() {
        for cmd in &m.commands {
            if let Label(name) = cmd {
                labels.push(name.clone());
            }
        }
    }
    labels
}

fn render_buttons(
    ui: &mut egui::Ui,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    target: &String,
    edit_idx: &Option<usize>,
    available_labels: &[String],
) {
    ui.horizontal(|ui| {
        let btn_label = if edit_idx.is_some() { "Save" } else { "Add" };
        let is_valid = !target.is_empty() && available_labels.contains(target);

        ui.add_enabled_ui(is_valid, |ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(btn_label).strong())
                        .min_size(egui::vec2(80.0, 28.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *commit = Some(MacroCommand::Goto(target.clone()));
                *close = true;
            }
        });

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
