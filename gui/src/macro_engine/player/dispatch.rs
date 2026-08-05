use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use wmacro_core_types::{Hotkey, MacroEvent, MousePosition};
use crate::backend::ClickBackend;
use crate::state::SharedState;

use crate::macro_engine::player::models::{HeldState, PlaybackParams};
use crate::macro_engine::player::utils::macro_button_to_click;

pub fn handle_pause_and_step(
    paused: &AtomicBool, step: &AtomicBool, kill: &AtomicBool,
    loop_start: &mut Instant, held: &HeldState,
) -> bool {
    let mut pause_start = None;
    let mut inputs_suspended = false;

    while paused.load(Ordering::Relaxed) {
        pause_start = pause_start.or_else(|| Some(Instant::now()));
        suspend_inputs_if_needed(&mut inputs_suspended, held);

        if kill.load(Ordering::Relaxed) { return false; }
        if clear_step_if_active(step) { break; }

        std::thread::sleep(Duration::from_millis(10));
    }

    resume_if_was_paused(pause_start, loop_start, inputs_suspended, held);
    !kill.load(Ordering::Relaxed)
}

fn suspend_inputs_if_needed(suspended: &mut bool, held: &HeldState) {
    if *suspended { return; }
    if let Some(backend_mutex) = crate::GLOBAL_BACKEND.get() {
        if let Ok(mut backend) = backend_mutex.lock() {
            held.suspend(&mut **backend);
            *suspended = true;
        }
    }
}

fn clear_step_if_active(step: &AtomicBool) -> bool {
    if step.load(Ordering::Relaxed) {
        step.store(false, Ordering::Relaxed);
        return true;
    }
    false
}

fn resume_if_was_paused(pause_start: Option<Instant>, loop_start: &mut Instant, inputs_suspended: bool, held: &HeldState) {
    if let Some(ps) = pause_start {
        *loop_start += ps.elapsed();
        if inputs_suspended { resume_inputs(held); }
    }
}

fn resume_inputs(held: &HeldState) {
    if let Some(backend_mutex) = crate::GLOBAL_BACKEND.get() {
        if let Ok(mut backend) = backend_mutex.lock() {
            held.resume(&mut **backend);
        }
    }
}

pub fn execute_type_text(text: &str) -> Result<(), String> {
    let backend_mutex = crate::GLOBAL_BACKEND.get().ok_or("Global backend not initialized")?;
    let mut backend = backend_mutex.lock().map_err(|_| "Failed to lock global backend")?;
    backend.type_text(text).map_err(|e| e.to_string())
}

pub fn execute_event_dispatch(
    event: &MacroEvent, params: &PlaybackParams, mouse_jitter: (i32, i32), held: &mut HeldState,
) -> Result<(), String> {
    let backend_mutex = crate::GLOBAL_BACKEND.get().ok_or("Global backend not initialized")?;
    let mut backend = backend_mutex.lock().map_err(|_| "Failed to lock global backend")?;

    let res = dispatch_single_event(&mut **backend, event, params.record_hotkey.as_ref(), params.play_hotkey.as_ref(), mouse_jitter);
    if res.is_ok() { held.update(event); }
    res
}

fn dispatch_single_event(
    backend: &mut dyn ClickBackend, event: &MacroEvent, record_hk: Option<&Hotkey>, play_hk: Option<&Hotkey>, jitter: (i32, i32),
) -> Result<(), String> {
    match event {
        MacroEvent::Delay(_) => Ok(()),
        MacroEvent::MouseMove { x, y, .. } => backend.move_to(*x, *y),
        MacroEvent::Click { position, button, hold_time_ms, .. } => execute_click(backend, position, button, *hold_time_ms, jitter),
        MacroEvent::MouseDown { position, button, .. } => execute_mouse_down(backend, position, button, jitter),
        MacroEvent::MouseUp { button, .. } => backend.release(&macro_button_to_click(button)),
        MacroEvent::Scroll { dx, dy, .. } => backend.scroll(*dx, *dy),
        MacroEvent::KeyDown { key, code, .. } => execute_key_down(backend, key, *code, record_hk, play_hk),
        MacroEvent::KeyUp { key, code, .. } => execute_key_up(backend, key, *code, record_hk, play_hk),
        MacroEvent::KeyPress { key, code, hold_time_ms } => execute_key_press(backend, key, *code, *hold_time_ms, record_hk, play_hk),
    }
}

fn execute_click(backend: &mut dyn ClickBackend, pos: &MousePosition, btn: &wmacro_core_types::MacroButton, hold_ms: u32, jitter: (i32, i32)) -> Result<(), String> {
    move_to_position(backend, pos, jitter)?;
    let b = macro_button_to_click(btn);
    backend.press(&b)?;
    std::thread::sleep(Duration::from_millis(hold_ms as u64));
    backend.release(&b)
}

fn execute_mouse_down(backend: &mut dyn ClickBackend, pos: &MousePosition, btn: &wmacro_core_types::MacroButton, jitter: (i32, i32)) -> Result<(), String> {
    move_to_position(backend, pos, jitter)?;
    backend.press(&macro_button_to_click(btn))
}

fn move_to_position(backend: &mut dyn ClickBackend, pos: &MousePosition, jitter: (i32, i32)) -> Result<(), String> {
    if let MousePosition::Absolute { x, y } = pos {
        backend.move_to(*x + jitter.0, *y + jitter.1)?;
    }
    Ok(())
}

fn is_suppressed(code: u16, record_hk: Option<&Hotkey>, play_hk: Option<&Hotkey>) -> bool {
    record_hk.is_some_and(|hk| hk.code == code) || play_hk.is_some_and(|hk| hk.code == code)
}

fn execute_key_down(backend: &mut dyn ClickBackend, key: &str, code: u16, record: Option<&Hotkey>, play: Option<&Hotkey>) -> Result<(), String> {
    if is_suppressed(code, record, play) { return Ok(()); }
    backend.key_down(key, code)
}

fn execute_key_up(backend: &mut dyn ClickBackend, key: &str, code: u16, record: Option<&Hotkey>, play: Option<&Hotkey>) -> Result<(), String> {
    if is_suppressed(code, record, play) { return Ok(()); }
    backend.key_up(key, code)
}

fn execute_key_press(backend: &mut dyn ClickBackend, key: &str, code: u16, hold_ms: u32, record: Option<&Hotkey>, play: Option<&Hotkey>) -> Result<(), String> {
    if is_suppressed(code, record, play) { return Ok(()); }
    backend.key_down(key, code)?;
    std::thread::sleep(Duration::from_millis(hold_ms as u64));
    backend.key_up(key, code)
}

pub fn abort_playback_on_error(state: &SharedState, held: &mut HeldState, error_msg: String) {
    if let Some(backend_mutex) = crate::GLOBAL_BACKEND.get() {
        if let Ok(mut backend) = backend_mutex.lock() {
            held.release_all(&mut **backend);
        }
    }
    if let Ok(mut s) = state.lock() {
        s.macro_state.playing = false;
        s.status_msg = format!("Playback error: {}", error_msg);
    }
}
