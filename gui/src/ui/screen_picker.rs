//! fullscreen picker overlay that freezes a capture and lets the user select a target image or search region.

// TODO(multi-output): only the portal-matched output is captured; enumerate `query_outputs()` and let the user switch the frozen output.
// TODO(hide-window): the app window appears in the snapshot when overlapping the target; hiding it needs compositor-specific support (e.g. Hyprland layer-shell or a window rule).

use crate::image_utils::capture::capture_output_color;
use eframe::egui;
use image::RgbaImage;

/// what the captured region is used for; decides how the open modal consumes the outcome.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PickerTarget {
    /// crop of the frozen frame saved as the if-image target png.
    TargetImage,

    /// fills the if-image search-region fields.
    SearchRegion,
}

pub enum PickerOutcome {
    Region {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        image: RgbaImage,
    },
    Cancelled,
}

pub struct ScreenPicker {
    pub target: PickerTarget,
    texture: egui::TextureHandle,
    image: RgbaImage,
    output_pos: (i32, i32),
    output_size: (u32, u32),
    drag_start: Option<egui::Pos2>,
    drag_cur: Option<egui::Pos2>,
}

/// minimum selection size in image pixels; smaller drags are a miss.
const MIN_SELECT: f32 = 2.0;

impl ScreenPicker {
    /// captures a full-output color snapshot and switches the window to fullscreen.
    pub fn freeze(target: PickerTarget, ctx: &egui::Context) -> anyhow::Result<Self> {
        let (image, output_pos, output_size) = capture_output_color()?;
        if image.width() == 0 || image.height() == 0 {
            anyhow::bail!("capture returned an empty frame");
        }

        let size = [image.width() as usize, image.height() as usize];
        let pixels = image
            .pixels()
            .map(|p| egui::Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        let color_image = egui::ColorImage {
            size,
            pixels,
            source_size: egui::vec2(size[0] as f32, size[1] as f32),
        };
        let texture = ctx.load_texture(
            "screen_picker_frozen",
            color_image,
            egui::TextureOptions::LINEAR,
        );

        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        ctx.request_repaint();

        Ok(Self {
            target,
            texture,
            image,
            output_pos,
            output_size,
            drag_start: None,
            drag_cur: None,
        })
    }

    fn logical_scale(&self) -> (f32, f32) {
        (
            self.output_size.0 as f32 / self.image.width() as f32,
            self.output_size.1 as f32 / self.image.height() as f32,
        )
    }

    fn draw_rect(&self, screen: egui::Rect) -> egui::Rect {
        let (sx, sy) = self.logical_scale();
        let (lw, lh) = (
            self.image.width() as f32 * sx,
            self.image.height() as f32 * sy,
        );
        let zoom = (screen.width() / lw).min(screen.height() / lh);
        egui::Rect::from_center_size(screen.center(), egui::vec2(lw * zoom, lh * zoom))
    }

    /// pointer position -> image pixel coords, `None` when outside the image.
    fn pointer_to_image(&self, screen: egui::Rect, pos: egui::Pos2) -> Option<(u32, u32)> {
        let rect = self.draw_rect(screen);
        let (dx, dy) = (
            rect.width() / self.image.width() as f32,
            rect.height() / self.image.height() as f32,
        );
        let (ix, iy) = ((pos.x - rect.min.x) / dx, (pos.y - rect.min.y) / dy);
        if ix < 0.0
            || iy < 0.0
            || ix >= self.image.width() as f32
            || iy >= self.image.height() as f32
        {
            None
        } else {
            Some((ix as u32, iy as u32))
        }
    }

    fn image_rect_to_logical(&self, r: egui::Rect) -> (i32, i32, i32, i32) {
        let (sx, sy) = self.logical_scale();
        (
            (r.min.x * sx).round() as i32 + self.output_pos.0,
            (r.min.y * sy).round() as i32 + self.output_pos.1,
            ((r.width() * sx).round() as i32).max(1),
            ((r.height() * sy).round() as i32).max(1),
        )
    }

    fn image_rect_to_screen(&self, screen: egui::Rect, r: egui::Rect) -> egui::Rect {
        let rect = self.draw_rect(screen);
        let (dx, dy) = (
            rect.width() / self.image.width() as f32,
            rect.height() / self.image.height() as f32,
        );
        egui::Rect::from_min_max(
            rect.min + egui::vec2(r.min.x * dx, r.min.y * dy),
            rect.min + egui::vec2(r.max.x * dx, r.max.y * dy),
        )
    }

    fn region_from_drag(&self) -> Option<PickerOutcome> {
        let (a, b) = (self.drag_start?, self.drag_cur?);
        let ir = egui::Rect::from_two_pos(a, b);
        if ir.width() < MIN_SELECT || ir.height() < MIN_SELECT {
            return None;
        }
        let (x, y, w, h) = self.image_rect_to_logical(ir);
        let x0 = ir.min.x.max(0.0) as u32;
        let y0 = ir.min.y.max(0.0) as u32;
        let x1 = ir.max.x.min(self.image.width() as f32) as u32;
        let y1 = ir.max.y.min(self.image.height() as f32) as u32;
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let image = image::imageops::crop_imm(&self.image, x0, y0, x1 - x0, y1 - y0).to_image();
        Some(PickerOutcome::Region { x, y, w, h, image })
    }

    /// draws the in-progress drag selection (translucent fill, outline, size label) mapped onto the frozen image.
    fn draw_drag_selection(&self, painter: &egui::Painter, screen: egui::Rect, ir: egui::Rect) {
        let sr = self.image_rect_to_screen(screen, ir);
        painter.rect_filled(
            sr,
            0.0,
            egui::Color32::from_rgba_premultiplied(80, 160, 255, 45),
        );
        painter.rect_stroke(
            sr,
            0.0,
            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(120, 200, 255)),
            egui::StrokeKind::Inside,
        );
        let label = format!(
            "{} \u{d7} {}",
            ir.width().round() as i32,
            ir.height().round() as i32
        );
        painter.text(
            sr.left_bottom() + egui::vec2(0.0, 16.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::monospace(13.0),
            egui::Color32::WHITE,
        );
    }
}

/// saves an RGBA image into the wmacro captures dir and returns its path.
pub fn save_capture_png(image: &RgbaImage) -> anyhow::Result<String> {
    let path = new_capture_path();
    image.save(&path)?;
    Ok(path)
}

/// path for a new timestamped capture file in the wmacro captures dir.
pub fn new_capture_path() -> String {
    let base_dir = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").expect("HOME directory not found");
        format!("{}/.local/share", home)
    });
    let dir = format!("{base_dir}/wmacro/captures");
    let _ = std::fs::create_dir_all(&dir);
    format!(
        "{dir}/capture_{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    )
}

