use super::IdeState;
use super::theme::*;
use crate::state::DelayUnit;
use crate::state::SharedState;
use wmacro_core_types::{MacroButton, MacroCommand, MacroEvent, MousePosition};
use eframe::egui;

pub mod delay;
pub mod goto;
pub mod if_color;
pub mod if_image;
pub mod import_macro;
pub mod keyboard;
pub mod label;
pub mod loop_macro;
pub mod mouse;
pub mod mouse_move;
pub mod overwrite;
pub mod type_text;

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
    KeyPress,
    KeyDown,
    KeyUp,
}

impl KeyActionType {
    pub fn label(&self) -> &'static str {
        match self {
            KeyActionType::KeyPress => "Key Press",
            KeyActionType::KeyDown => "Key Down",
            KeyActionType::KeyUp => "Key Up",
        }
    }
}

#[derive(Default)]
pub enum Modal {
    #[default]
    None,
    OverwriteWarning,
    Delay {
        value: u64,
        unit: DelayUnit,
        target_indices: Vec<usize>,
    },
    Mouse {
        action: MouseActionType,
        x: i32,
        y: i32,
        use_current_pos: bool,
        jitter: u32,
        hold_time_ms: u32,
        scroll_dx: i32,
        scroll_dy: i32,
        edit_idx: Option<usize>,
    },
    MouseMove {
        x: i32,
        y: i32,
        edit_idx: Option<usize>,
    },
    Key {
        key: String,
        code: u16,
        action: KeyActionType,
        hold_time_ms: u32,
        edit_idx: Option<usize>,
        search: String,
    },
    IfPixelColor {
        x: i32,
        y: i32,
        r: u8,
        g: u8,
        b: u8,
        tolerance: u8,
        edit_idx: Option<usize>,
        last_check: Option<String>,
    },
    IfImageFound {
        target_image_path: String,
        similarity_threshold: f32,
        move_cursor_if_found: bool,
        trigger_if_not_found: bool,
        search_region: crate::ui::modals::if_image::SearchRegion,
        region_top: i32,
        region_left: i32,
        region_width: i32,
        region_height: i32,
        test_result: std::sync::Arc<std::sync::Mutex<Option<String>>>,
        preview_texture: Option<(String, egui::TextureHandle)>,
        edit_idx: Option<usize>,
    },
    Loop {
        count: u32,
        edit_idx: Option<usize>,
    },
    Label {
        name: String,
        edit_idx: Option<usize>,
    },
    Goto {
        target: String,
        edit_idx: Option<usize>,
    },
    ImportMacro {
        path: String,
        edit_idx: Option<usize>,
        pending_path: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    },
    TypeText {
        text: String,
        edit_idx: Option<usize>,
    },
}

pub fn extract_pos(pos: &MousePosition) -> (i32, i32, bool) {
    match pos {
        MousePosition::Absolute { x, y } => (*x, *y, false),
        MousePosition::Current => (0, 0, true),
    }
}

