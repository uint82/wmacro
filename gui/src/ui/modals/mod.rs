//! the modals module: declares every modal dialog and the shared helpers (backdrop, visuals, dispatch) they use.

use crate::state::{DelayUnit, SharedState};
use crate::ui::IdeState;
use crate::ui::screen_picker;
use crate::ui::theme::{ThemePalette, modal_frame};
use eframe::egui;
use std::sync::{Arc, Mutex};
use wmacro_core_types::{
    Coord, MacroButton, MacroCommand, MacroEvent, MousePosition, Operand, Value,
};

pub mod about;
mod base_alert;
pub mod calculate;
pub mod comment;
mod daemon_alert;
pub mod delay;
pub mod get_clipboard;
pub mod global_alert;
pub mod goto;
pub mod if_color;
pub mod if_color_found;
pub mod if_compare;
pub mod if_image;
pub mod import_macro;
pub mod keyboard;
pub mod label;
pub mod loop_macro;
pub mod modal_trait;
pub mod mouse;
pub mod mouse_move;
pub mod open_file;
pub mod overwrite;
pub mod set_clipboard;
pub mod set_variable;
pub mod type_text;
pub mod types;
pub mod variable;
mod warning_alert;

pub use global_alert::render_global_alert;
pub use modal_trait::ModalWidget;
pub use types::ModalOutcome;

#[derive(Default, Clone, PartialEq)]
pub enum MouseActionType {
    #[default]
    LeftClick,
    RightClick,
    MiddleClick,
    LeftDown,
    LeftUp,
    RightDown,
    RightUp,
    MiddleDown,
    MiddleUp,
    Scroll,
}

impl MouseActionType {
    pub fn label(&self) -> &'static str {
        match self {
            MouseActionType::LeftClick => "Left Click",
            MouseActionType::RightClick => "Right Click",
            MouseActionType::MiddleClick => "Middle Click",
            MouseActionType::LeftDown => "Left Button Down",
            MouseActionType::LeftUp => "Left Button Up",
            MouseActionType::RightDown => "Right Button Down",
            MouseActionType::RightUp => "Right Button Up",
            MouseActionType::MiddleDown => "Middle Button Down",
            MouseActionType::MiddleUp => "Middle Button Up",
            MouseActionType::Scroll => "Scroll Wheel",
        }
    }
}

#[derive(Default, Clone, PartialEq)]
pub enum KeyActionType {
    #[default]
    Press,
    Down,
    Up,
}

impl KeyActionType {
    pub fn label(&self) -> &'static str {
        match self {
            KeyActionType::Press => "Key Press",
            KeyActionType::Down => "Key Down",
            KeyActionType::Up => "Key Up",
        }
    }
}

#[derive(Default)]
pub enum Modal {
    #[default]
    None,
    Widget(Box<dyn ModalWidget>),
}

pub fn extract_pos(pos: &MousePosition) -> (Coord, Coord, bool) {
    match pos {
        MousePosition::Absolute { x, y } => (x.clone(), y.clone(), false),
        MousePosition::Current => (Coord::Const(0), Coord::Const(0), true),
    }
}

