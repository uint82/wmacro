pub mod general;
pub mod hotkeys;
pub mod playback;
pub mod recording;

use super::theme::*;
use super::{IdeState, SettingsTab};
use crate::state::SharedState;
use eframe::egui;

pub fn save_settings(state: &SharedState) {
    match state.lock() {
        Ok(s) => s.to_settings().save(),
        Err(e) => log::error!("Failed to save settings: state mutex is poisoned ({})", e),
    }
}

pub fn render_settings(ctx: &egui::Context, state: &SharedState, ide: &mut IdeState) {
    if !ide.show_settings {
        return;
    }

    let palette = {
        let Ok(s) = state.lock() else {
            log::error!("Failed to render settings: state mutex is poisoned.");
            return;
        };
        s.theme_manager.get_theme(&s.theme_name)
    };

    let mut close = draw_backdrop(ctx);

    egui::Window::new(format!("{}  Settings", egui_phosphor::regular::GEAR))
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::vec2(440.0, 430.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(modal_frame(&palette))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {

                render_footer(ui, &palette, &mut close);
                ui.add_space(8.0);
                ui.add_space(16.0);

                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    render_tab_bar(ui, &palette, &mut ide.settings_tab);

                    ui.add_space(6.0);
                    ui.add(egui::Separator::default());
                    ui.add_space(10.0);

                    route_tab(ui, state, &palette, &ide.settings_tab);
                });
            });

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
        });

    if close {
        ide.show_settings = false;
    }
}

fn draw_backdrop(ctx: &egui::Context) -> bool {
    let screen = ctx.content_rect();
    let mut clicked = false;

    egui::Area::new(egui::Id::new("settings_backdrop"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.left_top())
        .interactable(true)
        .show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, egui::Sense::click());
            if resp.clicked() {
                clicked = true;
            }

            ui.painter().rect_filled(
                screen,
                egui::CornerRadius::ZERO,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
        });

    clicked
}

fn render_tab_bar(ui: &mut egui::Ui, palette: &ThemePalette, current_tab: &mut SettingsTab) {
    ui.horizontal(|ui| {
        render_tab_button(ui, palette, current_tab, SettingsTab::General, egui_phosphor::regular::GEAR, "General", 110.0);
        ui.add_space(4.0);
        render_tab_button(ui, palette, current_tab, SettingsTab::Hotkeys, egui_phosphor::regular::KEYBOARD, "Hotkeys", 110.0);
        ui.add_space(4.0);
        render_tab_button(ui, palette, current_tab, SettingsTab::Playback, egui_phosphor::regular::PLAY, "Playback", 100.0);
        ui.add_space(4.0);
        render_tab_button(ui, palette, current_tab, SettingsTab::Recording, egui_phosphor::regular::RECORD, "Recording", 100.0);
    });
}

fn render_tab_button(ui: &mut egui::Ui, palette: &ThemePalette, current_tab: &mut SettingsTab, target_tab: SettingsTab, icon: &str, label: &str, width: f32) {
    let is_active = *current_tab == target_tab;
    let color = if is_active { palette.accent_primary } else { palette.text_muted };
    let fill = if is_active { palette.bg_element } else { palette.bg_surface };

    if ui.add(
        egui::Button::new(egui::RichText::new(format!("{}  {}", icon, label)).color(color).strong().size(13.0))
            .fill(fill)
            .min_size(egui::vec2(width, 30.0)),
    ).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        *current_tab = target_tab;
    }
}

fn route_tab(ui: &mut egui::Ui, state: &SharedState, palette: &ThemePalette, current_tab: &SettingsTab) {
    match current_tab {
        SettingsTab::General => general::render(ui, state, palette),
        SettingsTab::Hotkeys => hotkeys::render(ui, state, palette),
        SettingsTab::Playback => playback::render(ui, state, palette),
        SettingsTab::Recording => recording::render(ui, state, palette),
    }
}

fn render_footer(ui: &mut egui::Ui, palette: &ThemePalette, close: &mut bool) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(
                egui::Button::new(egui::RichText::new("Close").color(palette.text_primary).strong())
                    .fill(palette.bg_element_alt)
                    .min_size(egui::vec2(80.0, 28.0)),
            ).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                *close = true;
            }
        });
    });
}
