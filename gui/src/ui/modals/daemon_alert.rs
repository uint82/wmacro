use eframe::egui;
use crate::state::{SharedState, ModalAlert};
use crate::ui::theme::ThemePalette;
use super::base_alert::draw_base_alert;

pub fn render(
    ctx: &egui::Context,
    _state: &SharedState,
    alert: &mut ModalAlert,
    palette: &ThemePalette,
) -> (bool, Option<crate::state::ModalAction>) {
    draw_base_alert(ctx, alert, egui_phosphor::regular::PLUGS, palette)
}
