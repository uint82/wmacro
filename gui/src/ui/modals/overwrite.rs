//! the overwrite warning modal: asks whether to overwrite the current macro on import.

use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;

pub struct OverwriteModal;

// TODO: offer "append to the current macro" as a third choice, mirroring the recording behavior setting.

impl ModalWidget for OverwriteModal {
    fn title(&self) -> String {
        format!("{} Warning", egui_phosphor::regular::WARNING)
    }

    fn edit_idx(&self) -> Option<usize> {
        None
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        ui.label(
            egui::RichText::new(
                "This will overwrite your current macro.\nAll unsaved events will be lost!",
            )
            .color(palette.text_primary)
            .size(13.0),
        );
        ui.add_space(16.0);

        let mut outcome = ModalOutcome::Open;

        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("Yes, Overwrite")
                            .color(palette.accent_danger_fg)
                            .strong(),
                    )
                    .fill(palette.accent_danger)
                    .min_size(egui::vec2(100.0, 28.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                let name = {
                    let s = state.lock().unwrap_or_else(|e| {
                        log::error!("State mutex poisoned: {e}");
                        e.into_inner()
                    });
                    s.macro_state.macro_name.clone()
                };
                crate::macro_engine::recorder::start_recording(state, name, false);
                outcome = ModalOutcome::Cancelled; // just close; recording already started as a side-effect.
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
