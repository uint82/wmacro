//! the Comment modal: lets the user write a note to insert or edit in the macro.

use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::MacroCommand;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::auto_focus;

pub struct CommentModal {
    pub text: String,
    pub edit_idx: Option<usize>,
}

// TODO: add quick-insert templates for common notes (e.g. "setup", "cleanup").

impl ModalWidget for CommentModal {
    fn title(&self) -> String {
        format!("{} Comment", egui_phosphor::regular::NOTE)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["comment_text"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        _state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Note")
                    .color(palette.text_muted)
                    .size(11.0),
            );

            let resp = ui.add(
                egui::TextEdit::multiline(&mut self.text)
                    .id(egui::Id::new("comment_text"))
                    .hint_text("Explain what this part of the macro does")
                    .desired_width(260.0)
                    .desired_rows(4),
            );

            auto_focus(ui, "comment_text", &resp);
        });

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Comments do not run during playback")
                .color(palette.text_muted)
                .size(10.0),
        );
        ui.add_space(12.0);

        let label = if self.edit_idx.is_some() {
            "Save"
        } else {
            "Add"
        };
        let mut outcome = ModalOutcome::Open;

        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.text.is_empty(),
                    egui::Button::new(egui::RichText::new(label).strong())
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Commit(MacroCommand::Comment(self.text.clone()));
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
