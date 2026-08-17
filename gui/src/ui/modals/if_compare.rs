use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::{CompareOp, MacroCommand};

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::{auto_focus, parse_var_or_num, var_or_num_field};

pub struct IfCompareModal {
    pub left_text: String,
    pub op: CompareOp,
    pub right_text: String,
    pub edit_idx: Option<usize>,
}

// TODO: show a live evaluation preview when both operands are literals.
// TODO: warn when an operand references a variable that is never defined in the macro.

impl ModalWidget for IfCompareModal {
    fn title(&self) -> String {
        format!("{} If Variable", egui_phosphor::regular::FLOW_ARROW)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["if_compare_left", "if_compare_right"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        let left_focused = ui
            .ctx()
            .memory(|m| m.focused() == Some(egui::Id::new("if_compare_left")));
        let right_focused = ui
            .ctx()
            .memory(|m| m.focused() == Some(egui::Id::new("if_compare_right")));
        let submitted =
            (left_focused || right_focused) && ui.input(|i| i.key_pressed(egui::Key::Enter));

        let mut left_resp = None;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Left Operand")
                        .color(palette.text_muted)
                        .size(11.0),
                );
                let (_, r) =
                    var_or_num_field(ui, state, "if_compare_left", &mut self.left_text, 100.0);
                left_resp = Some(r);
            });

            ui.add_space(8.0);

            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Operator")
                        .color(palette.text_muted)
                        .size(11.0),
                );
                egui::ComboBox::from_id_salt("if_compare_op_combo")
                    .selected_text(self.op.symbol())
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.op, CompareOp::Eq, "==");
                        ui.selectable_value(&mut self.op, CompareOp::Ne, "!=");
                        ui.selectable_value(&mut self.op, CompareOp::Lt, "<");
                        ui.selectable_value(&mut self.op, CompareOp::Le, "<=");
                        ui.selectable_value(&mut self.op, CompareOp::Gt, ">");
                        ui.selectable_value(&mut self.op, CompareOp::Ge, ">=");
                        ui.selectable_value(&mut self.op, CompareOp::Contains, "contains");
                    });
            });

            ui.add_space(8.0);

            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Right Operand")
                        .color(palette.text_muted)
                        .size(11.0),
                );
                let _ =
                    var_or_num_field(ui, state, "if_compare_right", &mut self.right_text, 100.0);
            });
        });

        if let Some(resp) = left_resp {
            auto_focus(ui, "if_compare_left", &resp);
        }

        ui.add_space(16.0);

        let can_commit = !self.left_text.trim().is_empty() && !self.right_text.trim().is_empty();

        let make_cmd = || MacroCommand::IfCompare {
            left: parse_var_or_num(&self.left_text),
            op: self.op,
            right: parse_var_or_num(&self.right_text),
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
