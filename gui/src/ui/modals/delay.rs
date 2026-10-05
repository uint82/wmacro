//! the Delay modal: lets the user pick a duration (literal or variable) for one or more commands.

use crate::state::{DelayUnit, SharedState};
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::{MacroCommand, Operand, Value};

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::{
    auto_focus, format_duration_string, parse_duration_string, parse_var_or_num, var_or_num_field,
};

pub struct DelayModal {
    pub value: u64,
    pub unit: DelayUnit,
    pub target_indices: Vec<usize>,
    pub duration_text: String,
    pub edit_idx: Option<usize>,
}

impl ModalWidget for DelayModal {
    fn title(&self) -> String {
        let is_bulk = self.is_bulk();
        if is_bulk {
            format!("{} Edit Delays", egui_phosphor::regular::TIMER)
        } else {
            format!("{} Delay", egui_phosphor::regular::TIMER)
        }
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["delay_duration"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        if self.is_bulk() {
            self.render_bulk(ui, state, palette)
        } else {
            self.render_single(ui, state, palette)
        }
    }
}

impl DelayModal {
    /// bulk mode is any modal opened from multi-select (no single edit
    /// target, non-empty target list); single-edit and add-new modals use
    /// the single duration field instead.
    fn is_bulk(&self) -> bool {
        self.edit_idx.is_none() && !self.target_indices.is_empty()
    }

    /// builds a modal pre-filled from a stored duration in milliseconds (e.g. 5000 → `5s`).
    pub fn from_ms(ms: u64, idx: usize) -> Self {
        Self {
            value: ms,
            unit: DelayUnit::Milliseconds,
            target_indices: vec![idx],
            duration_text: format_duration_string(ms),
            edit_idx: Some(idx),
        }
    }

    fn is_variable_duration(&self) -> bool {
        self.duration_text.trim().starts_with('$')
    }

    fn parsed_ms(&self) -> Option<u64> {
        if self.is_variable_duration() {
            return None;
        }
        parse_duration_string(&self.duration_text)
    }

    /// builds the command to commit from the current duration field; callers must ensure the input is valid.
    fn make_command(&self) -> MacroCommand {
        if self.is_variable_duration() {
            MacroCommand::Delay {
                duration_ms: parse_var_or_num(&self.duration_text),
            }
        } else {
            let ms = self.parsed_ms().expect("commit requires a valid duration");
            MacroCommand::Delay {
                duration_ms: Operand::Literal(Value::Number(ms as i64)),
            }
        }
    }

