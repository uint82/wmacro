use super::KeyActionType;
use crate::state::SharedState;
use crate::ui::key_names::COMMON_KEYS;
use crate::ui::theme::ThemePalette;
use core_types::{MacroCommand, MacroEvent};
use eframe::egui;

pub fn render(
    ui: &mut egui::Ui,
    _state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    key: &mut String,
    code: &mut u16,
    action: &mut KeyActionType,
    hold_time_ms: &mut u32,
    edit_idx: &Option<usize>,
    search: &mut String,
) {
    render_grid(ui, palette, key, code, action, hold_time_ms, search);

    ui.add_space(16.0);
    render_buttons(ui, close, commit, edit_idx, key, *code, action, *hold_time_ms);
}

fn render_grid(
    ui: &mut egui::Ui,
    palette: &ThemePalette,
    key: &mut String,
    code: &mut u16,
    action: &mut KeyActionType,
    hold_time_ms: &mut u32,
    search: &mut String,
) {
    egui::Grid::new("key_modal_grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Key").color(palette.text_muted));
            render_key_selector(ui, key, code, search);
            ui.end_row();

            ui.label(egui::RichText::new("Event Type").color(palette.text_muted));
            render_action_selector(ui, action);
            ui.end_row();

            ui.label(egui::RichText::new("Scancode").color(palette.text_muted));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(code).speed(1).range(0..=767_u16));
                ui.label(
                    egui::RichText::new("(auto-filled)")
                        .color(palette.text_muted)
                        .size(10.0),
                );
            });
            ui.end_row();

            if *action == KeyActionType::KeyPress {
                ui.label(egui::RichText::new("Hold Time").color(palette.text_muted));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(hold_time_ms)
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
}

fn render_key_selector(ui: &mut egui::Ui, key: &mut String, code: &mut u16, search: &mut String) {
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
            ui.selectable_value(
                action,
                KeyActionType::KeyPress,
                KeyActionType::KeyPress.label(),
            );
            ui.selectable_value(
                action,
                KeyActionType::KeyDown,
                KeyActionType::KeyDown.label(),
            );
            ui.selectable_value(action, KeyActionType::KeyUp, KeyActionType::KeyUp.label());
        });
}

fn render_buttons(
    ui: &mut egui::Ui,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    edit_idx: &Option<usize>,
    key: &str,
    code: u16,
    action: &KeyActionType,
    hold_time_ms: u32,
) {
    ui.horizontal(|ui| {
        let btn_label = if edit_idx.is_some() { "Save" } else { "Add" };
        if ui
            .add(
                egui::Button::new(egui::RichText::new(btn_label).strong())
                    .min_size(egui::vec2(80.0, 28.0)),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            let ev = match action {
                KeyActionType::KeyPress => MacroEvent::KeyPress {
                    key: key.to_string(),
                    code,
                    hold_time_ms,
                },
                KeyActionType::KeyDown => MacroEvent::KeyDown {
                    key: key.to_string(),
                    code,
                },
                KeyActionType::KeyUp => MacroEvent::KeyUp {
                    key: key.to_string(),
                    code,
                },
            };
            
            *commit = Some(MacroCommand::Action(ev));
            *close = true;
        }
        
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
