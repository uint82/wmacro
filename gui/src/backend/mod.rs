use std::sync::mpsc::Sender;
use core_types::{ClickButton, ClickType, DaemonEvent};

pub mod ipc_backend;
pub mod keymap;

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

    fn start_recording(&mut self) -> Result<(), String> { Ok(()) }
    fn stop_recording(&mut self) -> Result<(), String> { Ok(()) }
}

pub fn build_backend(event_tx: Sender<DaemonEvent>) -> Box<dyn ClickBackend> {
    let backend = ipc_backend::IpcBackend::new(event_tx).unwrap_or_else(|e| {
        panic!("FATAL: Root daemon not found ({}). Please start the wmacro daemon.", e);
    });

    log::info!("Successfully connected to the root daemon!");
    Box::new(backend)
}
