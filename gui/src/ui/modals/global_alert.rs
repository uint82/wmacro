use eframe::egui;
use crate::state::{SharedState, AlertKind};
use crate::ui::modals::{warning_alert, daemon_alert};

pub fn render_global_alert(ctx: &egui::Context, state: &SharedState) {
    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });

    let mut alert = match s.modal_alert.take() {
        Some(a) => a,
        None => return,
    };

    let palette = s.theme_manager.get_theme(&s.theme_name);

    let (close, action_to_execute) = match alert.kind {
        AlertKind::Warning => warning_alert::render(ctx, state, &mut alert, &palette),
        AlertKind::DaemonError => daemon_alert::render(ctx, state, &mut alert, &palette),
    };

    if !close {
        s.modal_alert = Some(alert);
    } else if let Some(action) = action_to_execute {
        drop(s);

        match action {
            crate::state::ModalAction::SaveAndQuit => {
                crate::ui::top_bar::spawn_save_macro_as(ctx, state, true);
            }
            crate::state::ModalAction::QuitWithoutSaving
            | crate::state::ModalAction::Quit => {
                if let Ok(mut state_guard) = state.lock() {
                    state_guard.unsaved_changes = false;
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            crate::state::ModalAction::Close => {}
        }
    }
}
