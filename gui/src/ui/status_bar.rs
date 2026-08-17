//! bottom status bar summarizing playback/recording state, step position, and block analysis of the current macro.

use crate::state::SharedState;
use crate::ui::block_analysis::{BlockAnalysis, analyze_blocks};
use crate::ui::editor::row::WARNING_AMBER;
use eframe::egui;
use egui_phosphor::regular;

pub fn render_status_bar(ui: &mut egui::Ui, state: &SharedState) {
    // snapshot everything under one lock so the bar shows one consistent
    // moment instead of mixing values from different frames.
    let (palette, msg, recording, playing, current_step, current_loop, ev_total, analysis) = {
        let Ok(s) = state.lock() else {
            log::error!("Failed to render status bar: state mutex is poisoned.");
            return;
        };

        let total = s
            .macro_state
            .current_macro
            .as_ref()
            .map_or(0, |m| m.commands.len());

        let analysis = s
            .macro_state
            .current_macro
            .as_ref()
            .map(|m| analyze_blocks(&m.commands))
            .unwrap_or_default();

        (
            s.theme_manager.get_theme(&s.theme_name),
            s.status_msg.clone(),
            s.macro_state.recording,
            s.macro_state.playing,
            s.macro_state.current_step,
            s.macro_state.current_loop,
            total,
            analysis,
        )
    };

    // TODO: crate central config for width, size, etc.
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

                ui.label(egui::RichText::new(icon).color(color).size(12.0));

                let display_msg = if playing && current_loop > 1 {
                    format!("{} (Loop {})", msg, current_loop)
                } else {
                    msg
                };

                ui.label(egui::RichText::new(display_msg).color(color).size(11.0));

                if playing && ev_total > 0 {
                    render_playback_progress(ui, current_step, ev_total, palette.text_muted);
                }

                render_block_problems(ui, &analysis);
            });
        });
}

fn render_block_problems(ui: &mut egui::Ui, analysis: &BlockAnalysis) {
    // broken block structure breaks playback, so surface it both here and on the offending rows in the editor.
    let unclosed = analysis.open_ifs + analysis.open_loops;
    let orphan_count = analysis.orphan_end.iter().filter(|&&o| o).count();

    if unclosed == 0 && orphan_count == 0 {
        return;
    }

    let mut parts: Vec<String> = Vec::new();
    if unclosed > 0 {
        parts.push(format!(
            "{unclosed} unclosed block{}",
            if unclosed == 1 { "" } else { "s" }
        ));
    }
    if orphan_count > 0 {
        parts.push(format!(
            "{orphan_count} orphan close{}",
            if orphan_count == 1 { "" } else { "s" }
        ));
    }

    let text = format!("{} {}", regular::WARNING, parts.join(" · "));

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(egui::RichText::new(text).color(WARNING_AMBER).size(11.0))
            .on_hover_text("Unclosed blocks and orphan End commands — marked in the command list");
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
