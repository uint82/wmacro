//! the If Image Found modal: lets the user pick an image file, search region, and result stores.

use crate::state::SharedState;
use crate::ui::screen_picker::{self, PickerOutcome, PickerTarget};
use crate::ui::theme::ThemePalette;
use eframe::egui;
use std::sync::{Arc, Mutex};
use wmacro_core_types::MacroCommand;

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum SearchRegion {
    #[default]
    WholeScreen,
    SpecificRegion,
}

pub(super) fn load_preview(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let img = image::open(path).ok()?;
    let rgb = img.to_rgba8();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let pixels: Vec<egui::Color32> = rgb
        .pixels()
        .map(|p| egui::Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    let color_image = egui::ColorImage {
        size: [w, h],
        pixels,
        source_size: egui::vec2(w as f32, h as f32),
    };
    Some(ctx.load_texture(
        format!("if_image_preview_{}", path),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

fn preview_display_size(tex: &egui::TextureHandle, max_size: f32) -> egui::Vec2 {
    let [tw, th] = tex.size();
    let mut scale = (max_size / tw as f32).min(max_size / th as f32);
    if (scale - 1.0).abs() < 0.15 {
        scale = 0.82;
    }
    egui::vec2(tw as f32 * scale, th as f32 * scale)
}

pub struct IfImageFoundModal {
    pub target_image_path: String,
    pub similarity_threshold: f32,
    pub move_cursor_if_found: bool,
    pub trigger_if_not_found: bool,
    pub search_region: SearchRegion,
    pub region_top: i32,
    pub region_left: i32,
    pub region_width: i32,
    pub region_height: i32,
    pub store_x: Option<String>,
    pub store_y: Option<String>,
    pub test_result: Arc<Mutex<Option<String>>>,
    pub preview_texture: Option<(String, egui::TextureHandle)>,
    pub edit_idx: Option<usize>,
}

impl ModalWidget for IfImageFoundModal {
    fn title(&self) -> String {
        format!("{} If Image Found", egui_phosphor::regular::IMAGE)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn on_picker_outcome(
        &mut self,
        ctx: &egui::Context,
        target: PickerTarget,
        outcome: PickerOutcome,
    ) {
        match outcome {
            PickerOutcome::Cancelled => {}
            PickerOutcome::Region { x, y, w, h, image } => match target {
                PickerTarget::SearchRegion => {
                    self.region_top = y;
                    self.region_left = x;
                    self.region_width = w;
                    self.region_height = h;
                }
                PickerTarget::TargetImage => match screen_picker::save_capture_png(&image) {
                    Ok(path) => {
                        crate::image_utils::invalidate_target_cache(&path);
                        self.target_image_path = path.clone();
                        self.preview_texture = load_preview(ctx, &path).map(|tex| (path, tex));
                    }
                    Err(e) => log::error!("failed to save captured image: {e:#}"),
                },
            },
        }
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        _state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        ui.label(
            egui::RichText::new("Checks if a specific image appears on the screen.")
                .color(palette.text_muted)
                .size(11.0),
        );
        ui.add_space(8.0);

        // sync the preview texture with the current path so it never shows a stale image.
        let texture_matches = self
            .preview_texture
            .as_ref()
            .is_some_and(|(p, _)| p == &self.target_image_path);
        if !texture_matches {
            self.preview_texture = if self.target_image_path.is_empty() {
                None
            } else {
                load_preview(ui.ctx(), &self.target_image_path)
                    .map(|tex| (self.target_image_path.clone(), tex))
            };
        }

        let mut open_picker_target: Option<PickerTarget> = None;
        ui.horizontal(|ui| {
            const PREVIEW_MAX: f32 = 80.0;
            let (preview_rect, _) =
                ui.allocate_exact_size(egui::vec2(PREVIEW_MAX, PREVIEW_MAX), egui::Sense::hover());

            ui.painter().rect_filled(
                preview_rect,
                egui::CornerRadius::same(6),
                ui.visuals().extreme_bg_color,
            );
            ui.painter().rect_stroke(
                preview_rect,
                egui::CornerRadius::same(6),
                egui::Stroke::new(1.0_f32, palette.border),
                egui::StrokeKind::Inside,
            );

            if let Some((_, tex)) = self.preview_texture.as_ref() {
                let size = preview_display_size(tex, PREVIEW_MAX - 8.0);
                let img_rect = egui::Rect::from_center_size(preview_rect.center(), size);
                ui.painter().image(
                    tex.id(),
                    img_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            } else {
                let galley = ui.painter().layout_no_wrap(
                    "No image".to_string(),
                    egui::FontId::proportional(10.0),
                    palette.text_muted,
                );
                let text_pos = preview_rect.center() - galley.size() / 2.0;
                ui.painter().galley(text_pos, galley, palette.text_muted);
            }

            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Target Image:").color(palette.text_muted));
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("{} Capture", egui_phosphor::regular::CAMERA))
                        .clicked()
                    {
                        open_picker_target = Some(PickerTarget::TargetImage);
                    }
                    if ui
                        .button(format!("{} Browse", egui_phosphor::regular::FOLDER_OPEN))
                        .clicked()
                        && let Some(file) = rfd::FileDialog::new()
                            .add_filter("Image", &["png", "jpg", "jpeg", "bmp"])
                            .pick_file()
                    {
                        let path = file.to_string_lossy().to_string();
                        crate::image_utils::invalidate_target_cache(&path);
                        self.preview_texture =
                            load_preview(ui.ctx(), &path).map(|tex| (path.clone(), tex));
                        self.target_image_path = path;
                    }
                });
                if !self.target_image_path.is_empty() {
                    let filename = std::path::Path::new(&self.target_image_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| self.target_image_path.clone());
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(filename)
                            .monospace()
                            .size(10.0)
                            .color(palette.text_primary),
                    )
                    .on_hover_text(self.target_image_path.as_str());
                }
            });
        });

        if let Some(target) = open_picker_target {
            match screen_picker::ScreenPicker::freeze(target, ui.ctx()) {
                Ok(_) => return ModalOutcome::OpenPicker { target },
                Err(e) => {
                    log::warn!("frozen capture unavailable, falling back to slurp: {e:#}");
                    if target == PickerTarget::TargetImage {
                        let path = screen_picker::new_capture_path();
                        if crate::image_utils::capture_area(&path).is_ok() {
                            crate::image_utils::invalidate_target_cache(&path);
                            self.preview_texture =
                                load_preview(ui.ctx(), &path).map(|tex| (path.clone(), tex));
                            self.target_image_path = path;
                        }
                    }
                }
            }
        }

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Min. Similarity (0.0–1.0):").color(palette.text_muted));
            ui.add(
                egui::DragValue::new(&mut self.similarity_threshold)
                    .speed(0.005)
                    .range(0.0..=1.0),
            );
        });
        ui.label(
            egui::RichText::new("  1.0 = pixel-perfect  •  0.8–0.95 = recommended  •  0.5 = loose")
                .size(10.0)
                .color(palette.text_muted),
        );
        ui.add_space(8.0);

        ui.checkbox(
            &mut self.move_cursor_if_found,
            "Move cursor to center of found image",
        );
        ui.checkbox(
            &mut self.trigger_if_not_found,
            "Trigger if image NOT found on screen",
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
        let mut open_region_picker = false;
        ui.indent("specific_region_indent", |ui| {
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
                        open_region_picker = true;
                    }
                    if ui
                        .button(egui_phosphor::regular::LIGHTBULB.to_string())
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

        if open_region_picker {
            match screen_picker::ScreenPicker::freeze(PickerTarget::SearchRegion, ui.ctx()) {
                Ok(_) => {
                    return ModalOutcome::OpenPicker {
                        target: PickerTarget::SearchRegion,
                    };
                }
                Err(e) => {
                    log::warn!("frozen capture unavailable, falling back to slurp: {e:#}");
                    if let Ok(geom) = crate::image_utils::select_region() {
                        let geom = geom.trim();
                        if let Some(((x_str, y_str), (w_str, h_str))) =
                            geom.split_once(' ').and_then(|(pos, size)| {
                                Some((pos.split_once(',')?, size.split_once('x')?))
                            })
                            && let (Ok(x), Ok(y), Ok(w), Ok(h)) =
                                (x_str.parse(), y_str.parse(), w_str.parse(), h_str.parse())
                        {
                            self.region_left = x;
                            self.region_top = y;
                            self.region_width = w;
                            self.region_height = h;
                        }
                    }
                }
            }
        }

        ui.add_space(8.0);
        render_store_grid(ui, palette, &mut self.store_x, &mut self.store_y);

        if self.store_x.is_some() || self.store_y.is_some() {
            ui.label(
                egui::RichText::new(
                    "found position is saved into these variables when the image matches",
                )
                .color(palette.text_muted)
                .size(10.0),
            );
        }

        ui.add_space(12.0);
        self.render_test_button(ui, palette);
        ui.add_space(16.0);
        self.render_commit_buttons(ui, _state, palette)
    }
}

