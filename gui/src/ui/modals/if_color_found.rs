//! the If Color Found modal: lets the user pick a color, tolerance, search region, and result stores.

use crate::state::SharedState;
use crate::ui::screen_picker::{PickerOutcome, PickerTarget};
use crate::ui::theme::ThemePalette;
use eframe::egui;
use std::sync::{Arc, Mutex};
use wmacro_core_types::MacroCommand;

use super::if_image::SearchRegion;
use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;

pub struct IfColorFoundModal {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub tolerance: u8,
    pub min_width: u32,
    pub min_height: u32,
    pub move_cursor_if_found: bool,
    pub search_region: SearchRegion,
    pub region_top: i32,
    pub region_left: i32,
    pub region_width: i32,
    pub region_height: i32,
    pub store_x: Option<String>,
    pub store_y: Option<String>,
    pub store_w: Option<String>,
    pub store_h: Option<String>,
    pub test_result: Arc<Mutex<Option<String>>>,
    pub edit_idx: Option<usize>,
}

impl ModalWidget for IfColorFoundModal {
    fn title(&self) -> String {
        format!("{} If Color Found", egui_phosphor::regular::CHECKERBOARD)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn on_capture(&mut self, _cx: i32, _cy: i32) {
        let (pr, pg, pb) = crate::cursor::get_pixel_color(_cx, _cy);
        self.r = pr;
        self.g = pg;
        self.b = pb;
    }

    fn on_picker_outcome(
        &mut self,
        _ctx: &egui::Context,
        _target: PickerTarget,
        outcome: PickerOutcome,
    ) {
        if let PickerOutcome::Region { x, y, w, h, .. } = outcome {
            self.region_top = y;
            self.region_left = x;
            self.region_width = w;
            self.region_height = h;
        }
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        ui.label(
            egui::RichText::new(
                "Finds the colored area on the screen; gives you its center and size.",
            )
            .color(palette.text_muted)
            .size(11.0),
        );
        ui.add_space(8.0);

        let capture_hk = state
            .lock()
            .map(|s| s.macro_state.capture_hotkey)
            .unwrap_or(None);
        let hotkey_str = crate::ui::key_names::hotkey_display_name_opt(capture_hk, "Unbound");
        ui.label(
            egui::RichText::new(format!(
                "Press {} on the colored object to sample its color and center the search region on it",
                hotkey_str
            ))
            .color(palette.text_muted)
            .size(10.0),
        );
        ui.add_space(8.0);

        egui::Grid::new("ifcf_modal_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
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

                ui.label(egui::RichText::new("Min. Size").color(palette.text_muted));
                ui.horizontal(|ui| {
                    ui.label("W:");
                    ui.add(
                        egui::DragValue::new(&mut self.min_width)
                            .speed(1)
                            .range(1..=100_000),
                    );
                    ui.add_space(8.0);
                    ui.label("H:");
                    ui.add(
                        egui::DragValue::new(&mut self.min_height)
                            .speed(1)
                            .range(1..=100_000),
                    );
                    ui.label(egui::RichText::new("px").color(palette.text_muted));
                });
                ui.end_row();
            });

        ui.label(
            egui::RichText::new(
                "  colors that don't form a shape at least this wide and tall are ignored",
            )
            .size(10.0)
            .color(palette.text_muted),
        );
        ui.add_space(8.0);

        ui.checkbox(
            &mut self.move_cursor_if_found,
            "Move cursor to center of found color area",
        );
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Search Region:").color(palette.text_muted));
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.search_region,
                SearchRegion::WholeScreen,
                "Search Whole Screen",
            );
            ui.radio_value(
                &mut self.search_region,
                SearchRegion::SpecificRegion,
                "Search Specific Region",
            );
        });

        ui.add_space(4.0);
        ui.indent("ifcf_specific_region_indent", |ui| {
            let is_specific = self.search_region == SearchRegion::SpecificRegion;
            ui.add_enabled_ui(is_specific, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Top:");
                    ui.add(egui::DragValue::new(&mut self.region_top).speed(1));
                    ui.add_space(8.0);
                    ui.label("Left:");
                    ui.add(egui::DragValue::new(&mut self.region_left).speed(1));
                    ui.add_space(8.0);
                    ui.label("Width:");
                    ui.add(egui::DragValue::new(&mut self.region_width).speed(1));
                    ui.add_space(8.0);
                    ui.label("Height:");
                    ui.add(egui::DragValue::new(&mut self.region_height).speed(1));
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Capture Region").clicked() {
                        try_capture_region(
                            ui,
                            &mut self.region_left,
                            &mut self.region_top,
                            &mut self.region_width,
                            &mut self.region_height,
                        );
                    }
                    if ui
                        .button(egui_phosphor::regular::LIGHTBULB)
                        .on_hover_text("Highlight region on screen")
                        .clicked()
                    {
                        crate::image_utils::highlight_region(
                            self.region_left,
                            self.region_top,
                            self.region_width,
                            self.region_height,
                            3,
                        );
                    }
                });
            });
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Store found region in variables:").color(palette.text_muted));
        ui.horizontal(|ui| {
            add_store_var(ui, "X:", &mut self.store_x);
            add_store_var(ui, "Y:", &mut self.store_y);
            add_store_var(ui, "W:", &mut self.store_w);
            add_store_var(ui, "H:", &mut self.store_h);
        });

        if self.store_x.is_some() || self.store_y.is_some() {
            ui.label(
                egui::RichText::new(
                    "the colored area's center (x, y) and size (width, height) are saved when found",
                )
                .color(palette.text_muted)
                .size(10.0),
            );
        }

        ui.add_space(12.0);
        self.render_test_button(ui, palette);
        ui.add_space(16.0);

        self.render_commit_buttons(ui, palette)
    }
}