pub fn modal_from_command(cmd: &MacroCommand, idx: usize) -> Option<Modal> {
    match cmd {
        MacroCommand::Action(ev) => match ev {
            MacroEvent::Delay(us) => {
                let ms = *us / 1000;
                Some(Modal::Widget(Box::new(self::delay::DelayModal::from_ms(
                    ms, idx,
                ))))
            }
            MacroEvent::Click {
                position,
                button,
                jitter,
                hold_time_ms,
            } => {
                let (x, y, use_current_pos) = extract_pos(position);
                let action = match button {
                    MacroButton::Left => MouseActionType::LeftClick,
                    MacroButton::Right => MouseActionType::RightClick,
                    MacroButton::Middle => MouseActionType::MiddleClick,
                };
                Some(Modal::Widget(Box::new(self::mouse::MouseModal {
                    action,
                    x,
                    y,
                    use_current_pos,
                    jitter: *jitter,
                    hold_time_ms: *hold_time_ms,
                    scroll_dx: 0,
                    scroll_dy: 0,
                    edit_idx: Some(idx),
                })))
            }
            MacroEvent::MouseDown {
                position,
                button,
                jitter,
            } => {
                let (x, y, use_current_pos) = extract_pos(position);
                let action = match button {
                    MacroButton::Left => MouseActionType::LeftDown,
                    MacroButton::Right => MouseActionType::RightDown,
                    MacroButton::Middle => MouseActionType::MiddleDown,
                };
                Some(Modal::Widget(Box::new(self::mouse::MouseModal {
                    action,
                    x,
                    y,
                    use_current_pos,
                    jitter: *jitter,
                    hold_time_ms: 30,
                    scroll_dx: 0,
                    scroll_dy: 0,
                    edit_idx: Some(idx),
                })))
            }
            MacroEvent::MouseUp {
                position,
                button,
                jitter,
            } => {
                let (x, y, use_current_pos) = extract_pos(position);
                let action = match button {
                    MacroButton::Left => MouseActionType::LeftUp,
                    MacroButton::Right => MouseActionType::RightUp,
                    MacroButton::Middle => MouseActionType::MiddleUp,
                };
                Some(Modal::Widget(Box::new(self::mouse::MouseModal {
                    action,
                    x,
                    y,
                    use_current_pos,
                    jitter: *jitter,
                    hold_time_ms: 30,
                    scroll_dx: 0,
                    scroll_dy: 0,
                    edit_idx: Some(idx),
                })))
            }
            MacroEvent::Scroll { dx, dy } => {
                Some(Modal::Widget(Box::new(self::mouse::MouseModal {
                    action: MouseActionType::Scroll,
                    x: Coord::Const(0),
                    y: Coord::Const(0),
                    use_current_pos: true,
                    jitter: 0,
                    hold_time_ms: 30,
                    scroll_dx: *dx,
                    scroll_dy: *dy,
                    edit_idx: Some(idx),
                })))
            }
            MacroEvent::KeyDown { key, code, .. } => {
                Some(Modal::Widget(Box::new(self::keyboard::KeyModal {
                    key: key.clone(),
                    code: *code,
                    action: KeyActionType::Down,
                    hold_time_ms: 30,
                    edit_idx: Some(idx),
                    search: String::new(),
                })))
            }
            MacroEvent::KeyUp { key, code, .. } => {
                Some(Modal::Widget(Box::new(self::keyboard::KeyModal {
                    key: key.clone(),
                    code: *code,
                    action: KeyActionType::Up,
                    hold_time_ms: 30,
                    edit_idx: Some(idx),
                    search: String::new(),
                })))
            }
            MacroEvent::KeyPress {
                key,
                code,
                hold_time_ms,
            } => Some(Modal::Widget(Box::new(self::keyboard::KeyModal {
                key: key.clone(),
                code: *code,
                action: KeyActionType::Press,
                hold_time_ms: *hold_time_ms,
                edit_idx: Some(idx),
                search: String::new(),
            }))),
            MacroEvent::MouseMove { x, y } => {
                Some(Modal::Widget(Box::new(self::mouse_move::MouseMoveModal {
                    x: x.clone(),
                    y: y.clone(),
                    edit_idx: Some(idx),
                })))
            }
        },
        MacroCommand::IfPixelColor {
            x,
            y,
            r,
            g,
            b,
            tolerance,
        } => Some(Modal::Widget(Box::new(self::if_color::IfPixelColorModal {
            x: x.clone(),
            y: y.clone(),
            r: *r,
            g: *g,
            b: *b,
            tolerance: *tolerance,
            edit_idx: Some(idx),
            last_check: None,
        }))),
        MacroCommand::IfImageFound {
            target_image_path,
            similarity_threshold,
            move_cursor_if_found,
            trigger_if_not_found,
            region,
            store_x,
            store_y,
        } => {
            let (search_region, region_top, region_left, region_width, region_height) = match region
            {
                Some((l, t, w, h)) => {
                    (self::if_image::SearchRegion::SpecificRegion, *t, *l, *w, *h)
                }
                None => (self::if_image::SearchRegion::WholeScreen, 0, 0, 0, 0),
            };
            Some(Modal::Widget(Box::new(self::if_image::IfImageFoundModal {
                target_image_path: target_image_path.clone(),
                similarity_threshold: *similarity_threshold,
                move_cursor_if_found: *move_cursor_if_found,
                trigger_if_not_found: *trigger_if_not_found,
                search_region,
                region_top,
                region_left,
                region_width,
                region_height,
                store_x: store_x.clone(),
                store_y: store_y.clone(),
                test_result: Arc::new(Mutex::new(None)),
                preview_texture: None,
                edit_idx: Some(idx),
            })))
        }
        MacroCommand::IfColorFound {
            region,
            r,
            g,
            b,
            tolerance,
            min_width,
            min_height,
            move_cursor_if_found,
            store_x,
            store_y,
            store_w,
            store_h,
        } => {
            let (search_region, region_top, region_left, region_width, region_height) = match region
            {
                Some((l, t, w, h)) => {
                    (self::if_image::SearchRegion::SpecificRegion, *t, *l, *w, *h)
                }
                None => (self::if_image::SearchRegion::WholeScreen, 0, 0, 0, 0),
            };
            Some(Modal::Widget(Box::new(
                self::if_color_found::IfColorFoundModal {
                    r: *r,
                    g: *g,
                    b: *b,
                    tolerance: *tolerance,
                    min_width: *min_width,
                    min_height: *min_height,
                    move_cursor_if_found: *move_cursor_if_found,
                    search_region,
                    region_top,
                    region_left,
                    region_width,
                    region_height,
                    store_x: store_x.clone(),
                    store_y: store_y.clone(),
                    store_w: store_w.clone(),
                    store_h: store_h.clone(),
                    test_result: Arc::new(Mutex::new(None)),
                    edit_idx: Some(idx),
                },
            )))
        }
        MacroCommand::Loop { count } => {
            Some(Modal::Widget(Box::new(self::loop_macro::LoopModal {
                count_text: self::variable::format_var_operand(count),
                edit_idx: Some(idx),
            })))
        }
        MacroCommand::Label(name) => Some(Modal::Widget(Box::new(self::label::LabelModal {
            name: name.clone(),
            edit_idx: Some(idx),
        }))),
        MacroCommand::Goto(target) => Some(Modal::Widget(Box::new(self::goto::GotoModal {
            target: target.clone(),
            edit_idx: Some(idx),
        }))),
        MacroCommand::TypeText(text) => {
            Some(Modal::Widget(Box::new(self::type_text::TypeTextModal {
                text: text.clone(),
                edit_idx: Some(idx),
            })))
        }
        MacroCommand::OpenFile {
            path,
            args,
            run_as_admin,
        } => Some(Modal::Widget(Box::new(self::open_file::OpenFileModal {
            path: path.clone(),
            args: args.clone(),
            run_as_admin: *run_as_admin,
            edit_idx: Some(idx),
            pending_path: Arc::new(Mutex::new(None)),
        }))),
        MacroCommand::PlayMacro(path) => Some(Modal::Widget(Box::new(
            self::import_macro::ImportMacroModal {
                path: path.clone(),
                edit_idx: Some(idx),
                pending_path: Arc::new(Mutex::new(None)),
            },
        ))),
        MacroCommand::SetVariable { target, value } => Some(Modal::Widget(Box::new(
            self::set_variable::SetVariableModal {
                target: target.clone(),
                value_text: self::variable::format_var_operand(value),
                edit_idx: Some(idx),
            },
        ))),
        MacroCommand::Calculate { target, expression } => {
            Some(Modal::Widget(Box::new(self::calculate::CalculateModal {
                target: target.clone(),
                expression: expression.clone(),
                edit_idx: Some(idx),
            })))
        }
        MacroCommand::IfCompare { left, op, right } => {
            Some(Modal::Widget(Box::new(self::if_compare::IfCompareModal {
                left_text: self::variable::format_var_operand(left),
                op: *op,
                right_text: self::variable::format_var_operand(right),
                edit_idx: Some(idx),
            })))
        }
        MacroCommand::Delay { duration_ms } => {
            let modal = match duration_ms {
                Operand::Literal(Value::Number(ms)) => {
                    self::delay::DelayModal::from_ms((*ms).max(0) as u64, idx)
                }
                _ => self::delay::DelayModal {
                    value: 0,
                    unit: DelayUnit::Milliseconds,
                    target_indices: vec![idx],
                    duration_text: self::variable::format_var_operand(duration_ms),
                    edit_idx: Some(idx),
                },
            };
            Some(Modal::Widget(Box::new(modal)))
        }
        MacroCommand::SetClipboard { text } => Some(Modal::Widget(Box::new(
            self::set_clipboard::SetClipboardModal {
                text: self::variable::format_var_operand(text),
                edit_idx: Some(idx),
            },
        ))),
        MacroCommand::GetClipboard { target } => Some(Modal::Widget(Box::new(
            self::get_clipboard::GetClipboardModal {
                target: target.clone(),
                edit_idx: Some(idx),
            },
        ))),
        MacroCommand::Else | MacroCommand::EndIf | MacroCommand::EndLoop => None,
        MacroCommand::Comment(text) => Some(Modal::Widget(Box::new(self::comment::CommentModal {
            text: text.clone(),
            edit_idx: Some(idx),
        }))),
    }
}

