//! the `ModalOutcome` enum: what a modal reports each frame (open, cancelled, commit, picker request).

use crate::ui::screen_picker;
use wmacro_core_types::MacroCommand;

/// the outcome a modal produces each frame.
#[derive(Default, PartialEq)]
pub enum ModalOutcome {
    /// modal is still open; no action taken.
    #[default]
    Open,
    /// user cancelled (Cancel button or Escape/backdrop click).
    Cancelled,
    /// user confirmed an add or edit.
    Commit(MacroCommand),
    /// modal wants the screen-picker overlay opened.
    OpenPicker { target: screen_picker::PickerTarget },
}

// TODO: add a variant that carries an error message so modals can report validation failures.
