//! the Type Text modal: lets the user type text with `$var` interpolation to insert or edit.

use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::MacroCommand;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::{auto_focus, available_variable_names, unknown_vars_in_formula};

pub struct TypeTextModal {
    pub text: String,
    pub edit_idx: Option<usize>,
}

// TODO: add an inline variable picker (caret dropdown) for `$name` insertions.

impl ModalWidget for TypeTextModal {
    fn title(&self) -> String {
        format!("{} Type Text", egui_phosphor::regular::TEXT_T)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["type_text_content"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        let known = available_variable_names(state);

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Text")
                    .color(palette.text_muted)
                    .size(11.0),
            );

            let resp = ui.add(
                egui::TextEdit::multiline(&mut self.text)
                    .id(egui::Id::new("type_text_content"))
                    .hint_text("Hello $name, your score is $score")
                    .desired_width(260.0)
                    .desired_rows(4),
            );

            auto_focus(ui, "type_text_content", &resp);

            // reserve one warning line of height unconditionally (stable layout).
            let unknown = unknown_vars_in_formula(&self.text, &known);
            let (warn_text, warn_color) = if unknown.is_empty() {
                (String::new(), egui::Color32::TRANSPARENT)
            } else {
                let names = unknown
                    .iter()
                    .map(|n| format!("${}", n))
                    .collect::<Vec<_>>()
                    .join(", ");
                (
                    format!("\u{26a0} {}  not found (defaults to 0)", names),
                    egui::Color32::from_rgb(220, 170, 50),
                )
            };
            ui.add_space(4.0);
            ui.label(egui::RichText::new(warn_text).color(warn_color).size(10.0));
        });

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Use $varname to insert a variable value at runtime")
                .color(palette.text_muted)
                .size(10.0),
        );
        ui.add_space(12.0);

        let label = if self.edit_idx.is_some() {
            "Save"
        } else {
            "Add"
        };
        let mut outcome = ModalOutcome::Open;

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.text.is_empty(),
                    egui::Button::new(egui::RichText::new(label).strong())
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Commit(MacroCommand::TypeText(self.text.clone()));
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
