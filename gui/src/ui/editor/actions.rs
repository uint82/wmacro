use super::super::modals::Modal;
use super::IdeState;
use crate::state::SharedState;
use wmacro_core_types::MacroCommand;

#[derive(Default)]
pub struct EditorActions {
    pub delete_selected: bool,
    pub duplicate_selected: bool,
    pub copy_selected: bool,
    pub paste_after: Option<usize>,
    pub paste_end: bool,
    pub select_all: bool,
    pub deselect_all: bool,
    pub bulk_delay: bool,
    pub move_up: bool,
    pub move_down: bool,
    pub move_payload: Option<(Vec<usize>, usize)>,
}

fn deduplicate_label_name(base_name: &str, commands: &[MacroCommand]) -> String {
    let mut candidate = format!("{}_copy", base_name);
    let mut counter = 1;
    loop {
        let exists = commands.iter().any(|cmd| {
            matches!(cmd, MacroCommand::Label(existing) if existing == &candidate)
        });
        if !exists {
            return candidate;
        }
        counter += 1;
        candidate = format!("{}_copy{}", base_name, counter);
    }
}

pub(crate) fn rename_label_if_needed(cmd: &mut MacroCommand, commands: &[MacroCommand]) {
    if let MacroCommand::Label(name) = cmd {
        *name = deduplicate_label_name(name, commands);
    }
}

fn sorted_indices(ide: &IdeState) -> Vec<usize> {
    let mut idxs: Vec<usize> = ide.selected.iter().copied().collect();
    idxs.sort_unstable();
    idxs
}

fn handle_move_up(ide: &mut IdeState, commands: &mut [MacroCommand]) {
    let idxs = sorted_indices(ide);
    ide.selected.clear();
    for &idx in &idxs {
        if idx > 0 {
            commands.swap(idx, idx - 1);
            ide.selected.insert(idx - 1);
        }
    }
}

fn handle_move_down(ide: &mut IdeState, commands: &mut [MacroCommand]) {
    let mut idxs: Vec<usize> = ide.selected.iter().copied().collect();
    idxs.sort_unstable_by(|a, b| b.cmp(a));
    ide.selected.clear();
    let max_idx = commands.len().saturating_sub(1);
    for &idx in &idxs {
        if idx < max_idx {
            commands.swap(idx, idx + 1);
            ide.selected.insert(idx + 1);
        }
    }
}

fn handle_copy(ide: &mut IdeState, commands: &[MacroCommand]) {
    let idxs = sorted_indices(ide);
    ide.clipboard = idxs
        .iter()
        .filter_map(|&i| commands.get(i).cloned())
        .collect();
}

fn handle_duplicate(ide: &mut IdeState, commands: &mut Vec<MacroCommand>) {
    let idxs = sorted_indices(ide);
    let to_duplicate: Vec<MacroCommand> = idxs
        .iter()
        .filter_map(|&i| commands.get(i).cloned())
        .collect();

    let insert_idx = idxs
        .last()
        .copied()
        .map(|i| i + 1)
        .unwrap_or(commands.len());

    for (i, original) in to_duplicate.iter().enumerate() {
        let mut cloned = original.clone();
        rename_label_if_needed(&mut cloned, commands);
        commands.insert(insert_idx + i, cloned);
    }
}

fn handle_paste_at(ide: &IdeState, commands: &mut Vec<MacroCommand>, insert_idx: usize) {
    for (i, original) in ide.clipboard.iter().enumerate() {
        let mut cloned = original.clone();
        rename_label_if_needed(&mut cloned, commands);
        commands.insert(insert_idx + i, cloned);
    }
}

fn handle_delete(ide: &mut IdeState, commands: &mut Vec<MacroCommand>) {
    let mut idxs: Vec<usize> = ide.selected.iter().copied().collect();
    idxs.sort_unstable_by(|a, b| b.cmp(a));
    for &idx in &idxs {
        commands.remove(idx);
    }
    ide.selected.clear();
}

fn handle_move_payload(
    ide: &mut IdeState,
    commands: &mut Vec<MacroCommand>,
    from_indices: &[usize],
    to_idx: usize,
) {
    let mut sorted_from = from_indices.to_vec();
    sorted_from.sort_unstable();

    let to_move: Vec<MacroCommand> = sorted_from
        .iter()
        .map(|&i| commands[i].clone())
        .collect();

    for &i in sorted_from.iter().rev() {
        commands.remove(i);
    }

    let actual_target = to_idx.min(commands.len());
    for (offset, cmd) in to_move.into_iter().enumerate() {
        commands.insert(actual_target + offset, cmd);
    }

    ide.selected.clear();
    for offset in 0..from_indices.len() {
        ide.selected.insert(actual_target + offset);
    }
}

fn sync_event_count(macro_state: &mut crate::state::MacroState) {
    if let Some(m) = macro_state.current_macro.as_ref() {
        macro_state.events_captured = m.commands.len();
    }
}

pub fn handle_editor_actions(
    state: &SharedState,
    ide: &mut IdeState,
    actions: &EditorActions,
) {
    if actions.deselect_all {
        ide.selected.clear();
    }

    if actions.bulk_delay {
        let idxs: Vec<usize> = ide.selected.iter().copied().collect();
        ide.modal = Modal::Delay {
            value: 100,
            unit: crate::state::DelayUnit::Milliseconds,
            target_indices: idxs,
        };
    }

    let mut s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });

    if actions.select_all {
        if let Some(m) = s.macro_state.current_macro.as_ref() {
            ide.selected = (0..m.commands.len()).collect();
        }
    }

    if actions.move_up {
        if let Some(m) = s.macro_state.current_macro.as_mut() {
            handle_move_up(ide, &mut m.commands);
        }
    }

    if actions.move_down {
        if let Some(m) = s.macro_state.current_macro.as_mut() {
            handle_move_down(ide, &mut m.commands);
        }
    }

    if actions.copy_selected {
        if let Some(m) = s.macro_state.current_macro.as_ref() {
            handle_copy(ide, &m.commands);
        }
    }

    if actions.duplicate_selected {
        if let Some(m) = s.macro_state.current_macro.as_mut() {
            handle_duplicate(ide, &mut m.commands);
        }
        sync_event_count(&mut s.macro_state);
    }

    if let Some(target_idx) = actions.paste_after {
        if let Some(m) = s.macro_state.current_macro.as_mut() {
            handle_paste_at(ide, &mut m.commands, target_idx + 1);
        }
        sync_event_count(&mut s.macro_state);
    }

    if actions.paste_end {
        if let Some(m) = s.macro_state.current_macro.as_mut() {
            for cmd in ide.clipboard.iter() {
                m.commands.push(cmd.clone());
            }
        }
        sync_event_count(&mut s.macro_state);
    }

    if actions.delete_selected {
        if let Some(m) = s.macro_state.current_macro.as_mut() {
            handle_delete(ide, &mut m.commands);
        }
        sync_event_count(&mut s.macro_state);
    }

    if let Some((from_indices, to_idx)) = &actions.move_payload {
        if let Some(m) = s.macro_state.current_macro.as_mut() {
            handle_move_payload(ide, &mut m.commands, from_indices, *to_idx);
        }
    }
}
