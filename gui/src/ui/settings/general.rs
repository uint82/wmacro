use super::save_settings;
use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;

pub fn render(ui: &mut egui::Ui, state: &SharedState, palette: &ThemePalette) {
    // TODO: add an accent color picker so users can customize beyond the built-in themes.
    ui.label(
        egui::RichText::new("Application-wide settings and appearance.")
            .color(palette.text_muted)
            .size(11.0),
    );
    ui.add_space(10.0);

    egui::Grid::new("general_grid")
        .num_columns(2)
        .spacing([16.0, 12.0])
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Theme")
                    .color(palette.text_primary)
                    .size(13.0),
            );

            let (current_theme, available_themes) = {
                let Ok(state_guard) = state.lock() else {
                    return;
                };
                (
                    state_guard.theme_name.clone(),
                    state_guard.theme_manager.available_themes.clone(),
                )
            };

            let mut selected_theme = current_theme.clone();
            let mut changed = false;

            ui.scope(|ui| {
                let visuals = ui.visuals_mut();
                visuals.widgets.inactive.weak_bg_fill = palette.bg_element_alt;
                visuals.widgets.inactive.fg_stroke.color = palette.text_primary;
                visuals.widgets.hovered.weak_bg_fill = palette.bg_element;
                visuals.widgets.hovered.fg_stroke.color = palette.text_primary;
                visuals.widgets.active.weak_bg_fill = palette.bg_element;
                visuals.widgets.active.fg_stroke.color = palette.text_primary;
                visuals.widgets.open.weak_bg_fill = palette.bg_element;
                visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.open.bg_stroke = egui::Stroke::NONE;

                egui::ComboBox::from_id_salt("theme_selector")
                    .selected_text(egui::RichText::new(&selected_theme).color(palette.text_primary))
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for theme in available_themes {
                            ui.selectable_value(
                                &mut selected_theme,
                                theme.name.clone(),
                                &theme.name,
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        }
                    });
            });

            if selected_theme != current_theme
                && let Ok(mut state_guard) = state.lock()
            {
                state_guard.theme_name = selected_theme.clone();
                let new_palette = state_guard.theme_manager.get_theme(&selected_theme);
                ui.ctx().set_visuals(new_palette.to_egui_visuals());
                changed = true;
            }

            if changed {
                save_settings(state);
            }
            ui.end_row();
        });
}
