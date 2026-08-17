use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::MacroCommand;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::{auto_focus, parse_var_or_num, var_or_num_field};

pub struct SetVariableModal {
    pub target: String,
    pub value_text: String,
    pub edit_idx: Option<usize>,
}

// TODO: warn when the typed name collides with an existing variable in the macro.

impl ModalWidget for SetVariableModal {
    fn title(&self) -> String {
        format!("{} Set Variable", egui_phosphor::regular::FUNCTION)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["set_var_name", "set_var_value"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        let name_focused = ui
            .ctx()
            .memory(|m| m.focused() == Some(egui::Id::new("set_var_name")));
        let value_focused = ui
            .ctx()
            .memory(|m| m.focused() == Some(egui::Id::new("set_var_value")));
        let submitted =
            (name_focused || value_focused) && ui.input(|i| i.key_pressed(egui::Key::Enter));

        let mut name_resp = None;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Name")
                        .color(palette.text_muted)
                        .size(11.0),
                );
                name_resp = Some(
                    ui.add(
                        egui::TextEdit::singleline(&mut self.target)
                            .id(egui::Id::new("set_var_name"))
                            .hint_text("variable_name")
                            .desired_width(120.0),
                    ),
                );
            });

            ui.add_space(8.0);

            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Value")
                        .color(palette.text_muted)
                        .size(11.0),
                );
                let _ = var_or_num_field(ui, state, "set_var_value", &mut self.value_text, 120.0);
            });
        });

        if let Some(resp) = name_resp {
            auto_focus(ui, "set_var_name", &resp);
        }

        ui.add_space(16.0);

        let can_commit = !self.target.trim().is_empty() && !self.value_text.trim().is_empty();

        let make_cmd = || MacroCommand::SetVariable {
            target: self.target.trim().to_string(),
            value: parse_var_or_num(&self.value_text),
        };

        if submitted && can_commit {
            return ModalOutcome::Commit(make_cmd());
        }

        let btn_label = if self.edit_idx.is_some() {
            "Save"
        } else {
            "Add"
        };
        let mut outcome = ModalOutcome::Open;

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    can_commit,
                    egui::Button::new(egui::RichText::new(btn_label).strong())
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Commit(make_cmd());
            }

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
