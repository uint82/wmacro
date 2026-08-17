use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use std::sync::{Arc, Mutex};
use wmacro_core_types::MacroCommand;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::auto_focus;

pub struct ImportMacroModal {
    pub path: String,
    pub edit_idx: Option<usize>,
    pub pending_path: Arc<Mutex<Option<String>>>,
}

impl ModalWidget for ImportMacroModal {
    fn title(&self) -> String {
        format!("{} Import Macro", egui_phosphor::regular::FOLDER_OPEN)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["import_macro_path"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        _state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        if let Ok(mut pending) = self.pending_path.lock()
            && let Some(new_path) = pending.take()
        {
            self.path = new_path;
        }

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Choose a macro file to play:")
                .color(palette.text_muted)
                .size(12.0),
        );
        ui.add_space(8.0);

        let mut path_resp = None;
        let mut submitted = false;

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Play macro:")
                    .color(palette.text_primary)
                    .size(13.0),
            );
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.path)
                    .id(egui::Id::new("import_macro_path"))
                    .desired_width(180.0),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submitted = true;
            }
            path_resp = Some(resp);

            if ui
                .add(
                    egui::Button::new(egui::RichText::new("Browse…").size(12.0))
                        .min_size(egui::vec2(70.0, ui.spacing().interact_size.y)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                spawn_file_picker(self.pending_path.clone(), ui.ctx().clone());
            }
        });

        if let Some(resp) = path_resp {
            auto_focus(ui, "import_macro_path", &resp);
        }

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        let is_valid = !self.path.trim().is_empty();
        let make_cmd = || MacroCommand::PlayMacro(self.path.trim().to_string());

        if submitted && is_valid {
            return ModalOutcome::Commit(make_cmd());
        }

        let label = if self.edit_idx.is_some() {
            "Save"
        } else {
            "Add"
        };
        let mut outcome = ModalOutcome::Open;

        super::right_aligned_row(ui, |ui| {
            if ui
                .add_enabled(
                    is_valid,
                    egui::Button::new(egui::RichText::new(label).strong())
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
                    egui::Button::new(egui::RichText::new("Cancel"))
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

fn spawn_file_picker(pending_path: Arc<Mutex<Option<String>>>, ctx: egui::Context) {
    // TODO: validate that the picked file is a playable `.wmr` script before enabling save.
    std::thread::spawn(move || {
        let default_dir = crate::macro_engine::storage::default_macro_dir();
        if let Some(picked) = rfd::FileDialog::new()
            .add_filter("wmacro script", &["wmr"][..])
            .set_directory(default_dir)
            .pick_file()
            && let Ok(mut p) = pending_path.lock()
        {
            *p = Some(picked.to_string_lossy().into_owned());
        }
        ctx.request_repaint();
    });
}
