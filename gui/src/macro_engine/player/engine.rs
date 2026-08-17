use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::macro_engine::player::commands::process_command;
use crate::macro_engine::player::dispatch::handle_pause_and_step;
use crate::macro_engine::player::frame::ExecFrame;
use crate::macro_engine::player::models::{FlowControl, PlaybackContext, PlaybackParams};
use crate::state::{MacroRepeatMode, SharedState};

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
        speed: if s.macro_state.speed_multiplier <= 0.0 {
            1.0
        } else {
            s.macro_state.speed_multiplier as f64
        },
        max_loops,
        record_hotkey: s.macro_state.record_hotkey,
        play_hotkey: s.macro_state.play_hotkey,
        smart_path: s.macro_state.playback_options.smart_path.clone(),
        clipboard: Some(Arc::new(
            crate::macro_engine::player::models::SystemClipboard::new(),
        )),
    })
}

fn run_playback_thread(
    state: SharedState,
    kill: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    step: Arc<AtomicBool>,
) {
    let playback_start = Instant::now();
    let Some(params) = prepare_playback(&state) else {
        return;
    };
    // keep the retained frame fresh while the macro runs so if-pixel-color
    // checks are served from it without a request roundtrip; cleared again
    // in finalize_playback.
    crate::image_utils::capture::set_capture_continuous(true);
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
    ctx.timeline.reset();

    let mut current_commands = ctx.params.commands.clone();
    if ctx.params.smart_path.enabled {
        crate::macro_engine::humanize::humanize_commands(
            &mut current_commands,
            &ctx.params.smart_path,
        );
    }

    ctx.exec_stack.clear();
    ctx.exec_stack.push(ExecFrame::new(current_commands));
}

/// runs every frame on the exec stack for the current loop; returns `true`
/// when playback must stop.
fn execute_loop_frames(
    state: &SharedState,
    kill: &AtomicBool,
    paused: &AtomicBool,
    step: &AtomicBool,
    ctx: &mut PlaybackContext,
) -> bool {
    while let Some(mut frame) = ctx.exec_stack.pop() {
        let flow = process_single_frame(state, kill, paused, step, ctx, &mut frame);
        if matches!(flow, FlowControl::Stop) {
            return true;
        }
    }
    false
}

fn process_single_frame(
    state: &SharedState,
    kill: &AtomicBool,
    paused: &AtomicBool,
    step: &AtomicBool,
    ctx: &mut PlaybackContext,
    frame: &mut ExecFrame,
) -> FlowControl {
    while frame.idx < frame.commands.len() {
        let (keep_playing, paused_duration) = handle_pause_and_step(paused, step, kill, &ctx.held);
        if !keep_playing {
            return FlowControl::Stop;
        }
        ctx.timeline.shift(paused_duration);

        let cmd = frame.commands[frame.idx].clone();
        let flow = process_command(&cmd, state, kill, ctx, frame);

        match flow {
            FlowControl::Continue => update_and_advance(state, frame, ctx.loop_num),
            FlowControl::Jump => update_playback_state(state, frame, ctx.loop_num),
            FlowControl::BreakFrame => return FlowControl::BreakFrame,
            FlowControl::Stop => return FlowControl::Stop,
        }
    }
    FlowControl::Continue
}

/// updates the shared playback counters without moving the frame cursor.
fn update_playback_state(state: &SharedState, frame: &ExecFrame, loop_num: u32) {
    if let Ok(mut s) = state.lock() {
        s.macro_state.events_played += 1;
        s.macro_state.current_step = frame.idx + 1;
        s.macro_state.current_loop = loop_num;
    }
}

fn update_and_advance(state: &SharedState, frame: &mut ExecFrame, loop_num: u32) {
    update_playback_state(state, frame, loop_num);
    frame.idx += 1;
}

fn finalize_playback(state: &SharedState, ctx: &mut PlaybackContext, playback_start: Instant) {
    crate::image_utils::capture::set_capture_continuous(false);
    if let Some(backend_mutex) = crate::GLOBAL_BACKEND.get()
        && let Ok(mut backend) = backend_mutex.lock()
    {
        ctx.held.release_all(&mut **backend);
    }
    let actual_duration = playback_start.elapsed();
    if let Ok(mut s) = state.lock() {
        s.macro_state.playing = false;
        s.status_msg = format!("Macro done in {:.2?}", actual_duration);
        ctx.metrics.report(actual_duration);
    }
}
