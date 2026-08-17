use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::{Coord, MacroButton, MacroCommand, MacroEvent, MousePosition};

use super::MouseActionType;
use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::coord_controls;

pub struct MouseModal {
    pub action: MouseActionType,
    pub x: Coord,
    pub y: Coord,
    pub use_current_pos: bool,
    pub jitter: u32,
    pub hold_time_ms: u32,
    pub scroll_dx: i32,
    pub scroll_dy: i32,
    pub edit_idx: Option<usize>,
}

impl ModalWidget for MouseModal {
    fn title(&self) -> String {
        format!("{} Mouse Action", egui_phosphor::regular::MOUSE)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn on_capture(&mut self, cx: i32, cy: i32) {
        // scroll events have no position, so capture only applies to click/down/up actions.
        if self.action != MouseActionType::Scroll {
            self.x = Coord::Const(cx);
            self.y = Coord::Const(cy);
            self.use_current_pos = false;
        }
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        egui::Grid::new("mouse_modal_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Action").color(palette.text_muted));
                render_action_combo(ui, &mut self.action);
                ui.end_row();

                if self.action == MouseActionType::Scroll {
                    render_scroll_inputs(ui, palette, &mut self.scroll_dx, &mut self.scroll_dy);
                } else {
                    render_position_inputs(
                        ui,
                        state,
                        palette,
                        &mut self.x,
                        &mut self.y,
                        &mut self.use_current_pos,
                    );

                    if matches!(
                        self.action,
                        MouseActionType::LeftClick
                            | MouseActionType::RightClick
                            | MouseActionType::MiddleClick
                    ) {
                        render_hold_time_input(ui, palette, &mut self.hold_time_ms);
                    }

                    render_jitter_input(ui, palette, &mut self.jitter);
                }
            });

        ui.add_space(16.0);

        let btn_label = if self.edit_idx.is_some() {
            "Save"
        } else {
            "Add"
        };
        let mut outcome = ModalOutcome::Open;

        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(btn_label).strong())
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Commit(self.make_cmd());
            }

            ui.add_space(8.0);

            if ui
                .add(
                    egui::Button::new("Cancel")
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Cancelled;
            }
        });

        outcome
    }
}

impl MouseModal {
    // TODO: unify the hold-time and jitter input rows with the keyboard modal's.
    fn make_cmd(&self) -> MacroCommand {
        let position = if self.use_current_pos {
            MousePosition::Current
        } else {
            MousePosition::Absolute {
                x: self.x.clone(),
                y: self.y.clone(),
            }
        };

        let ev = match self.action {
            MouseActionType::LeftClick => MacroEvent::Click {
                position,
                button: MacroButton::Left,
                jitter: self.jitter,
                hold_time_ms: self.hold_time_ms,
            },
            MouseActionType::RightClick => MacroEvent::Click {
                position,
                button: MacroButton::Right,
                jitter: self.jitter,
                hold_time_ms: self.hold_time_ms,
            },
            MouseActionType::MiddleClick => MacroEvent::Click {
                position,
                button: MacroButton::Middle,
                jitter: self.jitter,
                hold_time_ms: self.hold_time_ms,
            },
            MouseActionType::LeftDown => MacroEvent::MouseDown {
                position,
                button: MacroButton::Left,
                jitter: self.jitter,
            },
            MouseActionType::LeftUp => MacroEvent::MouseUp {
                position,
                button: MacroButton::Left,
                jitter: self.jitter,
            },
            MouseActionType::RightDown => MacroEvent::MouseDown {
                position,
                button: MacroButton::Right,
                jitter: self.jitter,
            },
            MouseActionType::RightUp => MacroEvent::MouseUp {
                position,
                button: MacroButton::Right,
                jitter: self.jitter,
            },
            MouseActionType::MiddleDown => MacroEvent::MouseDown {
                position,
                button: MacroButton::Middle,
                jitter: self.jitter,
            },
            MouseActionType::MiddleUp => MacroEvent::MouseUp {
                position,
                button: MacroButton::Middle,
                jitter: self.jitter,
            },
            MouseActionType::Scroll => MacroEvent::Scroll {
                dx: self.scroll_dx,
                dy: self.scroll_dy,
            },
        };

        MacroCommand::Action(ev)
    }
}

fn render_action_combo(ui: &mut egui::Ui, action: &mut MouseActionType) {
    egui::ComboBox::from_id_salt("mouse_action_combo")
        .selected_text(action.label())
        .width(160.0)
        .show_ui(ui, |ui| {
            for variant in [
                MouseActionType::LeftClick,
                MouseActionType::RightClick,
                MouseActionType::MiddleClick,
            ] {
                ui.selectable_value(action, variant.clone(), variant.label());
            }
            ui.separator();
            for variant in [
                MouseActionType::LeftDown,
                MouseActionType::LeftUp,
                MouseActionType::RightDown,
                MouseActionType::RightUp,
                MouseActionType::MiddleDown,
                MouseActionType::MiddleUp,
            ] {
                ui.selectable_value(action, variant.clone(), variant.label());
            }
            ui.separator();
            ui.selectable_value(
                action,
                MouseActionType::Scroll,
                MouseActionType::Scroll.label(),
            );
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
    x: &mut Coord,
    y: &mut Coord,
    use_current_pos: &mut bool,
) {
    ui.label(egui::RichText::new("Position").color(palette.text_muted));
    ui.checkbox(use_current_pos, "Use current cursor position");
    ui.end_row();

    if *use_current_pos {
        for label in ["X", "Y"] {
            ui.label(egui::RichText::new(label).color(palette.text_muted));
            ui.label(
                egui::RichText::new("current position")
                    .color(palette.text_muted)
                    .size(11.0),
            );
            ui.end_row();
        }
    } else {
        coord_controls(ui, state, palette, "X", x, true);
        coord_controls(ui, state, palette, "Y", y, false);
    }

    let (cx, cy, capture_hk) = {
        let s = state.lock().unwrap_or_else(|e| {
            log::error!("State mutex poisoned: {e}");
            e.into_inner()
        });
        (s.cursor_x, s.cursor_y, s.macro_state.capture_hotkey)
    };

    ui.label(egui::RichText::new("Live Cursor").color(palette.text_muted));
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
    // TODO: expose a repeat count here (e.g. double-click) instead of only the hold time.
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
