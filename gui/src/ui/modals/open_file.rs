use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use wmacro_core_types::MacroCommand;
use eframe::egui;
use std::sync::{Arc, Mutex};

pub fn render(
    ui: &mut egui::Ui,
    _state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    path: &mut String,
    args: &mut String,
    run_as_admin: &mut bool,
    edit_idx: &Option<usize>,
    pending_path: &Arc<Mutex<Option<String>>>,
) {
    if let Ok(mut pending) = pending_path.lock() {
        if let Some(new_path) = pending.take() {
            *path = new_path;
        }
    }

    ui.add_space(8.0);

    ui.label(egui::RichText::new("Program or File Path:").color(palette.text_primary).size(13.0))
        .on_hover_text("Enter a program name (like 'obsidian' or 'firefox') or an absolute file path.");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(path)
                .desired_width(230.0)
                .hint_text("e.g., firefox or /path/to/file")
        );

        if ui
            .add(
                egui::Button::new(egui::RichText::new("Browse…").size(12.0))
                    .min_size(egui::vec2(70.0, 24.0)),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            spawn_file_picker(pending_path.clone(), ui.ctx().clone());
        }
    });

    let path_trimmed = path.trim();
    let path_is_empty = path_trimmed.is_empty();

    let path_exists = !path_is_empty && which::which(path_trimmed).is_ok();

    if !path_is_empty && !path_exists {
        ui.add_space(3.0);
        ui.label(
            egui::RichText::new(format!("{} Path does not exist.", egui_phosphor::regular::WARNING))
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
        egui::TextEdit::singleline(args)
            .desired_width(f32::INFINITY)
            .hint_text("--flag \"value with spaces\""),
    );

    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.checkbox(run_as_admin, "");
        ui.label(
            egui::RichText::new("Run as administrator (pkexec)")
                .color(palette.text_primary)
                .size(13.0),
        )
        .on_hover_text("Uses PolicyKit (pkexec) to securely request admin rights via a graphical prompt.");
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    let is_valid = path_exists;
    render_buttons(ui, close, commit, edit_idx, path, args, *run_as_admin, is_valid);
}

fn spawn_file_picker(pending_path: Arc<Mutex<Option<String>>>, ctx: egui::Context) {
    std::thread::spawn(move || {
        if let Some(picked) = rfd::FileDialog::new().pick_file() {
            if let Ok(mut p) = pending_path.lock() {
                *p = Some(picked.to_string_lossy().into_owned());
            }
        }
        ctx.request_repaint();
    });
}

fn render_buttons(
    ui: &mut egui::Ui,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    edit_idx: &Option<usize>,
    path: &str,
    args: &str,
    run_as_admin: bool,
    is_valid: bool,
) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if edit_idx.is_some() { "Save" } else { "Add" };

            if ui
                .add_enabled(
                    is_valid,
                    egui::Button::new(egui::RichText::new(label).strong())
                        .min_size(egui::vec2(80.0, 28.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *commit = Some(MacroCommand::OpenFile {
                    path: path.trim().to_string(),
                    args: args.trim().to_string(),
                    run_as_admin,
                });
                *close = true;
            }

            ui.add_space(8.0);

            if ui
                .add(
                    egui::Button::new(egui::RichText::new("Cancel"))
                        .min_size(egui::vec2(80.0, 28.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *close = true;
            }
        });
    });
}
