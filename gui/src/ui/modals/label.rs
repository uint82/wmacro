use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::{MacroCommand, MacroCommand::Label};

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::auto_focus;

pub struct LabelModal {
    pub name: String,
    pub edit_idx: Option<usize>,
}

impl ModalWidget for LabelModal {
    fn title(&self) -> String {
        format!("{} Label", egui_phosphor::regular::TAG)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["label_name"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        ui.label(egui::RichText::new("Label name:").color(palette.text_muted));

        let resp =
            ui.add(egui::TextEdit::singleline(&mut self.name).id(egui::Id::new("label_name")));

        auto_focus(ui, "label_name", &resp);

        let submitted = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        let error_msg = validate_label_name(&self.name, state, &self.edit_idx);

        if error_msg.is_some() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(error_msg.unwrap_or(""))
                    .color(palette.accent_danger)
                    .size(11.0),
            );
        } else {
            ui.add_space(15.0);
        }

        let is_valid = error_msg.is_none();
        ui.add_space(12.0);

        if submitted && is_valid {
            return ModalOutcome::Commit(MacroCommand::Label(self.name.to_string()));
        }

        let btn_label = if self.edit_idx.is_some() {
            "Save"
        } else {
            "Add"
        };
        let mut outcome = ModalOutcome::Open;

        ui.horizontal(|ui| {
            ui.add_enabled_ui(is_valid, |ui| {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(btn_label).strong())
                            .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    outcome = ModalOutcome::Commit(MacroCommand::Label(self.name.to_string()));
                }
            });

            ui.add_space(8.0);
            if ui
                .add(
                    egui::Button::new("Cancel")
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Cancelled;
            }
        });

        outcome
    }
}

fn validate_label_name(
    name: &str,
    state: &SharedState,
    edit_idx: &Option<usize>,
) -> Option<&'static str> {
    // TODO: suggest a default name (e.g. "label_1") when the macro has none yet.
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
            if let Label(existing_name) = cmd
                && Some(idx) != *edit_idx
                && existing_name == name_trimmed
            {
                return Some("Label name must be unique within this macro.");
            }
        }
    }

    None
}
