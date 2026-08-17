pub mod actions;
pub mod context_menu;
pub mod find;
pub mod header;
pub mod inline;
pub mod marquee;
pub mod preview;
pub mod replace;
pub mod row;
pub mod shortcuts;

use crate::state::SharedState;
use crate::ui::IdeState;
use crate::ui::block_analysis::{BlockAnalysis, analyze_blocks};
use crate::ui::modals::Modal;
use crate::ui::theme::{ThemePalette, editor_bg_frame};
use eframe::egui;
use wmacro_core_types::MacroCommand;

use actions::{EditorActions, handle_editor_actions};
use context_menu::render_global_context_menu;
use header::render_editor_header;
use marquee::handle_marquee;
use row::render_command_row;
use shortcuts::handle_shortcuts;

/// which rows are rendered, and where, after applying block folds.
#[derive(Default)]
pub struct RowLayout {
    /// row indices that are not hidden inside a folded block.
    pub visible_rows: Vec<usize>,
    /// `visible_prefix[i]` = number of visible rows before index `i`, for mapping a scrolled-to row back to its pixel offset.
    pub visible_prefix: Vec<usize>,
}

pub(crate) fn compute_row_layout(
    commands: &[MacroCommand],
    analysis: &BlockAnalysis,
    folded: &std::collections::HashSet<usize>,
) -> RowLayout {
    let mut visible_rows = Vec::with_capacity(commands.len());
    let mut visible_prefix = vec![0; commands.len() + 1];
    let mut active_folds: Vec<usize> = Vec::new();

    for (idx, _) in commands.iter().enumerate() {
        visible_prefix[idx] = visible_rows.len();

        // TODO: track the topmost open fold directly instead of popping expired ends, so deep nesting does not re-scan the stack per row.
        while active_folds.last().is_some_and(|&end| end < idx) {
            active_folds.pop();
        }

        if !active_folds.is_empty() {
            continue;
        }

        visible_rows.push(idx);
        if folded.contains(&idx)
            && let Some(end) = analysis.fold_end[idx]
        {
            active_folds.push(end);
        }
    }
    visible_prefix[commands.len()] = visible_rows.len();

    RowLayout {
        visible_rows,
        visible_prefix,
    }
}

pub fn render_editor(ui: &mut egui::Ui, state: &SharedState, ide: &mut IdeState) {
    let palette = {
        let s = state.lock().unwrap_or_else(|e| {
            log::error!("State mutex poisoned: {e}");
            e.into_inner()
        });
        s.theme_manager.get_theme(&s.theme_name)
    };

    expire_row_flash(ui, ide);

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

            if ide.find_open {
                find::render_find_bar(ui, state, ide, &palette);
            }

            let editor_scroll_delta = compute_auto_scroll(ui, ide, &bg_response);

            let mut edit_modal: Option<Modal> = None;
            let mut actions = EditorActions::default();
            let mut visible_row_rects = Vec::new();
            let mut analysis = BlockAnalysis::default();
            let mut row_layout = RowLayout::default();

            let has_commands = render_command_list(
                ui,
                state,
                ide,
                &palette,
                editor_scroll_delta,
                &mut actions,
                &mut edit_modal,
                &mut visible_row_rects,
                &mut analysis,
                &mut row_layout,
            );

            inline::handle_outside_click(ui.ctx(), ide, &visible_row_rects);

            handle_marquee(
                ui,
                ide,
                &bg_response,
                &visible_row_rects,
                &palette,
                &row_layout,
            );

            bg_response.context_menu(|ui| {
                render_global_context_menu(ui, ide, &mut actions);
            });

            if !has_commands {
                render_empty_state(ui, &palette);
            }

            handle_editor_actions(state, ide, &actions);

            if let Some(m) = edit_modal {
                ide.inline_edit = None;
                ide.modal = m;
            }
        });
}

/// clears the append flash once it has faded, stamping the start time on the first frame.
fn expire_row_flash(ui: &egui::Ui, ide: &mut IdeState) {
    if ide.flash_rows.is_empty() {
        return;
    }

    let now = ui.input(|i| i.time);
    let start = *ide.flash_started_at.get_or_insert(now);
    if now - start >= row::FLASH_DURATION {
        ide.flash_rows.clear();
        ide.flash_started_at = None;
    } else {
        ui.ctx().request_repaint();
    }
}

