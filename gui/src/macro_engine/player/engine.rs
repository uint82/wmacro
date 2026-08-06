use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use log::{debug, error};
use wmacro_core_types::MacroCommand;
use crate::state::{MacroRepeatMode, SharedState};

use crate::macro_engine::player::models::{FlowControl, PlaybackContext, PlaybackParams};
use crate::macro_engine::player::frame::ExecFrame;
use crate::macro_engine::player::utils::{event_kind_tag, extract_jitter, extract_position, wait_until};
use crate::macro_engine::player::dispatch::{abort_playback_on_error, execute_event_dispatch, execute_type_text, handle_pause_and_step};

pub fn spawn_player(state: SharedState) -> (Arc<AtomicBool>, Arc<AtomicBool>, Arc<AtomicBool>) {
    let kill = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(false));
    let step = Arc::new(AtomicBool::new(false));

    let (k, p, s) = (Arc::clone(&kill), Arc::clone(&paused), Arc::clone(&step));
    std::thread::spawn(move || run_playback_thread(state, k, p, s));

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

fn run_playback_thread(state: SharedState, kill: Arc<AtomicBool>, paused: Arc<AtomicBool>, step: Arc<AtomicBool>) {
    let playback_start = Instant::now();
    let Some(params) = prepare_playback(&state) else { return; };
    let mut ctx = PlaybackContext::new(params);

    while !should_stop_playback(&ctx, &kill) {
        start_new_loop(&mut ctx);
        if execute_loop_frames(&state, &kill, &paused, &step, &mut ctx) {
            break;
        }
    }

    finalize_playback(&state, &mut ctx, playback_start);
}

fn should_stop_playback(ctx: &PlaybackContext, kill: &AtomicBool) -> bool {
    kill.load(Ordering::Relaxed) || ctx.params.max_loops.is_some_and(|max| ctx.loop_num >= max)
}

fn start_new_loop(ctx: &mut PlaybackContext) {
    ctx.loop_num += 1;
    ctx.loop_start = Instant::now();
    ctx.accumulated_delay_us = 0.0;

    let mut current_commands = ctx.params.commands.clone();
    if ctx.params.smart_path.enabled {
        crate::macro_engine::humanize::humanize_commands(&mut current_commands, &ctx.params.smart_path);
    }

    ctx.exec_stack.clear();
    ctx.exec_stack.push(ExecFrame::new(current_commands));
}

fn execute_loop_frames(
    state: &SharedState, kill: &AtomicBool, paused: &AtomicBool, step: &AtomicBool, ctx: &mut PlaybackContext
) -> bool {
    while let Some(mut frame) = ctx.exec_stack.pop() {
        let flow = process_single_frame(state, kill, paused, step, ctx, &mut frame);
        if matches!(flow, FlowControl::Stop) { return true; }
    }
    false
}

fn process_single_frame(
    state: &SharedState, kill: &AtomicBool, paused: &AtomicBool, step: &AtomicBool,
    ctx: &mut PlaybackContext, frame: &mut ExecFrame
) -> FlowControl {
    while frame.idx < frame.commands.len() {
        if !handle_pause_and_step(paused, step, kill, &mut ctx.loop_start, &ctx.held) {
            return FlowControl::Stop;
        }

        let cmd = frame.commands[frame.idx].clone();
        let flow = process_command(&cmd, state, kill, ctx, frame);

        match flow {
            FlowControl::Continue => update_and_advance(state, frame, ctx.loop_num),
            FlowControl::BreakFrame => return FlowControl::BreakFrame,
            FlowControl::Stop => return FlowControl::Stop,
        }
    }
    FlowControl::Continue
}

fn update_and_advance(state: &SharedState, frame: &mut ExecFrame, loop_num: u32) {
    if let Ok(mut s) = state.lock() {
        s.macro_state.events_played += 1;
        s.macro_state.current_step = frame.idx + 1;
        s.macro_state.current_loop = loop_num;
    }
    frame.idx += 1;
}

