use crate::ui::components::{badge, event_display_info};
use crate::ui::editor::actions::EditorActions;
use crate::ui::editor::context_menu::render_context_menu;
use crate::ui::editor::IdeState;
use crate::ui::modals::{Modal, modal_from_command};
use crate::ui::theme::ThemePalette;
use wmacro_core_types::MacroCommand;
use eframe::egui;

const ROW_HEIGHT: f32 = 36.0;
const INDENT_STEP_PX: f32 = 20.0;

struct RowDisplayInfo {
    icon: &'static str,
    badge_label: &'static str,
    badge_color: egui::Color32,
    detail: String,
}

fn display_info_for(cmd: &MacroCommand, palette: &ThemePalette) -> RowDisplayInfo {
    let (icon, badge_label, badge_color, detail) = match cmd {
        MacroCommand::Action(act) => event_display_info(act, palette),
        MacroCommand::IfPixelColor { x, y, r, g, b, tolerance } => (
            egui_phosphor::regular::PALETTE,
            "IF PIXEL COLOR",
            palette.col_if,
            format!(
                "x = {}   y = {}   color = #{:02X}{:02X}{:02X}{}",
                x, y, r, g, b,
                if *tolerance > 0 { format!("   tol = {}%", tolerance) } else { "".to_string() }
            ),
        ),
        MacroCommand::IfImageFound { target_image_path, similarity_threshold, .. } => (
            egui_phosphor::regular::IMAGE,
            "IF IMAGE FOUND",
            palette.col_if,
            format!(
                "target = {}   tol = {:.2}",
                std::path::Path::new(target_image_path).file_name().unwrap_or_default().to_string_lossy(),
                similarity_threshold
            ),
        ),
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
            format!("{} times", count),
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
    actions: &mut EditorActions,
    edit_modal: &mut Option<Modal>,
    visible_row_rects: &mut Vec<(usize, egui::Rect)>,
) {
    let info = display_info_for(cmd, palette);
    let is_selected = ide.selected.contains(&idx);
    let item_id = egui::Id::new("dnd_ev").with(idx);

    let row_width = ui.available_width() - 30.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(row_width, ROW_HEIGHT), egui::Sense::hover());
    let row_interact = ui.interact(rect, item_id, egui::Sense::click_and_drag());

    handle_drag_start(ui, &row_interact, ide, idx, is_selected);
    handle_drop(ui, &row_interact, rect, actions, idx);

    paint_row_background(ui, rect, is_selected, row_interact.hovered(), palette);
    paint_row_content(ui, rect, idx, indent_level, &info, palette);
    visible_row_rects.push((idx, rect));

    paint_drop_indicator(ui, rect, idx, palette);
    paint_drag_overlay(ui, rect, item_id, idx, palette);

    if row_interact.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }

    handle_click(ui, &row_interact, ide, idx, cmd, edit_modal);

    row_interact.context_menu(|ui| {
        render_context_menu(ui, ide, idx, palette, edit_modal, cmd, commands, actions);
    });
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

    if is_drag_hovered && ui.ctx().input(|i| i.pointer.any_released()) {
        if let Some(payload) = egui::DragAndDrop::take_payload::<Vec<usize>>(ui.ctx()) {
            actions.move_payload = Some(((*payload).clone(), idx));
        }
    }

    let _ = response;
}

fn paint_row_background(
    ui: &egui::Ui,
    rect: egui::Rect,
    is_selected: bool,
    is_hovered: bool,
    palette: &ThemePalette,
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
}

fn paint_row_content(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    idx: usize,
    indent_level: usize,
    info: &RowDisplayInfo,
    palette: &ThemePalette,
) {
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child_ui.style_mut().interaction.selectable_labels = false;

    paint_row_number(&mut child_ui, rect, idx, palette);
    child_ui.add_space(8.0);
    paint_indent_guides(&mut child_ui, rect, indent_level, palette);
    paint_badge_and_icon(&mut child_ui, rect, info);
    child_ui.add_space(8.0);
    paint_detail_text(&mut child_ui, &info.detail, palette);
}

fn paint_row_number(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    idx: usize,
    palette: &ThemePalette,
) {
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

fn paint_indent_guides(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    indent_level: usize,
    palette: &ThemePalette,
) {
    if indent_level == 0 {
        return;
    }

    for i in 1..=indent_level {
        let x_line = ui.cursor().left() + (i as f32 * INDENT_STEP_PX) - 10.0;
        ui.painter().vline(
            x_line,
            rect.y_range(),
            egui::Stroke::new(1.0_f32, palette.border.linear_multiply(0.6)),
        );
    }
    ui.add_space(indent_level as f32 * INDENT_STEP_PX);
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
            let mut icon_ui = c_ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(icon_rect)
                    .layout(egui::Layout::centered_and_justified(
                        egui::Direction::LeftToRight,
                    )),
            );
            icon_ui.label(egui::RichText::new(info.icon).color(info.badge_color).size(15.0));
            c_ui.add_space(4.0);
            badge(c_ui, info.badge_label, info.badge_color, Some(85.0));
        },
    );
}

fn paint_detail_text(ui: &mut egui::Ui, detail: &str, palette: &ThemePalette) {
    let detail_single_line = detail.replace('\n', " ↵ ");
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |c_ui| {
        c_ui.add(
            egui::Label::new(
                egui::RichText::new(&detail_single_line)
                    .color(palette.text_primary)
                    .size(13.0),
            )
            .truncate(),
        );
    });
}

fn paint_drop_indicator(
    ui: &egui::Ui,
    rect: egui::Rect,
    idx: usize,
    palette: &ThemePalette,
) {
    let is_drag_hovered =
        ui.rect_contains_pointer(rect) && egui::DragAndDrop::has_any_payload(ui.ctx());

    if !is_drag_hovered {
        return;
    }

    let Some(payload) = egui::DragAndDrop::payload::<Vec<usize>>(ui.ctx()) else {
        return;
    };

    let is_drag_down = payload.first().map_or(false, |&src| src < idx);
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
    let is_dragging_this = dragging_payload
        .as_ref()
        .map_or(false, |p| p.contains(&idx));

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

    if ui.ctx().is_being_dragged(item_id) {
        if let Some(payload) = &dragging_payload {
            if payload.len() > 1 {
                paint_multi_drag_badge(ui, rect, payload.len(), palette);
            }
        }
    }
}

fn paint_multi_drag_badge(
    ui: &egui::Ui,
    rect: egui::Rect,
    count: usize,
    palette: &ThemePalette,
) {
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
        if let Some(m) = modal_from_command(cmd, idx) {
            *edit_modal = Some(m);
        }
        return;
    }

    if !response.clicked() {
        return;
    }

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
