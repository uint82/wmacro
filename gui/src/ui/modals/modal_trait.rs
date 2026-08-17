//! the `ModalWidget` trait every modal implements; the parent dispatches rendering through it.

use crate::state::SharedState;
use crate::ui::screen_picker::{PickerOutcome, PickerTarget};
use crate::ui::theme::ThemePalette;
use eframe::egui;

use super::types::ModalOutcome;

/// every modal implements this trait; the parent `render_modal` dispatches through it without knowing modal internals.
pub trait ModalWidget: Send {
    fn title(&self) -> String;

    /// render the body of the modal and return what should happen next.
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome;

    /// called when a hotkey capture produces a coordinate; only modals that use the cursor need to override it.
    fn on_capture(&mut self, _cx: i32, _cy: i32) {}

    /// called when the screen-picker completes; only modals that use the picker need to override it.
    fn on_picker_outcome(
        &mut self,
        _ctx: &egui::Context,
        _target: PickerTarget,
        _outcome: PickerOutcome,
    ) {
    }

    /// the index of the command being edited, or `None` when adding a new one.
    fn edit_idx(&self) -> Option<usize>;

    /// widget IDs whose auto-focus flag is cleared when the modal closes, so re-opening re-focuses the primary field.
    // TODO: consider deriving the primary field id from the modal type instead
    // of listing it per modal.
    fn autofocus_ids(&self) -> &[&'static str] {
        &[]
    }
}
