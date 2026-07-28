use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use core_types::MacroCommand;
use eframe::egui;
use std::sync::{Arc, Mutex};

pub fn render(
    ui: &mut egui::Ui,
    _state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    path: &mut String,
    edit_idx: &Option<usize>,
    pending_path: &Arc<Mutex<Option<String>>>,
) {
    update_pending_path(path, pending_path);

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Choose a macro file to play:")
            .color(palette.text_muted)
            .size(12.0),
    );
    ui.add_space(8.0);

    render_path_input(ui, palette, path, pending_path);

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    render_buttons(ui, close, commit, edit_idx, path);
}

fn update_pending_path(path: &mut String, pending_path: &Arc<Mutex<Option<String>>>) {
    if let Ok(mut pending) = pending_path.lock() {
        if let Some(new_path) = pending.take() {
            *path = new_path;
        }
    }
}

fn render_path_input(
    ui: &mut egui::Ui,
    palette: &ThemePalette,
    path: &mut String,
    pending_path: &Arc<Mutex<Option<String>>>,
) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Play macro:")
                .color(palette.text_primary)
                .size(13.0),
        );
        ui.add(egui::TextEdit::singleline(path).desired_width(180.0));

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
}

fn spawn_file_picker(pending_path: Arc<Mutex<Option<String>>>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let default_dir = crate::macro_engine::storage::default_macro_dir();
        if let Some(picked) = rfd::FileDialog::new()
            .add_filter("wmacro script", &["wmr"][..])
            .set_directory(default_dir)
            .pick_file()
        {
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
) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = if edit_idx.is_some() { "Save" } else { "Add" };
            let is_valid = !path.trim().is_empty();

            if ui
                .add_enabled(
                    is_valid,
                    egui::Button::new(egui::RichText::new(label).strong())
                        .min_size(egui::vec2(80.0, 28.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *commit = Some(MacroCommand::PlayMacro(path.trim().to_string()));
                *close = true;
            }

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
