use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use std::sync::{Arc, Mutex};
use wmacro_core_types::MacroCommand;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::auto_focus;

pub struct OpenFileModal {
    pub path: String,
    pub args: String,
    pub run_as_admin: bool,
    pub edit_idx: Option<usize>,
    pub pending_path: Arc<Mutex<Option<String>>>,
}

impl ModalWidget for OpenFileModal {
    fn title(&self) -> String {
        format!(
            "{} Open File / Program",
            egui_phosphor::regular::FILE_ARROW_UP
        )
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["open_file_path", "open_file_args"]
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

        let path_focused = ui
            .ctx()
            .memory(|m| m.focused() == Some(egui::Id::new("open_file_path")));
        let args_focused = ui
            .ctx()
            .memory(|m| m.focused() == Some(egui::Id::new("open_file_args")));
        let submitted =
            (path_focused || args_focused) && ui.input(|i| i.key_pressed(egui::Key::Enter));

        let mut path_resp = None;

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Program or File Path:")
                .color(palette.text_primary)
                .size(13.0),
        )
        .on_hover_text(
            "Enter a program name (like 'obsidian' or 'firefox') or an absolute file path.",
        );
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            path_resp = Some(
                ui.add(
                    egui::TextEdit::singleline(&mut self.path)
                        .id(egui::Id::new("open_file_path"))
                        .desired_width(230.0)
                        .hint_text("e.g., firefox or /path/to/file"),
                ),
            );

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
            auto_focus(ui, "open_file_path", &resp);
        }

        let path_trimmed = self.path.trim();
        let path_exists = !path_trimmed.is_empty() && which::which(path_trimmed).is_ok();

        // the save button stays disabled until the path exists.
        if !path_trimmed.is_empty() && !path_exists {
            ui.add_space(3.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} Path does not exist.",
                    egui_phosphor::regular::WARNING
                ))
                .color(palette.accent_danger)
                .size(11.0),
            );
        } else {
            ui.add_space(15.0);
        }

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Arguments (optional):")
                .color(palette.text_primary)
                .size(13.0),
        );
        ui.add_space(4.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.args)
                .id(egui::Id::new("open_file_args"))
                .desired_width(f32::INFINITY)
                .hint_text("--flag \"value with spaces\""),
        );

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.run_as_admin, "");
            ui.label(
                egui::RichText::new("Run as administrator (pkexec)")
                    .color(palette.text_primary)
                    .size(13.0),
            )
            .on_hover_text(
                "Uses PolicyKit (pkexec) to securely request admin rights via a graphical prompt.",
            );
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        let make_cmd = || MacroCommand::OpenFile {
            path: self.path.trim().to_string(),
            args: self.args.trim().to_string(),
            run_as_admin: self.run_as_admin,
        };

        if submitted && path_exists {
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
                    path_exists,
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
    // TODO: remember the last used directory so the dialog opens there next time.
    std::thread::spawn(move || {
        if let Some(picked) = rfd::FileDialog::new().pick_file()
            && let Ok(mut p) = pending_path.lock()
        {
            *p = Some(picked.to_string_lossy().into_owned());
        }
        ctx.request_repaint();
    });
}
