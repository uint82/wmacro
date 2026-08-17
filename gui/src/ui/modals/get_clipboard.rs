use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::MacroCommand;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::auto_focus;

pub struct GetClipboardModal {
    pub target: String,
    pub edit_idx: Option<usize>,
}

// TODO: show the current clipboard content as a preview for sanity checks.

impl ModalWidget for GetClipboardModal {
    fn title(&self) -> String {
        format!("{} Get Clipboard", egui_phosphor::regular::CLIPBOARD)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["get_clipboard_name"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        _state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        let name_focused = ui
            .ctx()
            .memory(|m| m.focused() == Some(egui::Id::new("get_clipboard_name")));
        let submitted = name_focused && ui.input(|i| i.key_pressed(egui::Key::Enter));

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Store in variable")
                    .color(palette.text_muted)
                    .size(11.0),
            );
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.target)
                    .id(egui::Id::new("get_clipboard_name"))
                    .hint_text("variable_name")
                    .desired_width(200.0),
            );
            auto_focus(ui, "get_clipboard_name", &resp);
        });

        ui.add_space(16.0);

        let can_commit = !self.target.trim().is_empty();

        let make_cmd = || MacroCommand::GetClipboard {
            target: self.target.trim().to_string(),
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
