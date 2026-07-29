use crate::state::SharedState;
use wmacro_core_types::{HardwareEvent, HardwareEventKind, Macro, MacroCommand, MacroEvent, MousePosition};
use log::{error, info, warn};
use std::sync::mpsc::Receiver;
use std::time::{Duration, SystemTime};

fn call_backend<F>(action: F) -> Result<(), String>
where
    F: FnOnce(&mut dyn crate::backend::ClickBackend) -> Result<(), String>,
{
    let backend_mutex = crate::GLOBAL_BACKEND
        .get()
        .ok_or("Global backend not initialized")?;
    let mut backend = backend_mutex
        .lock()
        .map_err(|_| "Failed to lock global backend")?;

    action(&mut **backend)
}

fn get_cursor_position(fallback_x: i32, fallback_y: i32) -> (i32, i32) {
    if let Some(pos) = crate::cursor::hyprland_cursorpos_socket() {
        pos
    } else {
        warn!(
            "hyprctl socket unavailable, falling back to cached state coordinates ({}, {})",
            fallback_x, fallback_y
        );
        (fallback_x, fallback_y)
    }
}

#[derive(Default)]
struct TimeTracker {
    first_event_time: Option<SystemTime>,
    total_macro_time_us: u64,
    pause_start: Option<SystemTime>,
    total_paused_us: u64,
    pending_move_delay: u64,
    last_recorded_pos: Option<(i32, i32)>,
}

impl TimeTracker {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn process_time(
        &mut self,
        event_time: SystemTime,
        session_start: Option<SystemTime>,
        is_paused: bool,
    ) -> Option<u64> {
        if is_paused {
            if self.pause_start.is_none() {
                self.pause_start = Some(event_time);
            }
            return None;
        } else if let Some(ps) = self.pause_start.take() {
            let duration = event_time.duration_since(ps).unwrap_or(Duration::ZERO);
            self.total_paused_us += duration.as_micros() as u64;
        }

        if let Some(start_t) = session_start {
            if event_time < start_t {
                return None;
            }
            if let Some(first_t) = self.first_event_time {
                if first_t < start_t {
                    self.reset();
                }
            }
        }

        let current_elapsed_us = match self.first_event_time {
            Some(start_t) => {
                let mut elapsed = event_time
                    .duration_since(start_t)
                    .unwrap_or(Duration::ZERO)
                    .as_micros() as u64;
                elapsed = elapsed.saturating_sub(self.total_paused_us);
                elapsed
            }
            None => {
                self.first_event_time = Some(event_time);
                self.total_paused_us = 0;
                0
            }
        };

        let delay_us = current_elapsed_us.saturating_sub(self.total_macro_time_us);
        self.total_macro_time_us = current_elapsed_us;

        Some(delay_us)
    }
}

struct RecorderConfig {
    is_recording: bool,
    is_paused: bool,
    session_start: Option<SystemTime>,
    record_mouse: bool,
    record_movements: bool,
    record_keyboard: bool,
    cursor_x: i32,
    cursor_y: i32,
}

impl RecorderConfig {
    fn snapshot(state: &SharedState) -> Self {
        if let Ok(s) = state.lock() {
            Self {
                is_recording: s.macro_state.recording,
                is_paused: s.macro_state.record_paused,
                session_start: s.macro_state.recording_start,
                record_mouse: s.macro_state.record_mouse,
                record_movements: s.macro_state.record_movements,
                record_keyboard: s.macro_state.record_keyboard,
                cursor_x: s.cursor_x,
                cursor_y: s.cursor_y,
            }
        } else {
            Self {
                is_recording: false,
                is_paused: false,
                session_start: None,
                record_mouse: true,
                record_movements: true,
                record_keyboard: true,
                cursor_x: 0,
                cursor_y: 0,
            }
        }
    }
}

