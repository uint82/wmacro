use super::MouseActionType;
use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use core_types::{MacroButton, MacroCommand, MacroEvent, MousePosition};
use eframe::egui;

pub fn render(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    action: &mut MouseActionType,
    x: &mut i32,
    y: &mut i32,
    use_current_pos: &mut bool,
    jitter: &mut u32,
    hold_time_ms: &mut u32,
    scroll_dx: &mut i32,
    scroll_dy: &mut i32,
    edit_idx: &Option<usize>,
) {
    render_grid(
        ui, state, palette, action, x, y, use_current_pos, jitter, hold_time_ms, scroll_dx, scroll_dy,
    );

    ui.add_space(16.0);
    render_buttons(
        ui, close, commit, edit_idx, action, *x, *y, *use_current_pos, *jitter, *hold_time_ms, *scroll_dx, *scroll_dy,
    );
}

fn render_grid(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    action: &mut MouseActionType,
    x: &mut i32,
    y: &mut i32,
    use_current_pos: &mut bool,
    jitter: &mut u32,
    hold_time_ms: &mut u32,
    scroll_dx: &mut i32,
    scroll_dy: &mut i32,
) {
    egui::Grid::new("mouse_modal_grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Action").color(palette.text_muted));
            render_action_combo(ui, action);
            ui.end_row();

            if *action == MouseActionType::Scroll {
                render_scroll_inputs(ui, palette, scroll_dx, scroll_dy);
            } else {
                render_position_inputs(ui, state, palette, x, y, use_current_pos);
                
                if matches!(
                    *action,
                    MouseActionType::LeftClick
                        | MouseActionType::RightClick
                        | MouseActionType::MiddleClick
                ) {
                    render_hold_time_input(ui, palette, hold_time_ms);
                }

                render_jitter_input(ui, palette, jitter);
            }
        });
}

fn render_action_combo(ui: &mut egui::Ui, action: &mut MouseActionType) {
    egui::ComboBox::from_id_salt("mouse_action_combo")
        .selected_text(action.label())
        .width(160.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(action, MouseActionType::LeftClick, MouseActionType::LeftClick.label());
            ui.selectable_value(action, MouseActionType::RightClick, MouseActionType::RightClick.label());
            ui.selectable_value(action, MouseActionType::MiddleClick, MouseActionType::MiddleClick.label());
            ui.separator();
            ui.selectable_value(action, MouseActionType::LeftDown, MouseActionType::LeftDown.label());
            ui.selectable_value(action, MouseActionType::LeftUp, MouseActionType::LeftUp.label());
            ui.selectable_value(action, MouseActionType::RightDown, MouseActionType::RightDown.label());
            ui.selectable_value(action, MouseActionType::RightUp, MouseActionType::RightUp.label());
            ui.selectable_value(action, MouseActionType::MiddleDown, MouseActionType::MiddleDown.label());
            ui.selectable_value(action, MouseActionType::MiddleUp, MouseActionType::MiddleUp.label());
            ui.separator();
            ui.selectable_value(action, MouseActionType::Scroll, MouseActionType::Scroll.label());
        });
}

fn render_scroll_inputs(
    ui: &mut egui::Ui,
    palette: &ThemePalette,
    scroll_dx: &mut i32,
    scroll_dy: &mut i32,
) {
    ui.label(egui::RichText::new("Scroll X").color(palette.text_muted));
    ui.add(egui::DragValue::new(scroll_dx).speed(1));
    ui.end_row();

    ui.label(egui::RichText::new("Scroll Y").color(palette.text_muted));
    ui.add(egui::DragValue::new(scroll_dy).speed(1));
    ui.end_row();
}