pub fn render_modal(ctx: &egui::Context, state: &SharedState, ide: &mut IdeState) {
    if matches!(ide.modal, Modal::None) {
        if let Ok(mut s) = state.lock() {
            s.active_capture = None;
        } else {
            log::error!("Failed to acquire state lock to clear active capture");
        }
        return;
    }

    if ide.screen_picker.is_some() {
        {
            let mut s = state.lock().unwrap_or_else(|e| {
                log::error!("State mutex poisoned: {e}");
                e.into_inner()
            });
            s.active_capture = None;
        }
        let outcome = ide
            .screen_picker
            .as_mut()
            .and_then(|picker| screen_picker::render_picker(ctx, picker));
        if let Some(outcome) = outcome
            && let Some(picker) = ide.screen_picker.take()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            if let Modal::Widget(w) = &mut ide.modal {
                w.on_picker_outcome(ctx, picker.target, outcome);
            }
        }
        return;
    }

    let palette = {
        let s = state.lock().unwrap_or_else(|e| {
            log::error!("State mutex poisoned: {e}");
            e.into_inner()
        });
        s.theme_manager.get_theme(&s.theme_name)
    };

    let close = draw_modal_backdrop(ctx);

    // TODO: move this state-lock handling into the modal trait so modals can opt in.
    if let Ok(mut s) = state.lock()
        && let Some((cx, cy)) = s.active_capture.take()
        && let Modal::Widget(w) = &mut ide.modal
    {
        w.on_capture(cx, cy);
    }

    let mut final_outcome = ModalOutcome::Open;

    if let Modal::Widget(w) = &mut ide.modal {
        egui::Window::new(w.title())
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .frame(modal_frame(&palette))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_min_width(340.0);
                let saved_visuals = apply_modal_visuals(ui, &palette);

                let outcome = w.show(ui, state, &palette);
                if outcome != ModalOutcome::Open {
                    final_outcome = outcome;
                }

                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    final_outcome = ModalOutcome::Cancelled;
                }

                *ui.visuals_mut() = saved_visuals;
            });
    }

    if close && matches!(final_outcome, ModalOutcome::Open) {
        clear_autofocus_flags(ctx, &ide.modal);
        ide.modal = Modal::None;
    }

    match final_outcome {
        ModalOutcome::Open => {}
        ModalOutcome::Cancelled => {
            clear_autofocus_flags(ctx, &ide.modal);
            ide.modal = Modal::None;
        }
        ModalOutcome::Commit(cmd) => {
            let edit_idx = if let Modal::Widget(w) = &ide.modal {
                w.edit_idx()
            } else {
                None
            };
            if let Some(idx) = edit_idx {
                let mut s = state.lock().unwrap_or_else(|e| {
                    log::error!("State mutex poisoned: {e}");
                    e.into_inner()
                });
                if let Some(m) = s.macro_state.current_macro.as_mut()
                    && idx < m.commands.len()
                {
                    m.commands[idx] = cmd;
                }
            } else {
                ide.append_command_after_selection(state, cmd);
            }
            clear_autofocus_flags(ctx, &ide.modal);
            ide.modal = Modal::None;
        }
        ModalOutcome::OpenPicker { target } => {
            match screen_picker::ScreenPicker::freeze(target, ctx) {
                Ok(picker) => ide.screen_picker = Some(picker),
                Err(e) => log::error!("Failed to open screen picker: {e}"),
            }
        }
    }
}

