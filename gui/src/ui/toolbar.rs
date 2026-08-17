//! toolbar with record, play, pause, and stop controls for the current macro.

use super::IdeState;
use super::Modal;
use super::components::status_chip;
use super::settings::save_settings;
use crate::macro_engine::player::spawn_player;
use crate::macro_engine::recorder::{start_recording, stop_recording};
use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use std::sync::atomic::Ordering;

struct ToolbarState {
    recording: bool,
    playing: bool,
    has_commands: bool,
    record_paused: bool,
    play_paused: bool,
}

fn snapshot_toolbar_state(state: &SharedState) -> ToolbarState {
    // one short lock at the top of the frame so all buttons agree on the same state instead of each polling the mutex independently.
    let s = state.lock().unwrap();
    let has_commands = s
        .macro_state
        .current_macro
        .as_ref()
        .is_some_and(|m| !m.commands.is_empty());

    ToolbarState {
        recording: s.macro_state.recording,
        playing: s.macro_state.playing,
        has_commands,
        record_paused: s.macro_state.record_paused,
        play_paused: s.macro_state.play_paused,
    }
}

pub fn render_toolbar(ui: &mut egui::Ui, state: &SharedState, ide: &mut IdeState) {
    let palette = {
        let s = state.lock().unwrap();
        s.theme_manager.get_theme(&s.theme_name)
    };

    ui.horizontal(|ui| {
        let toolbar = snapshot_toolbar_state(state);
        let btn_rounding = egui::CornerRadius::same(6);

        render_record_button(ui, state, ide, &palette, &toolbar, btn_rounding);
        if toolbar.recording {
            ui.add_space(1.0);
            render_record_pause_button(ui, state, &palette, &toolbar, btn_rounding);
        }

        ui.add_space(1.0);
        render_play_button(ui, state, &palette, &toolbar, btn_rounding);
        if toolbar.playing {
            ui.add_space(1.0);
            render_play_pause_button(ui, state, &palette, &toolbar, btn_rounding);
        }

        ui.add_space(1.0);
        render_append_button(ui, state, &palette, &toolbar, btn_rounding);

        ui.add_space(1.0);
        render_settings_button(ui, ide, &palette, btn_rounding);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            status_chip(
                ui,
                toolbar.recording,
                toolbar.playing,
                toolbar.record_paused,
                toolbar.play_paused,
                &palette,
            );
        });
    });
}

fn render_record_button(
    ui: &mut egui::Ui,
    state: &SharedState,
    ide: &mut IdeState,
    palette: &crate::ui::theme::ThemePalette,
    toolbar: &ToolbarState,
    rounding: egui::CornerRadius,
) {
    let label = if toolbar.recording {
        format!("{}  Stop", egui_phosphor::regular::SQUARE)
    } else {
        format!("{}  Record", egui_phosphor::regular::RECORD)
    };

    let button = egui::Button::new(
        egui::RichText::new(label)
            .color(palette.accent_danger_fg)
            .strong(),
    )
    .fill(palette.accent_danger)
    .stroke(egui::Stroke::new(1.0_f32, palette.accent_danger))
    .corner_radius(rounding)
    .min_size(egui::vec2(86.0, 36.0));

    let clicked = ui
        .add_enabled(!toolbar.playing, button)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked();

    if clicked {
        handle_record_click(state, ide, toolbar);
    }
}

fn handle_record_click(state: &SharedState, ide: &mut IdeState, toolbar: &ToolbarState) {
    if toolbar.recording {
        stop_recording(state);
        return;
    }

    if toolbar.has_commands {
        // recording over an existing macro would silently clobber it, so ask first via the overwrite modal.
        ide.modal = Modal::Widget(Box::new(crate::ui::modals::overwrite::OverwriteModal));
        return;
    }

    let name = state.lock().unwrap().macro_state.macro_name.clone();
    start_recording(state, name, false);
}

fn render_record_pause_button(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &crate::ui::theme::ThemePalette,
    toolbar: &ToolbarState,
    rounding: egui::CornerRadius,
) {
    let label = if toolbar.record_paused {
        format!("{}  Resume", egui_phosphor::regular::RECORD)
    } else {
        format!("{}  Pause", egui_phosphor::regular::PAUSE)
    };
    let fill = if toolbar.record_paused {
        palette.accent_danger
    } else {
        palette.bg_element_alt
    };
    let fg = if toolbar.record_paused {
        palette.accent_danger_fg
    } else {
        palette.text_primary
    };

    let button = egui::Button::new(egui::RichText::new(label).color(fg).strong())
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, palette.border))
        .corner_radius(rounding)
        .min_size(egui::vec2(86.0, 36.0));

    let clicked = ui
        .add(button)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked();

    if clicked {
        let mut s = state.lock().unwrap();
        s.macro_state.record_paused = !s.macro_state.record_paused;
        if let Some(flag) = &s.macro_state.record_paused_flag {
            flag.store(s.macro_state.record_paused, Ordering::Relaxed);
        }
    }
}