fn render_position_inputs(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    x: &mut i32,
    y: &mut i32,
    use_current_pos: &mut bool,
) {
    ui.label(egui::RichText::new("Position").color(palette.text_muted));
    ui.checkbox(use_current_pos, "Use current cursor position");
    ui.end_row();

    ui.label(egui::RichText::new("X").color(palette.text_muted));
    ui.add_enabled(!*use_current_pos, egui::DragValue::new(x).speed(1));
    ui.end_row();

    ui.label(egui::RichText::new("Y").color(palette.text_muted));
    ui.add_enabled(!*use_current_pos, egui::DragValue::new(y).speed(1));
    ui.end_row();

    ui.label(egui::RichText::new("Live Cursor").color(palette.text_muted));
    
    let (cx, cy, capture_hk) = {
        let s = state.lock().unwrap_or_else(|e| {
            log::error!("State mutex poisoned: {e}");
            e.into_inner()
        });
        (s.cursor_x, s.cursor_y, s.macro_state.capture_hotkey)
    };
    
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("X: {}  Y: {}", cx, cy))
                .monospace()
                .color(palette.text_primary)
                .size(12.0),
        );
        ui.add_space(8.0);
        let hotkey_str = crate::ui::key_names::hotkey_display_name_opt(capture_hk, "Unbound");
        ui.label(
            egui::RichText::new(format!("({} to capture)", hotkey_str))
                .color(palette.text_muted)
                .size(10.0),
        );
    });
    ui.end_row();
}

fn render_hold_time_input(ui: &mut egui::Ui, palette: &ThemePalette, hold_time_ms: &mut u32) {
    ui.label(egui::RichText::new("Hold Time").color(palette.text_muted));
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(hold_time_ms)
                .speed(1)
                .range(1..=10000)
                .suffix(" ms"),
        );
        ui.label(
            egui::RichText::new("duration of the click")
                .color(palette.text_muted)
                .size(10.0),
        );
    });
    ui.end_row();
}

fn render_jitter_input(ui: &mut egui::Ui, palette: &ThemePalette, jitter: &mut u32) {
    ui.label(egui::RichText::new("Jitter").color(palette.text_muted));
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(jitter)
                .speed(1)
                .range(0..=50)
                .suffix(" px"),
        );
        ui.label(
            egui::RichText::new("random offset")
                .color(palette.text_muted)
                .size(10.0),
        );
    });
    ui.end_row();
}

fn render_buttons(
    ui: &mut egui::Ui,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
    edit_idx: &Option<usize>,
    action: &MouseActionType,
    x: i32,
    y: i32,
    use_current_pos: bool,
    jitter: u32,
    hold_time_ms: u32,
    scroll_dx: i32,
    scroll_dy: i32,
) {
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
            let position = if use_current_pos {
                MousePosition::Current
            } else {
                MousePosition::Absolute { x, y }
            };

            let ev = match action {
                MouseActionType::LeftClick => MacroEvent::Click { position, button: MacroButton::Left, jitter, hold_time_ms },
                MouseActionType::RightClick => MacroEvent::Click { position, button: MacroButton::Right, jitter, hold_time_ms },
                MouseActionType::MiddleClick => MacroEvent::Click { position, button: MacroButton::Middle, jitter, hold_time_ms },
                MouseActionType::LeftDown => MacroEvent::MouseDown { position, button: MacroButton::Left, jitter },
                MouseActionType::LeftUp => MacroEvent::MouseUp { position, button: MacroButton::Left, jitter },
                MouseActionType::RightDown => MacroEvent::MouseDown { position, button: MacroButton::Right, jitter },
                MouseActionType::RightUp => MacroEvent::MouseUp { position, button: MacroButton::Right, jitter },
                MouseActionType::MiddleDown => MacroEvent::MouseDown { position, button: MacroButton::Middle, jitter },
                MouseActionType::MiddleUp => MacroEvent::MouseUp { position, button: MacroButton::Middle, jitter },
                MouseActionType::Scroll => MacroEvent::Scroll { dx: scroll_dx, dy: scroll_dy },
            };

            *commit = Some(MacroCommand::Action(ev));
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
