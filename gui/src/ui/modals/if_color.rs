use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use wmacro_core_types::MacroCommand;
use eframe::egui;

pub fn render(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    x: &mut i32,
    y: &mut i32,
    r: &mut u8,
    g: &mut u8,
    b: &mut u8,
    tolerance: &mut u8,
    edit_idx: &Option<usize>,
    last_check: &mut Option<String>,
) {
    ui.label(
        egui::RichText::new(
            "Checks if the pixel at the given coordinate matches the target color.",
        )
        .color(palette.text_muted)
        .size(11.0),
    );
    ui.add_space(8.0);

    render_grid(ui, state, palette, x, y, r, g, b, tolerance);

    ui.add_space(12.0);
    render_check_button(ui, palette, x, y, r, g, b, tolerance, last_check);

    if let Some(msg) = last_check {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(msg.as_str())
                .color(palette.text_primary)
                .strong(),
        );
    }

    ui.add_space(16.0);
    render_buttons(ui, close, commit, edit_idx, *x, *y, *r, *g, *b, *tolerance);
}

fn render_grid(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    x: &mut i32,
    y: &mut i32,
    r: &mut u8,
    g: &mut u8,
    b: &mut u8,
    tolerance: &mut u8,
) {
    egui::Grid::new("ifpx_modal_grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("X").color(palette.text_muted));
            ui.add(egui::DragValue::new(x).speed(1));
            ui.end_row();

            ui.label(egui::RichText::new("Y").color(palette.text_muted));
            ui.add(egui::DragValue::new(y).speed(1));
            ui.end_row();

            ui.label(egui::RichText::new("Live Cursor").color(palette.text_muted));

            let (cx, cy, capture_hk) = {
                let s = state.lock().unwrap_or_else(|e| {
                    log::error!("State mutex poisoned: {e}");
                    e.into_inner()
                });
                (s.cursor_x, s.cursor_y, s.macro_state.capture_hotkey)
            };

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("X: {}  Y: {}", cx, cy))
                        .monospace()
                        .color(palette.text_primary)
                        .size(12.0),
                );
                ui.add_space(8.0);
                let hotkey_str = crate::ui::key_names::hotkey_display_name_opt(capture_hk, "Unbound");
                ui.label(
                    egui::RichText::new(format!("({} to capture)", hotkey_str))
                        .color(palette.text_muted)
                        .size(10.0),
                );
            });
            ui.end_row();

            ui.label(egui::RichText::new("Target Color").color(palette.text_muted));
            ui.horizontal(|ui| {
                ui.label("R:");
                ui.add(egui::DragValue::new(r).speed(1).range(0..=255_u8));
                ui.label("G:");
                ui.add(egui::DragValue::new(g).speed(1).range(0..=255_u8));
                ui.label("B:");
                ui.add(egui::DragValue::new(b).speed(1).range(0..=255_u8));
                let swatch = egui::Color32::from_rgb(*r, *g, *b);
                ui.add_space(4.0);

                let (swatch_rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                ui.painter().rect_filled(swatch_rect, egui::CornerRadius::same(4), swatch);
                ui.painter().rect_stroke(
                    swatch_rect,
                    egui::CornerRadius::same(4),
                    egui::Stroke::new(1.0_f32, palette.border),
                    egui::StrokeKind::Outside,
                );
            });
            ui.end_row();

            ui.label(egui::RichText::new("Hex").color(palette.text_muted));
            ui.label(
                egui::RichText::new(format!("#{:02X}{:02X}{:02X}", r, g, b))
                    .monospace()
                    .color(palette.text_primary),
            );
            ui.end_row();

            ui.label(egui::RichText::new("Tolerance").color(palette.text_muted));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(tolerance).speed(1).range(0..=100_u8));
                ui.label("%");
            });
            ui.end_row();
        });
}

fn render_check_button(
    ui: &mut egui::Ui,
    palette: &ThemePalette,
    x: &i32,
    y: &i32,
    r: &u8,
    g: &u8,
    b: &u8,
    tolerance: &u8,
    last_check: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new("Check if statement").fill(palette.bg_element_alt))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            let (cr, cg, cb) = crate::cursor::get_pixel_color(*x, *y);

            let is_match = if *tolerance == 0 {
                cr == *r && cg == *g && cb == *b
            } else {
                let dist = ((cr as f32 - *r as f32).powi(2) +
                             (cg as f32 - *g as f32).powi(2) +
                             (cb as f32 - *b as f32).powi(2)).sqrt();
                let max_dist = 441.673_f32;
                let tolerance_dist = max_dist * (*tolerance as f32 / 100.0);
                dist <= tolerance_dist
            };

            let match_status = if is_match { "True" } else { "False" };
            *last_check = Some(format!(
                "Result: {} (Found: #{:02X}{:02X}{:02X})",
                match_status, cr, cg, cb
            ));
        }
    });
}

fn render_buttons(
    ui: &mut egui::Ui,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    edit_idx: &Option<usize>,
    x: i32,
    y: i32,
    r: u8,
    g: u8,
    b: u8,
    tolerance: u8,
) {
    ui.horizontal(|ui| {
        let btn_label = if edit_idx.is_some() { "Save" } else { "Add" };
        if ui
            .add(
                egui::Button::new(egui::RichText::new(btn_label).strong())
                    .min_size(egui::vec2(80.0, 28.0)),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            *commit = Some(MacroCommand::IfPixelColor { x, y, r, g, b, tolerance });
            *close = true;
        }

        ui.add_space(8.0);

        if ui
            .add(egui::Button::new("Cancel").min_size(egui::vec2(80.0, 28.0)))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            *close = true;
        }
    });
}
