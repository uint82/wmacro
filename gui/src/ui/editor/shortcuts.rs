//! keyboard shortcuts for the editor: selection, copy/cut/paste, undo/redo, find and replace, and row movement.

use crate::state::SharedState;
use crate::ui::IdeState;
use eframe::egui;
use wmacro_core_types::{Macro, MacroCommand};

use super::actions::{modal_for_selection, plural};

#[derive(Default)]
struct KeyState {
    select_all: bool,
    copy: bool,
    cut: bool,
    paste: Option<String>,
    duplicate: bool,
    undo: bool,
    redo: bool,
    arrow_up: bool,
    arrow_down: bool,
    shift: bool,
    edit: bool,
    delete: bool,
    find: bool,
    find_replace: bool,
    escape: bool,
    move_up: bool,
    move_down: bool,
}

pub fn handle_shortcuts(ctx: &egui::Context, state: &SharedState, ide: &mut IdeState) {
    // keyboard input goes to the focused widget (text fields, modals), so steal nothing while something else is typing.
    if ctx.memory(|m| m.focused().is_some()) {
        return;
    }

    let keys = ctx.input(|i| {
        let ctrl = i.modifiers.ctrl;
        let shift = i.modifiers.shift;
        let alt = i.modifiers.alt;

        KeyState {
            select_all: ctrl && i.key_pressed(egui::Key::A),
            copy: i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
            cut: i.events.iter().any(|e| matches!(e, egui::Event::Cut)),
            paste: i.events.iter().find_map(|e| match e {
                egui::Event::Paste(text) => Some(text.clone()),
                _ => None,
            }),
            duplicate: ctrl && i.key_pressed(egui::Key::D),
            undo: ctrl && i.key_pressed(egui::Key::Z) && !shift,
            redo: (ctrl && i.key_pressed(egui::Key::Z) && shift)
                || (ctrl && i.key_pressed(egui::Key::Y)),
            arrow_up: !alt && i.key_pressed(egui::Key::ArrowUp),
            arrow_down: !alt && i.key_pressed(egui::Key::ArrowDown),
            shift,
            edit: !ctrl && i.key_pressed(egui::Key::Enter),
            delete: i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            find: ctrl && shift && i.key_pressed(egui::Key::F),
            find_replace: ctrl && shift && i.key_pressed(egui::Key::H),
            escape: i.key_pressed(egui::Key::Escape),
            move_up: alt && i.key_pressed(egui::Key::ArrowUp),
            move_down: alt && i.key_pressed(egui::Key::ArrowDown),
        }
    });

    if keys.select_all {
        handle_select_all(state, ide);
    }
    if keys.copy {
        handle_copy(ctx, state, ide);
    }
    if keys.cut {
        handle_cut(ctx, state, ide);
    }
    if keys.paste.is_some() {
        handle_paste(state, ide, keys.paste.as_deref());
    }
    if keys.duplicate {
        handle_duplicate(state, ide);
    }
    if keys.undo {
        handle_undo(state, ide);
    }
    if keys.redo {
        handle_redo(state, ide);
    }
    if keys.arrow_up {
        handle_arrow_move(state, ide, MoveDir::Up, keys.shift);
    }
    if keys.arrow_down {
        handle_arrow_move(state, ide, MoveDir::Down, keys.shift);
    }
    if keys.edit && !ide.find_open {
        handle_edit_selected(state, ide);
    }
    if keys.delete {
        handle_delete(state, ide);
    }
    if keys.find {
        ide.find_open = true;
        ide.find_just_opened = true;
        ide.find_replace_mode = false;
    }
    if keys.find_replace {
        ide.find_open = true;
        ide.find_just_opened = true;
        ide.find_replace_mode = true;
    }
    if keys.escape && ide.find_open {
        ide.find_open = false;
    }
    if keys.move_up {
        handle_move_selection(state, ide, MoveDir::Up);
    }
    if keys.move_down {
        handle_move_selection(state, ide, MoveDir::Down);
    }
}
fn handle_select_all(state: &SharedState, ide: &mut IdeState) {
    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    let count = s
        .macro_state
        .current_macro
        .as_ref()
        .map(|m| m.commands.len())
        .unwrap_or(0);
    ide.selected = (0..count).collect();
    ide.last_clicked_idx = Some(0);
    s.status_msg = format!("Selected {count} command{}", plural(count));
}