/// renders the frozen overlay; returns the outcome once the selection is finished (or cancelled).
pub fn render_picker(ctx: &egui::Context, picker: &mut ScreenPicker) -> Option<PickerOutcome> {
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return Some(PickerOutcome::Cancelled);
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Enter))
        && picker.drag_start.is_some()
        && picker.drag_cur.is_some()
    {
        return picker.region_from_drag();
    }

    let mut outcome = None;
    egui::Area::new(egui::Id::new("screen_picker_overlay"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .interactable(true)
        .show(ctx, |ui| {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            let screen = ui.ctx().content_rect();

            let resp = ui.allocate_rect(screen, egui::Sense::click_and_drag());
            let hover = resp
                .hover_pos()
                .and_then(|p| picker.pointer_to_image(screen, p))
                .map(|(ix, iy)| egui::pos2(ix as f32, iy as f32));

            let painter = ui.painter();
            painter.rect_filled(screen, 0.0, egui::Color32::BLACK);

            let draw_rect = picker.draw_rect(screen);
            painter.image(
                picker.texture.id(),
                draw_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            if resp.drag_started() {
                picker.drag_start = hover;
                picker.drag_cur = hover;
            }
            if resp.dragged() {
                picker.drag_cur = hover.or(picker.drag_cur);
            }
            if resp.drag_stopped() {
                if let Some(done) = picker.region_from_drag() {
                    outcome = Some(done);
                }
                picker.drag_start = None;
                picker.drag_cur = None;
            }
            if let (Some(a), Some(b)) = (picker.drag_start, picker.drag_cur) {
                picker.draw_drag_selection(painter, screen, egui::Rect::from_two_pos(a, b));
            }

            painter.text(
                screen.center_top() + egui::vec2(0.0, 18.0),
                egui::Align2::CENTER_TOP,
                "drag to select a region, enter to confirm",
                egui::FontId::proportional(16.0),
                egui::Color32::WHITE,
            );

            let cancel_rect = egui::Rect::from_min_size(
                screen.right_top() + egui::vec2(-120.0, 12.0),
                egui::vec2(108.0, 30.0),
            );
            if ui.put(cancel_rect, egui::Button::new("Cancel")).clicked() {
                outcome = Some(PickerOutcome::Cancelled);
            }
        });
    outcome
}