fn render_play_button(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &crate::ui::theme::ThemePalette,
    toolbar: &ToolbarState,
    rounding: egui::CornerRadius,
) {
    let (fill, fg, stroke) = if toolbar.playing {
        (
            palette.accent_danger,
            palette.accent_danger_fg,
            egui::Stroke::NONE,
        )
    } else {
        (
            palette.accent_success,
            palette.accent_success_fg,
            egui::Stroke::new(1.0_f32, palette.accent_success),
        )
    };
    let label = if toolbar.playing {
        format!("{}  Stop", egui_phosphor::regular::SQUARE)
    } else {
        format!("{}  Play", egui_phosphor::regular::PLAY)
    };

    let button = egui::Button::new(egui::RichText::new(label).color(fg).strong())
        .fill(fill)
        .stroke(stroke)
        .corner_radius(rounding)
        .min_size(egui::vec2(86.0, 36.0));

    let enabled = !toolbar.recording && (toolbar.has_commands || toolbar.playing);
    let clicked = ui
        .add_enabled(enabled, button)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked();

    if clicked {
        handle_play_click(state, toolbar);
    }
}

fn handle_play_click(state: &SharedState, toolbar: &ToolbarState) {
    if toolbar.playing {
        // stopping is cooperative: flip the kill flag and let the player
        // thread wind down at its next checkpoint.
        let mut s = state.lock().unwrap();
        if let Some(kill) = s.macro_state.play_kill.take() {
            kill.store(true, Ordering::Relaxed);
        }
        s.macro_state.playing = false;
        s.macro_state.play_paused = false;
        s.status_msg = String::from("Macro stopped");
        return;
    }

    let (kill_flag, pause_flag, step_flag) = spawn_player(state.clone());
    let mut s = state.lock().unwrap();
    s.macro_state.play_kill = Some(kill_flag);
    s.macro_state.play_paused_flag = Some(pause_flag);
    s.macro_state.play_step_flag = Some(step_flag);
}

fn render_play_pause_button(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &crate::ui::theme::ThemePalette,
    toolbar: &ToolbarState,
    rounding: egui::CornerRadius,
) {
    let label = if toolbar.play_paused {
        format!("{}  Resume", egui_phosphor::regular::PLAY)
    } else {
        format!("{}  Pause", egui_phosphor::regular::PAUSE)
    };
    let fill = if toolbar.play_paused {
        palette.accent_success
    } else {
        palette.bg_element_alt
    };
    let fg = if toolbar.play_paused {
        palette.accent_success_fg
    } else {
        palette.text_primary
    };

    let button = egui::Button::new(egui::RichText::new(label).color(fg).strong())
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, palette.border))
        .corner_radius(rounding)
        .min_size(egui::vec2(86.0, 36.0));

    let clicked = ui
        .add(button)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked();

    if clicked {
        let mut s = state.lock().unwrap();
        s.macro_state.play_paused = !s.macro_state.play_paused;
        if let Some(flag) = &s.macro_state.play_paused_flag {
            flag.store(s.macro_state.play_paused, Ordering::Relaxed);
        }
    }
}

fn render_append_button(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &crate::ui::theme::ThemePalette,
    toolbar: &ToolbarState,
    rounding: egui::CornerRadius,
) {
    let button = egui::Button::new(
        egui::RichText::new(format!("{} Append", egui_phosphor::regular::PLUS))
            .color(palette.text_primary)
            .strong(),
    )
    .fill(palette.bg_element_alt)
    .stroke(egui::Stroke::new(1.0_f32, palette.border))
    .corner_radius(rounding)
    .min_size(egui::vec2(86.0, 36.0));

    let enabled = !toolbar.playing && !toolbar.recording && toolbar.has_commands;
    let clicked = ui
        .add_enabled(enabled, button)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked();

    if clicked {
        let name = state.lock().unwrap().macro_state.macro_name.clone();
        start_recording(state, name, true);
    }
}

fn render_settings_button(
    ui: &mut egui::Ui,
    ide: &mut IdeState,
    palette: &crate::ui::theme::ThemePalette,
    rounding: egui::CornerRadius,
) {
    let button = egui::Button::new(
        egui::RichText::new(format!("{} Settings", egui_phosphor::regular::GEAR))
            .color(palette.text_primary)
            .strong(),
    )
    .fill(palette.bg_element_alt)
    .stroke(egui::Stroke::new(1.0_f32, palette.border))
    .corner_radius(rounding)
    .min_size(egui::vec2(86.0, 36.0));

    let clicked = ui
        .add(button)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked();

    if clicked {
        ide.show_settings = true;
    }
}

pub fn render_toolbox_toggle(ui: &mut egui::Ui, state: &SharedState, palette: &ThemePalette) {
    // TODO: read the visibility from settings instead of state so the toggle survives restart; currently save_settings covers it, which is close.
    let visible = state.lock().unwrap().show_toolbox;
    let icon = if visible {
        egui_phosphor::regular::SQUARE_SPLIT_HORIZONTAL
    } else {
        egui_phosphor::regular::SIDEBAR_SIMPLE
    };

    let clicked = ui
        .add(
            egui::Button::new(
                egui::RichText::new(icon)
                    .color(palette.text_primary)
                    .size(15.0),
            )
            .frame(false)
            .min_size(egui::vec2(28.0, 24.0)),
        )
        .on_hover_text("Toggle toolbox (Ctrl+B)")
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked();

    if clicked {
        let mut s = state.lock().unwrap();
        s.show_toolbox = !s.show_toolbox;
        drop(s);
        save_settings(state);
    }
}
