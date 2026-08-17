use crate::macro_engine::script::format_operand;
use crate::ui::block_analysis::BlockAnalysis;
use crate::ui::components::{badge, event_display_info, format_coord_display};
use crate::ui::editor::IdeState;
use crate::ui::editor::actions::EditorActions;
use crate::ui::editor::context_menu::render_context_menu;
use crate::ui::editor::inline;
use crate::ui::editor::preview;
use crate::ui::modals::{Modal, modal_from_command};
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::MacroCommand;

const INDENT_STEP_PX: f32 = 22.0;
const ROW_NUMBER_PX: f32 = 24.0;
const ROW_GAP_PX: f32 = 8.0;
/// reserved gutter for fold chevrons; every row reserves it so content stays column-aligned.
pub(crate) const FOLD_GUTTER_PX: f32 = 18.0;
/// how long freshly appended rows stay highlighted (seconds).
pub(crate) const FLASH_DURATION: f64 = 0.9;
pub const WARNING_AMBER: egui::Color32 = egui::Color32::from_rgb(215, 153, 33);

#[derive(PartialEq, Clone, Copy)]
enum DetailClick {
    Single,
    Double,
}

struct RowDisplayInfo {
    icon: &'static str,
    badge_label: &'static str,
    badge_color: egui::Color32,
    detail: String,
}