fn process_command(
    cmd: &MacroCommand, state: &SharedState, kill: &AtomicBool, ctx: &mut PlaybackContext, frame: &mut ExecFrame
) -> FlowControl {
    match cmd {
        MacroCommand::Action(event) => execute_action(event, state, kill, ctx),
        MacroCommand::PlayMacro(path) => execute_play_macro(path, ctx, frame),
        MacroCommand::Label(_) => FlowControl::Continue,
        MacroCommand::Goto(target) => execute_goto(target, frame),
        MacroCommand::TypeText(text) => execute_type_text_cmd(text),
        MacroCommand::IfPixelColor { x, y, r, g, b, tolerance } => execute_if_pixel_color(*x, *y, *r, *g, *b, *tolerance, frame),
        MacroCommand::IfImageFound { target_image_path, similarity_threshold, move_cursor_if_found, trigger_if_not_found, region } => {
            execute_if_image_found(target_image_path, *similarity_threshold, *move_cursor_if_found, *trigger_if_not_found, region, frame)
        },
        MacroCommand::Else => execute_else(frame),
        MacroCommand::EndIf => FlowControl::Continue,
        MacroCommand::Loop { count } => execute_loop_cmd(*count, frame),
        MacroCommand::EndLoop => execute_end_loop(frame),
        MacroCommand::OpenFile { path, args, run_as_admin } => {
            if ctx.loop_num == 1 {
                execute_open_file(path, args, *run_as_admin);
            } else {
                log::debug!("OpenFile: skipping '{}' on loop {} to prevent multiple instances", path, ctx.loop_num);
            }
            FlowControl::Continue
        }
    }
}

fn execute_action(event: &wmacro_core_types::MacroEvent, state: &SharedState, kill: &AtomicBool, ctx: &mut PlaybackContext) -> FlowControl {
    ctx.accumulated_delay_us += event.delay_us() as f64;
    let target_offset = Duration::from_secs_f64((ctx.accumulated_delay_us / ctx.params.speed) / 1_000_000.0);
    let target_instant = ctx.loop_start + target_offset;

    if !wait_until(target_instant, kill) {
        return FlowControl::Stop;
    }

    let dispatch_start = Instant::now();
    let is_late = dispatch_start.duration_since(target_instant) > Duration::from_micros(50);
    let kind = event_kind_tag(event);
    let mouse_jitter = extract_jitter(event);
    let (ex, ey) = extract_position(event, mouse_jitter);

    if let Err(e) = execute_event_dispatch(event, &ctx.params, mouse_jitter, &mut ctx.held) {
        abort_playback_on_error(state, &mut ctx.held, e);
        return FlowControl::Stop;
    }

    ctx.metrics.record(kind, ex, ey, dispatch_start.elapsed(), is_late);
    log_click_timing(kind, ex, ey, target_offset, ctx.loop_start);

    FlowControl::Continue
}

fn log_click_timing(kind: &str, ex: i32, ey: i32, target_offset: Duration, loop_start: Instant) {
    if kind == "MouseDown" || kind == "Click" {
        let real_elapsed = loop_start.elapsed();
        debug!("CLICK TIMING: {} at ({},{}) - recorded_target={:.2?} real_elapsed_since_loop_start={:.2?} drift_so_far={:.2?}",
            kind, ex, ey, target_offset, real_elapsed, real_elapsed.saturating_sub(target_offset));
    }
}

fn execute_play_macro(path: &str, ctx: &mut PlaybackContext, frame: &mut ExecFrame) -> FlowControl {
    if let Ok(nested_macro) = crate::macro_engine::storage::load_wmr(std::path::Path::new(path)) {
        frame.idx += 1;
        ctx.exec_stack.push(frame.clone());
        ctx.exec_stack.push(ExecFrame::new(nested_macro.commands));
        FlowControl::BreakFrame
    } else {
        error!("Failed to load nested macro: {}", path);
        FlowControl::Continue
    }
}

fn execute_goto(target: &str, frame: &mut ExecFrame) -> FlowControl {
    if let Some(target_idx) = frame.find_label(target) {
        frame.idx = target_idx - 1; // -1 because it increments after
    } else {
        log::warn!("Warning: Goto target '{}' not found in current macro.", target);
    }
    FlowControl::Continue
}

fn execute_type_text_cmd(text: &str) -> FlowControl {
    if let Err(e) = execute_type_text(text) {
        error!("TypeText error: {}", e);
    }
    FlowControl::Continue
}

fn execute_if_pixel_color(x: i32, y: i32, r: u8, g: u8, b: u8, tolerance: u8, frame: &mut ExecFrame) -> FlowControl {
    let (cr, cg, cb) = crate::cursor::get_pixel_color(x, y);
    if !is_color_match(cr, cg, cb, r, g, b, tolerance) {
        frame.skip_to_else_or_endif();
    }
    FlowControl::Continue
}

