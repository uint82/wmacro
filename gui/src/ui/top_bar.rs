use super::IdeState;
use crate::state::SharedState;
use eframe::egui;

pub fn render_top_bar(ui: &mut egui::Ui, state: &SharedState, ide: &mut IdeState) {
    let palette = {
        let s = state.lock().unwrap();
        s.theme_manager.get_theme(&s.theme_name)
    };

    ui.horizontal(|ui| {
        render_title_and_version(ui, &palette);
        ui.add_space(12.0);

        let saved_visuals = ui.visuals().clone();
        apply_menu_bar_theme(ui, &palette);

        let file_action = render_file_menu(ui, &palette);
        render_help_menu(ui, &palette);

        *ui.visuals_mut() = saved_visuals;

        render_window_controls(ui, &palette);

        dispatch_file_action(file_action, ui.ctx(), state, ide);
    });
}

fn render_title_and_version(ui: &mut egui::Ui, palette: &crate::ui::theme::ThemePalette) {
    ui.label(
        egui::RichText::new("wmacro")
            .strong()
            .size(15.0)
            .color(palette.text_primary),
    );

    egui::Frame::NONE
        .fill(palette.bg_element)
        .corner_radius(egui::CornerRadius::same(3))
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("v1.0.0")
                    .size(9.0)
                    .color(palette.text_muted),
            );
        });
}

fn apply_menu_bar_theme(ui: &mut egui::Ui, palette: &crate::ui::theme::ThemePalette) {
    let widgets = &mut ui.visuals_mut().widgets;

    widgets.inactive.weak_bg_fill = palette.bg_surface;
    widgets.hovered.weak_bg_fill = palette.bg_element;
    widgets.active.weak_bg_fill = palette.bg_element;
    widgets.open.weak_bg_fill = palette.bg_element;

    widgets.inactive.fg_stroke.color = palette.text_primary;
    widgets.hovered.fg_stroke.color = palette.text_primary;
    widgets.active.fg_stroke.color = palette.text_primary;

    widgets.inactive.bg_stroke = egui::Stroke::NONE;
    widgets.hovered.bg_stroke = egui::Stroke::NONE;
    widgets.active.bg_stroke = egui::Stroke::NONE;
    widgets.open.bg_stroke = egui::Stroke::NONE;
}

enum FileAction {
    None,
    New,
    Open,
    Save,
    SaveAs,
}

fn render_file_menu(ui: &mut egui::Ui, palette: &crate::ui::theme::ThemePalette) -> FileAction {
    let mut action = FileAction::None;

    ui.menu_button(
        egui::RichText::new("File")
            .color(palette.text_primary)
            .size(12.0),
        |ui| {
            ui.set_min_width(140.0);

            if menu_item(ui, "New Macro") {
                action = FileAction::New;
                ui.close();
            }
            if menu_item(ui, "Open…") {
                action = FileAction::Open;
                ui.close();
            }
            ui.separator();
            if menu_item(ui, "Save") {
                action = FileAction::Save;
                ui.close();
            }
            if menu_item(ui, "Save As…") {
                action = FileAction::SaveAs;
                ui.close();
            }
        },
    )
    .response
    .on_hover_cursor(egui::CursorIcon::PointingHand);

    action
}

fn render_help_menu(ui: &mut egui::Ui, palette: &crate::ui::theme::ThemePalette) {
    ui.menu_button(
        egui::RichText::new("Help")
            .color(palette.text_primary)
            .size(12.0),
        |ui| {
            ui.set_min_width(140.0);

            if menu_item(ui, "Documentation") {
                ui.ctx()
                    .open_url(egui::OpenUrl::new_tab("https://github.com/uint82/wmacro"));
                ui.close();
            }
            if menu_item(ui, "Report a Bug") {
                ui.ctx().open_url(egui::OpenUrl::new_tab(
                    "https://github.com/uint82/wmacro/issues",
                ));
                ui.close();
            }
        },
    )
    .response
    .on_hover_cursor(egui::CursorIcon::PointingHand);
}