pub fn modal_from_command(cmd: &MacroCommand, idx: usize) -> Option<Modal> {
    match cmd {
        MacroCommand::Action(ev) => match ev {
            MacroEvent::Delay(us) => {
                let ms = *us / 1000;
                let (value, unit) = if ms > 0 && ms % 3_600_000 == 0 {
                    (ms / 3_600_000, DelayUnit::Hours)
                } else if ms > 0 && ms % 60_000 == 0 {
                    (ms / 60_000, DelayUnit::Minutes)
                } else if ms > 0 && ms % 1000 == 0 {
                    (ms / 1000, DelayUnit::Seconds)
                } else {
                    (ms, DelayUnit::Milliseconds)
                };
                Some(Modal::Delay {
                    value,
                    unit,
                    target_indices: vec![idx],
                })
            }
            MacroEvent::Click { position, button, jitter, hold_time_ms } => {
                let (x, y, use_current_pos) = extract_pos(position);
                let action = match button {
                    MacroButton::Left => MouseActionType::LeftClick,
                    MacroButton::Right => MouseActionType::RightClick,
                    MacroButton::Middle => MouseActionType::MiddleClick,
                };
                Some(Modal::Mouse {
                    action, x, y, use_current_pos, jitter: *jitter, hold_time_ms: *hold_time_ms,
                    scroll_dx: 0, scroll_dy: 0, edit_idx: Some(idx),
                })
            }
            MacroEvent::MouseDown { position, button, jitter } => {
                let (x, y, use_current_pos) = extract_pos(position);
                let action = match button {
                    MacroButton::Left => MouseActionType::LeftDown,
                    MacroButton::Right => MouseActionType::RightDown,
                    MacroButton::Middle => MouseActionType::MiddleDown,
                };
                Some(Modal::Mouse {
                    action, x, y, use_current_pos, jitter: *jitter, hold_time_ms: 30,
                    scroll_dx: 0, scroll_dy: 0, edit_idx: Some(idx),
                })
            }
            MacroEvent::MouseUp { position, button, jitter } => {
                let (x, y, use_current_pos) = extract_pos(position);
                let action = match button {
                    MacroButton::Left => MouseActionType::LeftUp,
                    MacroButton::Right => MouseActionType::RightUp,
                    MacroButton::Middle => MouseActionType::MiddleUp,
                };
                Some(Modal::Mouse {
                    action, x, y, use_current_pos, jitter: *jitter, hold_time_ms: 30,
                    scroll_dx: 0, scroll_dy: 0, edit_idx: Some(idx),
                })
            }
            MacroEvent::Scroll { dx, dy } => Some(Modal::Mouse {
                action: MouseActionType::Scroll,
                x: 0, y: 0, use_current_pos: true, jitter: 0, hold_time_ms: 30,
                scroll_dx: *dx, scroll_dy: *dy, edit_idx: Some(idx),
            }),
            MacroEvent::KeyDown { key, code, .. } => Some(Modal::Key {
                key: key.clone(), code: *code, action: KeyActionType::KeyDown,
                hold_time_ms: 30, edit_idx: Some(idx), search: String::new(),
            }),
            MacroEvent::KeyUp { key, code, .. } => Some(Modal::Key {
                key: key.clone(), code: *code, action: KeyActionType::KeyUp,
                hold_time_ms: 30, edit_idx: Some(idx), search: String::new(),
            }),
            MacroEvent::KeyPress { key, code, hold_time_ms } => Some(Modal::Key {
                key: key.clone(), code: *code, action: KeyActionType::KeyPress,
                hold_time_ms: *hold_time_ms, edit_idx: Some(idx), search: String::new(),
            }),
            MacroEvent::MouseMove { x, y } => Some(Modal::MouseMove {
                x: *x, y: *y, edit_idx: Some(idx),
            }),
        },
        MacroCommand::IfPixelColor { x, y, r, g, b, tolerance } => Some(Modal::IfPixelColor {
            x: *x, y: *y, r: *r, g: *g, b: *b, tolerance: *tolerance, edit_idx: Some(idx), last_check: None,
        }),
        MacroCommand::IfImageFound { target_image_path, similarity_threshold, move_cursor_if_found, trigger_if_not_found, region } => {
            let (search_region, region_top, region_left, region_width, region_height) = match region {
                Some((l, t, w, h)) => (crate::ui::modals::if_image::SearchRegion::SpecificRegion, *t, *l, *w, *h),
                None => (crate::ui::modals::if_image::SearchRegion::WholeScreen, 0, 0, 0, 0),
            };
            Some(Modal::IfImageFound {
                target_image_path: target_image_path.clone(),
                similarity_threshold: *similarity_threshold,
                move_cursor_if_found: *move_cursor_if_found,
                trigger_if_not_found: *trigger_if_not_found,
                search_region,
                region_top,
                region_left,
                region_width,
                region_height,
                test_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
                preview_texture: None,
                edit_idx: Some(idx),
            })
        },
        MacroCommand::Loop { count } => Some(Modal::Loop { count: *count, edit_idx: Some(idx) }),
        MacroCommand::Label(name) => Some(Modal::Label { name: name.clone(), edit_idx: Some(idx) }),
        MacroCommand::Goto(target) => Some(Modal::Goto { target: target.clone(), edit_idx: Some(idx) }),
        MacroCommand::TypeText(text) => Some(Modal::TypeText { text: text.clone(), edit_idx: Some(idx) }),
        MacroCommand::PlayMacro(path) => Some(Modal::ImportMacro {
            path: path.clone(), edit_idx: Some(idx), pending_path: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }),
        _ => None,
    }
}