fn display_info_for(cmd: &MacroCommand, palette: &ThemePalette) -> RowDisplayInfo {
    let (icon, badge_label, badge_color, detail) = match cmd {
        MacroCommand::Action(act) => event_display_info(act, palette),
        MacroCommand::IfPixelColor {
            x,
            y,
            r,
            g,
            b,
            tolerance,
        } => (
            egui_phosphor::regular::PALETTE,
            "IF PIXEL COLOR",
            palette.col_if,
            format!(
                "x = {}   y = {}   color = #{:02X}{:02X}{:02X}{}",
                format_coord_display(x),
                format_coord_display(y),
                r,
                g,
                b,
                if *tolerance > 0 {
                    format!("   tol = {}%", tolerance)
                } else {
                    "".to_string()
                }
            ),
        ),
        MacroCommand::IfImageFound {
            target_image_path,
            similarity_threshold,
            store_x,
            store_y,
            ..
        } => {
            let store_str = match (store_x, store_y) {
                (Some(sx), Some(sy)) => format!("   -> {} = x, {} = y", sx, sy),
                (Some(sx), None) => format!("   -> {} = x", sx),
                (None, Some(sy)) => format!("   -> {} = y", sy),
                (None, None) => String::new(),
            };
            (
                egui_phosphor::regular::IMAGE,
                "IF IMAGE FOUND",
                palette.col_if,
                format!(
                    "target = {}   tol = {:.2}{}",
                    std::path::Path::new(target_image_path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    similarity_threshold,
                    store_str
                ),
            )
        }
        MacroCommand::IfColorFound {
            region,
            r,
            g,
            b,
            tolerance,
            min_width,
            min_height,
            store_x,
            store_y,
            store_w,
            store_h,
            ..
        } => {
            let region_str = match region {
                Some((l, t, w, h)) => format!("   region = {},{},{}x{}", l, t, w, h),
                None => "   region = whole screen".to_string(),
            };
            let store_str = match (store_x, store_y, store_w, store_h) {
                (Some(sx), Some(sy), sw, sh) => format!(
                    "   -> {} = x, {} = y{}{}",
                    sx,
                    sy,
                    sw.as_deref()
                        .map(|w| format!(", {} = w", w))
                        .unwrap_or_default(),
                    sh.as_deref()
                        .map(|h| format!(", {} = h", h))
                        .unwrap_or_default()
                ),
                (Some(sx), None, sw, sh) => format!(
                    "   -> {} = x{}{}",
                    sx,
                    sw.as_deref()
                        .map(|w| format!(", {} = w", w))
                        .unwrap_or_default(),
                    sh.as_deref()
                        .map(|h| format!(", {} = h", h))
                        .unwrap_or_default()
                ),
                (None, Some(sy), sw, sh) => format!(
                    "   -> {} = y{}{}",
                    sy,
                    sw.as_deref()
                        .map(|w| format!(", {} = w", w))
                        .unwrap_or_default(),
                    sh.as_deref()
                        .map(|h| format!(", {} = h", h))
                        .unwrap_or_default()
                ),
                (None, None, _, _) => String::new(),
            };
            (
                egui_phosphor::regular::CHECKERBOARD,
                "IF COLOR FOUND",
                palette.col_if,
                format!(
                    "color = #{:02X}{:02X}{:02X}   tol = {}%   min = {}x{} px{}{}",
                    r, g, b, tolerance, min_width, min_height, region_str, store_str
                ),
            )
        }
        MacroCommand::Else => (
            egui_phosphor::regular::ARROWS_LEFT_RIGHT,
            "ELSE",
            palette.col_else,
            "Fallback execution block".to_string(),
        ),
        MacroCommand::EndIf => (
            egui_phosphor::regular::STOP_CIRCLE,
            "END IF",
            palette.col_end_if,
            "End of If block".to_string(),
        ),
        MacroCommand::Loop { count } => (
            egui_phosphor::regular::REPEAT,
            "LOOP",
            palette.col_loop,
            format!("{} times", format_operand(count)),
        ),
        MacroCommand::EndLoop => (
            egui_phosphor::regular::ARROW_U_UP_LEFT,
            "END LOOP",
            palette.col_end_loop,
            "End of Loop block".to_string(),
        ),
        MacroCommand::PlayMacro(path) => (
            egui_phosphor::regular::PLAY,
            "PLAY MACRO",
            palette.col_import_saved_macro,
            path.clone(),
        ),
        MacroCommand::Label(name) => (
            egui_phosphor::regular::TAG,
            "LABEL",
            palette.col_label,
            name.clone(),
        ),
        MacroCommand::Goto(target) => (
            egui_phosphor::regular::LINK,
            "GOTO",
            palette.col_goto,
            format!("-> {}", target),
        ),
        MacroCommand::TypeText(text) => (
            egui_phosphor::regular::TEXT_T,
            "TYPE TEXT",
            palette.col_type_text,
            text.clone(),
        ),
        MacroCommand::OpenFile {
            path,
            args,
            run_as_admin,
        } => (
            egui_phosphor::regular::FILE_ARROW_UP,
            "OPEN FILE",
            palette.col_import_saved_macro,
            {
                let name = std::path::Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();

                let admin_tag = if *run_as_admin { " (sudo)" } else { "" };
                let trimmed_args = args.trim();

                if trimmed_args.is_empty() {
                    format!("{}{}", name, admin_tag)
                } else {
                    format!("{} {}{}", name, trimmed_args, admin_tag)
                }
            },
        ),
        MacroCommand::SetVariable { target, value } => (
            egui_phosphor::regular::FUNCTION,
            "SET VAR",
            palette.col_var,
            format!("{} = {}", target, format_operand(value)),
        ),
        MacroCommand::Calculate { target, expression } => (
            egui_phosphor::regular::CALCULATOR,
            "CALCULATE",
            palette.col_calc,
            format!("{} = {}", target, expression),
        ),
        MacroCommand::IfCompare { left, op, right } => (
            egui_phosphor::regular::FLOW_ARROW,
            "IF VAR",
            palette.col_if,
            format!(
                "{} {} {}",
                format_operand(left),
                op.symbol(),
                format_operand(right)
            ),
        ),
        MacroCommand::Delay { duration_ms } => (
            egui_phosphor::regular::TIMER,
            "DELAY",
            palette.col_delay,
            match duration_ms {
                wmacro_core_types::Operand::Literal(wmacro_core_types::Value::Number(ms)) => {
                    crate::ui::modals::variable::format_duration_string(*ms as u64)
                }
                wmacro_core_types::Operand::Var(name) => format!("${} delay", name),
                wmacro_core_types::Operand::Literal(wmacro_core_types::Value::Text(_))
                | wmacro_core_types::Operand::Literal(wmacro_core_types::Value::Float(_)) => {
                    format_operand(duration_ms)
                }
            },
        ),
        MacroCommand::SetClipboard { text } => (
            egui_phosphor::regular::CLIPBOARD_TEXT,
            "SET CLIPBOARD",
            palette.col_clipboard,
            format_operand(text),
        ),
        MacroCommand::GetClipboard { target } => (
            egui_phosphor::regular::CLIPBOARD,
            "GET CLIPBOARD",
            palette.col_clipboard,
            format!("${} = clipboard", target),
        ),
        MacroCommand::Comment(text) => (
            egui_phosphor::regular::NOTE,
            "NOTE",
            palette.text_muted,
            text.clone(),
        ),
    };

    RowDisplayInfo {
        icon,
        badge_label,
        badge_color,
        detail,
    }
}

pub fn range_select(ide: &mut IdeState, target: usize) {
    if let Some(anchor) = ide.last_clicked_idx {
        let lo = anchor.min(target);
        let hi = anchor.max(target);
        for i in lo..=hi {
            ide.selected.insert(i);
        }
    } else {
        ide.selected.insert(target);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_command_row(
    ui: &mut egui::Ui,
    idx: usize,
    cmd: &MacroCommand,
    commands: &[MacroCommand],
    ide: &mut IdeState,
    palette: &ThemePalette,
    indent_level: usize,
    analysis: &BlockAnalysis,
    actions: &mut EditorActions,
    edit_modal: &mut Option<Modal>,
    visible_row_rects: &mut Vec<(usize, egui::Rect)>,
) {
    let info = display_info_for(cmd, palette);
    let is_selected = ide.selected.contains(&idx);
    let item_id = egui::Id::new("dnd_ev").with(idx);

    let row_width = ui.available_width() - 30.0;
    let row_height = ui.spacing().interact_size.y * 2.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(row_width, row_height), egui::Sense::hover());
    let row_interact = ui.interact(rect, item_id, egui::Sense::click_and_drag());

    handle_drag_start(ui, &row_interact, ide, idx, is_selected);
    handle_drop(ui, &row_interact, rect, actions, idx);

    let flash_alpha = if ide.flash_rows.contains(&idx) {
        flash_alpha(ui, ide)
    } else {
        0.0
    };
    paint_row_background(
        ui,
        rect,
        is_selected,
        row_interact.hovered(),
        palette,
        flash_alpha,
    );
    let (chevron_clicked, detail_click) =
        paint_row_content(ui, rect, idx, indent_level, &info, palette, analysis, ide);
    visible_row_rects.push((idx, rect));

    paint_drop_indicator(ui, rect, idx, palette);
    paint_drag_overlay(ui, rect, item_id, idx, palette);

    if row_interact.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }

    if chevron_clicked {
        toggle_fold(ide, idx);
    } else if let Some(click) = detail_click {
        handle_detail_click(ui, ide, idx, cmd, click, edit_modal);
    } else {
        handle_click(ui, &row_interact, ide, idx, cmd, edit_modal);
    }

    if row_interact.hovered() && !row_interact.dragged() {
        let preview_text = preview::row_preview(cmd);
        if !preview_text.is_empty() {
            row_interact.clone().on_hover_text(preview_text);
        }
    }

    inline::render_popup(ui, ide, palette, idx, &row_interact, actions);

    row_interact.context_menu(|ui| {
        render_context_menu(
            ui, ide, idx, palette, edit_modal, cmd, commands, actions, analysis,
        );
    });
}

fn flash_alpha(ui: &egui::Ui, ide: &IdeState) -> f32 {
    let Some(start) = ide.flash_started_at else {
        return 1.0;
    };
    let elapsed = ui.input(|i| i.time) - start;
    if elapsed >= FLASH_DURATION {
        return 0.0;
    }
    (1.0 - (elapsed / FLASH_DURATION) as f32).powf(1.5)
}

/// selection logic plus opening the inline editor or full modal, depending on the detail click.
fn handle_detail_click(
    ui: &egui::Ui,
    ide: &mut IdeState,
    idx: usize,
    cmd: &MacroCommand,
    click: DetailClick,
    edit_modal: &mut Option<Modal>,
) {
    match click {
        DetailClick::Double => {
            if let Some(m) = modal_from_command(cmd, idx) {
                *edit_modal = Some(m);
            }
        }
        DetailClick::Single => {
            apply_click_selection(ui, ide, idx);
            if let Some((_, value)) = inline::edit_field(&mut cmd.clone(), None) {
                ide.inline_edit = Some((idx, value));
            }
        }
    }
}

fn apply_click_selection(ui: &egui::Ui, ide: &mut IdeState, idx: usize) {
    let ctrl = ui.ctx().input(|i| i.modifiers.ctrl);
    let shift = ui.ctx().input(|i| i.modifiers.shift);

    if shift {
        range_select(ide, idx);
    } else if ctrl {
        if ide.selected.contains(&idx) {
            ide.selected.remove(&idx);
        } else {
            ide.selected.insert(idx);
        }
    } else if ide.selected.contains(&idx) && ide.selected.len() == 1 {
        ide.selected.clear();
    } else {
        ide.selected.clear();
        ide.selected.insert(idx);
    }

    ide.last_clicked_idx = Some(idx);
}

fn handle_drag_start(
    ui: &egui::Ui,
    response: &egui::Response,
    ide: &IdeState,
    idx: usize,
    is_selected: bool,
) {
    if !response.drag_started() {
        return;
    }

    let payload = if is_selected {
        // drag the whole selection, like picking up a stack of papers by the top sheet.
        let mut sel: Vec<usize> = ide.selected.iter().copied().collect();
        sel.sort_unstable();
        sel
    } else {
        vec![idx]
    };
    egui::DragAndDrop::set_payload(ui.ctx(), payload);
}

fn handle_drop(
    ui: &egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
    actions: &mut EditorActions,
    idx: usize,
) {
    let is_drag_hovered =
        ui.rect_contains_pointer(rect) && egui::DragAndDrop::has_any_payload(ui.ctx());

    if is_drag_hovered
        && ui.ctx().input(|i| i.pointer.any_released())
        && let Some(payload) = egui::DragAndDrop::take_payload::<Vec<usize>>(ui.ctx())
    {
        actions.move_payload = Some(((*payload).clone(), idx));
    }

    let _ = response;
}

fn paint_row_background(
    ui: &egui::Ui,
    rect: egui::Rect,
    is_selected: bool,
    is_hovered: bool,
    palette: &ThemePalette,
    flash_alpha: f32,
) {
    let bg_color = if is_selected {
        palette.bg_element_alt.linear_multiply(0.9)
    } else if is_hovered {
        palette.bg_element
    } else {
        palette.bg_base
    };

    if bg_color != egui::Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 0.0, bg_color);
    }

    if flash_alpha > 0.0 {
        ui.painter().rect_filled(
            rect,
            0.0,
            palette.accent_primary.linear_multiply(flash_alpha * 0.22),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_row_content(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    idx: usize,
    indent_level: usize,
    info: &RowDisplayInfo,
    palette: &ThemePalette,
    analysis: &BlockAnalysis,
    ide: &mut IdeState,
) -> (bool, Option<DetailClick>) {
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child_ui.style_mut().interaction.selectable_labels = false;

    let chevron_clicked = paint_fold_chevron(&mut child_ui, rect, idx, analysis, ide, palette);
    paint_row_number(&mut child_ui, rect, idx, palette);
    child_ui.add_space(ROW_GAP_PX);
    child_ui.add_space(indent_level as f32 * INDENT_STEP_PX);
    paint_badge_and_icon(&mut child_ui, rect, info);
    child_ui.add_space(8.0);
    let detail_click = paint_detail_text(&mut child_ui, &info.detail, palette);
    paint_block_warnings(&mut child_ui, rect, idx, analysis, palette);

    (chevron_clicked, detail_click)
}

fn paint_fold_chevron(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    idx: usize,
    analysis: &BlockAnalysis,
    ide: &IdeState,
    palette: &ThemePalette,
) -> bool {
    let is_foldable = analysis.fold_end.get(idx).copied().flatten().is_some();
    let sense = if is_foldable {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };

    let (chevron_rect, response) =
        ui.allocate_exact_size(egui::vec2(FOLD_GUTTER_PX, rect.height()), sense);

    if !is_foldable {
        return false;
    }

    let folded = ide.folded_blocks.contains(&idx);
    let icon = if folded {
        egui_phosphor::regular::CARET_RIGHT
    } else {
        egui_phosphor::regular::CARET_DOWN
    };

    let mut chevron_ui = ui.new_child(egui::UiBuilder::new().max_rect(chevron_rect).layout(
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
    ));
    chevron_ui.label(
        egui::RichText::new(icon)
            .color(if folded {
                palette.accent_primary
            } else {
                palette.text_muted
            })
            .size(12.0),
    );
    let clicked = response.clicked();
    response.on_hover_text(if folded {
        "Expand block"
    } else {
        "Collapse block"
    });

    clicked
}

pub(crate) fn toggle_fold(ide: &mut IdeState, idx: usize) {
    if ide.folded_blocks.contains(&idx) {
        ide.folded_blocks.remove(&idx);
    } else {
        ide.folded_blocks.insert(idx);
    }
}

fn paint_block_warnings(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    idx: usize,
    analysis: &BlockAnalysis,
    palette: &ThemePalette,
) {
    let is_orphan = analysis.orphan_end.get(idx).copied().unwrap_or(false);
    let is_unclosed = analysis.unclosed_opener.get(idx).copied().unwrap_or(false);

    let (icon, color, tooltip) = if is_orphan {
        (
            egui_phosphor::regular::WARNING,
            palette.accent_danger,
            "No matching opening command for this block close",
        )
    } else if is_unclosed {
        (
            egui_phosphor::regular::WARNING,
            WARNING_AMBER,
            "This block is never closed",
        )
    } else {
        return;
    };

    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 26.0, rect.center().y - 8.0),
        egui::vec2(16.0, 16.0),
    );
    let hover = ui.allocate_rect(icon_rect, egui::Sense::hover());
    ui.painter().text(
        icon_rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        color,
    );
    hover.on_hover_text(tooltip);
}

