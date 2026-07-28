use crate::state::SharedState;
use crate::ui::IdeState;
use eframe::egui;

pub fn handle_shortcuts(ctx: &egui::Context, state: &SharedState, ide: &mut IdeState) {
    ctx.input(|i| {
        let ctrl = i.modifiers.ctrl;

        if ctrl && i.key_pressed(egui::Key::A) {
            handle_select_all(state, ide);
        }

        if i.events.iter().any(|e| matches!(e, egui::Event::Copy)) {
            handle_copy(state, ide);
        }

        if i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))) {
            handle_paste(state, ide);
        }

        if ctrl && i.key_pressed(egui::Key::D) {
            handle_duplicate(state, ide);
        }

        if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
            handle_delete(state, ide);
        }
    });
}

fn handle_select_all(state: &SharedState, ide: &mut IdeState) {
    let count = state
        .lock()
        .unwrap_or_else(|e| {
            log::error!("State mutex poisoned: {e}");
            e.into_inner()
        })
        .macro_state
        .current_macro
        .as_ref()
        .map(|m| m.commands.len())
        .unwrap_or(0);
    ide.selected = (0..count).collect();
}

fn handle_copy(state: &SharedState, ide: &mut IdeState) {
    if ide.selected.is_empty() {
        return;
    }

    let s = state.lock().unwrap_or_else(|e| {
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
}

fn handle_paste(state: &SharedState, ide: &mut IdeState) {
    if ide.clipboard.is_empty() {
        return;
    }

    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
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

    for (i, cmd) in ide.clipboard.iter().enumerate() {
        let mut cloned = cmd.clone();
        super::actions::rename_label_if_needed(&mut cloned, &m.commands);
        m.commands.insert(insert_idx + i, cloned);
    }
    s.macro_state.events_captured = m.commands.len();
}

fn handle_duplicate(state: &SharedState, ide: &mut IdeState) {
    if ide.selected.is_empty() {
        return;
    }

    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
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
    if let Some(m) = s.macro_state.current_macro.as_mut() {
        for &idx in &idxs {
            m.commands.remove(idx);
        }
        s.macro_state.events_captured = m.commands.len();
    }
    ide.selected.clear();
}
