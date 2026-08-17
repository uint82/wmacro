//! shared drawing code for alert dialogs: backdrop, window frame, and dismiss handling.

use crate::state::{ModalAction, ModalAlert};
use crate::ui::modals::{apply_modal_visuals, draw_modal_backdrop};
use crate::ui::theme::ThemePalette;
use eframe::egui;

pub fn draw_base_alert(
    ctx: &egui::Context,
    alert: &mut ModalAlert,
    title_icon: &str,
    palette: &ThemePalette,
) -> (bool, Option<ModalAction>) {
    let clicked_outside = draw_modal_backdrop(ctx);
    let mut close = alert.dismissible && clicked_outside;
    let mut action_to_execute = None;

    egui::Window::new(format!("{} {}", title_icon, alert.title))
        .id(egui::Id::new("global_alert_modal_v4"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(crate::ui::theme::modal_frame(palette))
        .order(egui::Order::Foreground)
        .max_width(480.0)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);

            let saved_visuals = apply_modal_visuals(ui, palette);

            ui.add_space(8.0);
            ui.label(egui::RichText::new(&alert.message).color(palette.text_muted));

        if let Some(note_text) = &alert.note {
                ui.add_space(12.0);
                egui::Frame::NONE
                    .fill(palette.bg_element)
                    .inner_margin(12.0)
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(note_text)
                                    .color(palette.text_primary)
                                    .family(egui::FontFamily::Monospace)
                                    .size(12.0),
                            );

                            let available = ui.available_width();
                            if available > 32.0 {
                                ui.add_space(available - 32.0);
                            }

                            let copied = alert
                                .copied_at
                                .map(|t| t.elapsed().as_secs() < 2)
                                .unwrap_or(false);

                            let (btn_icon, hover_text, color) = if copied {
                                (
                                    egui_phosphor::regular::CHECK,
                                    "Copied!",
                                    palette.accent_success,
                                )
                            } else {
                                (egui_phosphor::regular::COPY, "Copy", palette.text_primary)
                            };

                            if ui
                                .button(egui::RichText::new(btn_icon).size(16.0).color(color))
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(hover_text)
                                .clicked()
                            {
                                let cmd = note_text.lines().last().unwrap_or(note_text).to_string();
                                ui.ctx().copy_text(cmd);
                                alert.copied_at = Some(std::time::Instant::now());
                            }
                        });
                    });
            }

            if let Some(footer) = &alert.footer_note {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(footer)
                        .color(palette.text_muted)
                        .size(11.0),
                );
            }

            ui.add_space(12.0);

            if !alert.actions.is_empty() {
                ui.horizontal(|ui| {
                    let approx_btn_width = 88.0;
                    let gap = 8.0;
                    let n = alert.actions.len() as f32;
                    let total_width = n * approx_btn_width + (n - 1.0) * gap;
                    let available = ui.available_width();

                    if available > total_width {
                        ui.add_space(available - total_width);
                    }

                    let mut action_index = alert.actions.len();
                    while action_index > 0 {
                        action_index -= 1;
                        let (btn_label, action) = &alert.actions[action_index];

                        let (bg_color, text_color) = match action {
                            ModalAction::SaveAndQuit => (palette.bg_element, palette.text_primary),
                            ModalAction::QuitWithoutSaving | ModalAction::Quit => {
                                (palette.accent_danger, palette.accent_danger_fg)
                            }
                            _ => (palette.bg_element, palette.text_primary),
                        };

                        let saved_visuals = ui.visuals().clone();
                        ui.visuals_mut().widgets.inactive.weak_bg_fill = bg_color;
                        ui.visuals_mut().widgets.hovered.weak_bg_fill =
                            bg_color.linear_multiply(1.2);
                        ui.visuals_mut().widgets.active.weak_bg_fill =
                            bg_color.linear_multiply(0.8);

                        let btn = egui::Button::new(
                            egui::RichText::new(btn_label).strong().color(text_color),
                        )
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2));

                        if ui
                            .add(btn)
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            action_to_execute = Some(action.clone());
                            close = true;
                        }

                        *ui.visuals_mut() = saved_visuals;
                        if action_index > 0 {
                            ui.add_space(gap);
                        }
                    }
                });
            }

            // TODO: unify this per-button visual override with the commit buttons in the other modals.
            if alert.dismissible && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }

            *ui.visuals_mut() = saved_visuals;
        });

    (close, action_to_execute)
}