fn paint_row_number(ui: &mut egui::Ui, rect: egui::Rect, idx: usize, palette: &ThemePalette) {
    let (num_rect, _) =
        ui.allocate_exact_size(egui::vec2(24.0, rect.height()), egui::Sense::hover());
    let mut num_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(num_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    num_ui.label(
        egui::RichText::new(format!("{}", idx + 1))
            .color(palette.text_muted)
            .size(11.0)
            .monospace(),
    );
}

pub(crate) fn search_text(cmd: &MacroCommand, palette: &ThemePalette) -> String {
    let info = display_info_for(cmd, palette);
    format!("{} {}", info.badge_label, info.detail).to_lowercase()
}

/// paints indent guide lines as one continuous segment per level, spanning
/// rows hidden by folds so the lines stay unbroken.
pub(crate) fn paint_indent_guides(
    ui: &egui::Ui,
    visible_row_rects: &[(usize, egui::Rect)],
    indent_levels: &[usize],
    palette: &ThemePalette,
) {
    let Some((_, first_rect)) = visible_row_rects.first() else {
        return;
    };
    let max_level = indent_levels.iter().copied().max().unwrap_or(0);
    let x_base = first_rect.min.x + FOLD_GUTTER_PX + ROW_NUMBER_PX + ROW_GAP_PX;
    let stroke = egui::Stroke::new(1.0_f32, palette.border.linear_multiply(0.6));

    for level in 1..=max_level {
        let mut start = 0;
        while start < indent_levels.len() {
            if indent_levels[start] < level {
                start += 1;
                continue;
            }

            let mut end = start;
            while end < indent_levels.len() && indent_levels[end] >= level {
                end += 1;
            }

            // TODO: collect visible runs in place instead of allocating this Vec per level per frame; it is hot for long macros.
            let visible_in_run: Vec<&(usize, egui::Rect)> = visible_row_rects
                .iter()
                .filter(|(idx, _)| *idx >= start && *idx < end)
                .collect();
            if let (Some((_, top_rect)), Some((_, bottom_rect))) =
                (visible_in_run.first(), visible_in_run.last())
            {
                let x_line = x_base + level as f32 * INDENT_STEP_PX;
                ui.painter()
                    .vline(x_line, top_rect.top()..=bottom_rect.bottom(), stroke);
            }

            start = end;
        }
    }
}

fn paint_badge_and_icon(ui: &mut egui::Ui, rect: egui::Rect, info: &RowDisplayInfo) {
    ui.allocate_ui_with_layout(
        egui::vec2(111.0, rect.height()),
        egui::Layout::left_to_right(egui::Align::Center),
        |c_ui| {
            let (icon_rect, _) = c_ui.allocate_exact_size(
                egui::vec2(22.0, c_ui.available_height()),
                egui::Sense::hover(),
            );
            let mut icon_ui = c_ui.new_child(egui::UiBuilder::new().max_rect(icon_rect).layout(
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            ));
            icon_ui.label(
                egui::RichText::new(info.icon)
                    .color(info.badge_color)
                    .size(15.0),
            );
            c_ui.add_space(4.0);
            badge(c_ui, info.badge_label, info.badge_color, Some(85.0));
        },
    );
}

fn paint_detail_text(
    ui: &mut egui::Ui,
    detail: &str,
    palette: &ThemePalette,
) -> Option<DetailClick> {
    let detail_single_line = detail.replace('\n', " ↵ ");
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |c_ui| {
        let response = c_ui
            .add(
                egui::Label::new(
                    egui::RichText::new(&detail_single_line)
                        .color(palette.text_primary)
                        .size(13.0),
                )
                .truncate(),
            )
            .interact(egui::Sense::click());

        let click = if response.double_clicked() {
            Some(DetailClick::Double)
        } else if response.clicked() {
            Some(DetailClick::Single)
        } else {
            None
        };

        response.clone().on_hover_cursor(egui::CursorIcon::Text);
        click
    })
    .inner
}

