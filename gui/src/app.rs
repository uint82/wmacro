use eframe::egui;
use std::sync::Arc;

use crate::cursor::spawn_cursor_tracker;
use crate::hotkey::spawn_hotkey_listener;
use crate::ipc_bridge::spawn_ipc_bridge;
use crate::macro_engine::recorder::spawn_recorder;
use crate::state::{SharedState, new_shared_state};
use crate::ui;

pub struct WmacroApp {
    state: SharedState,
    ide_state: ui::IdeState,
}

impl WmacroApp {
    pub fn new(cc: &eframe::CreationContext, backend_status: Result<(), String>) -> Self {
        let state = new_shared_state();

        if let Err(e) = backend_status {
            state.lock().unwrap().modal_alert = Some(crate::state::ModalAlert {
                kind: crate::state::AlertKind::DaemonError,
                title: "Daemon Error".to_string(),
                message: format!(
                    "{}\n\nTo start the daemon, run the command below, then restart WMacro.",
                    e.trim(),
                ),
                note: Some("sudo systemctl enable --now wmacro-daemon".to_string()),
                footer_note: Some("(Paste this into your terminal before exiting)".to_string()),
                actions: vec![
                    ("Exit App".to_string(), crate::state::ModalAction::Quit),
                ],
                dismissible: false,
                copied_at: None,
            });
        }

        let recorder_events_rx = crate::IPC_EVENT_RX
            .get()
            .and_then(|m| m.lock().ok())
            .and_then(|mut guard| guard.take())
            .expect("IPC_EVENT_RX must be set in main() before WmacroApp::new runs");

        let (recorder_tx, recorder_rx) = std::sync::mpsc::channel();
        let (hotkey_tx, hotkey_rx) = std::sync::mpsc::channel();

        spawn_ipc_bridge(recorder_events_rx, recorder_tx, hotkey_tx);
        spawn_recorder(Arc::clone(&state), recorder_rx);
        spawn_hotkey_listener(Arc::clone(&state), hotkey_rx);
        spawn_cursor_tracker(Arc::clone(&state));

        setup_egui_styles(&cc.egui_ctx, &state);

        Self {
            state,
            ide_state: ui::IdeState::default(),
        }
    }
}

fn setup_egui_styles(ctx: &egui::Context, state: &SharedState) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);

    let palette = {
        let s = state.lock().unwrap();
        s.theme_manager.get_theme(&s.theme_name)
    };
    ctx.set_visuals(palette.to_egui_visuals());

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.indent = 10.0;

    style.visuals = ctx.global_style().visuals.clone();

    #[cfg(debug_assertions)]
    {
        style.debug.warn_if_rect_changes_id = false;
        style.debug.show_unaligned = false;
    }

    let r = egui::CornerRadius::same(4);
    style.visuals.widgets.inactive.corner_radius = r;
    style.visuals.widgets.active.corner_radius = r;
    style.visuals.widgets.hovered.corner_radius = r;
    style.visuals.widgets.open.corner_radius = r;
    style.visuals.menu_corner_radius = r;
    style.visuals.window_corner_radius = egui::CornerRadius::same(6);

    ctx.set_global_style(style);
}

impl eframe::App for WmacroApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            let mut state = self.state.lock().unwrap();
            let has_content = state.macro_state.current_macro
                .as_ref()
                .map(|m| !m.commands.is_empty())
                .unwrap_or(false);

            if state.unsaved_changes && has_content {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                state.modal_alert = Some(crate::state::ModalAlert {
                    kind: crate::state::AlertKind::Warning,
                    title: "Save changes before closing?".to_string(),
                    message: "If you close without saving, your recent changes to the macro will be lost.".to_string(),
                    note: None,
                    footer_note: None,
                    actions: vec![
                        ("Save".to_string(), crate::state::ModalAction::SaveAndQuit),
                        ("Don't Save".to_string(), crate::state::ModalAction::QuitWithoutSaving),
                        ("Cancel".to_string(), crate::state::ModalAction::Close),
                    ],
                    dismissible: true,
                    copied_at: None,
                });
            }
        }

        let occluded = ctx.input(|i| i.viewport().occluded).unwrap_or(false);

        if occluded {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::render(ui, &self.state, &mut self.ide_state);

        crate::ui::modals::render_global_alert(ui.ctx(), &self.state);
    }
}
