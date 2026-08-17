//! the Calculate modal: lets the user pick a target variable and the formula to evaluate.

use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::MacroCommand;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::{auto_focus, available_variable_names, unknown_vars_in_formula};

pub struct CalculateModal {
    pub target: String,
    pub expression: String,
    pub edit_idx: Option<usize>,
}

// TODO: show a live evaluation preview when the formula has no variables.

impl ModalWidget for CalculateModal {
    fn title(&self) -> String {
        format!("{} Calculate", egui_phosphor::regular::FUNCTION)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["calculate_target"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        let mut target_resp = None;

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Save to variable")
                    .color(palette.text_muted)
                    .size(11.0),
            );

            let known = available_variable_names(state);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.target)
                        .id(egui::Id::new("calculate_target"))
                        .hint_text("var_name")
                        .desired_width(140.0),
                );
                target_resp = Some(resp);

                if !known.is_empty() {
                    ui.menu_button(egui_phosphor::regular::CARET_DOWN, |ui| {
                        ui.set_min_width(120.0);
                        for name in &known {
                            if ui.button(name).clicked() {
                                self.target = name.clone();
                                ui.close();
                            }
                        }
                    });
                }
            });

            ui.add_space(12.0);

            ui.label(
                egui::RichText::new("Formula")
                    .color(palette.text_muted)
                    .size(11.0),
            );
            ui.add(
                egui::TextEdit::multiline(&mut self.expression)
                    .hint_text("$health - 10\n($a + 1) * 2\n\"Player \" . $name\n$hp < 50 ? 1 : 2\nrandom(1, 100)")
                    .desired_width(260.0)
                    .desired_rows(5),
            );

            // reserve one warning line of height unconditionally (stable layout).
            let unknown = unknown_vars_in_formula(&self.expression, &known);
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

        let mut submitted = false;
        if let Some(resp) = target_resp {
            auto_focus(ui, "calculate_target", &resp);
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submitted = true;
            }
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Variables start with $, strings in \"quotes\". Operators: + - * / % ( ), . joins text, == != < <= > >= compare, ? : ternary. Functions: abs min max floor ceil round sqrt random",
            )
            .color(palette.text_muted)
            .size(10.0),
        );
        ui.add_space(12.0);

        let can_commit = !self.target.trim().is_empty() && !self.expression.trim().is_empty();

        let make_cmd = || MacroCommand::Calculate {
            target: self.target.trim().to_string(),
            expression: self.expression.trim().to_string(),
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
                .add(
                    egui::Button::new(egui::RichText::new(btn_label).strong())
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
                && can_commit
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
