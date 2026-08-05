use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use wmacro_core_types::MacroCommand;
use eframe::egui;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum SearchRegion {
    #[default]
    WholeScreen,
    SpecificRegion,
}

fn load_preview(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let img = image::open(path).ok()?;
    let rgb = img.to_rgba8();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let pixels: Vec<egui::Color32> = rgb
        .pixels()
        .map(|p| egui::Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    let color_image = egui::ColorImage { size: [w, h], pixels, source_size: egui::vec2(w as f32, h as f32) };
    Some(ctx.load_texture(
        format!("if_image_preview_{}", path),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

fn preview_display_size(tex: &egui::TextureHandle, max_size: f32) -> egui::Vec2 {
    let [tw, th] = tex.size();
    let mut scale = (max_size / tw as f32).min(max_size / th as f32);

    // zncc is a pixel-perfect 1:1 structural match. if the preview
    // is rendered on screen at exactly 1:1 scale (scale == 1.0), the screenshot
    // will contain a perfect copy of the target template inside the modal ui,
    // causing a false positive match. by lowering the scale will prevent that.
    if (scale - 1.0).abs() < 0.15 {
        scale = 0.82;
    }

    egui::vec2(tw as f32 * scale, th as f32 * scale)
}

pub fn render(
    ui: &mut egui::Ui,
    _state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    target_image_path: &mut String,
    similarity_threshold: &mut f32,
    move_cursor_if_found: &mut bool,
    trigger_if_not_found: &mut bool,
    search_region: &mut SearchRegion,
    region_top: &mut i32,
    region_left: &mut i32,
    region_width: &mut i32,
    region_height: &mut i32,
    test_result: &mut std::sync::Arc<std::sync::Mutex<Option<String>>>,
    preview_texture: &mut Option<(String, egui::TextureHandle)>,
    edit_idx: &Option<usize>,
) {
    ui.label(
        egui::RichText::new("Checks if a specific image appears on the screen.")
            .color(palette.text_muted)
            .size(11.0),
    );
    ui.add_space(8.0);

    let texture_matches_path = preview_texture
        .as_ref()
        .is_some_and(|(loaded_path, _)| loaded_path == target_image_path);

    if !texture_matches_path {
        if target_image_path.is_empty() {
            *preview_texture = None;
        } else {
            *preview_texture = load_preview(ui.ctx(), target_image_path)
                .map(|tex| (target_image_path.clone(), tex));
        }
    }

    ui.horizontal(|ui| {
        const PREVIEW_MAX: f32 = 80.0;

        let (preview_rect, _) = ui.allocate_exact_size(
            egui::vec2(PREVIEW_MAX, PREVIEW_MAX),
            egui::Sense::hover(),
        );

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

        if let Some((_, tex)) = preview_texture.as_ref() {
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
                if ui.button(format!("{} Capture", egui_phosphor::regular::CAMERA)).clicked() {
                    let base_dir = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
                        let home = std::env::var("HOME").expect("HOME directory not found");
                        format!("{}/.local/share", home)
                    });

                    let wmacro_dir = format!("{}/wmacro/captures", base_dir);

                    if let Err(e) = std::fs::create_dir_all(&wmacro_dir) {
                        log::error!("Failed to create capture directory: {}", e);
                    }

                    let path = format!(
                        "{}/capture_{}.png",
                        wmacro_dir,
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    );

                    if crate::image_utils::capture_area(&path).is_ok() {
                        crate::image_utils::invalidate_target_cache(&path);
                        *preview_texture = load_preview(ui.ctx(), &path)
                            .map(|tex| (path.clone(), tex));
                        *target_image_path = path;
                    } else {
                        log::error!("Failed to capture area");
                    }
                }

                if ui.button(format!("{} Browse", egui_phosphor::regular::FOLDER_OPEN)).clicked() {
                    if let Some(file) = rfd::FileDialog::new()
                        .add_filter("Image", &["png", "jpg", "jpeg", "bmp"])
                        .pick_file()
                    {
                        let path = file.to_string_lossy().to_string();
                        crate::image_utils::invalidate_target_cache(&path);
                        *preview_texture = load_preview(ui.ctx(), &path)
                            .map(|tex| (path.clone(), tex));
                        *target_image_path = path;
                    }
                }
            });

            if !target_image_path.is_empty() {
                ui.add_space(4.0);
                let filename = std::path::Path::new(target_image_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| target_image_path.clone());
                ui.label(
                    egui::RichText::new(filename)
                        .monospace()
                        .size(10.0)
                        .color(palette.text_primary),
                )
                .on_hover_text(target_image_path.as_str());
            }
        });
    });

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Min. Similarity (0.0–1.0):").color(palette.text_muted));
        ui.add(
            egui::DragValue::new(similarity_threshold)
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

    ui.checkbox(move_cursor_if_found, "Move cursor to image if found");
    ui.checkbox(trigger_if_not_found, "Trigger if image NOT found on screen");
    ui.add_space(8.0);

    ui.label(egui::RichText::new("Search Region:").color(palette.text_muted));
    ui.horizontal(|ui| {
        ui.radio_value(search_region, SearchRegion::WholeScreen, "Search Whole Screen");
        ui.radio_value(search_region, SearchRegion::SpecificRegion, "Search Specific Region");
    });

    ui.add_space(4.0);
    ui.indent("specific_region_indent", |ui| {
        let is_specific = *search_region == SearchRegion::SpecificRegion;
        ui.add_enabled_ui(is_specific, |ui| {
            ui.horizontal(|ui| {
                ui.label("Top:");
                ui.add(egui::DragValue::new(region_top).speed(1));
                ui.add_space(8.0);

                ui.label("Left:");
                ui.add(egui::DragValue::new(region_left).speed(1));
                ui.add_space(8.0);

                ui.label("Width:");
                ui.add(egui::DragValue::new(region_width).speed(1));
                ui.add_space(8.0);

                ui.label("Height:");
                ui.add(egui::DragValue::new(region_height).speed(1));
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Capture Region").clicked() {
                    if let Ok(geom) = crate::image_utils::select_region() {
                        let geom = geom.trim();
                        if let Some((pos_str, size_str)) = geom.split_once(' ') {
                            if let (Some((x_str, y_str)), Some((w_str, h_str))) =
                                (pos_str.split_once(','), size_str.split_once('x'))
                            {
                                if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
                                    x_str.parse(),
                                    y_str.parse(),
                                    w_str.parse(),
                                    h_str.parse(),
                                ) {
                                    *region_left = x;
                                    *region_top = y;
                                    *region_width = w;
                                    *region_height = h;
                                }
                            }
                        }
                    }
                }
                if ui
                    .button(format!("{}", egui_phosphor::regular::LIGHTBULB))
                    .on_hover_text("Highlight region on screen")
                    .clicked()
                {
                    crate::image_utils::highlight_region(
                        *region_left,
                        *region_top,
                        *region_width,
                        *region_height,
                        3,
                    );
                }
            });
        });
    });

    ui.add_space(12.0);

    ui.horizontal(|ui| {
        let is_testing = {
            let tr = test_result.lock().unwrap();
            tr.as_ref().map(|s| s == "Testing...").unwrap_or(false)
        };

        let btn = if is_testing {
            ui.add_enabled(false, egui::Button::new("Testing..."))
        } else {
            ui.button("Test Statement")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
        };

        if btn.clicked() {
            if target_image_path.is_empty() {
                *test_result.lock().unwrap() =
                    Some("Error: Target image not specified".to_string());
            } else {
                *test_result.lock().unwrap() = Some("Testing...".to_string());

                let target_path = target_image_path.clone();
                let threshold = *similarity_threshold;
                let move_cursor = *move_cursor_if_found;
                let reg = if *search_region == SearchRegion::SpecificRegion {
                    Some((*region_left, *region_top, *region_width, *region_height))
                } else {
                    None
                };
                let result_arc = test_result.clone();
                let ctx = ui.ctx().clone();

                std::thread::spawn(move || {
                    match crate::image_utils::find_image(target_path.as_str(), reg, threshold) {
                        Ok(Some((x, y))) => {
                            let msg = "Image found.".to_string();
                            if move_cursor {
                                if let Ok(img) = image::open(target_path.as_str()) {
                                    let center_x = x as i32 + (img.width() / 2) as i32;
                                    let center_y = y as i32 + (img.height() / 2) as i32;
                                    if let Ok(mut backend_guard) =
                                        crate::GLOBAL_BACKEND.get().unwrap().lock()
                                    {
                                        let _ = backend_guard.move_to(center_x, center_y);
                                    }
                                }
                            }
                            *result_arc.lock().unwrap() = Some(msg);
                        }
                        Ok(None) => {
                            *result_arc.lock().unwrap() =
                                Some("Image not Found".to_string());
                        }
                        Err(e) => {
                            *result_arc.lock().unwrap() = Some(format!("Error: {}", e));
                        }
                    }
                    ctx.request_repaint();
                });
            }
        }

        if let Some(msg) = &*test_result.lock().unwrap() {
            ui.label(
                egui::RichText::new(msg.as_str())
                    .strong()
                    .color(palette.text_primary),
            );
        }
    });

    ui.add_space(16.0);

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
            let region = if *search_region == SearchRegion::SpecificRegion {
                Some((*region_left, *region_top, *region_width, *region_height))
            } else {
                None
            };

            *commit = Some(MacroCommand::IfImageFound {
                target_image_path: target_image_path.clone(),
                similarity_threshold: *similarity_threshold,
                move_cursor_if_found: *move_cursor_if_found,
                trigger_if_not_found: *trigger_if_not_found,
                region,
            });
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