fn menu_item(ui: &mut egui::Ui, label: &str) -> bool {
    ui.button(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

fn render_window_controls(ui: &mut egui::Ui, palette: &crate::ui::theme::ThemePalette) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if window_control_button(ui, egui_phosphor::regular::X, palette.accent_danger) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }

        ui.add_space(8.0);

        if window_control_button(ui, egui_phosphor::regular::SQUARE, palette.text_muted) {
            let is_maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
        }

        ui.add_space(8.0);

        if window_control_button(ui, egui_phosphor::regular::MINUS, palette.text_muted) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }

        let title_bar_response = ui.allocate_response(ui.available_size(), egui::Sense::click());
        if title_bar_response.is_pointer_button_down_on() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    });
}

fn window_control_button(ui: &mut egui::Ui, icon: &str, color: egui::Color32) -> bool {
    ui.add(egui::Button::new(egui::RichText::new(icon).color(color).size(14.0)).frame(false))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

fn dispatch_file_action(
    action: FileAction,
    ctx: &egui::Context,
    state: &SharedState,
    ide: &mut IdeState,
) {
    match action {
        FileAction::None => {}
        FileAction::New => start_new_macro(state, ide),
        FileAction::Open => spawn_open_macro(ctx, state, ide),
        FileAction::Save => spawn_save_macro(ctx, state),
        FileAction::SaveAs => spawn_save_macro_as(ctx, state),
    }
}

fn start_new_macro(state: &SharedState, ide: &mut IdeState) {
    let mut s = state.lock().unwrap();
    s.macro_state.current_macro = Some(core_types::Macro::new("untitled"));
    s.macro_state.events_captured = 0;
    s.macro_state.macro_name = "untitled".to_string();
    drop(s);
    ide.selected.clear();
}

fn spawn_open_macro(ctx: &egui::Context, state: &SharedState, ide: &mut IdeState) {
    let state = std::sync::Arc::clone(state);
    let ctx = ctx.clone();

    std::thread::spawn(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_directory(crate::macro_engine::storage::default_macro_dir())
            .add_filter("wmacro script", &["wmr"][..])
            .pick_file()
        else {
            return;
        };

        match crate::macro_engine::storage::load_wmr(&path) {
            Ok(m) => {
                let mut s = state.lock().unwrap();
                s.macro_state.macro_name = m.name.clone();
                s.macro_state.events_captured = m.commands.len();
                s.macro_state.current_macro = Some(m);
                s.status_msg = format!("Loaded {}", path.display());
            }
            Err(e) => {
                log::warn!("failed to load macro from {}: {e}", path.display());
                set_status(&state, format!("Failed to load: {e}"));
            }
        }

        ctx.request_repaint();
    });


    ide.pending_clear_selection = true;
}

fn spawn_save_macro(ctx: &egui::Context, state: &SharedState) {
    let Some(m) = state.lock().unwrap().macro_state.current_macro.clone() else {
        return;
    };

    let path = crate::macro_engine::storage::macro_wmr_path(&m.name);
    let state = std::sync::Arc::clone(state);
    let ctx = ctx.clone();

    std::thread::spawn(move || {
        match crate::macro_engine::storage::save_wmr(&m, &path) {
            Ok(_) => set_status(&state, format!("Saved to {}", path.display())),
            Err(e) => {
                log::warn!("failed to save macro to {}: {e}", path.display());
                set_status(&state, format!("Failed to save: {e}"));
            }
        }
        ctx.request_repaint();
    });
}

fn spawn_save_macro_as(ctx: &egui::Context, state: &SharedState) {
    let Some(mut m) = state.lock().unwrap().macro_state.current_macro.clone() else {
        return;
    };

    let state = std::sync::Arc::clone(state);
    let ctx = ctx.clone();

    std::thread::spawn(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_directory(crate::macro_engine::storage::default_macro_dir())
            .add_filter("wmacro script", &["wmr"][..])
            .set_file_name(&format!("{}.wmr", m.name))
            .save_file()
        else {
            return;
        };

        if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
            m.name = file_stem.to_string();
        }

        match crate::macro_engine::storage::save_wmr(&m, &path) {
            Ok(_) => {
                let mut s = state.lock().unwrap();
                s.macro_state.macro_name = m.name.clone();
                s.macro_state.current_macro = Some(m);
                s.status_msg = format!("Saved to {}", path.display());
            }
            Err(e) => {
                log::warn!("failed to save macro to {}: {e}", path.display());
                set_status(&state, format!("Failed to save: {e}"));
            }
        }

        ctx.request_repaint();
    });
}

fn set_status(state: &SharedState, msg: String) {
    state.lock().unwrap().status_msg = msg;
}
