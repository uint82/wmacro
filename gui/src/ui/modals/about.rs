//! the about modal: shows the app icon, version, and links.

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const ICON_SIZE: f32 = 80.0;
const ICON_PNG: &[u8] = include_bytes!("../../../../assets/icon/wmacro.png");

static ICON_RGBA: std::sync::OnceLock<Result<egui::ColorImage, String>> =
    std::sync::OnceLock::new();

fn icon_color_image() -> &'static Result<egui::ColorImage, String> {
    ICON_RGBA.get_or_init(|| {
        let image = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)
            .map_err(|e| format!("failed to decode icon: {e}"))?;
        let rgba = image.to_rgba8();
        let (w, h) = rgba.dimensions();
        Ok(egui::ColorImage::from_rgba_unmultiplied(
            [w as usize, h as usize],
            rgba.as_raw(),
        ))
    })
}

pub struct AboutModal {
    icon: Option<egui::TextureHandle>,
}

// TODO: show build metadata (git commit, active backend) under the version badge.

impl AboutModal {
    pub fn new() -> Self {
        Self { icon: None }
    }

    fn render_icon(&mut self, ui: &mut egui::Ui) {
        if self.icon.is_none()
            && let Ok(img) = icon_color_image()
        {
            self.icon = Some(ui.ctx().load_texture(
                "wmacro_about_icon",
                img.clone(),
                egui::TextureOptions::LINEAR,
            ));
        }

        let Some(tex) = self.icon.as_ref() else {
            return;
        };
        let size = egui::vec2(ICON_SIZE, ICON_SIZE);
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter().image(
            tex.id(),
            rect.shrink(1.0),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same((ICON_SIZE * 0.22) as u8),
            egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(70)),
            egui::StrokeKind::Inside,
        );
    }
}

impl Default for AboutModal {
    fn default() -> Self {
        Self::new()
    }
}

impl ModalWidget for AboutModal {
    fn title(&self) -> String {
        "About wmacro".to_string()
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        _state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        ui.set_max_width(300.0);
        let mut outcome = ModalOutcome::Open;

        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            self.render_icon(ui);

            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("wmacro")
                    .strong()
                    .size(22.0)
                    .color(palette.text_primary),
            );
            ui.add_space(4.0);

            egui::Frame::NONE
                .fill(palette.bg_element_alt)
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(8, 2))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("Version {APP_VERSION}"))
                            .size(11.0)
                            .color(palette.text_muted),
                    );
                });
            ui.add_space(12.0);
        });

        ui.add_space(2.0);
        ui.separator();
        ui.add_space(12.0);

        ui.label(
            egui::RichText::new(
                "Record, edit, and replay mouse and keyboard macros with variables, conditions, and loops.",
            )
            .size(12.5)
            .color(palette.text_primary),
        );
        ui.add_space(14.0);

        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                if link_button(
                    ui,
                    palette,
                    egui_phosphor::regular::BOOK_OPEN,
                    "Documentation",
                    "https://github.com/uint82/wmacro",
                ) {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab("https://github.com/uint82/wmacro"));
                }
                ui.add_space(8.0);
                if link_button(
                    ui,
                    palette,
                    egui_phosphor::regular::BUG,
                    "Report a Bug",
                    "https://github.com/uint82/wmacro/issues",
                ) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(
                        "https://github.com/uint82/wmacro/issues",
                    ));
                }
            });
        });

        ui.add_space(14.0);

        ui.label(
            egui::RichText::new("© 2026 Hilmi Abroor · Released under GPL-3.0")
                .size(11.0)
                .color(palette.text_muted),
        );
        ui.add_space(8.0);

        super::right_aligned_row(ui, |ui| {
            let close = ui
                .add(
                    egui::Button::new(egui::RichText::new("Close").strong())
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked();

            if close {
                outcome = ModalOutcome::Cancelled;
            }
        });

        outcome
    }

    fn edit_idx(&self) -> Option<usize> {
        None
    }
}

fn link_button(
    ui: &mut egui::Ui,
    palette: &ThemePalette,
    icon: &str,
    label: &str,
    url: &str,
) -> bool {
    ui.add(
        egui::Button::new(
            egui::RichText::new(format!("{icon}  {label}"))
                .color(palette.accent_primary)
                .size(13.0),
        )
        .fill(palette.bg_element_alt)
        .stroke(egui::Stroke::new(1.0_f32, palette.border))
        .corner_radius(egui::CornerRadius::same(6)),
    )
    .on_hover_text(url)
    .on_hover_cursor(egui::CursorIcon::PointingHand)
    .clicked()
}
