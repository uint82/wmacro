//! right-click context menu for a command row: edit, insert, duplicate, move, and fold actions.

use super::actions::EditorActions;
use crate::ui::IdeState;
use crate::ui::block_analysis::BlockAnalysis;
use crate::ui::modals::{Modal, modal_from_command};
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::MacroCommand;

#[allow(clippy::too_many_arguments)]
pub fn render_context_menu(
    ui: &mut egui::Ui,
    ide: &mut IdeState,
    clicked_idx: usize,
    _palette: &ThemePalette,
    edit_modal: &mut Option<Modal>,
    cmd: &MacroCommand,
    commands: &[MacroCommand],
    actions: &mut EditorActions,
    analysis: &BlockAnalysis,
) {
    // right-clicking an unselected row re-targets the selection to it, so the menu always acts on exactly what the user pointed at.
    if !ide.selected.contains(&clicked_idx) {
        ide.selected.clear();
        ide.selected.insert(clicked_idx);
        ide.last_clicked_idx = Some(clicked_idx);
    }

    let sel_count = ide.selected.len();

    let can_move_up = !ide.selected.is_empty() && !ide.selected.contains(&0);
    let can_move_down =
        !ide.selected.is_empty() && !ide.selected.contains(&(commands.len().saturating_sub(1)));

    if ui
        .add_enabled(
            can_move_up,
            egui::Button::new(format!("{}  Move Up", egui_phosphor::regular::ARROW_UP)),
        )
        .clicked()
    {
        actions.move_up = true;
        ui.close();
    }

    if ui
        .add_enabled(
            can_move_down,
            egui::Button::new(format!("{}  Move Down", egui_phosphor::regular::ARROW_DOWN)),
        )
        .clicked()
    {
        actions.move_down = true;
        ui.close();
    }

    if sel_count == 1
        && ui
            .button(format!("{}  Edit", egui_phosphor::regular::PENCIL))
            .clicked()
    {
        if let Some(m) = modal_from_command(cmd, clicked_idx) {
            *edit_modal = Some(m);
        }
        ui.close();
    }

    if analysis
        .fold_end
        .get(clicked_idx)
        .copied()
        .flatten()
        .is_some()
    {
        // only rows that open a block get a fold toggle, so the option appears exactly where it can do something useful.
        let folded = ide.folded_blocks.contains(&clicked_idx);
        let (icon, label) = if folded {
            (egui_phosphor::regular::CARET_RIGHT, "Unfold")
        } else {
            (egui_phosphor::regular::CARET_DOWN, "Fold")
        };
        if ui.button(format!("{}  {label}", icon)).clicked() {
            actions.fold_toggle = Some(clicked_idx);
            ui.close();
        }
    }

    if ui
        .button(format!(
            "{}  Duplicate ({})",
            egui_phosphor::regular::COPY,
            sel_count
        ))
        .clicked()
    {
        actions.duplicate_selected = true;
        ui.close();
    }

    ui.separator();

    if ui
        .button(format!(
            "{}  Copy ({})",
            egui_phosphor::regular::FILES,
            sel_count
        ))
        .clicked()
    {
        actions.copy_selected = true;
        ui.close();
    }

    if ui
        .add_enabled(
            !ide.clipboard.is_empty(),
            egui::Button::new(format!(
                "{}  Paste After",
                egui_phosphor::regular::CLIPBOARD
            )),
        )
        .clicked()
    {
        actions.paste_after = Some(clicked_idx);
        ui.close();
    }

    ui.separator();

    let has_delay = ide.selected.iter().any(|&i| {
        matches!(
            commands.get(i),
            Some(MacroCommand::Delay { .. })
                | Some(MacroCommand::Action(wmacro_core_types::MacroEvent::Delay(
                    _
                )))
        )
    });
    if has_delay {
        if ui
            .button(format!(
                "{}  Edit Delay (Bulk)",
                egui_phosphor::regular::TIMER
            ))
            .clicked()
        {
            actions.bulk_delay = true;
            ui.close();
        }
        ui.separator();
    }

    if ui.button("Select All").clicked() {
        actions.select_all = true;
        ui.close();
    }

    if ui
        .add_enabled(sel_count > 0, egui::Button::new("Deselect All"))
        .clicked()
    {
        actions.deselect_all = true;
        ui.close();
    }

    ui.separator();

    if ui
        .button(format!(
            "{}  Delete ({})",
            egui_phosphor::regular::TRASH,
            sel_count
        ))
        .clicked()
    {
        actions.delete_selected = true;
        ui.close();
    }
}

pub fn render_global_context_menu(
    ui: &mut egui::Ui,
    ide: &mut IdeState,
    actions: &mut EditorActions,
) {
    // empty-background menu: same clipboard commands as the row menu, minus the row-relative ones. TODO: factor these shared items into one helper.
    if ui
        .add_enabled(
            !ide.clipboard.is_empty(),
            egui::Button::new(format!("{}  Paste", egui_phosphor::regular::CLIPBOARD)),
        )
        .clicked()
    {
        actions.paste_end = true;
        ui.close();
    }

    ui.separator();

    if ui.button("Select All").clicked() {
        actions.select_all = true;
        ui.close();
    }

    if ui
        .add_enabled(!ide.selected.is_empty(), egui::Button::new("Deselect All"))
        .clicked()
    {
        actions.deselect_all = true;
        ui.close();
    }

    ui.separator();

    if ui
        .add_enabled(
            !ide.selected.is_empty(),
            egui::Button::new(format!(
                "{}  Delete ({})",
                egui_phosphor::regular::TRASH,
                ide.selected.len()
            )),
        )
        .clicked()
    {
        actions.delete_selected = true;
        ui.close();
    }
}