fn handle_copy(ctx: &egui::Context, state: &SharedState, ide: &mut IdeState) {
    if ide.selected.is_empty() {
        return;
    }

    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    let Some(m) = s.macro_state.current_macro.as_ref() else {
        return;
    };

    let mut idxs: Vec<usize> = ide.selected.iter().copied().collect();
    idxs.sort_unstable();
    ide.clipboard = idxs
        .iter()
        .filter_map(|&idx| m.commands.get(idx).cloned())
        .collect();

    copy_to_os_clipboard(ctx, m.name.as_str(), &ide.clipboard);

    let count = ide.clipboard.len();
    s.status_msg = format!("Copied {count} command{}", plural(count));
}

fn copy_to_os_clipboard(ctx: &egui::Context, name: &str, commands: &[MacroCommand]) {
    let mut m = Macro::new(name);
    m.commands = commands.to_vec();
    let text = crate::macro_engine::script::serialize(&m);
    ctx.copy_text(text);
}

fn handle_paste(state: &SharedState, ide: &mut IdeState, os_text: Option<&str>) {
    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });

    // TODO: re-check the OS clipboard on a timer or window focus; egui only reports it when a paste event fires.
    // the OS clipboard is authoritative (like most editors); the in-app clipboard is a fallback when the OS read failed or was empty.
    let (commands, from_os_text) = if let Some(text) = os_text {
        match crate::macro_engine::script::deserialize(text) {
            Ok(parsed) if !parsed.commands.is_empty() => (parsed.commands, true),
            _ => {
                s.status_msg = String::from("Clipboard text is not a wmacro script");
                return;
            }
        }
    } else if !ide.clipboard.is_empty() {
        (ide.clipboard.clone(), false)
    } else {
        s.status_msg = String::from("Clipboard is empty");
        return;
    };

    s.macro_state.push_undo();
    let Some(m) = s.macro_state.current_macro.as_mut() else {
        return;
    };

    let insert_idx = ide
        .selected
        .iter()
        .max()
        .copied()
        .map(|i| i + 1)
        .unwrap_or(m.commands.len());

    for (i, cmd) in commands.iter().enumerate() {
        let mut cloned = cmd.clone();
        super::actions::rename_label_if_needed(&mut cloned, &m.commands);
        m.commands.insert(insert_idx + i, cloned);
    }
    s.macro_state.events_captured = m.commands.len();

    let count = commands.len();
    let suffix = if from_os_text {
        " from clipboard text"
    } else {
        ""
    };
    s.status_msg = format!("Pasted {count} command{}{suffix}", plural(count));
}

fn handle_duplicate(state: &SharedState, ide: &mut IdeState) {
    if ide.selected.is_empty() {
        return;
    }

    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    s.macro_state.push_undo();
    let Some(m) = s.macro_state.current_macro.as_mut() else {
        return;
    };

    let mut idxs: Vec<usize> = ide.selected.iter().copied().collect();
    idxs.sort_unstable();
    let to_duplicate: Vec<_> = idxs
        .iter()
        .filter_map(|&idx| m.commands.get(idx).cloned())
        .collect();
    let insert_idx = idxs
        .last()
        .copied()
        .map(|i| i + 1)
        .unwrap_or(m.commands.len());

    for (i, cmd) in to_duplicate.iter().enumerate() {
        let mut cloned = cmd.clone();
        super::actions::rename_label_if_needed(&mut cloned, &m.commands);
        m.commands.insert(insert_idx + i, cloned);
    }
    s.macro_state.events_captured = m.commands.len();

    let count = to_duplicate.len();
    s.status_msg = format!("Duplicated {count} command{}", plural(count));
}

fn handle_delete(state: &SharedState, ide: &mut IdeState) {
    if ide.selected.is_empty() {
        return;
    }

    let mut idxs: Vec<usize> = ide.selected.iter().copied().collect();
    idxs.sort_unstable_by(|a, b| b.cmp(a));

    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    s.macro_state.push_undo();
    let Some(m) = s.macro_state.current_macro.as_mut() else {
        return;
    };

    for &idx in &idxs {
        m.commands.remove(idx);
    }
    s.macro_state.events_captured = m.commands.len();
    let count = idxs.len();
    s.status_msg = format!("Deleted {count} command{}", plural(count));
    ide.selected.clear();
}

