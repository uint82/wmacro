pub mod components;
pub mod editor;
pub mod key_names;
pub mod modals;
pub mod settings;
pub mod status_bar;
pub mod theme;
pub mod toolbar;
pub mod toolbox;
pub mod top_bar;

use crate::state::SharedState;
use eframe::egui;

pub use modals::Modal;

#[derive(Default, Clone, PartialEq)]
pub enum SettingsTab {
    #[default]
    General,
    Hotkeys,
    Playback,
    Recording,
}

#[derive(Default)]
pub struct IdeState {
    pub modal: Modal,

    pub selected: std::collections::HashSet<usize>,
    pub pending_clear_selection: bool,
    pub show_settings: bool,
    pub settings_tab: SettingsTab,
    pub clipboard: Vec<wmacro_core_types::MacroCommand>,
    pub last_clicked_idx: Option<usize>,
    pub selection_start_pos: Option<egui::Pos2>,
    pub drag_start_selection: std::collections::HashSet<usize>,
}

impl IdeState {
    pub fn append_command_after_selection(
        &mut self,
        state: &SharedState,
        cmd: wmacro_core_types::MacroCommand,
    ) {
        let Ok(mut s) = state.lock() else {
            log::error!("Failed to acquire state lock inside append_command_after_selection");
            return;
        };

        let m = s
            .macro_state
            .current_macro
            .get_or_insert_with(|| wmacro_core_types::Macro::new("untitled"));

        let insert_idx = if let Some(last) = self.last_clicked_idx {
            last + 1
        } else if let Some(max) = self.selected.iter().max() {
            max + 1
        } else {
            m.commands.len()
        };

        let insert_idx = insert_idx.min(m.commands.len());
        m.commands.insert(insert_idx, cmd);
        s.macro_state.events_captured = m.commands.len();
        s.unsaved_changes = true;

        self.last_clicked_idx = Some(insert_idx);
        self.selected.clear();
        self.selected.insert(insert_idx);
    }
}

pub fn render(ui: &mut egui::Ui, state: &SharedState, ide: &mut IdeState) {
    if ide.pending_clear_selection {
        ide.selected.clear();
        ide.pending_clear_selection = false;
    }

    let ctx = ui.ctx().clone();

    let palette = match state.lock() {
        Ok(s) => s.theme_manager.get_theme(&s.theme_name),
        Err(e) => {
            log::error!("State mutex is poisoned ({}). Falling back to default theme.", e);
            theme::ThemeManager::default_theme()
        }
    };

    egui::Panel::top("ide_header")
        .frame(theme::topbar_frame(&palette))
        .show_inside(ui, |ui| {
            ui.vertical(|ui| {
                top_bar::render_top_bar(ui, state, ide);
                ui.add_space(4.0);
                toolbar::render_toolbar(ui, state, ide);
            });
        });

    status_bar::render_status_bar(ui, state);
    toolbox::render_toolbox(ui, state, ide);
    editor::render_editor(ui, state, ide);

    modals::render_modal(&ctx, state, ide);
    settings::render_settings(&ctx, state, ide);
}
