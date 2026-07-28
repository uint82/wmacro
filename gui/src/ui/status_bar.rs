use egui_phosphor::regular;
use crate::state::SharedState;
use eframe::egui;

pub fn render_status_bar(ui: &mut egui::Ui, state: &SharedState) {
    let (palette, msg, recording, playing, current_step, current_loop, ev_total) = {
        let Ok(s) = state.lock() else {
            log::error!("Failed to render status bar: state mutex is poisoned.");
            return;
        };

        let total = s
            .macro_state
            .current_macro
            .as_ref()
            .map_or(0, |m| m.commands.len());

        (
            s.theme_manager.get_theme(&s.theme_name),
            s.status_msg.clone(),
            s.macro_state.recording,
            s.macro_state.playing,
            s.macro_state.current_step,
            s.macro_state.current_loop,
            total,
        )
    };

    // TODO: crate central config for width, size, etc
    egui::Panel::bottom("ide_status")
        .frame(
            egui::Frame::NONE
                .fill(palette.bg_surface)
                .inner_margin(egui::Margin::symmetric(12, 4))
                .stroke(egui::Stroke::new(1.0_f32, palette.border)),
        )
        .show_inside(ui, |ui| {
            let color = if recording {
                palette.accent_danger
            } else if playing {
                palette.accent_success
            } else {
                palette.text_muted
            };

            ui.horizontal(|ui| {
                let icon = if recording || playing {
                    regular::RECORD
                } else {
                    regular::CIRCLE
                };

                ui.label(
                    egui::RichText::new(icon)
                        .color(color)
                        .size(12.0),
                );

                let display_msg = if playing && current_loop > 1 {
                    format!("{} (Loop {})", msg, current_loop)
                } else {
                    msg
                };

                ui.label(egui::RichText::new(display_msg).color(color).size(11.0));

                if playing && ev_total > 0 {
                    render_playback_progress(ui, current_step, ev_total, palette.text_muted);
                }
            });
        });
}

fn render_playback_progress(
    ui: &mut egui::Ui,
    current_step: usize,
    total_steps: usize,
    text_color: egui::Color32,
) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(
            egui::RichText::new(format!("{} / {} events", current_step, total_steps))
                .color(text_color)
                .size(10.0),
        );

        let progress = current_step as f32 / total_steps as f32;
        ui.add(egui::ProgressBar::new(progress).desired_width(200.0));
    });
}
