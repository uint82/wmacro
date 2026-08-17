use crate::state::SharedState;
use crate::ui::key_names::COMMON_KEYS;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::{MacroCommand, MacroEvent};

use super::KeyActionType;
use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;

pub struct KeyModal {
    pub key: String,
    pub code: u16,
    pub action: KeyActionType,
    pub hold_time_ms: u32,
    pub edit_idx: Option<usize>,
    pub search: String,
}

impl ModalWidget for KeyModal {
    fn title(&self) -> String {
        format!("{} Keyboard Event", egui_phosphor::regular::KEYBOARD)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        _state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        egui::Grid::new("key_modal_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Key").color(palette.text_muted));
                render_key_selector(ui, &mut self.key, &mut self.code, &mut self.search);
                ui.end_row();

                ui.label(egui::RichText::new("Event Type").color(palette.text_muted));
                render_action_selector(ui, &mut self.action);
                ui.end_row();

                ui.label(egui::RichText::new("Scancode").color(palette.text_muted));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.code)
                            .speed(1)
                            .range(0..=767_u16),
                    );
                    ui.label(
                        egui::RichText::new("(auto-filled)")
                            .color(palette.text_muted)
                            .size(10.0),
                    );
                });
                ui.end_row();

                if self.action == KeyActionType::Press {
                    ui.label(egui::RichText::new("Hold Time").color(palette.text_muted));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.hold_time_ms)
                                .speed(1)
                                .range(1..=10000)
                                .suffix(" ms"),
                        );
                        ui.label(
                            egui::RichText::new("press + release duration")
                                .color(palette.text_muted)
                                .size(10.0),
                        );
                    });
                    ui.end_row();
                }
            });

        ui.add_space(16.0);

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
            {
                outcome = ModalOutcome::Commit(self.make_cmd());
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

impl KeyModal {
    fn make_cmd(&self) -> MacroCommand {
        let ev = match self.action {
            KeyActionType::Press => MacroEvent::KeyPress {
                key: self.key.clone(),
                code: self.code,
                hold_time_ms: self.hold_time_ms,
            },
            KeyActionType::Down => MacroEvent::KeyDown {
                key: self.key.clone(),
                code: self.code,
            },
            KeyActionType::Up => MacroEvent::KeyUp {
                key: self.key.clone(),
                code: self.code,
            },
        };
        MacroCommand::Action(ev)
    }
}

fn render_key_selector(ui: &mut egui::Ui, key: &mut String, code: &mut u16, search: &mut String) {
    // TODO: show the evdev key name for the selected scancode to help debugging.
    egui::ComboBox::from_id_salt("key_select_combo")
        .selected_text(if key.is_empty() {
            "Select a key..."
        } else {
            key.as_str()
        })
        .width(160.0)
        .show_ui(ui, |ui| {
            ui.text_edit_singleline(search).request_focus();
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(250.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for &(k_code, k_name) in COMMON_KEYS {
                        if !search.is_empty()
                            && !k_name.to_lowercase().starts_with(&search.to_lowercase())
                        {
                            continue;
                        }
                        if ui
                            .selectable_label(*code == k_code, k_name)
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            *key = k_name.to_string();
                            *code = k_code;
                            ui.close();
                        }
                    }
                });
        });
}

fn render_action_selector(ui: &mut egui::Ui, action: &mut KeyActionType) {
    egui::ComboBox::from_id_salt("key_action_combo")
        .selected_text(action.label())
        .width(160.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(action, KeyActionType::Press, KeyActionType::Press.label());
            ui.selectable_value(action, KeyActionType::Down, KeyActionType::Down.label());
            ui.selectable_value(action, KeyActionType::Up, KeyActionType::Up.label());
        });
}
