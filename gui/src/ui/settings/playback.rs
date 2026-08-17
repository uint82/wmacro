//! the playback settings tab: speed, repeat mode, and other playback options.

use super::save_settings;
use crate::state::{MacroRepeatMode, SharedState};
use crate::ui::theme::ThemePalette;
use eframe::egui;

pub fn render(ui: &mut egui::Ui, state: &SharedState, palette: &ThemePalette) {
    // TODO: add a preview button that plays a short sample path with the current humanize settings.
    ui.label(
        egui::RichText::new("Configure how macros are played back.")
            .color(palette.text_muted)
            .size(11.0),
    );
    ui.add_space(10.0);

    let (mut speed, repeat_mode, mut smart_path) = {
        let Ok(s) = state.lock() else { return };
        (
            s.macro_state.speed_multiplier,
            s.macro_state.repeat_mode.clone(),
            s.macro_state.playback_options.smart_path.clone(),
        )
    };

    let mut repeat_count: u32 = match &repeat_mode {
        MacroRepeatMode::Once => 1,
        MacroRepeatMode::Count(n) => *n,
        MacroRepeatMode::Infinite => 0,
    };

    egui::Grid::new("playback_grid").num_columns(2).spacing([16.0, 12.0]).show(ui, |ui| {
        ui.label(egui::RichText::new("Speed").color(palette.text_primary).size(13.0));

        ui.horizontal(|ui| {
            ui.scope(|ui| {
                let visuals = ui.visuals_mut();
                visuals.widgets.inactive.weak_bg_fill = palette.bg_element_alt;
                visuals.widgets.inactive.fg_stroke.color = palette.text_primary;
                visuals.widgets.hovered.weak_bg_fill = palette.bg_element;
                visuals.widgets.hovered.fg_stroke.color = palette.text_primary;
                visuals.widgets.active.weak_bg_fill = palette.bg_element;
                visuals.widgets.active.fg_stroke.color = palette.text_primary;

                if ui.add(egui::DragValue::new(&mut speed).speed(0.05).range(0.1_f32..=10.0).suffix("×").max_decimals(2)).changed() {
                    if let Ok(mut s) = state.lock() { s.macro_state.speed_multiplier = speed; }
                    save_settings(state);
                }
            });
            ui.add_space(8.0);
            for preset in &[0.25_f32, 0.5, 1.0, 2.0, 5.0] {
                let label = if *preset == 1.0 { "1×".to_string() } else { format!("{}×", preset) };
                let is_active = (speed - preset).abs() < 0.01;
                let fill = if is_active { palette.accent_primary } else { palette.bg_element_alt };
                let text_color = if is_active { palette.accent_primary_fg } else { palette.text_muted };

                if ui.add(
                    egui::Button::new(egui::RichText::new(label).color(text_color).size(10.0))
                        .fill(fill)
                        .min_size(egui::vec2(32.0, 20.0)),
                ).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    speed = *preset;
                    if let Ok(mut s) = state.lock() { s.macro_state.speed_multiplier = speed; }
                    save_settings(state);
                }
            }
        });
        ui.end_row();

        ui.label(egui::RichText::new("Repeat").color(palette.text_primary).size(13.0));

        ui.horizontal(|ui| {
            ui.scope(|ui| {
                let visuals = ui.visuals_mut();
                visuals.widgets.inactive.weak_bg_fill = palette.bg_element_alt;
                visuals.widgets.inactive.fg_stroke.color = palette.text_primary;
                visuals.widgets.hovered.weak_bg_fill = palette.bg_element;
                visuals.widgets.hovered.fg_stroke.color = palette.text_primary;
                visuals.widgets.active.weak_bg_fill = palette.bg_element;
                visuals.widgets.active.fg_stroke.color = palette.text_primary;

                if ui.add(egui::DragValue::new(&mut repeat_count).speed(1).range(0..=99999_u32)).changed() {
                    let mode = match repeat_count {
                        0 => MacroRepeatMode::Infinite,
                        1 => MacroRepeatMode::Once,
                        n => MacroRepeatMode::Count(n),
                    };
                    if let Ok(mut s) = state.lock() { s.macro_state.repeat_mode = mode; }
                    save_settings(state);
                }
            });
            ui.add_space(6.0);
            let hint = match repeat_count {
                0 => format!("{}  infinite loop", egui_phosphor::regular::INFINITY),
                1 => "play once".to_string(),
                _ => "times".to_string(),
            };
            ui.label(egui::RichText::new(hint).color(palette.text_muted).size(11.0));
        });
        ui.end_row();

        ui.label(egui::RichText::new("Humanize").color(palette.text_primary).size(13.0));
        ui.vertical(|ui| {
            if ui.checkbox(&mut smart_path.enabled, egui::RichText::new("Hybrid Synthetic Path Engine").color(palette.text_primary).size(13.0)).changed() {
                if let Ok(mut s) = state.lock() { s.macro_state.playback_options.smart_path.enabled = smart_path.enabled; }
                save_settings(state);
            }
            if smart_path.enabled {
                ui.add_space(8.0);

                let wobble_changed = ui.horizontal(|ui| {
                    let mut changed = false;
                    ui.label(egui::RichText::new("Tremor Max").color(palette.text_muted).size(11.0))
                        .on_hover_text("Maximum micro-wobble variation during movement (range: 0 to this value).");
                    ui.add_space(4.0);
                    changed |= ui.add(egui::Slider::new(&mut smart_path.path_wobble, 0.0..=5.0).text("px")).changed();
                    changed
                }).inner;

                let curve_changed = ui.horizontal(|ui| {
                    let mut changed = false;
                    ui.label(egui::RichText::new("Path Curve Max").color(palette.text_muted).size(11.0))
                        .on_hover_text("Maximum multiplier for the Bézier curve magnitude (range: -max to +max).");
                    ui.add_space(4.0);
                    changed |= ui.add(egui::Slider::new(&mut smart_path.path_curve, 0.0..=0.3).text("x")).changed();
                    changed
                }).inner;

                let jitter_changed = ui.horizontal(|ui| {
                    let mut changed = false;
                    ui.label(egui::RichText::new("Endpoint Jitter Max").color(palette.text_muted).size(11.0))
                        .on_hover_text("Maximum deviation of the final cursor position (range: 0 to this value). Set to 0 for pixel-perfect clicks.");
                    ui.add_space(4.0);
                    changed |= ui.add(egui::Slider::new(&mut smart_path.endpoint_jitter, 0.0..=20.0).text("px")).changed();
                    changed
                }).inner;

                let delay_changed = ui.horizontal(|ui| {
                    let mut changed = false;
                    ui.label(egui::RichText::new("Path Break Delay Max").color(palette.text_muted).size(11.0))
                        .on_hover_text("Maximum recorded delay (in ms) that breaks the movement into a new path segment (sampled between 10ms and this value).");
                    ui.add_space(4.0);
                    changed |= ui.add(egui::Slider::new(&mut smart_path.segment_delay_threshold_ms, 10..=2000).text("ms").logarithmic(true)).changed();
                    changed
                }).inner;

                let sub_changed = ui.checkbox(&mut smart_path.submovement_enabled, egui::RichText::new("Enable Submovements").color(palette.text_muted).size(11.0))
                    .on_hover_text("Break medium/long paths into Fitts-distanced submovements with waypoints")
                    .changed();

                if wobble_changed || curve_changed || jitter_changed || delay_changed || sub_changed {
                    if let Ok(mut s) = state.lock() {
                        s.macro_state.playback_options.smart_path.path_wobble = smart_path.path_wobble;
                        s.macro_state.playback_options.smart_path.path_curve = smart_path.path_curve;
                        s.macro_state.playback_options.smart_path.endpoint_jitter = smart_path.endpoint_jitter;
                        s.macro_state.playback_options.smart_path.segment_delay_threshold_ms = smart_path.segment_delay_threshold_ms;
                        s.macro_state.playback_options.smart_path.submovement_enabled = smart_path.submovement_enabled;
                    }
                    save_settings(state);
                }
            }
        });
        ui.end_row();
    });

    ui.add_space(12.0);
    egui::Frame::NONE.fill(palette.bg_element).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::same(10)).stroke(egui::Stroke::new(1.0_f32, palette.border))
            .show(ui, |ui| {
            ui.set_width(ui.available_width());

            ui.label(egui::RichText::new("Tip")
                .color(palette.accent_primary).size(11.0)
                .strong()
            );

            ui.add_space(4.0);

            ui.label(egui::RichText::new("Speed 1× plays at original recorded speed.\nSpeed 2× plays twice as fast. 0.5× plays at half speed.\nSet repeat to 0 for infinite looping.")
                .color(palette.text_muted)
                .size(10.0)
            );
    });
}