fn handle_cut(ctx: &egui::Context, state: &SharedState, ide: &mut IdeState) {
    if ide.selected.is_empty() {
        return;
    }

    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    s.macro_state.push_undo();
    let Some(m) = s.macro_state.current_macro.as_mut() else {
        return;
    };

    let mut idxs: Vec<usize> = ide.selected.iter().copied().collect();
    idxs.sort_unstable();
    let copied: Vec<MacroCommand> = idxs
        .iter()
        .filter_map(|&idx| m.commands.get(idx).cloned())
        .collect();

    copy_to_os_clipboard(ctx, m.name.as_str(), &copied);

    for &idx in idxs.iter().rev() {
        m.commands.remove(idx);
    }
    s.macro_state.events_captured = m.commands.len();

    ide.clipboard = copied;
    ide.selected.clear();
    let count = ide.clipboard.len();
    s.status_msg = format!("Cut {count} command{}", plural(count));
}

fn handle_undo(state: &SharedState, ide: &mut IdeState) {
    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    if s.macro_state.undo() {
        ide.selected.clear();
        ide.last_clicked_idx = None;
        s.status_msg = String::from("Undo");
    }
}

fn handle_redo(state: &SharedState, ide: &mut IdeState) {
    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    if s.macro_state.redo() {
        ide.selected.clear();
        ide.last_clicked_idx = None;
        s.status_msg = String::from("Redo");
    }
}

#[derive(Clone, Copy)]
enum MoveDir {
    Up,
    Down,
}

fn handle_arrow_move(state: &SharedState, ide: &mut IdeState, dir: MoveDir, extend: bool) {
    let s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    let Some(m) = s.macro_state.current_macro.as_ref() else {
        return;
    };
    if m.commands.is_empty() {
        return;
    }

    let visible_rows = super::compute_row_layout(
        &m.commands,
        &crate::ui::block_analysis::analyze_blocks(&m.commands),
        &ide.folded_blocks,
    )
    .visible_rows;

    let last_idx = m.commands.len() - 1;
    let mut target = match dir {
        MoveDir::Up => ide
            .selected
            .iter()
            .min()
            .copied()
            .map(|min| min.saturating_sub(1))
            .unwrap_or(0),
        MoveDir::Down => ide
            .selected
            .iter()
            .max()
            .copied()
            .map(|max| max.saturating_add(1).min(last_idx))
            .unwrap_or(0),
    };
    target = snap_into_view(target, &visible_rows, dir);

    if extend {
        let anchor = ide.last_clicked_idx.unwrap_or(target);
        let lo = anchor.min(target);
        let hi = anchor.max(target);
        ide.selected = (lo..=hi).collect();
        ide.last_clicked_idx = Some(anchor);
    } else {
        ide.selected.clear();
        ide.selected.insert(target);
        ide.last_clicked_idx = Some(target);
    }

    ide.pending_scroll_to_row = Some(target);
}

/// arrow navigation skips rows hidden by folds, so folded blocks behave like one collapsed item.
fn snap_into_view(target: usize, visible: &[usize], dir: MoveDir) -> usize {
    if visible.contains(&target) {
        return target;
    }
    match dir {
        MoveDir::Up => visible
            .iter()
            .rev()
            .find(|&&r| r <= target)
            .copied()
            .unwrap_or(target),
        MoveDir::Down => visible
            .iter()
            .find(|&&r| r >= target)
            .copied()
            .unwrap_or(target),
    }
}

fn handle_move_selection(state: &SharedState, ide: &mut IdeState, dir: MoveDir) {
    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    s.macro_state.push_undo();
    let Some(m) = s.macro_state.current_macro.as_mut() else {
        return;
    };

    match dir {
        MoveDir::Up => super::actions::handle_move_up(ide, &mut m.commands),
        MoveDir::Down => super::actions::handle_move_down(ide, &mut m.commands),
    }
    s.macro_state.events_captured = m.commands.len();

    let moved = ide.selected.len();
    let direction = match dir {
        MoveDir::Up => "up",
        MoveDir::Down => "down",
    };
    s.status_msg = format!("Moved {moved} command{} {direction}", plural(moved));
}

fn handle_edit_selected(state: &SharedState, ide: &mut IdeState) {
    let s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    let Some(m) = s.macro_state.current_macro.as_ref() else {
        return;
    };
    if let Some(modal) = modal_for_selection(m, ide) {
        ide.modal = modal;
    }
}
