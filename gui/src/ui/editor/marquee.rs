use crate::ui::IdeState;
use crate::ui::theme::ThemePalette;
use eframe::egui;

const FALLBACK_STRIDE: f32 = 50.0;

fn row_stride(visible_row_rects: &[(usize, egui::Rect)]) -> f32 {
    if visible_row_rects.len() > 1 {
        visible_row_rects[1].1.top() - visible_row_rects[0].1.top()
    } else {
        FALLBACK_STRIDE
    }
}

fn screen_y_to_virtual(screen_y: f32, ref_idx: usize, ref_top: f32, stride: f32) -> f32 {
    screen_y - ref_top + (ref_idx as f32 * stride)
}

fn virtual_y_to_screen(virtual_y: f32, ref_idx: usize, ref_top: f32, stride: f32) -> f32 {
    virtual_y - (ref_idx as f32 * stride) + ref_top
}

pub fn handle_marquee(
    ui: &mut egui::Ui,
    ide: &mut IdeState,
    bg_response: &egui::Response,
    visible_row_rects: &[(usize, egui::Rect)],
    palette: &ThemePalette,
    total_rows: usize,
) {
    if egui::DragAndDrop::payload::<Vec<usize>>(ui.ctx()).is_some() {
        ide.selection_start_pos = None;
        return;
    }

    if bg_response.drag_started() {
        handle_drag_start(ide, bg_response, visible_row_rects, total_rows);
    }

    let Some(start_pos_virtual) = ide.selection_start_pos else {
        return;
    };

    if !ui.ctx().input(|i| i.pointer.primary_down()) {
        ide.selection_start_pos = None;
        return;
    }

    let Some(current_pos_screen) = ui.ctx().input(|i| i.pointer.interact_pos()) else {
        return;
    };

    if total_rows == 0 || visible_row_rects.is_empty() {
        return;
    }

    let (ref_idx, ref_rect) = visible_row_rects[0];
    let stride = row_stride(visible_row_rects);

    let current_y_virtual =
        screen_y_to_virtual(current_pos_screen.y, ref_idx, ref_rect.top(), stride);

    let selection_rect_virtual = egui::Rect::from_two_pos(
        start_pos_virtual,
        egui::pos2(current_pos_screen.x, current_y_virtual),
    );

    update_selection(ui, ide, &selection_rect_virtual, ref_rect, stride, total_rows);
    draw_marquee_overlay(
        ui,
        start_pos_virtual,
        current_pos_screen,
        ref_idx,
        ref_rect.top(),
        stride,
        palette,
    );
}

fn handle_drag_start(
    ide: &mut IdeState,
    bg_response: &egui::Response,
    visible_row_rects: &[(usize, egui::Rect)],
    total_rows: usize,
) {
    let Some(mut pos) = bg_response.interact_pointer_pos() else {
        return;
    };

    if total_rows > 0 && !visible_row_rects.is_empty() {
        let (ref_idx, ref_rect) = visible_row_rects[0];
        let stride = row_stride(visible_row_rects);
        pos.y = screen_y_to_virtual(pos.y, ref_idx, ref_rect.top(), stride);
    }

    ide.selection_start_pos = Some(pos);
    ide.drag_start_selection = ide.selected.clone();
}

fn update_selection(
    ui: &egui::Ui,
    ide: &mut IdeState,
    selection_rect: &egui::Rect,
    ref_rect: egui::Rect,
    stride: f32,
    total_rows: usize,
) {
    let ctrl = ui.ctx().input(|i| i.modifiers.ctrl);
    if ctrl {
        ide.selected = ide.drag_start_selection.clone();
    } else {
        ide.selected.clear();
    }

    for i in 0..total_rows {
        let virtual_top = i as f32 * stride;
        let virtual_bottom = virtual_top + ref_rect.height();
        let row_rect = egui::Rect::from_min_max(
            egui::pos2(ref_rect.left(), virtual_top),
            egui::pos2(ref_rect.right(), virtual_bottom),
        );
        if selection_rect.intersects(row_rect) {
            ide.selected.insert(i);
        }
    }
}

fn draw_marquee_overlay(
    ui: &mut egui::Ui,
    start_pos_virtual: egui::Pos2,
    current_pos_screen: egui::Pos2,
    ref_idx: usize,
    ref_top: f32,
    stride: f32,
    palette: &ThemePalette,
) {
    let start_screen_y = virtual_y_to_screen(start_pos_virtual.y, ref_idx, ref_top, stride);
    let start_screen_pos = egui::pos2(start_pos_virtual.x, start_screen_y);
    let screen_rect = egui::Rect::from_two_pos(start_screen_pos, current_pos_screen);

    ui.painter().rect(
        screen_rect,
        0.0,
        palette.accent_primary.linear_multiply(0.1),
        egui::Stroke::new(1.0_f32, palette.accent_primary),
        egui::StrokeKind::Inside,
    );
}
