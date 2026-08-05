use crate::backend::ClickBackend;
use crate::state::{MacroRepeatMode, SharedState};
use wmacro_core_types::{ClickButton, Hotkey, MacroButton, MacroCommand, MacroEvent};
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn macro_button_to_click(btn: &MacroButton) -> ClickButton {
    match btn {
        MacroButton::Left => ClickButton::Left,
        MacroButton::Right => ClickButton::Right,
        MacroButton::Middle => ClickButton::Middle,
    }
}

fn jitter_offset(jitter: u32) -> (i32, i32) {
    if jitter == 0 {
        return (0, 0);
    }
    let jitter = jitter as i64;
    let range = 2 * jitter + 1;

    let mut seed = Instant::now().elapsed().as_nanos() as u64 ^ (std::ptr::addr_of!(jitter) as u64);
    let mut next = || {
        seed = seed.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    };

    let dx = (next() % range as u64) as i64 - jitter;
    let dy = (next() % range as u64) as i64 - jitter;
    (dx as i32, dy as i32)
}

fn event_kind_tag(event: &MacroEvent) -> &'static str {
    match event {
        MacroEvent::Delay(_) => "Delay",
        MacroEvent::MouseMove { .. } => "MouseMove",
        MacroEvent::Click { .. } => "Click",
        MacroEvent::MouseDown { .. } => "MouseDown",
        MacroEvent::MouseUp { .. } => "MouseUp",
        MacroEvent::Scroll { .. } => "Scroll",
        MacroEvent::KeyDown { .. } => "KeyDown",
        MacroEvent::KeyUp { .. } => "KeyUp",
        MacroEvent::KeyPress { .. } => "KeyPress",
    }
}

#[derive(Default)]
struct HeldState {
    buttons: HashSet<MacroButton>,
    keys: HashSet<u16>,
}

impl HeldState {
    fn update(&mut self, event: &MacroEvent) {
        match event {
            MacroEvent::MouseDown { button, .. } => _ = self.buttons.insert(button.clone()),
            MacroEvent::MouseUp { button, .. } => _ = self.buttons.remove(button),
            MacroEvent::KeyDown { code, .. } => _ = self.keys.insert(*code),
            MacroEvent::KeyUp { code, .. } => _ = self.keys.remove(code),
            _ => {}
        }
    }

