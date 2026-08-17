//! the Goto modal: lets the user pick the label a `Goto` command jumps to.

use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::{MacroCommand, MacroCommand::Label};

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;

pub struct GotoModal {
    pub target: String,
    pub edit_idx: Option<usize>,
}

// TODO: let users type a label name, since goto targets may refer to labels added later in the macro.

impl ModalWidget for GotoModal {
    fn title(&self) -> String {
        format!("{} Goto", egui_phosphor::regular::LINK)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        ui.label(egui::RichText::new("Where to jump (label name):").color(palette.text_muted));

        let available_labels = get_available_labels(state);

        if available_labels.is_empty() {
            ui.label(
                egui::RichText::new("No labels found in this macro.").color(palette.accent_danger),
            );
        } else {
            if self.target.is_empty() {
                self.target = available_labels[0].clone();
            }

            egui::ComboBox::from_id_salt("goto_label_combo")
                .selected_text(if self.target.is_empty() {
                    "Select a label..."
                } else {
                    self.target.as_str()
                })
                .width(200.0)
                .show_ui(ui, |ui| {
                    for label in &available_labels {
                        ui.selectable_value(&mut self.target, label.clone(), label);
                    }
                });
        }

        ui.add_space(16.0);

        let is_valid = !self.target.is_empty() && available_labels.contains(&self.target);
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
                    outcome = ModalOutcome::Commit(MacroCommand::Goto(self.target.clone()));
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

fn get_available_labels(state: &SharedState) -> Vec<String> {
    let s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    let Some(m) = s.macro_state.current_macro.as_ref() else {
        return Vec::new();
    };
    m.commands
        .iter()
        .filter_map(|cmd| {
            if let Label(name) = cmd {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect()
}
