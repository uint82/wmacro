use log::debug;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use wmacro_core_types::{MacroCommand, MacroEvent};

use crate::macro_engine::player::control::{
    execute_else, execute_end_loop, execute_goto, execute_loop_cmd, execute_play_macro,
};
use crate::macro_engine::player::detection::{
    execute_if_color_found, execute_if_image_found, execute_if_pixel_color,
};
use crate::macro_engine::player::dispatch::{abort_playback_on_error, execute_event_dispatch};
use crate::macro_engine::player::effects::{
    execute_get_clipboard, execute_open_file, execute_set_clipboard, execute_type_text_cmd,
};
use crate::macro_engine::player::frame::ExecFrame;
use crate::macro_engine::player::models::{FlowControl, PlaybackContext};
use crate::macro_engine::player::utils::{
    event_kind_tag, extract_jitter, extract_position, resolve_event_position,
};
use crate::macro_engine::player::variables::{
    execute_calculate, execute_if_compare, execute_set_variable, resolve_num,
};
use crate::state::SharedState;

/// dispatches one command; the returned flow tells the engine how to move the
/// frame cursor.
pub(super) fn process_command(
    cmd: &MacroCommand,
    state: &SharedState,
    kill: &AtomicBool,
    ctx: &mut PlaybackContext,
    frame: &mut ExecFrame,
) -> FlowControl {
    match cmd {
        MacroCommand::Action(event) => execute_action(event, state, kill, ctx),
        MacroCommand::PlayMacro(path) => execute_play_macro(path, ctx, frame),
        MacroCommand::Comment(_) => FlowControl::Continue,
        MacroCommand::Label(_) => FlowControl::Continue,
        MacroCommand::Goto(target) => execute_goto(target, frame),
        MacroCommand::TypeText(text) => execute_type_text_cmd(text, &ctx.variables),
        MacroCommand::IfPixelColor {
            x,
            y,
            r,
            g,
            b,
            tolerance,
        } => execute_if_pixel_color(x, y, *r, *g, *b, *tolerance, ctx, frame),
        MacroCommand::IfImageFound {
            target_image_path,
            similarity_threshold,
            move_cursor_if_found,
            trigger_if_not_found,
            region,
            store_x,
            store_y,
        } => execute_if_image_found(
            target_image_path,
            *similarity_threshold,
            *move_cursor_if_found,
            *trigger_if_not_found,
            region,
            store_x.as_deref(),
            store_y.as_deref(),
            ctx,
            frame,
        ),
        MacroCommand::IfColorFound {
            region,
            r,
            g,
            b,
            tolerance,
            min_width,
            min_height,
            move_cursor_if_found,
            store_x,
            store_y,
            store_w,
            store_h,
        } => execute_if_color_found(
            region,
            *r,
            *g,
            *b,
            *tolerance,
            *min_width,
            *min_height,
            *move_cursor_if_found,
            store_x.as_deref(),
            store_y.as_deref(),
            store_w.as_deref(),
            store_h.as_deref(),
            ctx,
            frame,
        ),
        MacroCommand::Else => execute_else(frame),
        MacroCommand::EndIf => FlowControl::Continue,
        MacroCommand::Loop { count } => {
            let n = resolve_num(count, &ctx.variables).max(0) as u64;
            execute_loop_cmd(n, frame)
        }
        MacroCommand::Delay { duration_ms } => {
            let us = (resolve_num(duration_ms, &ctx.variables).max(0) as u64).saturating_mul(1000);
            if !ctx.timeline.wait_exact(us, ctx.params.speed, kill) {
                FlowControl::Stop
            } else {
                FlowControl::Continue
            }
        }
        MacroCommand::EndLoop => execute_end_loop(frame),
        MacroCommand::OpenFile {
            path,
            args,
            run_as_admin,
        } => {
            if ctx.loop_num == 1 {
                execute_open_file(path, args, *run_as_admin);
            } else {
                debug!(
                    "OpenFile: skipping '{}' on loop {} to prevent multiple instances",
                    path, ctx.loop_num
                );
            }
            FlowControl::Continue
        }
        MacroCommand::SetVariable { target, value } => {
            execute_set_variable(target, value, &mut ctx.variables);
            FlowControl::Continue
        }
        MacroCommand::Calculate { target, expression } => {
            execute_calculate(target, expression, &mut ctx.variables);
            FlowControl::Continue
        }
        MacroCommand::IfCompare { left, op, right } => {
            execute_if_compare(left, *op, right, &ctx.variables, frame)
        }
        MacroCommand::SetClipboard { text } => {
            execute_set_clipboard(text, &ctx.variables, ctx.params.clipboard.as_deref());
            FlowControl::Continue
        }
        MacroCommand::GetClipboard { target } => {
            execute_get_clipboard(target, &mut ctx.variables, ctx.params.clipboard.as_deref());
            FlowControl::Continue
        }
    }
}

/// plays a single recorded input event, honouring speed, jitter and the kill
/// flag; the recorded gap is anchored to the timeline, so slow dispatches are
/// absorbed instead of stacking.
fn execute_action(
    event: &MacroEvent,
    state: &SharedState,
    kill: &AtomicBool,
    ctx: &mut PlaybackContext,
) -> FlowControl {
    if !ctx
        .timeline
        .wait_scheduled(event.delay_us(), ctx.params.speed, kill)
    {
        return FlowControl::Stop;
    }

    // variable coordinates resolve to absolute so the daemon and metrics see the same value.
    let mut event = event.clone();
    resolve_event_position(&mut event, &ctx.variables);

    let dispatch_start = Instant::now();
    let kind = event_kind_tag(&event);
    let mouse_jitter = extract_jitter(&event);
    let (ex, ey) = extract_position(&event, mouse_jitter);

    if let Err(e) = execute_event_dispatch(&event, &ctx.params, mouse_jitter, &mut ctx.held) {
        abort_playback_on_error(state, &mut ctx.held, e);
        return FlowControl::Stop;
    }

    ctx.metrics.record(kind, ex, ey, dispatch_start.elapsed());

    FlowControl::Continue
}