fn clear_autofocus_flags(ctx: &egui::Context, modal: &Modal) {
    if let Modal::Widget(w) = modal {
        for id in w.autofocus_ids() {
            let eid = egui::Id::new(id);
            ctx.data_mut(|d| d.remove::<bool>(eid.with("__autofocused")));
            // text edit state is persisted per widget id, so a stale caret
            // would otherwise survive modal close/reopen.
            // TODO: unify this cleanup with the per-modal autofocus flag handling.
            ctx.data_mut(|d| d.remove::<egui::text_edit::TextEditState>(eid));
        }
    }
}

/// runs `add_contents` in a right-aligned row sized to the modal's actual
/// width; a plain `right_to_left` child would grow to the full region and pin
/// the window to its default height (the dead band below the last button).
pub fn right_aligned_row<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut result = None;
    ui.allocate_ui_with_layout(
        egui::vec2(ui.min_rect().width(), 0.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| result = Some(add_contents(ui)),
    );
    result.unwrap()
}

pub fn draw_modal_backdrop(ctx: &egui::Context) -> bool {
    let screen = ctx.content_rect();
    let mut clicked_outside = false;

    egui::Area::new(egui::Id::new("modal_backdrop"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.left_top())
        .interactable(true)
        .show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, egui::Sense::click());
            if resp.clicked() {
                clicked_outside = true;
            }

            ui.painter().rect_filled(
                screen,
                egui::CornerRadius::ZERO,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
        });

    clicked_outside
}

pub fn apply_modal_visuals(ui: &mut egui::Ui, palette: &ThemePalette) -> egui::Visuals {
    let saved_visuals = ui.visuals().clone();
    ui.visuals_mut().widgets.inactive.weak_bg_fill = palette.bg_element_alt;
    ui.visuals_mut().widgets.inactive.fg_stroke.color = palette.text_primary;
    ui.visuals_mut().widgets.hovered.weak_bg_fill = palette.bg_element;
    ui.visuals_mut().widgets.hovered.fg_stroke.color = palette.text_primary;
    ui.visuals_mut().widgets.active.weak_bg_fill = palette.bg_element;
    ui.visuals_mut().widgets.active.fg_stroke.color = palette.text_primary;
    ui.visuals_mut().widgets.open.weak_bg_fill = palette.bg_element;
    saved_visuals
}
