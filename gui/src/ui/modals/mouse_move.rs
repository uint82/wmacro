use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::{Coord, MacroCommand, MacroEvent};

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::coord_controls;

pub struct MouseMoveModal {
    pub x: Coord,
    pub y: Coord,
    pub edit_idx: Option<usize>,
}

// TODO: offer the same "use current cursor position" toggle as the mouse modal.

impl ModalWidget for MouseMoveModal {
    fn title(&self) -> String {
        format!("{} Move Mouse", egui_phosphor::regular::ARROWS_OUT_CARDINAL)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn on_capture(&mut self, cx: i32, cy: i32) {
        self.x = Coord::Const(cx);
        self.y = Coord::Const(cy);
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        let mut submitted = false;

        egui::Grid::new("mouse_move_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                if coord_controls(ui, state, palette, "X", &mut self.x, true) {
                    submitted = true;
                }
                if coord_controls(ui, state, palette, "Y", &mut self.y, false) {
                    submitted = true;
                }

                ui.label(egui::RichText::new("Live Cursor").color(palette.text_muted));

                let (cx, cy, capture_hk) = {
                    let s = state.lock().unwrap_or_else(|e| {
                        log::error!("State mutex poisoned: {e}");
                        e.into_inner()
                    });
                    (s.cursor_x, s.cursor_y, s.macro_state.capture_hotkey)
                };

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("X: {}  Y: {}", cx, cy))
                            .monospace()
                            .color(palette.text_primary)
                            .size(12.0),
                    );
                    ui.add_space(8.0);
                    let hotkey_str =
                        crate::ui::key_names::hotkey_display_name_opt(capture_hk, "Unbound");
                    ui.label(
                        egui::RichText::new(format!("({} to capture)", hotkey_str))
                            .color(palette.text_muted)
                            .size(10.0),
                    );
                });
                ui.end_row();
            });

        ui.add_space(16.0);

        let make_cmd = || {
            MacroCommand::Action(MacroEvent::MouseMove {
                x: self.x.clone(),
                y: self.y.clone(),
            })
        };

        if submitted {
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