    // ── single-command mode ───────────────────────────────────────────────────────
    // TODO: share the duration parsing between single and bulk modes.
    fn render_single(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        let mut submitted = false;
        let mut field_resp = None;

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Duration")
                    .color(palette.text_muted)
                    .size(11.0),
            );
            let (s, r) =
                var_or_num_field(ui, state, "delay_duration", &mut self.duration_text, 160.0);
            submitted = s;
            field_resp = Some(r);
        });

        if let Some(resp) = field_resp {
            auto_focus(ui, "delay_duration", &resp);
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Enter duration (e.g. 1h 30m, 500ms) or $varname")
                .color(palette.text_muted)
                .size(10.0),
        );

        // keep the preview anchored to the last valid value so it does not jump while editing.
        let is_var = self.is_variable_duration();
        let parsed_ms = self.parsed_ms();
        if let Some(ms) = parsed_ms {
            self.value = ms;
        }
        let preview_text = if is_var {
            String::from(" ") // space preserves UI height.
        } else {
            format!("= {}", format_duration_string(self.value))
        };

        ui.label(
            egui::RichText::new(preview_text)
                .color(palette.text_muted)
                .size(11.0),
        );

        ui.add_space(12.0);

        let is_valid = is_var || parsed_ms.is_some();
        if submitted && is_valid {
            return ModalOutcome::Commit(self.make_command());
        }

        let mut outcome = ModalOutcome::Open;
        let btn_label = if self.edit_idx.is_some() {
            "Save"
        } else {
            "Add"
        };

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
                outcome = ModalOutcome::Commit(self.make_command());
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

    // ── bulk mode (multi-select recorded delays) ──────────────────────────────────
    // TODO: support `$variable` durations here instead of only the literal drag value.
    fn render_bulk(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        let valid_count = self.count_valid_delays(state);
        self.render_bulk_header(ui, palette, valid_count);

        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.value)
                    .speed(1)
                    .range(1..=9_999_999),
            );
            egui::ComboBox::from_id_salt("delay_unit_combo")
                .selected_text(self.unit.label())
                .width(60.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.unit, DelayUnit::Milliseconds, "ms");
                    ui.selectable_value(&mut self.unit, DelayUnit::Seconds, "s");
                    ui.selectable_value(&mut self.unit, DelayUnit::Minutes, "m");
                    ui.selectable_value(&mut self.unit, DelayUnit::Hours, "h");
                });
        });

        let ms = self.unit.to_ms(self.value as f64);
        ui.label(
            egui::RichText::new(format!("= {} ms", ms))
                .color(palette.text_muted)
                .size(11.0),
        );
        ui.add_space(16.0);

        let mut outcome = ModalOutcome::Open;

        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("Save").strong())
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                let mut s = state.lock().unwrap_or_else(|e| {
                    log::error!("State mutex poisoned: {e}");
                    e.into_inner()
                });
                // collect the delay-only targets first so the immutable borrow
                // ends before push_undo takes a mutable one; non-delays never
                // reach the mutation step.
                let delay_targets: Vec<usize> = s
                    .macro_state
                    .current_macro
                    .as_ref()
                    .map(|m| m.delay_indices(self.target_indices.iter().copied()))
                    .unwrap_or_default();
                if !delay_targets.is_empty() {
                    s.macro_state.push_undo();
                    if let Some(m) = s.macro_state.current_macro.as_mut() {
                        let updated = m.apply_bulk_delay(&delay_targets, ms);
                        s.unsaved_changes = true;
                        s.status_msg = format!(
                            "Updated {updated} delay{} to {ms} ms",
                            if updated == 1 { "" } else { "s" }
                        );
                    }
                }
                outcome = ModalOutcome::Cancelled; // treat bulk edit as complete, no cmd returned; the commands were already updated in place.
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

    fn count_valid_delays(&self, state: &SharedState) -> usize {
        let s = state.lock().unwrap_or_else(|e| {
            log::error!("State mutex poisoned: {e}");
            e.into_inner()
        });
        let Some(m) = &s.macro_state.current_macro else {
            return 0;
        };
        m.delay_indices(self.target_indices.iter().copied()).len()
    }

    fn render_bulk_header(&self, ui: &mut egui::Ui, palette: &ThemePalette, count: usize) {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            let grammar = if count == 1 { "Item" } else { "Items" };
            ui.label(
                egui::RichText::new(format!("Change Delay For {} Selected {}", count, grammar))
                    .color(palette.text_primary)
                    .size(14.0),
            );
            ui.add_space(10.0);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmacro_core_types::{Operand, Value};

    fn modal(duration_text: &str) -> DelayModal {
        DelayModal {
            value: 0,
            unit: DelayUnit::Milliseconds,
            target_indices: vec![0],
            duration_text: duration_text.to_string(),
            edit_idx: Some(0),
        }
    }

    fn literal_delay(ms: u64) -> MacroCommand {
        MacroCommand::Delay {
            duration_ms: Operand::Literal(Value::Number(ms as i64)),
        }
    }

    #[test]
    fn stored_milliseconds_round_trip() {
        let m = DelayModal::from_ms(5000, 3);
        assert_eq!(m.duration_text, "5s");
        assert_eq!(m.parsed_ms(), Some(5000));
        assert_eq!(m.make_command(), literal_delay(5000));
    }

    #[test]
    fn canonical_text_round_trips_through_parse() {
        for ms in [0, 1, 999, 1000, 1500, 5000, 60_000, 5_400_000, 3_600_000] {
            let m = DelayModal::from_ms(ms, 0);
            assert_eq!(m.parsed_ms(), Some(ms));
        }
    }

    #[test]
    fn bare_number_is_interpreted_as_milliseconds() {
        let m = modal("1500");
        assert_eq!(m.parsed_ms(), Some(1500));
    }

    #[test]
    fn compound_duration_is_parsed_to_milliseconds() {
        let m = modal("1h 30m");
        assert_eq!(m.parsed_ms(), Some(5_400_000));
    }

    #[test]
    fn variable_duration_is_preserved() {
        let m = modal("$my_delay");
        assert_eq!(m.parsed_ms(), None);
        assert_eq!(
            m.make_command(),
            MacroCommand::Delay {
                duration_ms: Operand::Var("my_delay".into())
            }
        );
    }

    #[test]
    fn bulk_modal_triggers_for_single_delay_target() {
        // regression: bulk path used `len() > 1`, so one mixed selection fell
        // through to single mode and appended instead of editing in place.
        let single_bulk = DelayModal {
            value: 100,
            unit: DelayUnit::Milliseconds,
            target_indices: vec![3],
            duration_text: "100".to_string(),
            edit_idx: None,
        };
        assert!(single_bulk.is_bulk());

        let add_new = DelayModal {
            value: 500,
            unit: DelayUnit::Milliseconds,
            target_indices: vec![],
            duration_text: "500".to_string(),
            edit_idx: None,
        };
        assert!(!add_new.is_bulk());

        let single_edit = DelayModal::from_ms(500, 3);
        assert!(!single_edit.is_bulk());
    }
}