fn paint_drop_indicator(ui: &egui::Ui, rect: egui::Rect, idx: usize, palette: &ThemePalette) {
    let is_drag_hovered =
        ui.rect_contains_pointer(rect) && egui::DragAndDrop::has_any_payload(ui.ctx());

    if !is_drag_hovered {
        return;
    }

    let Some(payload) = egui::DragAndDrop::payload::<Vec<usize>>(ui.ctx()) else {
        return;
    };

    let is_drag_down = payload.first().is_some_and(|&src| src < idx);
    let line_y = if is_drag_down {
        rect.bottom() - 1.0
    } else {
        rect.top() + 1.0
    };
    ui.painter().hline(
        rect.x_range(),
        line_y,
        egui::Stroke::new(2.0_f32, palette.accent_primary),
    );
}

fn paint_drag_overlay(
    ui: &egui::Ui,
    rect: egui::Rect,
    item_id: egui::Id,
    idx: usize,
    palette: &ThemePalette,
) {
    let dragging_payload =
        egui::DragAndDrop::payload::<Vec<usize>>(ui.ctx()).map(|arc| (*arc).clone());
    let is_dragging_this = dragging_payload.as_ref().is_some_and(|p| p.contains(&idx));

    if !is_dragging_this {
        return;
    }

    ui.painter().rect(
        rect,
        0.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10),
        egui::Stroke::NONE,
        egui::StrokeKind::Inside,
    );

    if ui.ctx().is_being_dragged(item_id)
        && let Some(payload) = &dragging_payload
        && payload.len() > 1
    {
        paint_multi_drag_badge(ui, rect, payload.len(), palette);
    }
}

