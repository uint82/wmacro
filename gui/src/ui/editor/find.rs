//! in-editor find bar (Ctrl+Shift+F): searches command text, Enter/Shift+Enter cycle matches, Esc closes.

use crate::state::SharedState;
use crate::ui::IdeState;
use crate::ui::editor::actions::{plural, sync_event_count};
use crate::ui::editor::replace;
use crate::ui::theme::ThemePalette;
use eframe::egui;

/// in-editor find bar (Ctrl+Shift+F): searches command text, Enter/Shift+Enter cycle matches, Esc closes.
pub fn render_find_bar(
    ui: &mut egui::Ui,
    state: &SharedState,
    ide: &mut IdeState,
    palette: &ThemePalette,
) {
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        ide.find_open = false;
        return;
    }

    let mut query = ide.find_query.clone();
    let mut replace_text = ide.find_replace_query.clone();
    let mut replace_focus = false;
    let matches = find_matches(state, ide, palette);
    if !matches.is_empty() {
        ide.find_match_idx = ide.find_match_idx.min(matches.len() - 1);
    }

    egui::Frame::NONE
        .fill(palette.bg_element)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .corner_radius(egui::CornerRadius::same(6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                        .color(palette.text_muted),
                );

                let query_edit = ui.add(
                    egui::TextEdit::singleline(&mut query)
                        .desired_width(220.0)
                        .hint_text("Find in macro"),
                );
                if ide.find_just_opened {
                    query_edit.request_focus();
                    ide.find_just_opened = false;
                    if !matches.is_empty() {
                        ide.find_match_idx = 0;
                        jump_to(ide, &matches, 0);
                    }
                }

                let count_label = if ide.find_query.trim().is_empty() || matches.is_empty() {
                    "0 / 0".to_string()
                } else {
                    format!("{} / {}", ide.find_match_idx + 1, matches.len())
                };
                ui.label(
                    egui::RichText::new(count_label)
                        .color(palette.text_muted)
                        .monospace(),
                );

                if ui
                    .add(egui::Button::new(egui_phosphor::regular::CARET_UP))
                    .on_hover_text("Previous match (Shift+Enter)")
                    .clicked()
                {
                    advance(ide, &matches, -1);
                }
                if ui
                    .add(egui::Button::new(egui_phosphor::regular::CARET_DOWN))
                    .on_hover_text("Next match (Enter)")
                    .clicked()
                {
                    advance(ide, &matches, 1);
                }
                if ui
                    .add(
                        egui::Button::new(egui_phosphor::regular::ARROWS_LEFT_RIGHT)
                            .selected(ide.find_replace_mode),
                    )
                    .on_hover_text("Toggle replace (Ctrl+Shift+H)")
                    .clicked()
                {
                    ide.find_replace_mode = !ide.find_replace_mode;
                }
                if ui
                    .add(egui::Button::new(egui_phosphor::regular::X))
                    .on_hover_text("Close (Esc)")
                    .clicked()
                {
                    ide.find_open = false;
                }
            });

            if ide.find_replace_mode {
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    let replace_edit = ui.add(
                        egui::TextEdit::singleline(&mut replace_text)
                            .desired_width(220.0)
                            .hint_text("Replace with"),
                    );
                    replace_focus = replace_edit.has_focus();

                    let can_replace = !matches.is_empty() && !ide.find_query.trim().is_empty();
                    if ui
                        .add_enabled(can_replace, egui::Button::new("Replace"))
                        .on_hover_text("Replace current match")
                        .clicked()
                    {
                        replace_current_match(state, ide, palette);
                    }
                    if ui
                        .add_enabled(can_replace, egui::Button::new("Replace All"))
                        .on_hover_text("Replace all matches")
                        .clicked()
                    {
                        replace_all_matches(state, ide, palette);
                    }
                });
            }
        });

    ide.find_query = query;
    ide.find_replace_query = replace_text;

    // order matters: egui's logical modifier matching ignores extra Shift, so the
    // shift variant must be consumed first, or plain-Enter would swallow Shift+Enter.
    let shift_enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter));
    let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));

    if replace_focus {
        if enter {
            replace_current_match(state, ide, palette);
        }
        if shift_enter {
            replace_all_matches(state, ide, palette);
        }
    } else if enter || shift_enter {
        advance(ide, &matches, if shift_enter { -1 } else { 1 });
    }
}