pub fn render_modal(ctx: &egui::Context, state: &SharedState, ide: &mut IdeState) {
    if matches!(ide.modal, Modal::None) {
        if let Ok(mut s) = state.lock() {
            let _ = s.active_capture.take();
        } else {
            log::error!("Failed to acquire state lock to clear active capture");
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

    let mut close = draw_modal_backdrop(ctx);
    let mut commit: Option<MacroCommand> = None;

    egui::Window::new(modal_title(&ide.modal))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(modal_frame(&palette))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);

            let saved_visuals = apply_modal_visuals(ui, &palette);

            handle_active_capture(state, &mut ide.modal);
            route_modal_render(ui, state, ide, &palette, &mut close, &mut commit);

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }

            *ui.visuals_mut() = saved_visuals;
        });

    if let Some(cmd) = commit {
        let edit_idx = get_edit_idx(&ide.modal);
        if let Some(idx) = edit_idx {
            let mut s = state.lock().unwrap_or_else(|e| {
                log::error!("State mutex poisoned: {e}");
                e.into_inner()
            });
            if let Some(m) = s.macro_state.current_macro.as_mut() {
                if idx < m.commands.len() {
                    m.commands[idx] = cmd;
                }
            }
        } else {
            ide.append_command_after_selection(state, cmd);
        }
    }

    if close {
        ide.modal = Modal::None;
    }
}

fn draw_modal_backdrop(ctx: &egui::Context) -> bool {
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

fn modal_title(modal: &Modal) -> String {
    match modal {
        Modal::OverwriteWarning => format!("{} Warning", egui_phosphor::regular::WARNING),
        Modal::Delay { .. } => format!("{} Add Delay", egui_phosphor::regular::TIMER),
        Modal::Mouse { .. } => format!("{} Mouse Action", egui_phosphor::regular::MOUSE),
        Modal::Key { .. } => format!("{} Keyboard Event", egui_phosphor::regular::KEYBOARD),
        Modal::IfPixelColor { .. } => format!("{} If Pixel Color Equals", egui_phosphor::regular::PALETTE),
        Modal::IfImageFound { .. } => format!("{} If Image Found", egui_phosphor::regular::IMAGE),
        Modal::Loop { .. } => format!("{} Loop Sequence", egui_phosphor::regular::REPEAT),
        Modal::Label { .. } => format!("{} Label", egui_phosphor::regular::TAG),
        Modal::Goto { .. } => format!("{} Goto", egui_phosphor::regular::LINK),
        Modal::ImportMacro { .. } => format!("{} Import Macro", egui_phosphor::regular::FOLDER_OPEN),
        Modal::TypeText { .. } => format!("{} Type Text", egui_phosphor::regular::TEXT_T),
        Modal::MouseMove { .. } => format!("{} Move Mouse", egui_phosphor::regular::ARROWS_OUT_CARDINAL),
        Modal::None => String::new(),
    }
}

fn apply_modal_visuals(ui: &mut egui::Ui, palette: &ThemePalette) -> egui::Visuals {
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

fn handle_active_capture(state: &SharedState, modal: &mut Modal) {
    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });

    if let Some((cx, cy)) = s.active_capture.take() {
        match modal {
            Modal::Mouse { action, x, y, use_current_pos, .. } => {
                if *action != MouseActionType::Scroll {
                    *x = cx;
                    *y = cy;
                    *use_current_pos = false;
                }
            }
            Modal::MouseMove { x, y, .. } => {
                *x = cx;
                *y = cy;
            }
            Modal::IfPixelColor { x, y, r, g, b, .. } => {
                *x = cx;
                *y = cy;
                let (pr, pg, pb) = crate::cursor::get_pixel_color(cx, cy);
                *r = pr;
                *g = pg;
                *b = pb;
            }
            _ => {}
        }
    }
}