fn map_hardware_event(
    event: &HardwareEvent,
    config: &RecorderConfig,
    last_recorded_pos: &mut Option<(i32, i32)>,
) -> Option<MacroEvent> {
    match event.kind {
        HardwareEventKind::MouseMove => {
            let (cx, cy) = get_cursor_position(config.cursor_x, config.cursor_y);
            let is_new_pos = *last_recorded_pos != Some((cx, cy));

            if config.record_mouse && config.record_movements && is_new_pos {
                *last_recorded_pos = Some((cx, cy));
                Some(MacroEvent::MouseMove { x: cx, y: cy })
            } else {
                None
            }
        }
        HardwareEventKind::MouseDown(ref button) => {
            if config.record_mouse {
                let (cx, cy) = get_cursor_position(config.cursor_x, config.cursor_y);
                *last_recorded_pos = Some((cx, cy));
                Some(MacroEvent::MouseDown {
                    position: MousePosition::Absolute { x: cx, y: cy },
                    button: button.clone(),
                    jitter: 0,
                })
            } else {
                None
            }
        }
        HardwareEventKind::MouseUp(ref button) => {
            if config.record_mouse {
                let (cx, cy) = get_cursor_position(config.cursor_x, config.cursor_y);
                *last_recorded_pos = Some((cx, cy));
                Some(MacroEvent::MouseUp {
                    position: MousePosition::Absolute { x: cx, y: cy },
                    button: button.clone(),
                    jitter: 0,
                })
            } else {
                None
            }
        }
        HardwareEventKind::KeyDown(ref key, code) => {
            if config.record_keyboard {
                Some(MacroEvent::KeyDown { key: key.clone(), code })
            } else {
                None
            }
        }
        HardwareEventKind::KeyUp(ref key, code) => {
            if config.record_keyboard {
                Some(MacroEvent::KeyUp { key: key.clone(), code })
            } else {
                None
            }
        }
        HardwareEventKind::Scroll { dx, dy } => {
            if config.record_mouse {
                Some(MacroEvent::Scroll { dx, dy })
            } else {
                None
            }
        }
    }
}

pub fn spawn_recorder(state: SharedState, rx: Receiver<HardwareEvent>) {
    std::thread::spawn(move || {
        let mut tracker = TimeTracker::default();

        while let Ok(event) = rx.recv() {
            let config = RecorderConfig::snapshot(&state);

            if !config.is_recording {
                tracker.reset();
                continue;
            }

            let Some(delay_us) = tracker.process_time(event.hardware_time, config.session_start, config.is_paused) else {
                continue;
            };

            let mapped_event = map_hardware_event(&event, &config, &mut tracker.last_recorded_pos);

            if let Some(ev) = mapped_event {
                let effective_delay = delay_us + std::mem::take(&mut tracker.pending_move_delay);
                commit_event(&state, effective_delay, ev);
            } else {
                tracker.pending_move_delay += delay_us;
            }
        }
    });
}

fn commit_event(state: &SharedState, delay_us: u64, event: MacroEvent) {
    if let Ok(mut s) = state.lock() {
        if !s.macro_state.recording {
            return;
        }

        let events_to_add = if delay_us > 0 { 2 } else { 1 };
        s.macro_state.events_captured += events_to_add;

        if let Some(ref mut m) = s.macro_state.current_macro {
            if delay_us > 0 {
                m.commands.push(MacroCommand::Action(MacroEvent::Delay(delay_us)));
            }
            m.commands.push(MacroCommand::Action(event));
        }
    }
}

pub fn start_recording(state: &SharedState, name: String, append: bool) {
    if let Ok(mut s) = state.lock() {
        if append {
            if s.macro_state.current_macro.is_none() {
                s.macro_state.current_macro = Some(Macro::new(name));
            }
            s.macro_state.events_captured = s.macro_state.current_macro.as_ref().unwrap().commands.len();
            s.status_msg = String::from("Recording (Appending)… (move & click to capture)");
        } else {
            s.macro_state.current_macro = Some(Macro::new(name));
            s.macro_state.events_captured = 0;
            s.status_msg = String::from("Recording… (move & click to capture)");
        }

        s.macro_state.recording = true;
        s.macro_state.record_paused = false;
        s.macro_state.record_paused_flag = Some(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
        s.macro_state.recording_start = Some(SystemTime::now());

        info!("Recording started (append: {}) at {:?}", append, s.macro_state.recording_start);
    }

    if let Err(e) = call_backend(|b| b.start_recording()) {
        if let Ok(mut s) = state.lock() {
            s.status_msg = format!("Recording started locally, but daemon call failed: {}", e);
        }
        error!("Failed to send StartRecording to the daemon: {}", e);
    }
}

pub fn stop_recording(state: &SharedState) {
    if let Ok(mut s) = state.lock() {
        s.macro_state.recording = false;
        s.macro_state.record_paused = false;

        let count = s.macro_state.events_captured;
        let duration = s.macro_state.recording_start.and_then(|t| t.elapsed().ok()).unwrap_or_default();
        let total_ms = s.macro_state.current_macro.as_ref().map(|m| m.total_duration_ms()).unwrap_or(0);

        s.status_msg = format!("Recorded {} events in {:.2?}", count, duration);
        info!(
            "Recording finished. Captured {} events in {:.2?}. Total macro event timeline duration: {}ms",
            count, duration, total_ms
        );
    }

    if let Err(e) = call_backend(|b| b.stop_recording()) {
        error!("Failed to send StopRecording to the daemon: {}", e);
    }
}
