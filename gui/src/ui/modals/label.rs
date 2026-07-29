use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use wmacro_core_types::{MacroCommand, MacroCommand::Label};
use eframe::egui;

pub fn render(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    name: &mut String,
    edit_idx: &Option<usize>,
) {
    ui.label(egui::RichText::new("Label name:").color(palette.text_muted));
    ui.text_edit_singleline(name).request_focus();

    let error_msg = validate_label_name(name, state, edit_idx);

    if let Some(err) = &error_msg {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(*err)
                .color(palette.accent_danger)
                .size(11.0),
        );
    } else {
        ui.add_space(15.0);
    }

    ui.add_space(12.0);
    render_buttons(ui, close, commit, edit_idx, name, error_msg.is_none());
}

fn validate_label_name(name: &str, state: &SharedState, edit_idx: &Option<usize>) -> Option<&'static str> {
    let name_trimmed = name.trim();

    if name_trimmed.is_empty() {
        return Some("Label name cannot be empty.");
    }

    if name != name_trimmed {
        return Some("Label cannot have leading/trailing whitespace.");
    }

    let s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });

    if let Some(m) = s.macro_state.current_macro.as_ref() {
        for (idx, cmd) in m.commands.iter().enumerate() {
            if let Label(existing_name) = cmd {
                if Some(idx) != *edit_idx && existing_name == name_trimmed {
                    return Some("Label name must be unique within this macro.");
                }
            }
        }
    }

    None
}

fn render_buttons(
    ui: &mut egui::Ui,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    edit_idx: &Option<usize>,
    name: &str,
    is_valid: bool,
) {
    ui.horizontal(|ui| {
        let btn_label = if edit_idx.is_some() { "Save" } else { "Add" };

        ui.add_enabled_ui(is_valid, |ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(btn_label).strong())
                        .min_size(egui::vec2(80.0, 28.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *commit = Some(MacroCommand::Label(name.to_string()));
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
