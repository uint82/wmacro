//! input backends: the shared `ClickBackend` trait plus its `ipc_backend` and `keymap` implementations.

use std::sync::mpsc::Sender;
use wmacro_core_types::{ClickButton, ClickType, DaemonEvent};

pub mod ipc_backend;
pub mod keymap;

/// the surface every input path shares: mouse, keyboard, and text all funnel through this trait so the macro engine never cares which backend answers.
pub trait ClickBackend: Send {
    fn move_to(&mut self, x: i32, y: i32) -> Result<(), String>;
    fn press(&mut self, button: &ClickButton) -> Result<(), String>;
    fn release(&mut self, button: &ClickButton) -> Result<(), String>;
    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String>;
    fn key_down(&mut self, key: &str, code: u16) -> Result<(), String>;
    fn key_up(&mut self, key: &str, code: u16) -> Result<(), String>;
    fn type_text(&mut self, text: &str) -> Result<(), String>;

    #[allow(clippy::too_many_arguments)]
    fn click(
        &mut self,
        target_x: i32,
        target_y: i32,
        current_x: i32,
        current_y: i32,
        button: &ClickButton,
        click_type: &ClickType,
        hold_duration_ms: u64,
        move_cursor: bool,
    ) -> Result<(), String>;

    fn start_recording(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn stop_recording(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct DummyBackend;

// swallows every call, keeping the app alive in a degraded state when the daemon is missing; playback simply does nothing instead of crashing.
impl ClickBackend for DummyBackend {
    fn move_to(&mut self, _x: i32, _y: i32) -> Result<(), String> {
        Ok(())
    }
    fn press(&mut self, _button: &ClickButton) -> Result<(), String> {
        Ok(())
    }
    fn release(&mut self, _button: &ClickButton) -> Result<(), String> {
        Ok(())
    }
    fn scroll(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
        Ok(())
    }
    fn key_down(&mut self, _key: &str, _code: u16) -> Result<(), String> {
        Ok(())
    }
    fn key_up(&mut self, _key: &str, _code: u16) -> Result<(), String> {
        Ok(())
    }
    fn type_text(&mut self, _text: &str) -> Result<(), String> {
        Ok(())
    }

    fn click(
        &mut self,
        _target_x: i32,
        _target_y: i32,
        _current_x: i32,
        _current_y: i32,
        _button: &ClickButton,
        _click_type: &ClickType,
        _hold_duration_ms: u64,
        _move_cursor: bool,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub fn build_backend(event_tx: Sender<DaemonEvent>) -> Result<Box<dyn ClickBackend>, String> {
    match ipc_backend::IpcBackend::new(event_tx) {
        Ok(backend) => {
            log::info!("Successfully connected to the root daemon!");
            Ok(Box::new(backend))
        }
        Err(e) => Err(format!(
            "Root daemon not found ({}). Please start the wmacro daemon.",
            e
        )),
    }
}
