//! the Loop Sequence modal: lets the user pick how many times the selected commands repeat.

use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::MacroCommand;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::{auto_focus, parse_var_or_num, var_or_num_field};

pub struct LoopModal {
    pub count_text: String,
    pub edit_idx: Option<usize>,
}

// TODO: show the resolved iteration count live when the count is a variable.

impl ModalWidget for LoopModal {
    fn title(&self) -> String {
        format!("{} Loop Sequence", egui_phosphor::regular::REPEAT)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["loop_count"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        let mut field_resp = None;

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Iterations")
                    .color(palette.text_muted)
                    .size(11.0),
            );
            let (submitted, r) =
                var_or_num_field(ui, state, "loop_count", &mut self.count_text, 160.0);
            field_resp = Some((submitted, r));
        });

        let (submitted, resp) = field_resp.unzip();
        let submitted = submitted.unwrap_or(false);

        if let Some(resp) = resp {
            auto_focus(ui, "loop_count", &resp);
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Enter a number or $varname for a runtime count")
                .color(palette.text_muted)
                .size(10.0),
        );
        ui.add_space(12.0);

        let count = parse_var_or_num(&self.count_text);
        let is_valid = !self.count_text.trim().is_empty();
        let btn_label = if self.edit_idx.is_some() {
            "Save"
        } else {
            "Add"
        };

        if submitted && is_valid {
            return ModalOutcome::Commit(MacroCommand::Loop { count });
        }

        let mut outcome = ModalOutcome::Open;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    is_valid,
                    egui::Button::new(egui::RichText::new(btn_label).strong())
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Commit(MacroCommand::Loop {
                    count: parse_var_or_num(&self.count_text),
                });
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
