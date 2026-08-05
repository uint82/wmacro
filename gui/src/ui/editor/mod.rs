pub mod actions;
pub mod context_menu;
pub mod header;
pub mod marquee;
pub mod row;
pub mod shortcuts;

use crate::state::SharedState;
use crate::ui::IdeState;
use crate::ui::modals::Modal;
use crate::ui::theme::{ThemePalette, editor_bg_frame};
use wmacro_core_types::MacroCommand;
use eframe::egui;

use actions::{EditorActions, handle_editor_actions};
use context_menu::render_global_context_menu;
use header::render_editor_header;
use marquee::handle_marquee;
use row::render_command_row;
use shortcuts::handle_shortcuts;

pub fn render_editor(ui: &mut egui::Ui, state: &SharedState, ide: &mut IdeState) {
    let palette = {
        let s = state.lock().unwrap_or_else(|e| {
            log::error!("State mutex poisoned: {e}");
            e.into_inner()
        });
        s.theme_manager.get_theme(&s.theme_name)
    };

    if matches!(ide.modal, Modal::None) {
        handle_shortcuts(&ui.ctx().clone(), state, ide);
    }

    egui::CentralPanel::default()
        .frame(editor_bg_frame(&palette))
        .show_inside(ui, |ui| {
            render_editor_header(ui, state, &palette);

            let bg_response = ui.interact(
                ui.max_rect(),
                ui.id().with("editor_bg"),
                egui::Sense::click_and_drag(),
            );
            let editor_scroll_delta = compute_auto_scroll(ui, ide, &bg_response);

            let mut edit_modal: Option<Modal> = None;
            let mut actions = EditorActions::default();
            let mut total_rows = 0;
            let mut visible_row_rects = Vec::new();

            let has_commands = render_command_list(
                ui,
                state,
                ide,
                &palette,
                editor_scroll_delta,
                &mut actions,
                &mut edit_modal,
                &mut total_rows,
                &mut visible_row_rects,
            );

            handle_marquee(ui, ide, &bg_response, &visible_row_rects, &palette, total_rows);

            bg_response.context_menu(|ui| {
                render_global_context_menu(ui, ide, &mut actions);
            });

            if !has_commands {
                render_empty_state(ui, &palette);
            }

            handle_editor_actions(state, ide, &actions);

            if let Some(m) = edit_modal {
                ide.modal = m;
            }
        });
}

fn compute_auto_scroll(
    ui: &egui::Ui,
    ide: &IdeState,
    bg_response: &egui::Response,
) -> f32 {
    let is_dragging_item = egui::DragAndDrop::payload::<Vec<usize>>(ui.ctx()).is_some();
    let is_marquee =
        ide.selection_start_pos.is_some() && ui.ctx().input(|i| i.pointer.primary_down());

    if !is_dragging_item && !is_marquee {
        return 0.0;
    }

    let mut delta = 0.0_f32;
    ui.ctx().input(|i| {
        delta += i.smooth_scroll_delta.y;
        if let Some(ptr) = i.pointer.interact_pos() {
            const EDGE_TOLERANCE: f32 = 40.0;
            const SCROLL_SPEED: f32 = 14.0;
            if ptr.y < bg_response.rect.top() + EDGE_TOLERANCE {
                delta += SCROLL_SPEED;
            } else if ptr.y > bg_response.rect.bottom() - EDGE_TOLERANCE {
                delta -= SCROLL_SPEED;
            }
        }
    });

    if delta != 0.0 {
        ui.ctx().request_repaint();
    }
    delta
}

fn compute_indent_levels(commands: &[MacroCommand]) -> Vec<usize> {
    let mut levels = Vec::with_capacity(commands.len());
    let mut depth: usize = 0;

    for cmd in commands {
        if matches!(cmd, MacroCommand::EndIf | MacroCommand::EndLoop | MacroCommand::Else) {
            depth = depth.saturating_sub(1);
        }
        levels.push(depth);
        if matches!(cmd, MacroCommand::IfPixelColor { .. } | MacroCommand::IfImageFound { .. } | MacroCommand::Loop { .. } | MacroCommand::Else) {
            depth += 1;
        }
    }

    levels
}

#[allow(clippy::too_many_arguments)]
fn render_command_list(
    ui: &mut egui::Ui,
    state: &SharedState,
    ide: &mut IdeState,
    palette: &ThemePalette,
    scroll_delta: f32,
    actions: &mut EditorActions,
    edit_modal: &mut Option<Modal>,
    total_rows: &mut usize,
    visible_row_rects: &mut Vec<(usize, egui::Rect)>,
) -> bool {
    let s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });

    let Some(m) = s.macro_state.current_macro.as_ref() else {
        return false;
    };

    *total_rows = m.commands.len();
    if m.commands.is_empty() {
        return false;
    }

    let commands = &m.commands;
    let indent_levels = compute_indent_levels(commands);

    ui.style_mut().spacing.scroll.bar_width = 4.0;

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .scroll_source(
            egui::containers::scroll_area::ScrollSource::SCROLL_BAR
                | egui::containers::scroll_area::ScrollSource::MOUSE_WHEEL,
        )
        .show_rows(ui, 32.0, commands.len(), |ui, row_range| {
            ui.spacing_mut().item_spacing.y = 0.0;

            if scroll_delta != 0.0 {
                ui.scroll_with_delta(egui::vec2(0.0, scroll_delta));
            }

            for idx in row_range {
                render_command_row(
                    ui,
                    idx,
                    &commands[idx],
                    commands,
                    ide,
                    palette,
                    indent_levels[idx],
                    actions,
                    edit_modal,
                    visible_row_rects,
                );
            }
        });

    true
}

fn render_empty_state(ui: &mut egui::Ui, palette: &ThemePalette) {
    let available = ui.available_size();
    ui.allocate_ui_with_layout(
        available,
        egui::Layout::centered_and_justified(egui::Direction::TopDown),
        |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(available.y * 0.25);
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::RECORD)
                        .size(48.0)
                        .color(palette.text_muted),
                );
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("No commands yet")
                        .size(18.0)
                        .color(palette.text_muted),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "Press Record to capture input, or use the\n\
                         Add Action panel to build a macro manually.",
                    )
                    .size(12.0)
                    .color(egui::Color32::from_rgb(90, 90, 110)),
                );
            });
        },
    );
}
