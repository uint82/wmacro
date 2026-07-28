use super::IdeState;
use super::components::tool_button;
use super::modals::{KeyActionType, Modal};
use super::theme::*;
use crate::state::{DelayUnit, SharedState};
use eframe::egui;

pub fn render_toolbox(ui: &mut egui::Ui, state: &SharedState, ide: &mut IdeState) {
    let palette = {
        let s = state.lock().unwrap();
        s.theme_manager.get_theme(&s.theme_name)
    };

    egui::Panel::left("ide_toolbox")
        .resizable(false)
        .exact_size(220.0)
        .frame(sidebar_frame(&palette))
        .show_inside(ui, |ui| {

            ui.style_mut().spacing.scroll.bar_width = 4.0;

            egui::ScrollArea::vertical().show(ui, |ui| {
                render_toolbox_contents(ui, state, ide, &palette);
            });
        });
}

fn render_toolbox_contents(
    ui: &mut egui::Ui,
    state: &SharedState,
    ide: &mut IdeState,
    palette: &crate::ui::theme::ThemePalette,
) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Add Command")
            .strong()
            .size(13.0)
            .color(palette.text_primary),
    );
    ui.add_space(10.0);

    macro_rules! tool_btn {
        ($icon:expr, $label:expr, $color:expr) => {{
            let clicked = tool_button(ui, $icon, $label, $color, palette);
            ui.add_space(4.0);
            clicked
        }};
    }

    if tool_btn!(egui_phosphor::regular::TIMER, "Delay", palette.col_delay) {
        ide.modal = Modal::Delay {
            value: 500,
            unit: DelayUnit::Milliseconds,
            target_indices: vec![],
        };
    }
    if tool_btn!(egui_phosphor::regular::MOUSE, "Mouse", palette.col_click) {
        let (x, y) = current_cursor_pos(state);
        ide.modal = Modal::Mouse {
            action: super::modals::MouseActionType::LeftClick,
            x,
            y,
            use_current_pos: false,
            jitter: 0,
            hold_time_ms: 30,
            scroll_dx: 0,
            scroll_dy: 0,
            edit_idx: None,
        };
    }
    if tool_btn!(
        egui_phosphor::regular::KEYBOARD,
        "Keyboard",
        palette.col_keyboard
    ) {
        ide.modal = Modal::Key {
            key: String::new(),
            code: 0,
            action: KeyActionType::KeyPress,
            hold_time_ms: 30,
            edit_idx: None,
            search: String::new(),
        };
    }
    if tool_btn!(
        egui_phosphor::regular::PALETTE,
        "If Pixel Color Equals",
        palette.col_if
    ) {
        let (x, y) = current_cursor_pos(state);
        ide.modal = Modal::IfPixelColor {
            x,
            y,
            r: 255,
            g: 255,
            b: 255,
            tolerance: 0,
            edit_idx: None,
            last_check: None,
        };
    }
    if tool_btn!(
        egui_phosphor::regular::ARROWS_LEFT_RIGHT,
        "Else",
        palette.col_else
    ) {
        ide.append_command_after_selection(state, core_types::MacroCommand::Else);
    }
    if tool_btn!(
        egui_phosphor::regular::STOP_CIRCLE,
        "End If",
        palette.col_end_if
    ) {
        ide.append_command_after_selection(state, core_types::MacroCommand::EndIf);
    }

    ui.add_space(8.0);

    if tool_btn!(egui_phosphor::regular::REPEAT, "Loop", palette.col_loop) {
        ide.modal = Modal::Loop {
            count: 5,
            edit_idx: None,
        };
    }
    if tool_btn!(
        egui_phosphor::regular::ARROW_U_UP_LEFT,
        "End Loop",
        palette.col_end_loop
    ) {
        ide.append_command_after_selection(state, core_types::MacroCommand::EndLoop);
    }

    ui.add_space(8.0);

    if tool_btn!(egui_phosphor::regular::TAG, "Label", palette.col_label) {
        ide.modal = Modal::Label {
            name: String::new(),
            edit_idx: None,
        };
    }
    if tool_btn!(egui_phosphor::regular::LINK, "GOTO", palette.col_goto) {
        ide.modal = Modal::Goto {
            target: String::new(),
            edit_idx: None,
        };
    }
    if tool_btn!(
        egui_phosphor::regular::TEXT_T,
        "Type Text",
        palette.col_type_text
    ) {
        ide.modal = Modal::TypeText {
            text: String::new(),
            edit_idx: None,
        };
    }

    ui.add_space(8.0);

    if tool_btn!(
        egui_phosphor::regular::FOLDER_OPEN,
        "Import Macro",
        palette.col_import_saved_macro
    ) {
        ide.modal = Modal::ImportMacro {
            path: String::new(),
            edit_idx: None,
            pending_path: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
    }
}

fn current_cursor_pos(state: &SharedState) -> (i32, i32) {
    let s = state.lock().unwrap();
    (s.cursor_x, s.cursor_y)
}
