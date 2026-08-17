use super::base_alert::draw_base_alert;
use crate::state::{ModalAlert, SharedState};
use crate::ui::theme::ThemePalette;
use eframe::egui;

pub fn render(
    ctx: &egui::Context,
    _state: &SharedState,
    alert: &mut ModalAlert,
    palette: &ThemePalette,
) -> (bool, Option<crate::state::ModalAction>) {
    draw_base_alert(ctx, alert, egui_phosphor::regular::WARNING, palette)
}
