//! inline editing popup for the primary editable text field of a command row.

use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::MacroCommand;

use super::actions::EditorActions;

pub fn popup_id(row: usize) -> egui::Id {
    egui::Id::new("inline_edit").with(row)
}

/// reads or writes the primary editable text field of a command; `None` when it has no inline-editable field.
pub fn edit_field(
    cmd: &mut MacroCommand,
    new_text: Option<&str>,
) -> Option<(&'static str, String)> {
    let (label, field) = match cmd {
        MacroCommand::TypeText(text) => ("Text", text),
        MacroCommand::Label(name) => ("Name", name),
        MacroCommand::Goto(target) => ("Target", target),
        MacroCommand::OpenFile { path, .. } => ("Path", path),
        MacroCommand::PlayMacro(path) => ("Macro path", path),
        MacroCommand::GetClipboard { target } => ("Target variable", target),
        MacroCommand::Calculate { expression, .. } => ("Expression", expression),
        MacroCommand::Comment(text) => ("Comment", text),
        _ => return None,
    };

    let previous = field.clone();
    if let Some(text) = new_text {
        *field = text.to_string();
    }
    Some((label, previous))
}

/// renders the popover for `row` below its anchor; commits on Enter, cancels on Esc.
pub fn render_popup(
    ui: &mut egui::Ui,
    ide: &mut crate::ui::IdeState,
    palette: &ThemePalette,
    row: usize,
    anchor: &egui::Response,
    actions: &mut EditorActions,
) {
    let Some((edit_row, draft)) = ide.inline_edit.clone() else {
        return;
    };
    if edit_row != row {
        return;
    }

    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        ide.inline_edit = None;
        return;
    }

    egui::Popup::from_response(anchor)
        .id(popup_id(row))
        .frame(
            egui::Frame::popup(ui.style())
                .fill(palette.bg_element_alt)
                .stroke(egui::Stroke::new(1.0_f32, palette.border))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(10, 8)),
        )
        .show(|ui| {
            ui.label(
                egui::RichText::new("Edit")
                    .color(palette.text_muted)
                    .size(11.0),
            );

            let mut text = draft;
            // TODO: size the field to the value's length so long paths and expressions don't force horizontal scrolling.
            let field_id = egui::Id::new("inline_text").with(row);
            let response = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .id(field_id)
                    .desired_width(260.0)
                    .hint_text("Value"),
            );
            response.request_focus();

            // keep the draft live so edits survive re-renders.
            ide.inline_edit = Some((row, text.clone()));

            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                actions.pending_inline_commit = Some((row, text));
                ide.inline_edit = None;
            }
        });
}

/// closes the inline editor when the user clicks outside the popup (ignoring the owning row).
pub fn handle_outside_click(
    ctx: &egui::Context,
    ide: &mut crate::ui::IdeState,
    visible_row_rects: &[(usize, egui::Rect)],
) {
    let Some((row, _)) = &ide.inline_edit else {
        return;
    };
    if !ctx.input(|i| i.pointer.primary_clicked()) {
        return;
    }
    let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    let Some(popup_rect) = egui::AreaState::load(ctx, popup_id(*row)).map(|s| s.rect()) else {
        return;
    };
    if popup_rect.contains(pos) {
        return;
    }

    let click_on_owning_row = visible_row_rects
        .iter()
        .any(|(idx, rect)| idx == row && rect.contains(pos));
    if !click_on_owning_row {
        ide.inline_edit = None;
    }
}
