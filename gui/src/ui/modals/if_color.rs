//! the If Pixel Color Equals modal: lets the user pick a coordinate, color, and tolerance.

use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::{Coord, MacroCommand};

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::coord_controls;

pub struct IfPixelColorModal {
    pub x: Coord,
    pub y: Coord,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub tolerance: u8,
    pub edit_idx: Option<usize>,
    pub last_check: Option<String>,
}

impl ModalWidget for IfPixelColorModal {
    fn title(&self) -> String {
        format!("{} If Pixel Color Equals", egui_phosphor::regular::PALETTE)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn on_capture(&mut self, cx: i32, cy: i32) {
        self.x = Coord::Const(cx);
        self.y = Coord::Const(cy);
        let (pr, pg, pb) = crate::cursor::get_pixel_color(cx, cy);
        self.r = pr;
        self.g = pg;
        self.b = pb;
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        ui.label(
            egui::RichText::new(
                "Checks if the pixel at the given coordinate matches the target color.",
            )
            .color(palette.text_muted)
            .size(11.0),
        );
        ui.add_space(8.0);

        egui::Grid::new("ifpx_modal_grid")
            .num_columns(2)
            .spacing([12.0, 12.0])
            .show(ui, |ui| {
                coord_controls(ui, state, palette, "X", &mut self.x, true);
                coord_controls(ui, state, palette, "Y", &mut self.y, false);

                let (cx, cy, capture_hk) = {
                    let s = state.lock().unwrap_or_else(|e| {
                        log::error!("State mutex poisoned: {e}");
                        e.into_inner()
                    });
                    (s.cursor_x, s.cursor_y, s.macro_state.capture_hotkey)
                };

                ui.label("");
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("X: {}  Y: {}", cx, cy))
                            .monospace()
                            .color(palette.text_primary)
                            .size(12.0),
                    );
                    ui.add_space(8.0);
                    let hotkey_str =
                        crate::ui::key_names::hotkey_display_name_opt(capture_hk, "Unbound");
                    ui.label(
                        egui::RichText::new(format!("({} to capture)", hotkey_str))
                            .color(palette.text_muted)
                            .size(10.0),
                    );
                });
                ui.end_row();

                ui.label(egui::RichText::new("Target Color").color(palette.text_muted));
                ui.horizontal(|ui| {
                    let mut color = [self.r, self.g, self.b];
                    if ui.color_edit_button_srgb(&mut color).changed() {
                        self.r = color[0];
                        self.g = color[1];
                        self.b = color[2];
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b))
                            .monospace()
                            .color(palette.text_primary),
                    );
                });
                ui.end_row();

                ui.label(egui::RichText::new("Tolerance").color(palette.text_muted));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.tolerance)
                            .speed(1)
                            .range(0..=100_u8),
                    );
                    ui.label(egui::RichText::new("%").color(palette.text_muted));
                });
                ui.end_row();
            });

        ui.add_space(12.0);

        ui.horizontal(|ui| {
            if ui
                .add(egui::Button::new("Check if statement").fill(palette.bg_element_alt))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.last_check = Some(run_check(
                    &self.x,
                    &self.y,
                    self.r,
                    self.g,
                    self.b,
                    self.tolerance,
                ));
            }
        });

        if let Some(msg) = &self.last_check {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(msg.as_str())
                    .color(palette.text_primary)
                    .strong(),
            );
        }

        ui.add_space(16.0);

        let make_cmd = || MacroCommand::IfPixelColor {
            x: self.x.clone(),
            y: self.y.clone(),
            r: self.r,
            g: self.g,
            b: self.b,
            tolerance: self.tolerance,
        };

        let btn_label = if self.edit_idx.is_some() {
            "Save"
        } else {
            "Add"
        };
        let mut outcome = ModalOutcome::Open;

        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(btn_label).strong())
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Commit(make_cmd());
            }

            ui.add_space(8.0);

            if ui
                .add(
                    egui::Button::new("Cancel")
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Cancelled;
            }
        });

        outcome
    }
}

fn run_check(x: &Coord, y: &Coord, r: u8, g: u8, b: u8, tolerance: u8) -> String {
    // TODO: show the sampled color swatch after capture instead of only the numbers.
    let (px, py) = match (x, y) {
        (Coord::Const(px), Coord::Const(py)) => (*px, *py),
        _ => {
            return "Cannot check: position uses variables (run the macro instead)".to_string();
        }
    };

    let (cr, cg, cb) = crate::cursor::get_pixel_color(px, py);

    let is_match = if tolerance == 0 {
        cr == r && cg == g && cb == b
    } else {
        let dist = ((cr as f32 - r as f32).powi(2)
            + (cg as f32 - g as f32).powi(2)
            + (cb as f32 - b as f32).powi(2))
        .sqrt();
        dist <= 441.673_f32 * (tolerance as f32 / 100.0)
    };

    format!(
        "Result: {} (Found: #{:02X}{:02X}{:02X})",
        if is_match { "True" } else { "False" },
        cr,
        cg,
        cb
    )
}