fn paint_multi_drag_badge(ui: &egui::Ui, rect: egui::Rect, count: usize, palette: &ThemePalette) {
    let badge_text = format!("{}  {} items", egui_phosphor::regular::STACK, count);
    let font_id = egui::FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(badge_text, font_id, egui::Color32::WHITE);
    let padding = egui::vec2(10.0, 5.0);
    let badge_size = galley.size() + padding * 2.0;
    let badge_pos = egui::pos2(
        rect.right() - badge_size.x - 12.0,
        rect.center().y - badge_size.y / 2.0,
    );
    let badge_rect = egui::Rect::from_min_size(badge_pos, badge_size);

    ui.painter().rect_filled(
        badge_rect,
        egui::CornerRadius::same(6),
        palette.accent_primary,
    );
    ui.painter()
        .galley(badge_pos + padding, galley, egui::Color32::WHITE);
}

fn handle_click(
    ui: &egui::Ui,
    response: &egui::Response,
    ide: &mut IdeState,
    idx: usize,
    cmd: &MacroCommand,
    edit_modal: &mut Option<Modal>,
) {
    if response.double_clicked() {
        ide.inline_edit = None;
        if let Some(m) = modal_from_command(cmd, idx) {
            *edit_modal = Some(m);
        }
        return;
    }

    if response.clicked() {
        apply_click_selection(ui, ide, idx);
    }
}
