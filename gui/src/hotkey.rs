use crate::macro_engine::player::spawn_player;
use crate::macro_engine::recorder::{start_recording, stop_recording};
use crate::state::{AppState, RecordHotkeyBehavior, SharedState};
use crate::ui::settings::save_settings;
use wmacro_core_types::{Hotkey, HotkeyEvent, Modifiers};
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::MutexGuard;

pub fn spawn_hotkey_listener(state: SharedState, rx_hotkey: Receiver<HotkeyEvent>) {
    std::thread::spawn(move || {
        let mut mods = Modifiers::default();
        while let Ok(event) = rx_hotkey.recv() {
            handle_event(&event, &state, &mut mods);
        }
    });
}

fn handle_event(event: &HotkeyEvent, state: &SharedState, mods: &mut Modifiers) {
    if mods.apply(event.code, event.pressed) || !event.pressed {
        return;
    }

    let code = event.code;
    let held_mods = *mods;
    let mut guard = state.lock().unwrap();

    macro_rules! bind_if_pending {
        ($flag:ident, $hotkey:ident, $msg:expr) => {
            if guard.macro_state.$flag {
                guard.macro_state.$hotkey = Some(Hotkey::new(code, held_mods));
                guard.macro_state.$flag = false;
                guard.status_msg = format!($msg, code);

                drop(guard);
                save_settings(state);
                return;
            }
        };
    }

    bind_if_pending!(binding_record, record_hotkey, "Record key set to code {}");
    bind_if_pending!(
        binding_abort_record,
        abort_record_hotkey,
        "Abort Record key set to code {}"
    );
    bind_if_pending!(binding_play, play_hotkey, "Play key set to code {}");
    bind_if_pending!(
        binding_abort_play,
        abort_play_hotkey,
        "Abort Play key set to code {}"
    );
    bind_if_pending!(
        binding_step_play,
        step_play_hotkey,
        "Step Play key set to code {}"
    );
    bind_if_pending!(binding_capture, capture_hotkey, "Capture key set to code {}");

    let is_match = |hotkey: Option<Hotkey>| hotkey.is_some_and(|h| h.matches(code, &held_mods));

    if is_match(guard.macro_state.capture_hotkey) {
        handle_capture_hotkey(&mut guard);
    } else if is_match(guard.macro_state.record_hotkey) {
        handle_record_hotkey(guard, state);
    } else if is_match(guard.macro_state.abort_record_hotkey) {
        handle_abort_record_hotkey(guard, state);
    } else if is_match(guard.macro_state.play_hotkey) {
        handle_play_hotkey(guard, state);
    } else if is_match(guard.macro_state.abort_play_hotkey) {
        handle_abort_play_hotkey(&mut guard);
    } else if is_match(guard.macro_state.step_play_hotkey) {
        handle_step_play_hotkey(guard, state);
    }
}

fn handle_capture_hotkey(guard: &mut MutexGuard<'_, AppState>) {
    let (cursor_x, cursor_y) = (guard.cursor_x, guard.cursor_y);
    guard.active_capture = Some((cursor_x, cursor_y));
    guard.status_msg = format!("Captured coordinate: ({}, {})", cursor_x, cursor_y);
}

fn handle_record_hotkey(mut guard: MutexGuard<'_, AppState>, state: &SharedState) {
    if !guard.macro_state.recording && guard.macro_state.playing {
        guard.status_msg = String::from("Stop macro playback before recording");
        return;
    }

    if guard.macro_state.recording {
        toggle_record_pause(&mut guard);
        return;
    }

    let macro_name = guard.macro_state.macro_name.clone();
    let append = guard.macro_state.record_hotkey_behavior == RecordHotkeyBehavior::Append;
    drop(guard);
    start_recording(state, macro_name, append);
}

fn toggle_record_pause(guard: &mut MutexGuard<'_, AppState>) {
    guard.macro_state.record_paused = !guard.macro_state.record_paused;
    if let Some(flag) = &guard.macro_state.record_paused_flag {
        flag.store(guard.macro_state.record_paused, Ordering::Relaxed);
    }
    guard.status_msg = if guard.macro_state.record_paused {
        "Recording Paused".into()
    } else {
        "Recording Resumed".into()
    };
}

fn handle_abort_record_hotkey(guard: MutexGuard<'_, AppState>, state: &SharedState) {
    if !guard.macro_state.recording {
        return;
    }
    drop(guard);
    stop_recording(state);
}

fn handle_play_hotkey(mut guard: MutexGuard<'_, AppState>, state: &SharedState) {
    if !guard.macro_state.playing && guard.macro_state.recording {
        guard.status_msg = String::from("Stop recording before playing a macro");
        return;
    }

    if guard.macro_state.playing {
        toggle_play_pause(&mut guard);
        return;
    }

    drop(guard);
    begin_playback(state, false);
}

fn toggle_play_pause(guard: &mut MutexGuard<'_, AppState>) {
    guard.macro_state.play_paused = !guard.macro_state.play_paused;
    if let Some(flag) = &guard.macro_state.play_paused_flag {
        flag.store(guard.macro_state.play_paused, Ordering::Relaxed);
    }
    guard.status_msg = if guard.macro_state.play_paused {
        "Playback Paused".into()
    } else {
        "Playback Resumed".into()
    };
}

fn handle_abort_play_hotkey(guard: &mut MutexGuard<'_, AppState>) {
    if !guard.macro_state.playing {
        return;
    }
    if let Some(kill) = guard.macro_state.play_kill.take() {
        kill.store(true, Ordering::Relaxed);
    }
    guard.macro_state.playing = false;
    guard.macro_state.play_paused = false;
    guard.status_msg = String::from("Playback Aborted");
}

fn handle_step_play_hotkey(mut guard: MutexGuard<'_, AppState>, state: &SharedState) {
    if !guard.macro_state.playing && guard.macro_state.recording {
        guard.status_msg = String::from("Stop recording before playing a macro");
        return;
    }

    if guard.macro_state.playing {
        pause_for_single_step(&mut guard);
        return;
    }

    drop(guard);
    begin_playback(state, true);
}

fn pause_for_single_step(guard: &mut MutexGuard<'_, AppState>) {
    guard.macro_state.play_paused = true;
    if let Some(flag) = &guard.macro_state.play_paused_flag {
        flag.store(true, Ordering::Relaxed);
    }
    if let Some(flag) = &guard.macro_state.play_step_flag {
        flag.store(true, Ordering::Relaxed);
    }
    guard.status_msg = String::from("Stepped one command");
}

fn begin_playback(state: &SharedState, start_paused: bool) {
    let (kill, paused_flag, step_flag) = spawn_player(state.clone());

    if start_paused {
        paused_flag.store(true, Ordering::Relaxed);
        step_flag.store(true, Ordering::Relaxed);
    }

    if let Ok(mut guard) = state.lock() {
        guard.macro_state.play_kill = Some(kill);
        guard.macro_state.play_paused_flag = Some(paused_flag);
        guard.macro_state.play_step_flag = Some(step_flag);
        if start_paused {
            guard.macro_state.play_paused = true;
        }
    }
}
