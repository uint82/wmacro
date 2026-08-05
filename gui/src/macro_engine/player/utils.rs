use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use wmacro_core_types::{ClickButton, MacroButton, MacroEvent, MousePosition};

pub fn macro_button_to_click(btn: &MacroButton) -> ClickButton {
    match btn {
        MacroButton::Left => ClickButton::Left,
        MacroButton::Right => ClickButton::Right,
        MacroButton::Middle => ClickButton::Middle,
    }
}

pub fn jitter_offset(jitter: u32) -> (i32, i32) {
    if jitter == 0 { return (0, 0); }
    let jitter_i64 = jitter as i64;
    let range = 2 * jitter_i64 + 1;
    let mut seed = Instant::now().elapsed().as_nanos() as u64 ^ (std::ptr::addr_of!(jitter) as u64);

    let mut next = || {
        seed = seed.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    };

    let dx = ((next() % range as u64) as i64) - jitter_i64;
    let dy = ((next() % range as u64) as i64) - jitter_i64;

    (dx as i32, dy as i32)
}

pub fn event_kind_tag(event: &MacroEvent) -> &'static str {
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

pub fn extract_jitter(event: &MacroEvent) -> (i32, i32) {
    match event {
        MacroEvent::Click { jitter, .. } | MacroEvent::MouseDown { jitter, .. } => jitter_offset(*jitter),
        _ => (0, 0),
    }
}

pub fn extract_position(event: &MacroEvent, mouse_jitter: (i32, i32)) -> (i32, i32) {
    match event {
        MacroEvent::MouseMove { x, y, .. } => (*x, *y),
        MacroEvent::Click { position, .. } | MacroEvent::MouseDown { position, .. } => {
            extract_absolute_position(position, mouse_jitter)
        },
        _ => (-1, -1),
    }
}

fn extract_absolute_position(position: &MousePosition, mouse_jitter: (i32, i32)) -> (i32, i32) {
    match position {
        MousePosition::Absolute { x, y } => (*x + mouse_jitter.0, *y + mouse_jitter.1),
        MousePosition::Current => (-1, -1),
    }
}

pub fn wait_until(target_instant: Instant, kill: &AtomicBool) -> bool {
    const SPIN_THRESHOLD: Duration = Duration::from_millis(2);
    const MIN_SLEEP: Duration = Duration::from_millis(1);

    loop {
        if kill.load(Ordering::Relaxed) { return false; }
        let now = Instant::now();
        if now >= target_instant { return true; }

        let remaining = target_instant.duration_since(now);
        if remaining > SPIN_THRESHOLD {
            std::thread::sleep(std::cmp::max(remaining / 2, MIN_SLEEP));
        } else {
            spin_wait_until(target_instant);
            return true;
        }
    }
}

fn spin_wait_until(target: Instant) {
    while Instant::now() < target {
        std::hint::spin_loop();
    }
}
