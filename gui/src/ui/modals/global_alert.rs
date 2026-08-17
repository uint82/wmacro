use crate::state::{AlertKind, SharedState};
use crate::ui::modals::{daemon_alert, warning_alert};
use eframe::egui;

pub fn render_global_alert(ctx: &egui::Context, state: &SharedState) {
    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });

    // take the alert out of state so re-rendering during this frame cannot double-draw it.
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
        // release the lock before running async save or closing the window.
        // TODO: offer a "save as" path here when the macro was never named.
        drop(s);

        match action {
            crate::state::ModalAction::SaveAndQuit => {
                crate::ui::top_bar::spawn_save_macro_as(ctx, state, true);
            }
            crate::state::ModalAction::QuitWithoutSaving | crate::state::ModalAction::Quit => {
                if let Ok(mut state_guard) = state.lock() {
                    state_guard.unsaved_changes = false;
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            crate::state::ModalAction::Close => {}
        }
    }
}