    fn release_all(&mut self, backend: &mut dyn ClickBackend) {
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

    fn suspend(&self, backend: &mut dyn ClickBackend) {
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

    fn resume(&self, backend: &mut dyn ClickBackend) {
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
struct PlaybackMetrics {
    total_dispatch: Duration,
    max_dispatch: Duration,
    max_dispatch_event: Option<(&'static str, i32, i32)>,
    late_events: u32,
    move_dispatch: (Duration, u32),
    down_dispatch: (Duration, u32),
    other_dispatch: (Duration, u32),
}

impl PlaybackMetrics {
    fn record(&mut self, kind: &'static str, ex: i32, ey: i32, duration: Duration, is_late: bool) {
        self.total_dispatch += duration;

        if duration > self.max_dispatch {
            self.max_dispatch = duration;
            self.max_dispatch_event = Some((kind, ex, ey));
        }

        if duration > Duration::from_millis(1) {
            warn!("SLOW DISPATCH: {} at ({},{}) took {:.2?}", kind, ex, ey, duration);
        }

        if is_late {
            self.late_events += 1;
        }

        match kind {
            "MouseMove" => {
                self.move_dispatch.0 += duration;
                self.move_dispatch.1 += 1;
            }
            "MouseDown" | "Click" => {
                self.down_dispatch.0 += duration;
                self.down_dispatch.1 += 1;
            }
            _ => {
                self.other_dispatch.0 += duration;
                self.other_dispatch.1 += 1;
            }
        }
    }

    fn report(&self, actual_duration: Duration) {
        let avg = |d: Duration, n: u32| if n > 0 { d / n } else { Duration::ZERO };

        info!("Playback finished. Actual played duration: {:.2?}", actual_duration);
        info!("Diagnostics: total_dispatch_time={:.2?} max_dispatch_time={:.2?} late_events={}",
            self.total_dispatch, self.max_dispatch, self.late_events
        );
        info!(
            "By kind: MouseMove n={} avg={:.2?} | MouseDown/Click n={} avg={:.2?} | Other n={} avg={:.2?}",
            self.move_dispatch.1, avg(self.move_dispatch.0, self.move_dispatch.1),
            self.down_dispatch.1, avg(self.down_dispatch.0, self.down_dispatch.1),
            self.other_dispatch.1, avg(self.other_dispatch.0, self.other_dispatch.1),
        );

        if let Some((kind, ex, ey)) = self.max_dispatch_event {
            info!("Slowest single dispatch: {} at ({},{})", kind, ex, ey);
        }
    }
}

struct ExecFrame {
    commands: Vec<MacroCommand>,
    idx: usize,
    loop_stack: Vec<(usize, u32)>,
    labels: HashMap<String, usize>,
}

impl ExecFrame {
    fn new(commands: Vec<MacroCommand>) -> Self {
        let labels = commands
            .iter()
            .enumerate()
            .filter_map(|(i, cmd)| match cmd {
                MacroCommand::Label(name) => Some((name.clone(), i)),
                _ => None,
            })
            .collect();

        Self { commands, idx: 0, loop_stack: Vec::new(), labels }
    }

    fn skip_to_else_or_endif(&mut self) {
        let mut nested = 0;
        while self.idx + 1 < self.commands.len() {
            self.idx += 1;
            match &self.commands[self.idx] {
                MacroCommand::IfPixelColor { .. } | MacroCommand::IfImageFound { .. } => nested += 1,
                MacroCommand::Else if nested == 0 => return,
                MacroCommand::EndIf => {
                    if nested == 0 { return; }
                    nested -= 1;
                }
                _ => {}
            }
        }
    }

    fn skip_to_endif(&mut self) {
        let mut nested = 0;
        while self.idx + 1 < self.commands.len() {
            self.idx += 1;
            match &self.commands[self.idx] {
                MacroCommand::IfPixelColor { .. } | MacroCommand::IfImageFound { .. } => nested += 1,
                MacroCommand::EndIf => {
                    if nested == 0 { return; }
                    nested -= 1;
                }
                _ => {}
            }
        }
    }

    fn skip_to_endloop(&mut self) {
        let mut nested = 0;
        while self.idx + 1 < self.commands.len() {
            self.idx += 1;
            match &self.commands[self.idx] {
                MacroCommand::Loop { .. } => nested += 1,
                MacroCommand::EndLoop => {
                    if nested == 0 { return; }
                    nested -= 1;
                }
                _ => {}
            }
        }
    }

    fn find_label(&self, target: &str) -> Option<usize> {
        self.labels.get(target).copied()
    }
}

struct PlaybackParams {
    commands: Vec<MacroCommand>,
    speed: f64,
    max_loops: Option<u32>,
    record_hotkey: Option<Hotkey>,
    play_hotkey: Option<Hotkey>,
    smart_path: wmacro_core_types::SmartPathOptions,
}

pub fn spawn_player(state: SharedState) -> (Arc<AtomicBool>, Arc<AtomicBool>, Arc<AtomicBool>) {
    let kill = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(false));
    let step = Arc::new(AtomicBool::new(false));

    let kill_clone = Arc::clone(&kill);
    let paused_clone = Arc::clone(&paused);
    let step_clone = Arc::clone(&step);

    std::thread::spawn(move || {
        run_playback_thread(state, kill_clone, paused_clone, step_clone);
    });

    (kill, paused, step)
}

fn prepare_playback(state: &SharedState) -> Option<PlaybackParams> {
    let mut s = state.lock().ok()?;
    let m = s.macro_state.current_macro.clone()?;

    if m.commands.is_empty() {
        s.macro_state.playing = false;
        s.status_msg = String::from("Macro is empty");
        return None;
    }

    s.macro_state.playing = true;
    s.macro_state.play_paused = false;
    s.macro_state.events_played = 0;
    s.macro_state.current_step = 0;
    s.macro_state.current_loop = 1;
    s.status_msg = String::from("Playing macro…");

    let max_loops = match &s.macro_state.repeat_mode {
        MacroRepeatMode::Once => Some(1),
        MacroRepeatMode::Count(n) => Some(*n),
        MacroRepeatMode::Infinite => None,
    };

    Some(PlaybackParams {
        commands: m.commands,
        speed: if s.macro_state.speed_multiplier <= 0.0 { 1.0 } else { s.macro_state.speed_multiplier as f64 },
        max_loops,
        record_hotkey: s.macro_state.record_hotkey.clone(),
        play_hotkey: s.macro_state.play_hotkey.clone(),
        smart_path: s.macro_state.playback_options.smart_path.clone(),
    })
}

// TODO: optimization: compile the macro commands into a flat bytecode
// with pre-computed jump targets for If/Else/Loops to avoid linear scans.
fn run_playback_thread(
    state: SharedState,
    kill: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    step: Arc<AtomicBool>,
) {
    let playback_start = Instant::now();
    let Some(params) = prepare_playback(&state) else { return; };

    let mut held = HeldState::default();
    let mut metrics = PlaybackMetrics::default();
    let mut exec_stack: Vec<ExecFrame> = vec![ExecFrame::new(params.commands.clone())];
    let mut loop_num = 0u32;

    'outer: loop {
        if kill.load(Ordering::Relaxed) || params.max_loops.is_some_and(|max| loop_num >= max) {
            break;
        }
        loop_num += 1;

        let mut loop_start = Instant::now();
        let mut accumulated_delay_us = 0.0;

        let mut current_commands = params.commands.clone();
        if params.smart_path.enabled {
            crate::macro_engine::humanize::humanize_commands(&mut current_commands, &params.smart_path);
        }

        exec_stack.clear();
        exec_stack.push(ExecFrame::new(current_commands));

        'inner: while let Some(mut frame) = exec_stack.pop() {
            while frame.idx < frame.commands.len() {
                if !handle_pause_and_step(&paused, &step, &kill, &mut loop_start, &held) {
                    break 'outer;
                }

                let cmd = frame.commands[frame.idx].clone();

                match cmd {
                    MacroCommand::Action(event) => {
                        accumulated_delay_us += event.delay_us() as f64;
                        let target_offset = Duration::from_secs_f64((accumulated_delay_us / params.speed) / 1_000_000.0);
                        let target_instant = loop_start + target_offset;

                        if !wait_until(target_instant, &kill) {
                            break 'outer;
                        }

                        let dispatch_start = Instant::now();
                        let is_late = dispatch_start.duration_since(target_instant) > Duration::from_micros(50);

                        let kind = event_kind_tag(&event);
                        let mouse_jitter = extract_jitter(&event);
                        let (ex, ey) = extract_position(&event, mouse_jitter);

                        if let Err(e) = execute_event_dispatch(&event, &params, mouse_jitter, &mut held) {
                            abort_playback_on_error(&state, &mut held, e);
                            return;
                        }

                        let dispatch_duration = dispatch_start.elapsed();
                        metrics.record(kind, ex, ey, dispatch_duration, is_late);

                        if kind == "MouseDown" || kind == "Click" {
                            let real_elapsed = loop_start.elapsed();
                            debug!(
                                "CLICK TIMING: {} at ({},{}) - recorded_target={:.2?} real_elapsed_since_loop_start={:.2?} drift_so_far={:.2?}",
                                kind, ex, ey, target_offset, real_elapsed, real_elapsed.saturating_sub(target_offset)
                            );
                        }
                    }
                    MacroCommand::PlayMacro(path) => {
                        // TODO: cache loaded macros to prevent blocking disk I/O in tight loops.
                        if let Ok(nested_macro) = crate::macro_engine::storage::load_wmr(std::path::Path::new(&path)) {
                            frame.idx += 1;
                            exec_stack.push(frame);
                            exec_stack.push(ExecFrame::new(nested_macro.commands));
                            continue 'inner;
                        } else {
                            error!("Failed to load nested macro: {}", path);
                        }
                    }
                    MacroCommand::Label(_) => {}
                    MacroCommand::Goto(target) => {
                        if let Some(target_idx) = frame.find_label(&target) {
                            frame.idx = target_idx;
                            continue;
                        }
                        warn!("Warning: Goto target '{}' not found in current macro.", target);
                    }
                    MacroCommand::TypeText(text) => {
                        if let Err(e) = execute_type_text(&text) {
                            error!("TypeText error: {}", e);
                        }
                    }
                    MacroCommand::IfPixelColor { x, y, r, g, b, tolerance } => {
                        let (cr, cg, cb) = crate::cursor::get_pixel_color(x, y);

                        if tolerance == 0 {
                            if cr != r || cg != g || cb != b {
                                frame.skip_to_else_or_endif();
                            }
                        } else {
                            let dist = ((cr as f32 - r as f32).powi(2) +
                                         (cg as f32 - g as f32).powi(2) +
                                         (cb as f32 - b as f32).powi(2)).sqrt();

                            let max_dist = 441.673_f32;
                            let tolerance_dist = max_dist * (tolerance as f32 / 100.0);

                            if dist > tolerance_dist {
                                frame.skip_to_else_or_endif();
                            }
                        }
                    }
                    MacroCommand::IfImageFound { target_image_path, similarity_threshold, move_cursor_if_found, trigger_if_not_found, region } => {
                        let reg = region.map(|(l, t, w, h)| (l, t, w, h));
                        match crate::image_utils::find_image(&target_image_path, reg, similarity_threshold) {
                            Ok(Some((x, y))) => {
                                if trigger_if_not_found {
                                    frame.skip_to_else_or_endif();
                                } else if move_cursor_if_found {
                                    if let Ok(img) = image::open(&target_image_path) {
                                        let center_x = x as i32 + (img.width() / 2) as i32;
                                        let center_y = y as i32 + (img.height() / 2) as i32;
                                        if let Ok(mut backend_guard) = crate::GLOBAL_BACKEND.get().unwrap().lock() {
                                            let _ = backend_guard.move_to(center_x, center_y);
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                if !trigger_if_not_found {
                                    frame.skip_to_else_or_endif();
                                }
                            }
                            Err(e) => {
                                log::error!("IfImageFound error: {}", e);
                                if !trigger_if_not_found {
                                    frame.skip_to_else_or_endif();
                                }
                            }
                        }
                    }
                    MacroCommand::Else => frame.skip_to_endif(),
                    MacroCommand::EndIf => {}
                    MacroCommand::Loop { count } => {
                        if count == 0 {
                            frame.skip_to_endloop();
                        } else if frame.loop_stack.last().map(|&(i, _)| i) != Some(frame.idx) {
                            frame.loop_stack.push((frame.idx, count));
                        }
                    }
                    MacroCommand::EndLoop => {
                        if let Some((start_idx, remaining)) = frame.loop_stack.pop() {
                            if remaining > 1 {
                                frame.loop_stack.push((start_idx, remaining - 1));
                                frame.idx = start_idx;
                            }
                        }
                    }
                }

                if let Ok(mut s) = state.lock() {
                    s.macro_state.events_played += 1;
                    s.macro_state.current_step = frame.idx + 1;
                    s.macro_state.current_loop = loop_num;
                }

                frame.idx += 1;
            }
        }
    }

    if let Some(backend_mutex) = crate::GLOBAL_BACKEND.get() {
        if let Ok(mut backend) = backend_mutex.lock() {
            held.release_all(&mut **backend);
        }
    }

    let actual_duration = playback_start.elapsed();
    if let Ok(mut s) = state.lock() {
        s.macro_state.playing = false;
        s.status_msg = format!("Macro done in {:.2?}", actual_duration);
        metrics.report(actual_duration);
    }
}

fn extract_jitter(event: &MacroEvent) -> (i32, i32) {
    match event {
        MacroEvent::Click { jitter, .. } | MacroEvent::MouseDown { jitter, .. } => jitter_offset(*jitter),
        _ => (0, 0),
    }
}

fn extract_position(event: &MacroEvent, mouse_jitter: (i32, i32)) -> (i32, i32) {
    match event {
        MacroEvent::MouseMove { x, y, .. } => (*x, *y),
        MacroEvent::Click { position, .. } | MacroEvent::MouseDown { position, .. } => match position {
            wmacro_core_types::MousePosition::Absolute { x, y } => (*x + mouse_jitter.0, *y + mouse_jitter.1),
            wmacro_core_types::MousePosition::Current => (-1, -1),
        },
        _ => (-1, -1),
    }
}

fn handle_pause_and_step(
    paused: &AtomicBool,
    step: &AtomicBool,
    kill: &AtomicBool,
    loop_start: &mut Instant,
    held: &HeldState,
) -> bool {
    let mut pause_start = None;
    let mut inputs_suspended = false;

    while paused.load(Ordering::Relaxed) {
        if pause_start.is_none() {
            pause_start = Some(Instant::now());
        }

        if !inputs_suspended {
            if let Some(backend_mutex) = crate::GLOBAL_BACKEND.get() {
                if let Ok(mut backend) = backend_mutex.lock() {
                    held.suspend(&mut **backend);
                    inputs_suspended = true;
                }
            }
        }

        if kill.load(Ordering::Relaxed) {
            return false;
        }
        if step.load(Ordering::Relaxed) {
            step.store(false, Ordering::Relaxed);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    if let Some(ps) = pause_start {
        *loop_start += ps.elapsed();

        if inputs_suspended {
            if let Some(backend_mutex) = crate::GLOBAL_BACKEND.get() {
                if let Ok(mut backend) = backend_mutex.lock() {
                    held.resume(&mut **backend);
                }
            }
        }
    }

    !kill.load(Ordering::Relaxed)
}

fn wait_until(target_instant: Instant, kill: &AtomicBool) -> bool {
    const SPIN_THRESHOLD: Duration = Duration::from_millis(2);
    const MIN_SLEEP: Duration = Duration::from_millis(1);

    loop {
        if kill.load(Ordering::Relaxed) {
            return false;
        }

        let now = Instant::now();
        if now >= target_instant {
            return true;
        }

        let remaining = target_instant.duration_since(now);
        if remaining > SPIN_THRESHOLD {
            std::thread::sleep(std::cmp::max(remaining / 2, MIN_SLEEP));
        } else {
            while Instant::now() < target_instant {
                std::hint::spin_loop();
            }
            return true;
        }
    }
}

fn execute_type_text(text: &str) -> Result<(), String> {
    let backend_mutex = crate::GLOBAL_BACKEND.get().ok_or("Global backend not initialized")?;
    let mut backend = backend_mutex.lock().map_err(|_| "Failed to lock global backend")?;
    backend.type_text(text).map_err(|e| e.to_string())
}

fn execute_event_dispatch(
    event: &MacroEvent,
    params: &PlaybackParams,
    mouse_jitter: (i32, i32),
    held: &mut HeldState,
) -> Result<(), String> {
    let backend_mutex = crate::GLOBAL_BACKEND.get().ok_or("Global backend not initialized")?;
    let mut backend = backend_mutex.lock().map_err(|_| "Failed to lock global backend")?;

    let res = dispatch_event(&mut **backend, event, params.record_hotkey.as_ref(), params.play_hotkey.as_ref(), mouse_jitter);
    if res.is_ok() {
        held.update(event);
    }
    res
}

fn abort_playback_on_error(state: &SharedState, held: &mut HeldState, error_msg: String) {
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

fn dispatch_event(
    backend: &mut dyn ClickBackend,
    event: &MacroEvent,
    record_hotkey: Option<&Hotkey>,
    play_hotkey: Option<&Hotkey>,
    mouse_jitter: (i32, i32),
) -> Result<(), String> {
    let is_suppressed_hotkey = |code: u16| {
        record_hotkey.is_some_and(|hk| hk.code == code) || play_hotkey.is_some_and(|hk| hk.code == code)
    };

    match event {
        MacroEvent::Delay(_) => Ok(()),
        MacroEvent::MouseMove { x, y, .. } => backend.move_to(*x, *y),
        MacroEvent::Click { position, button, hold_time_ms, .. } => {
            if let wmacro_core_types::MousePosition::Absolute { x, y } = position {
                backend.move_to(*x + mouse_jitter.0, *y + mouse_jitter.1)?;
            }
            let btn = macro_button_to_click(button);
            backend.press(&btn)?;
            std::thread::sleep(Duration::from_millis(*hold_time_ms as u64));
            backend.release(&btn)
        }
        MacroEvent::MouseDown { position, button, .. } => {
            if let wmacro_core_types::MousePosition::Absolute { x, y } = position {
                backend.move_to(*x + mouse_jitter.0, *y + mouse_jitter.1)?;
            }
            backend.press(&macro_button_to_click(button))
        }
        MacroEvent::MouseUp { button, .. } => backend.release(&macro_button_to_click(button)),
        MacroEvent::Scroll { dx, dy, .. } => backend.scroll(*dx, *dy),
        MacroEvent::KeyDown { key, code, .. } => {
            if is_suppressed_hotkey(*code) {
                return Ok(());
            }
            backend.key_down(key, *code)
        }
        MacroEvent::KeyUp { key, code, .. } => {
            if is_suppressed_hotkey(*code) {
                return Ok(());
            }
            backend.key_up(key, *code)
        }
        MacroEvent::KeyPress { key, code, hold_time_ms } => {
            if is_suppressed_hotkey(*code) {
                return Ok(());
            }
            backend.key_down(key, *code)?;
            std::thread::sleep(Duration::from_millis(*hold_time_ms as u64));
            backend.key_up(key, *code)
        }
    }
}
