use std::collections::HashSet;
use std::time::{Duration, Instant};
use log::{info, warn, error};
use wmacro_core_types::{Hotkey, MacroButton, MacroCommand, MacroEvent, SmartPathOptions};
use crate::backend::ClickBackend;

use crate::macro_engine::player::frame::ExecFrame;
use crate::macro_engine::player::utils::macro_button_to_click;

#[derive(Default)]
pub struct HeldState {
    pub buttons: HashSet<MacroButton>,
    pub keys: HashSet<u16>,
}

impl HeldState {
    pub fn update(&mut self, event: &MacroEvent) {
        match event {
            MacroEvent::MouseDown { button, .. } => { self.buttons.insert(button.clone()); },
            MacroEvent::MouseUp { button, .. } => { self.buttons.remove(button); },
            MacroEvent::KeyDown { code, .. } => { self.keys.insert(*code); },
            MacroEvent::KeyUp { code, .. } => { self.keys.remove(code); },
            _ => {}
        }
    }

    pub fn release_all(&mut self, backend: &mut dyn ClickBackend) {
        for button in self.buttons.drain() {
            let click_button = macro_button_to_click(&button);
            if let Err(e) = backend.release(&click_button) {
                error!("Failed to release stuck button {:?}: {}", click_button, e);
            } else {
                warn!("Released stuck button {:?}.", click_button);
            }
        }
        for code in self.keys.drain() {
            if let Err(e) = backend.key_up("", code) {
                error!("Failed to release stuck key code {}: {}", code, e);
            } else {
                warn!("Released stuck key code {}.", code);
            }
        }
    }

    pub fn suspend(&self, backend: &mut dyn ClickBackend) {
        for button in &self.buttons {
            let click_button = macro_button_to_click(button);
            if let Err(e) = backend.release(&click_button) {
                error!("Failed to suspend button {:?}: {}", click_button, e);
            }
        }
        for code in &self.keys {
            if let Err(e) = backend.key_up("", *code) {
                error!("Failed to suspend key code {}: {}", code, e);
            }
        }
    }

    pub fn resume(&self, backend: &mut dyn ClickBackend) {
        for button in &self.buttons {
            let click_button = macro_button_to_click(button);
            if let Err(e) = backend.press(&click_button) {
                error!("Failed to resume button {:?}: {}", click_button, e);
            }
        }
        for code in &self.keys {
            if let Err(e) = backend.key_down("", *code) {
                error!("Failed to resume key code {}: {}", code, e);
            }
        }
    }
}

#[derive(Default)]
pub struct PlaybackMetrics {
    pub total_dispatch: Duration,
    pub max_dispatch: Duration,
    pub max_dispatch_event: Option<(&'static str, i32, i32)>,
    pub late_events: u32,
    pub move_dispatch: (Duration, u32),
    pub down_dispatch: (Duration, u32),
    pub other_dispatch: (Duration, u32),
}

impl PlaybackMetrics {
    pub fn record(&mut self, kind: &'static str, ex: i32, ey: i32, duration: Duration, is_late: bool) {
        self.total_dispatch += duration;
        self.update_max_dispatch(kind, ex, ey, duration);
        self.log_if_slow(kind, ex, ey, duration);
        if is_late { self.late_events += 1; }
        self.categorize_dispatch(kind, duration);
    }

    fn update_max_dispatch(&mut self, kind: &'static str, ex: i32, ey: i32, duration: Duration) {
        if duration > self.max_dispatch {
            self.max_dispatch = duration;
            self.max_dispatch_event = Some((kind, ex, ey));
        }
    }

    fn log_if_slow(&self, kind: &'static str, ex: i32, ey: i32, duration: Duration) {
        if duration > Duration::from_millis(1) {
            warn!("SLOW DISPATCH: {} at ({},{}) took {:.2?}", kind, ex, ey, duration);
        }
    }

    fn categorize_dispatch(&mut self, kind: &'static str, duration: Duration) {
        match kind {
            "MouseMove" => { self.move_dispatch.0 += duration; self.move_dispatch.1 += 1; }
            "MouseDown" | "Click" => { self.down_dispatch.0 += duration; self.down_dispatch.1 += 1; }
            _ => { self.other_dispatch.0 += duration; self.other_dispatch.1 += 1; }
        }
    }

    pub fn report(&self, actual_duration: Duration) {
        let avg = |d: Duration, n: u32| if n > 0 { d / n } else { Duration::ZERO };
        info!("Playback finished. Actual played duration: {:.2?}", actual_duration);
        info!("Diagnostics: total_dispatch_time={:.2?} max_dispatch_time={:.2?} late_events={}", self.total_dispatch, self.max_dispatch, self.late_events);
        info!("By kind: MouseMove n={} avg={:.2?} | MouseDown/Click n={} avg={:.2?} | Other n={} avg={:.2?}",
            self.move_dispatch.1, avg(self.move_dispatch.0, self.move_dispatch.1),
            self.down_dispatch.1, avg(self.down_dispatch.0, self.down_dispatch.1),
            self.other_dispatch.1, avg(self.other_dispatch.0, self.other_dispatch.1)
        );
        if let Some((kind, ex, ey)) = self.max_dispatch_event {
            info!("Slowest single dispatch: {} at ({},{})", kind, ex, ey);
        }
    }
}

pub struct PlaybackParams {
    pub commands: Vec<MacroCommand>,
    pub speed: f64,
    pub max_loops: Option<u32>,
    pub record_hotkey: Option<Hotkey>,
    pub play_hotkey: Option<Hotkey>,
    pub smart_path: SmartPathOptions,
}

pub struct PlaybackContext {
    pub params: PlaybackParams,
    pub held: HeldState,
    pub metrics: PlaybackMetrics,
    pub exec_stack: Vec<ExecFrame>,
    pub loop_num: u32,
    pub loop_start: Instant,
    pub accumulated_delay_us: f64,
}

impl PlaybackContext {
    pub fn new(params: PlaybackParams) -> Self {
        Self {
            params,
            held: HeldState::default(),
            metrics: PlaybackMetrics::default(),
            exec_stack: Vec::new(),
            loop_num: 0,
            loop_start: Instant::now(),
            accumulated_delay_us: 0.0,
        }
    }
}

pub enum FlowControl {
    Continue,
    BreakFrame,
    Stop,
}