fn get_edit_idx(modal: &Modal) -> Option<usize> {
    match modal {
        Modal::Delay { .. } => None,
        Modal::Mouse { edit_idx, .. }
        | Modal::MouseMove { edit_idx, .. }
        | Modal::Key { edit_idx, .. }
        | Modal::IfPixelColor { edit_idx, .. }
        | Modal::IfImageFound { edit_idx, .. }
        | Modal::Loop { edit_idx, .. }
        | Modal::Label { edit_idx, .. }
        | Modal::Goto { edit_idx, .. }
        | Modal::ImportMacro { edit_idx, .. }
        | Modal::TypeText { edit_idx, .. } => *edit_idx,
        _ => None,
    }
}

fn route_modal_render(
    ui: &mut egui::Ui,
    state: &SharedState,
    ide: &mut IdeState,
    palette: &ThemePalette,
    close: &mut bool,
    commit: &mut Option<MacroCommand>,
) {
    match &mut ide.modal {
        Modal::None => {}
        Modal::OverwriteWarning => overwrite::render(ui, state, palette, close),
        Modal::Delay { value, unit, target_indices } => {
            delay::render(ui, state, palette, close, commit, value, unit, target_indices)
        }
        Modal::Mouse { action, x, y, use_current_pos, jitter, hold_time_ms, scroll_dx, scroll_dy, edit_idx } => {
            mouse::render(ui, state, palette, close, commit, action, x, y, use_current_pos, jitter, hold_time_ms, scroll_dx, scroll_dy, edit_idx)
        }
        Modal::MouseMove { x, y, edit_idx } => {
            mouse_move::render(ui, state, palette, close, commit, x, y, edit_idx)
        }
        Modal::Key { key, code, action, hold_time_ms, edit_idx, search } => {
            keyboard::render(ui, state, palette, close, commit, key, code, action, hold_time_ms, edit_idx, search)
        }
        Modal::IfPixelColor { x, y, r, g, b, tolerance, edit_idx, last_check } => {
            if_color::render(ui, state, palette, close, commit, x, y, r, g, b, tolerance, edit_idx, last_check)
        }
        Modal::IfImageFound { target_image_path, similarity_threshold, move_cursor_if_found, trigger_if_not_found, search_region, region_top, region_left, region_width, region_height, test_result, preview_texture, edit_idx } => {
            if_image::render(ui, state, palette, close, commit, target_image_path, similarity_threshold, move_cursor_if_found, trigger_if_not_found, search_region, region_top, region_left, region_width, region_height, test_result, preview_texture, edit_idx)
        }
        Modal::Loop { count, edit_idx } => {
            loop_macro::render(ui, state, palette, close, commit, count, edit_idx)
        }
        Modal::Label { name, edit_idx } => {
            label::render(ui, state, palette, close, commit, name, edit_idx)
        }
        Modal::Goto { target, edit_idx } => {
            goto::render(ui, state, palette, close, commit, target, edit_idx)
        }
        Modal::ImportMacro { path, edit_idx, pending_path } => {
            import_macro::render(ui, state, palette, close, commit, path, edit_idx, pending_path)
        }
        Modal::TypeText { text, edit_idx } => {
            type_text::render(ui, state, palette, close, commit, text, edit_idx)
        }
    }
}
