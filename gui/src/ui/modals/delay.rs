use crate::state::{DelayUnit, SharedState};
use crate::ui::theme::ThemePalette;
use core_types::{MacroCommand, MacroEvent};
use eframe::egui;

pub fn render(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    value: &mut u64,
    unit: &mut DelayUnit,
    target_indices: &[usize],
) {
    let is_bulk = target_indices.len() > 1;

    let valid_delay_count = count_valid_delays(state, target_indices);

    if is_bulk {
        render_bulk_header(ui, palette, valid_delay_count);
    } else {
        ui.label(
            egui::RichText::new("Duration")
                .color(palette.text_muted)
                .size(11.0),
        );
    }

    render_duration_inputs(ui, value, unit);

    let ms_equivalent = unit.to_ms(*value as f64);
    ui.label(
        egui::RichText::new(format!("= {} ms", ms_equivalent))
            .color(palette.text_muted)
            .size(11.0),
    );
    ui.add_space(16.0);

    render_buttons(
        ui, state, close, commit, target_indices, ms_equivalent, is_bulk,
    );
}

fn count_valid_delays(state: &SharedState, target_indices: &[usize]) -> usize {
    let s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });

    if let Some(m) = &s.macro_state.current_macro {
        target_indices
            .iter()
            .filter(|&&idx| {
                matches!(
                    m.commands.get(idx),
                    Some(MacroCommand::Action(MacroEvent::Delay(_)))
                )
            })
            .count()
    } else {
        0
    }
}

fn render_bulk_header(ui: &mut egui::Ui, palette: &ThemePalette, valid_delay_count: usize) {
    ui.vertical_centered(|ui| {
        ui.add_space(10.0);

        let grammar = if valid_delay_count == 1 {
            "Item"
        } else {
            "Items"
        };
        ui.label(
            egui::RichText::new(format!(
                "Change Delay For {} Selected {}",
                valid_delay_count, grammar
            ))
            .color(palette.text_primary)
            .size(14.0),
        );

        ui.add_space(10.0);
    });
}

fn render_duration_inputs(ui: &mut egui::Ui, value: &mut u64, unit: &mut DelayUnit) {
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(value).speed(1).range(1..=9999999));
        egui::ComboBox::from_id_salt("delay_unit_combo")
            .selected_text(unit.label())
            .width(60.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(unit, DelayUnit::Milliseconds, "ms");
                ui.selectable_value(unit, DelayUnit::Seconds, "s");
                ui.selectable_value(unit, DelayUnit::Minutes, "m");
                ui.selectable_value(unit, DelayUnit::Hours, "h");
            });
    });
}

fn render_buttons(
    ui: &mut egui::Ui,
    state: &SharedState,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    target_indices: &[usize],
    ms_equivalent: u64,
    is_bulk: bool,
) {
    ui.horizontal(|ui| {
        let btn_label = if target_indices.is_empty() {
            "Add"
        } else {
            "Save"
        };

        if ui
            .add(
                egui::Button::new(egui::RichText::new(btn_label).strong())
                    .min_size(egui::vec2(80.0, 28.0)),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            let us_final = ms_equivalent * 1000;
            let cmd = MacroCommand::Action(MacroEvent::Delay(us_final));

            if target_indices.is_empty() {
                *commit = Some(cmd);
            } else {
                let mut s = state.lock().unwrap_or_else(|e| {
                    log::error!("State mutex poisoned: {e}");
                    e.into_inner()
                });

                if let Some(m) = s.macro_state.current_macro.as_mut() {
                    for &idx in target_indices {
                        if let Some(MacroCommand::Action(MacroEvent::Delay(delay_us))) =
                            m.commands.get_mut(idx)
                        {
                            *delay_us = us_final;
                        } else if !is_bulk {
                            if idx < m.commands.len() {
                                m.commands[idx] = cmd.clone();
                            }
                        }
                    }
                }
            }
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
