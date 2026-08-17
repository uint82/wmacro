//! the Set Clipboard modal: lets the user enter text to copy to the clipboard.

use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::MacroCommand;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::{auto_focus, parse_var_or_num, var_or_num_field};

pub struct SetClipboardModal {
    pub text: String,
    pub edit_idx: Option<usize>,
}

// TODO: add a small paste button that reads the current clipboard into the field.

impl ModalWidget for SetClipboardModal {
    fn title(&self) -> String {
        format!("{} Set Clipboard", egui_phosphor::regular::CLIPBOARD_TEXT)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["set_clipboard_text"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        let mut submitted = false;
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Text")
                    .color(palette.text_muted)
                    .size(11.0),
            );
            let (enter, resp) =
                var_or_num_field(ui, state, "set_clipboard_text", &mut self.text, 220.0);
            auto_focus(ui, "set_clipboard_text", &resp);
            submitted = enter;
        });

        ui.add_space(16.0);

        let can_commit = !self.text.trim().is_empty();

        let make_cmd = || MacroCommand::SetClipboard {
            text: parse_var_or_num(&self.text),
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