impl IfImageFoundModal {
    // TODO: allow dragging an image file onto the preview box to set the target.
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
        MacroCommand::IfImageFound {
            target_image_path: self.target_image_path.clone(),
            similarity_threshold: self.similarity_threshold,
            move_cursor_if_found: self.move_cursor_if_found,
            trigger_if_not_found: self.trigger_if_not_found,
            region,
            store_x: self.store_x.clone(),
            store_y: self.store_y.clone(),
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
                if self.target_image_path.is_empty() {
                    if let Ok(mut tr) = self.test_result.lock() {
                        *tr = Some("Error: Target image not specified".to_string());
                    }
                } else {
                    if let Ok(mut tr) = self.test_result.lock() {
                        *tr = Some("Testing...".to_string());
                    }

                    let target_path = self.target_image_path.clone();
                    let threshold = self.similarity_threshold;
                    let move_cursor = self.move_cursor_if_found;
                    let reg = if self.search_region == SearchRegion::SpecificRegion {
                        Some((
                            self.region_left,
                            self.region_top,
                            self.region_width,
                            self.region_height,
                        ))
                    } else {
                        None
                    };
                    let result_arc = self.test_result.clone();
                    let ctx = ui.ctx().clone();

                    std::thread::spawn(move || {
                        match crate::image_utils::find_image(target_path.as_str(), reg, threshold) {
                            Ok(Some((x, y))) => {
                                if move_cursor && let Ok(img) = image::open(target_path.as_str()) {
                                    let cx = x as i32 + (img.width() / 2) as i32;
                                    let cy = y as i32 + (img.height() / 2) as i32;
                                    if let Ok(mut backend) =
                                        crate::GLOBAL_BACKEND.get().unwrap().lock()
                                    {
                                        let _ = backend.move_to(cx, cy);
                                    }
                                }
                                *result_arc.lock().unwrap() = Some("Image found.".to_string());
                            }
                            Ok(None) => {
                                *result_arc.lock().unwrap() = Some("Image not Found".to_string());
                            }
                            Err(e) => {
                                *result_arc.lock().unwrap() = Some(format!("Error: {}", e));
                            }
                        }
                        ctx.request_repaint();
                    });
                }
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

    fn render_commit_buttons(
        &self,
        ui: &mut egui::Ui,
        _state: &SharedState,
        _palette: &ThemePalette,
    ) -> ModalOutcome {
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

fn render_store_grid(
    ui: &mut egui::Ui,
    palette: &ThemePalette,
    store_x: &mut Option<String>,
    store_y: &mut Option<String>,
) {
    egui::Grid::new("if_image_store_grid")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            for (label, store_opt) in [
                ("Store X in variable:", store_x as &mut Option<String>),
                ("Store Y in variable:", store_y),
            ] {
                ui.label(
                    egui::RichText::new(label)
                        .color(palette.text_muted)
                        .size(12.0),
                );
                let mut val = store_opt.clone().unwrap_or_default();
                let hint = if label.contains('X') {
                    "found_x"
                } else {
                    "found_y"
                };
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut val)
                            .hint_text(hint)
                            .desired_width(150.0),
                    )
                    .changed()
                {
                    let trimmed = val.trim().to_string();
                    *store_opt = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    };
                }
                ui.end_row();
            }
        });
}
