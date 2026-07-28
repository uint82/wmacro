use super::save_settings;
use crate::state::SharedState;
use crate::ui::key_names::hotkey_display_name_opt;
use crate::ui::theme::ThemePalette;
use eframe::egui;

pub fn render(ui: &mut egui::Ui, state: &SharedState, palette: &ThemePalette) {
    ui.label(egui::RichText::new("Press the button, then hold any modifiers and hit a key to rebind.").color(palette.text_muted).size(11.0));
    ui.add_space(10.0);

    let (record_hk, abort_record_hk, play_hk, abort_play_hk, step_play_hk, capture_hk, binding_rec, binding_abort_rec, binding_play, binding_abort_play, binding_step_play, binding_capture) = {
        let Ok(s) = state.lock() else { return };
        (
            s.macro_state.record_hotkey, s.macro_state.abort_record_hotkey, s.macro_state.play_hotkey, s.macro_state.abort_play_hotkey,
            s.macro_state.step_play_hotkey, s.macro_state.capture_hotkey, s.macro_state.binding_record, s.macro_state.binding_abort_record,
            s.macro_state.binding_play, s.macro_state.binding_abort_play, s.macro_state.binding_step_play, s.macro_state.binding_capture,
        )
    };

    macro_rules! toggle {
        ($field:ident) => {
            if let Ok(mut s) = state.lock() {
                let prev = s.macro_state.$field;
                s.macro_state.binding_record = false;
                s.macro_state.binding_abort_record = false;
                s.macro_state.binding_play = false;
                s.macro_state.binding_abort_play = false;
                s.macro_state.binding_step_play = false;
                s.macro_state.binding_capture = false;
                s.macro_state.$field = !prev;
            }
        };
    }

    macro_rules! clear {
        ($hk:ident, $bind:ident) => {
            if let Ok(mut s) = state.lock() {
                s.macro_state.$hk = None;
                s.macro_state.$bind = false;
            }
            save_settings(state);
        };
    }

    egui::Grid::new("hotkeys_grid").num_columns(3).spacing([12.0, 10.0]).show(ui, |ui| {
        let (b, c) = render_hotkey_row(ui, palette, "Record / Pause", hotkey_display_name_opt(record_hk, "None"), binding_rec);
        if b { toggle!(binding_record); } if c { clear!(record_hotkey, binding_record); }

        let (b, c) = render_hotkey_row(ui, palette, "Abort Record", hotkey_display_name_opt(abort_record_hk, "None"), binding_abort_rec);
        if b { toggle!(binding_abort_record); } if c { clear!(abort_record_hotkey, binding_abort_record); }

        let (b, c) = render_hotkey_row(ui, palette, "Play / Pause", hotkey_display_name_opt(play_hk, "None"), binding_play);
        if b { toggle!(binding_play); } if c { clear!(play_hotkey, binding_play); }

        let (b, c) = render_hotkey_row(ui, palette, "Abort Playback", hotkey_display_name_opt(abort_play_hk, "None"), binding_abort_play);
        if b { toggle!(binding_abort_play); } if c { clear!(abort_play_hotkey, binding_abort_play); }

        let (b, c) = render_hotkey_row(ui, palette, "Step-by-Step", hotkey_display_name_opt(step_play_hk, "None"), binding_step_play);
        if b { toggle!(binding_step_play); } if c { clear!(step_play_hotkey, binding_step_play); }

        let (b, c) = render_hotkey_row(ui, palette, "Capture Coordinate", hotkey_display_name_opt(capture_hk, "None"), binding_capture);
        if b { toggle!(binding_capture); } if c { clear!(capture_hotkey, binding_capture); }
    });

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(format!("Current:  Record = {}  •  Play = {}", hotkey_display_name_opt(record_hk, "None"), hotkey_display_name_opt(play_hk, "None")))
            .color(palette.text_muted)
            .size(10.0),
    );
}

fn render_hotkey_row(ui: &mut egui::Ui, palette: &ThemePalette, label: &str, display_name: String, is_binding: bool) -> (bool, bool) {
    ui.label(egui::RichText::new(label).color(palette.text_primary).size(13.0));
    let btn_label = if is_binding { format!("{}  Press a key…", egui_phosphor::regular::KEYBOARD) } else { display_name };
    let btn_fill = if is_binding { palette.accent_danger } else { palette.bg_element_alt };

    let bind_clicked = ui.add(
        egui::Button::new(egui::RichText::new(btn_label).color(palette.text_primary).monospace().size(12.0))
            .fill(btn_fill)
            .min_size(egui::vec2(140.0, 26.0)),
    ).on_hover_cursor(egui::CursorIcon::PointingHand).clicked();

    let clear_clicked = ui.add(
        egui::Button::new(egui::RichText::new("Clear").color(palette.accent_danger_fg).size(11.0))
            .fill(palette.accent_danger)
            .min_size(egui::vec2(50.0, 26.0)),
    ).on_hover_cursor(egui::CursorIcon::PointingHand).clicked();

    ui.end_row();
    (bind_clicked, clear_clicked)
}