fn compute_auto_scroll(ui: &egui::Ui, ide: &IdeState, bg_response: &egui::Response) -> f32 {
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
            // TODO: expose edge tolerance and scroll speed in settings; the magic numbers are tuned for a typical mouse on a laptop.
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
        if matches!(
            cmd,
            MacroCommand::EndIf | MacroCommand::EndLoop | MacroCommand::Else
        ) {
            depth = depth.saturating_sub(1);
        }
        levels.push(depth);
        if matches!(
            cmd,
            MacroCommand::IfPixelColor { .. }
                | MacroCommand::IfImageFound { .. }
                | MacroCommand::IfColorFound { .. }
                | MacroCommand::IfCompare { .. }
                | MacroCommand::Loop { .. }
                | MacroCommand::Else
        ) {
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
    visible_row_rects: &mut Vec<(usize, egui::Rect)>,
    analysis: &mut BlockAnalysis,
    row_layout: &mut RowLayout,
) -> bool {
    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });

    if let Some(row) = s.macro_state.appended_row.take() {
        ide.mark_row_appended(row);
    }

    let Some(m) = s.macro_state.current_macro.as_ref() else {
        return false;
    };

    if m.commands.is_empty() {
        return false;
    }

    let commands = &m.commands;
    *analysis = analyze_blocks(commands);
    *row_layout = compute_row_layout(commands, analysis, &ide.folded_blocks);
    let indent_levels = compute_indent_levels(commands);

    ui.style_mut().spacing.scroll.bar_width = 4.0;

    // must match the row height allocated in row::render_command_row.
    let row_stride = ui.spacing().interact_size.y * 2.0;

    let scroll_to_row = ide
        .pending_scroll_to_row
        .take()
        .map(|row| row.min(commands.len().saturating_sub(1)))
        .map(|row| {
            row_layout
                .visible_rows
                .iter()
                .rev()
                .find(|&&r| r <= row)
                .copied()
                .unwrap_or(row)
        });

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .scroll_source(
            egui::containers::scroll_area::ScrollSource::SCROLL_BAR
                | egui::containers::scroll_area::ScrollSource::MOUSE_WHEEL,
        )
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            if scroll_delta != 0.0 {
                ui.scroll_with_delta(egui::vec2(0.0, scroll_delta));
            }

            // TODO: virtualize this list for very large macros; every visible row still pays a full layout cost each frame.
            for &idx in &row_layout.visible_rows {
                render_command_row(
                    ui,
                    idx,
                    &commands[idx],
                    commands,
                    ide,
                    palette,
                    indent_levels[idx],
                    analysis,
                    actions,
                    edit_modal,
                    visible_row_rects,
                );
            }

            row::paint_indent_guides(ui, visible_row_rects, &indent_levels, palette);

            if let Some(row) = scroll_to_row {
                let row_top = row_layout.visible_prefix[row] as f32 * row_stride;
                let row_rect = egui::Rect::from_min_size(
                    egui::pos2(ui.max_rect().min.x, ui.max_rect().min.y + row_top),
                    egui::vec2(ui.max_rect().width(), row_stride),
                );
                ui.scroll_to_rect(row_rect, Some(egui::Align::Center));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn if_var() -> MacroCommand {
        MacroCommand::IfCompare {
            left: wmacro_core_types::Operand::Literal(wmacro_core_types::Value::Number(1)),
            op: wmacro_core_types::CompareOp::Gt,
            right: wmacro_core_types::Operand::Literal(wmacro_core_types::Value::Number(0)),
        }
    }

    #[test]
    fn indent_levels_treat_if_compare_as_opener() {
        let commands = vec![
            if_var(),
            if_var(),
            MacroCommand::Else,
            MacroCommand::EndIf,
            MacroCommand::EndIf,
        ];
        let levels = compute_indent_levels(&commands);
        assert_eq!(levels, vec![0, 1, 1, 1, 0]);
    }
}