fn is_color_match(cr: u8, cg: u8, cb: u8, r: u8, g: u8, b: u8, tolerance: u8) -> bool {
    if tolerance == 0 { return cr == r && cg == g && cb == b; }
    let dist = ((cr as f32 - r as f32).powi(2) + (cg as f32 - g as f32).powi(2) + (cb as f32 - b as f32).powi(2)).sqrt();
    dist <= (441.673_f32 * (tolerance as f32 / 100.0))
}

fn execute_if_image_found(
    path: &str, threshold: f32, move_cursor: bool, trigger_not_found: bool,
    region: &Option<(i32, i32, i32, i32)>, frame: &mut ExecFrame
) -> FlowControl {
    let result = crate::image_utils::find_image(path, region.clone(), threshold);

    match result {
        Ok(Some((x, y))) => handle_found_image(x as u32, y as u32, path, move_cursor, trigger_not_found, frame),
        Ok(None) => handle_missing_image(trigger_not_found, frame),
        Err(e) => handle_image_error(e, trigger_not_found, frame),
    }
    FlowControl::Continue
}

fn handle_found_image(x: u32, y: u32, path: &str, move_cursor: bool, trigger_not_found: bool, frame: &mut ExecFrame) {
    if trigger_not_found {
        frame.skip_to_else_or_endif();
    } else if move_cursor {
        move_cursor_to_image(path, x, y);
    }
}

fn move_cursor_to_image(path: &str, x: u32, y: u32) {
    if let Ok(img) = image::open(path) {
        let center_x = x as i32 + (img.width() / 2) as i32;
        let center_y = y as i32 + (img.height() / 2) as i32;
        if let Ok(mut backend_guard) = crate::GLOBAL_BACKEND.get().unwrap().lock() {
            let _ = backend_guard.move_to(center_x, center_y);
        }
    }
}

fn handle_missing_image(trigger_not_found: bool, frame: &mut ExecFrame) {
    if !trigger_not_found { frame.skip_to_else_or_endif(); }
}

fn handle_image_error(e: impl std::fmt::Display, trigger_not_found: bool, frame: &mut ExecFrame) {
    error!("IfImageFound error: {}", e);
    if !trigger_not_found { frame.skip_to_else_or_endif(); }
}

fn execute_else(frame: &mut ExecFrame) -> FlowControl {
    frame.skip_to_endif();
    FlowControl::Continue
}

fn execute_loop_cmd(count: u32, frame: &mut ExecFrame) -> FlowControl {
    if count == 0 {
        frame.skip_to_endloop();
    } else if frame.loop_stack.last().map(|&(i, _)| i) != Some(frame.idx) {
        frame.loop_stack.push((frame.idx, count));
    }
    FlowControl::Continue
}

fn execute_end_loop(frame: &mut ExecFrame) -> FlowControl {
    if let Some((start_idx, remaining)) = frame.loop_stack.pop() {
        if remaining > 1 {
            frame.loop_stack.push((start_idx, remaining - 1));
            frame.idx = start_idx - 1; // -1 to offset increment
        }
    }
    FlowControl::Continue
}

fn finalize_playback(state: &SharedState, ctx: &mut PlaybackContext, playback_start: Instant) {
    if let Some(backend_mutex) = crate::GLOBAL_BACKEND.get() {
        if let Ok(mut backend) = backend_mutex.lock() {
            ctx.held.release_all(&mut **backend);
        }
    }
    let actual_duration = playback_start.elapsed();
    if let Ok(mut s) = state.lock() {
        s.macro_state.playing = false;
        s.status_msg = format!("Macro done in {:.2?}", actual_duration);
        ctx.metrics.report(actual_duration);
    }
}

fn parse_args(raw: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';

    for ch in raw.chars() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

fn execute_open_file(path: &str, args: &str, run_as_admin: bool) {
    let parsed_args = parse_args(args);

    let resolved_executable = which::which(path);

    let result = if let Ok(exec_path) = resolved_executable {
        if run_as_admin {
            let mut cmd = std::process::Command::new("pkexec");
            cmd.arg(exec_path);
            cmd.args(&parsed_args);
            cmd.spawn()
        } else {
            let mut cmd = std::process::Command::new(exec_path);
            cmd.args(&parsed_args);
            cmd.spawn()
        }
    } else {
        let path_buf = std::path::Path::new(path);

        if !path_buf.exists() {
            error!("OpenFile: command not found in PATH and path does not exist: {}", path);
            return;
        }

        std::process::Command::new("xdg-open").arg(path).spawn()
    };

    match result {
        Ok(_) => {
            log::info!("OpenFile: launched '{}'", path);
        }
        Err(e) => {
            error!("OpenFile: failed to launch '{}': {}", path, e);
        }
    }
}