impl IfColorFoundModal {
    fn make_cmd(&self) -> MacroCommand {
        let region = if self.search_region == SearchRegion::SpecificRegion {
            Some((
                self.region_left,
                self.region_top,
                self.region_width,
                self.region_height,
            ))
        } else {
            None
        };
        MacroCommand::IfColorFound {
            region,
            r: self.r,
            g: self.g,
            b: self.b,
            tolerance: self.tolerance,
            min_width: self.min_width,
            min_height: self.min_height,
            move_cursor_if_found: self.move_cursor_if_found,
            store_x: self.store_x.clone(),
            store_y: self.store_y.clone(),
            store_w: self.store_w.clone(),
            store_h: self.store_h.clone(),
        }
    }

    fn render_test_button(&self, ui: &mut egui::Ui, palette: &ThemePalette) {
        ui.horizontal(|ui| {
            let is_testing = self
                .test_result
                .lock()
                .ok()
                .and_then(|tr| tr.as_ref().map(|s| s == "Testing..."))
                .unwrap_or(false);

            let btn = if is_testing {
                ui.add_enabled(false, egui::Button::new("Testing..."))
            } else {
                ui.button("Test Statement")
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
            };

            if btn.clicked() {
                if let Ok(mut tr) = self.test_result.lock() {
                    *tr = Some("Testing...".to_string());
                }

                let region = if self.search_region == SearchRegion::SpecificRegion {
                    Some((
                        self.region_left,
                        self.region_top,
                        self.region_width,
                        self.region_height,
                    ))
                } else {
                    None
                };

                let (cr, cg, cb) = (self.r, self.g, self.b);
                let tol = self.tolerance;
                let min_w = self.min_width;
                let min_h = self.min_height;
                let move_cursor = self.move_cursor_if_found;
                let result_arc = self.test_result.clone();
                let ctx = ui.ctx().clone();

                std::thread::spawn(move || {
                    match crate::image_utils::capture::capture_color_region(
                        region, cr, cg, cb, tol, min_w, min_h,
                    ) {
                        Ok(Some((x, y, w, h))) => {
                            if move_cursor
                                && let Ok(mut backend) = crate::GLOBAL_BACKEND.get().unwrap().lock()
                            {
                                let _ = backend.move_to(x, y);
                            }
                            *result_arc.lock().unwrap() =
                                Some(format!("Found at ({x}, {y}), {w}x{h} px"));
                        }
                        Ok(None) => {
                            *result_arc.lock().unwrap() =
                                Some("No matching color area found".to_string());
                        }
                        Err(e) => {
                            *result_arc.lock().unwrap() = Some(format!("Error: {}", e));
                        }
                    }
                    ctx.request_repaint();
                });
            }

            if let Ok(tr) = self.test_result.lock()
                && let Some(msg) = tr.as_ref()
            {
                ui.label(
                    egui::RichText::new(msg.as_str())
                        .strong()
                        .color(palette.text_primary),
                );
            }
        });
    }

    fn render_commit_buttons(&self, ui: &mut egui::Ui, _palette: &ThemePalette) -> ModalOutcome {
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
                outcome = ModalOutcome::Commit(self.make_cmd());
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

fn add_store_var(ui: &mut egui::Ui, label: &str, store_opt: &mut Option<String>) {
    ui.label(label);
    let mut val = store_opt.clone().unwrap_or_default();
    if ui
        .add(egui::TextEdit::singleline(&mut val).desired_width(60.0))
        .changed()
    {
        let trimmed = val.trim().to_string();
        *store_opt = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
    }
    ui.add_space(4.0);
}

fn try_capture_region(
    ui: &egui::Ui,
    region_left: &mut i32,
    region_top: &mut i32,
    region_width: &mut i32,
    region_height: &mut i32,
) {
    // the screen-picker path is handled by the parent modal dispatch via
    // `ModalOutcome::OpenPicker`; here we only handle the slurp fallback.
    // TODO: deduplicate this region-picking fallback with the one in if_image.rs.
    if let Ok(geom) = crate::image_utils::select_region() {
        let geom = geom.trim();
        if let Some(((x_str, y_str), (w_str, h_str))) = geom
            .split_once(' ')
            .and_then(|(pos, size)| Some((pos.split_once(',')?, size.split_once('x')?)))
            && let (Ok(x), Ok(y), Ok(w), Ok(h)) =
                (x_str.parse(), y_str.parse(), w_str.parse(), h_str.parse())
        {
            *region_left = x;
            *region_top = y;
            *region_width = w;
            *region_height = h;
        }
    }
    let _ = ui;
}