fn find_matches(state: &SharedState, ide: &IdeState, palette: &ThemePalette) -> Vec<usize> {
    let query = ide.find_query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }

    // TODO: cache match indices and only re-scan on query or command changes; this linear scan runs on every keystroke and every frame.
    let s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    let Some(m) = s.macro_state.current_macro.as_ref() else {
        return Vec::new();
    };

    m.commands
        .iter()
        .enumerate()
        .filter(|(_, cmd)| super::row::search_text(cmd, palette).contains(&query))
        .map(|(idx, _)| idx)
        .collect()
}

fn advance(ide: &mut IdeState, matches: &[usize], delta: isize) {
    if matches.is_empty() {
        return;
    }
    let len = matches.len() as isize;
    let new_idx = (ide.find_match_idx as isize + delta).rem_euclid(len) as usize;
    ide.find_match_idx = new_idx;
    jump_to(ide, matches, new_idx);
}

fn jump_to(ide: &mut IdeState, matches: &[usize], idx: usize) {
    if let Some(&row) = matches.get(idx) {
        ide.selected.clear();
        ide.selected.insert(row);
        ide.last_clicked_idx = Some(row);
        ide.pending_scroll_to_row = Some(row);
    }
}

/// replaces inside the currently highlighted match, then steps to the next.
fn replace_current_match(state: &SharedState, ide: &mut IdeState, palette: &ThemePalette) {
    let from = ide.find_query.trim().to_lowercase();
    if from.is_empty() {
        return;
    }

    let matches = find_matches(state, ide, palette);
    let Some(&row) = matches.get(ide.find_match_idx.min(matches.len().saturating_sub(1))) else {
        set_status(state, "No matches");
        return;
    };

    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    s.macro_state.push_undo();
    let count = if let Some(m) = s.macro_state.current_macro.as_mut() {
        replace::replace_in_row(&mut m.commands, row, &from, &ide.find_replace_query)
    } else {
        0
    };
    sync_event_count(&mut s.macro_state);
    s.status_msg = if count > 0 {
        format!("Replaced {count} occurrence{}", plural(count))
    } else {
        "No replaceable text in this match".to_string()
    };
    drop(s);

    step_to_next_match(state, ide, palette, row);
}

fn replace_all_matches(state: &SharedState, ide: &mut IdeState, palette: &ThemePalette) {
    let from = ide.find_query.trim().to_lowercase();
    if from.is_empty() {
        return;
    }

    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    s.macro_state.push_undo();
    let count = if let Some(m) = s.macro_state.current_macro.as_mut() {
        replace::replace_all_in(&mut m.commands, &from, &ide.find_replace_query)
    } else {
        0
    };
    sync_event_count(&mut s.macro_state);
    s.status_msg = if count > 0 {
        format!("Replaced {count} occurrence{}", plural(count))
    } else {
        "No matches to replace".to_string()
    };
    drop(s);

    let matches = find_matches(state, ide, palette);
    if matches.is_empty() {
        ide.find_match_idx = 0;
        ide.selected.clear();
    } else {
        ide.find_match_idx = ide.find_match_idx.min(matches.len() - 1);
        jump_to(ide, &matches, ide.find_match_idx);
    }
}

/// steps to the next still-matching row after the one just edited; else the first match after it, wrapping to the top.
fn step_to_next_match(
    state: &SharedState,
    ide: &mut IdeState,
    palette: &ThemePalette,
    replaced_row: usize,
) {
    let matches = find_matches(state, ide, palette);
    if matches.is_empty() {
        ide.find_match_idx = 0;
        ide.selected.clear();
        return;
    }
    let next = match matches.iter().position(|&r| r == replaced_row) {
        Some(pos) => (pos + 1) % matches.len(),
        None => matches.iter().position(|&r| r > replaced_row).unwrap_or(0),
    };
    ide.find_match_idx = next;
    jump_to(ide, &matches, next);
}

fn set_status(state: &SharedState, msg: &str) {
    if let Ok(mut s) = state.lock() {
        s.status_msg = msg.to_string();
    }
}
