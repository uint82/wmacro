use crate::backend::ClickBackend;
use log::{error, info, warn};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use wmacro_core_types::{Hotkey, MacroButton, MacroCommand, MacroEvent, SmartPathOptions, Value};

use crate::macro_engine::player::frame::ExecFrame;
use crate::macro_engine::player::utils::{macro_button_to_click, wait_until};

/// clipboard access used by SetClipboard/GetClipboard at playback time.
pub trait ClipboardBackend: Send + Sync {
    /// the current clipboard text, or `None` when empty, non-text, or unavailable.
    fn get_text(&self) -> Option<String>;
    /// replaces the clipboard with `text`.
    fn set_text(&self, text: &str);
}

/// Wayland clipboard access via the wl-clipboard protocol (in-process, no external tools).
pub struct WaylandClipboard;

enum ReadResult {
    Text(String),
    /// empty, non-text, or errored; usually transient, resolves within a few hundred ms.
    Unavailable,
    /// the compositor acknowledged the offer but never delivered the bytes.
    TimedOut,
}

fn read_clipboard() -> ReadResult {
    use std::io::Read;
    use std::os::fd::AsRawFd;

    const READ_TIMEOUT: Duration = Duration::from_secs(3);
    let (mut pipe, _mime) = match wl_clipboard_rs::paste::get_contents(
        wl_clipboard_rs::paste::ClipboardType::Regular,
        wl_clipboard_rs::paste::Seat::Unspecified,
        wl_clipboard_rs::paste::MimeType::Text,
    ) {
        Ok(pair) => pair,
        Err(e) => {
            log::debug!("GetClipboard: no clipboard offer: {e}");
            return ReadResult::Unavailable;
        }
    };
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        log::warn!("GetClipboard: failed to make clipboard pipe non-blocking");
        return ReadResult::Unavailable;
    }
    let mut buf = Vec::new();
    let deadline = Instant::now() + READ_TIMEOUT;
    loop {
        let mut chunk = [0u8; 4096];
        match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    log::warn!("GetClipboard: timed out reading clipboard data");
                    return ReadResult::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                log::warn!("GetClipboard: failed to read clipboard data: {e}");
                return ReadResult::Unavailable;
            }
        }
    }
    match String::from_utf8(buf) {
        Ok(text) => ReadResult::Text(text),
        Err(_) => {
            log::warn!("GetClipboard: clipboard content is not UTF-8 text");
            ReadResult::Unavailable
        }
    }
}

/// reads/writes both the Wayland selection and the X11 `CLIPBOARD` selection;
/// wlroots does not mirror one into the other, so XWayland apps only see X11.
pub struct SystemClipboard {
    x11: Option<super::x11_clipboard::X11Clipboard>,
}

impl SystemClipboard {
    pub fn new() -> Self {
        Self {
            x11: super::x11_clipboard::X11Clipboard::new(),
        }
    }
}

impl ClipboardBackend for SystemClipboard {
    fn get_text(&self) -> Option<String> {
        let wayland = WaylandClipboard.get_text();
        if wayland.is_some() {
            return wayland;
        }
        self.x11.as_ref().and_then(|x11| x11.get_text())
    }

    fn set_text(&self, text: &str) {
        WaylandClipboard.set_text(text);
        if let Some(x11) = &self.x11 {
            x11.set_text(text);
        }
    }
}

impl ClipboardBackend for WaylandClipboard {
    fn get_text(&self) -> Option<String> {
        // a read right after another client's set can transiently report empty; retry once.
        for _ in 0..2 {
            match read_clipboard() {
                ReadResult::Text(text) => return Some(text),
                ReadResult::Unavailable => std::thread::sleep(Duration::from_millis(100)),
                ReadResult::TimedOut => return None,
            }
        }
        None
    }

    fn set_text(&self, text: &str) {
        let result = wl_clipboard_rs::copy::Options::new().copy(
            wl_clipboard_rs::copy::Source::Bytes(text.as_bytes().to_vec().into()),
            wl_clipboard_rs::copy::MimeType::Text,
        );
        if let Err(e) = result {
            log::warn!("SetClipboard: failed to copy to Wayland clipboard: {e}");
            return;
        }
        // `copy()` only queues set_selection asynchronously; wait until a read-back confirms our text is live.
        let deadline = Instant::now() + Duration::from_millis(1000);
        while Instant::now() < deadline {
            if matches!(read_clipboard(), ReadResult::Text(ref t) if t == text) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        log::warn!("SetClipboard: selection not confirmed within 1s");
    }
}

#[derive(Default)]
pub struct HeldState {
    pub buttons: HashSet<MacroButton>,
    pub keys: HashSet<u16>,
}

impl HeldState {
    pub fn update(&mut self, event: &MacroEvent) {
        match event {
            MacroEvent::MouseDown { button, .. } => {
                self.buttons.insert(button.clone());
            }
            MacroEvent::MouseUp { button, .. } => {
                self.buttons.remove(button);
            }
            MacroEvent::KeyDown { code, .. } => {
                self.keys.insert(*code);
            }
            MacroEvent::KeyUp { code, .. } => {
                self.keys.remove(code);
            }
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
    pub move_dispatch: (Duration, u32),
    pub down_dispatch: (Duration, u32),
    pub other_dispatch: (Duration, u32),
}

impl PlaybackMetrics {
    pub fn record(&mut self, kind: &'static str, ex: i32, ey: i32, duration: Duration) {
        self.total_dispatch += duration;
        self.update_max_dispatch(kind, ex, ey, duration);
        self.log_if_slow(kind, ex, ey, duration);
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
            warn!(
                "SLOW DISPATCH: {} at ({},{}) took {:.2?}",
                kind, ex, ey, duration
            );
        }
    }

    fn categorize_dispatch(&mut self, kind: &'static str, duration: Duration) {
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

    pub fn report(&self, actual_duration: Duration) {
        let avg = |d: Duration, n: u32| if n > 0 { d / n } else { Duration::ZERO };
        info!(
            "Playback finished. Actual played duration: {:.2?}",
            actual_duration
        );
        info!(
            "Diagnostics: total_dispatch_time={:.2?} max_dispatch_time={:.2?}",
            self.total_dispatch, self.max_dispatch
        );
        info!(
            "By kind: MouseMove n={} avg={:.2?} | MouseDown/Click n={} avg={:.2?} | Other n={} avg={:.2?}",
            self.move_dispatch.1,
            avg(self.move_dispatch.0, self.move_dispatch.1),
            self.down_dispatch.1,
            avg(self.down_dispatch.0, self.down_dispatch.1),
            self.other_dispatch.1,
            avg(self.other_dispatch.0, self.other_dispatch.1)
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
    /// clipboard access for SetClipboard/GetClipboard; `None` makes those warn and no-op.
    pub clipboard: Option<Arc<dyn ClipboardBackend>>,
}

pub struct PlaybackContext {
    pub params: PlaybackParams,
    pub held: HeldState,
    pub metrics: PlaybackMetrics,
    pub exec_stack: Vec<ExecFrame>,
    pub loop_num: u32,
    /// keeps playback on the recorded schedule; reset at the start of every repeat loop.
    pub timeline: PlaybackTimeline,
    /// runtime variable values shared across loops and nested macros; undefined names read as 0.
    pub variables: HashMap<String, Value>,
    /// match position of the most recent successful IfImageFound, for storing coordinates into variables.
    pub last_image_pos: Option<(i32, i32)>,
}

impl PlaybackContext {
    pub fn new(params: PlaybackParams) -> Self {
        Self {
            params,
            held: HeldState::default(),
            metrics: PlaybackMetrics::default(),
            exec_stack: Vec::new(),
            loop_num: 0,
            timeline: PlaybackTimeline::new(),
            variables: HashMap::new(),
            last_image_pos: None,
        }
    }
}

/// max schedule lag before the timeline re-anchors to real time.
const LAG_TOLERANCE: Duration = Duration::from_millis(10);

/// anchors recorded gaps to a shared schedule, so slow dispatches shorten the following wait instead of drifting.
pub struct PlaybackTimeline {
    start: Option<Instant>,
    /// accumulated scheduled time in microseconds (unscaled by speed).
    schedule_us: u64,
}

impl PlaybackTimeline {
    fn new() -> Self {
        Self {
            start: None,
            schedule_us: 0,
        }
    }

    /// waits a recorded gap anchored to the timeline, re-anchoring first when lagging.
    pub fn wait_scheduled(&mut self, delay_us: u64, speed: f64, kill: &AtomicBool) -> bool {
        self.reanchor_if_lagging(speed);
        self.schedule_us = self.schedule_us.saturating_add(delay_us);
        let start = self.ensure_started();
        wait_until(start + scaled_duration(self.schedule_us, speed), kill)
    }

    fn reanchor_if_lagging(&mut self, speed: f64) {
        let Some(start) = self.start else {
            return;
        };
        let deadline = start + scaled_duration(self.schedule_us, speed);
        let lag = Instant::now().saturating_duration_since(deadline);
        if lag > LAG_TOLERANCE {
            self.start = Some(start + lag);
        }
    }

    /// waits an exact wall-clock duration, as manual Delay commands should.
    pub fn wait_exact(&mut self, delay_us: u64, speed: f64, kill: &AtomicBool) -> bool {
        self.schedule_us = self.schedule_us.saturating_add(delay_us);
        self.ensure_started();
        wait_until(Instant::now() + scaled_duration(delay_us, speed), kill)
    }

    /// pushes the timeline origin forward so paused or stepped time does not collapse recorded gaps.
    pub fn shift(&mut self, amount: Duration) {
        if let Some(start) = &mut self.start {
            *start += amount;
        }
    }

    /// starts a fresh timeline; called at the start of every repeat loop.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    fn ensure_started(&mut self) -> Instant {
        *self.start.get_or_insert_with(Instant::now)
    }
}

fn scaled_duration(delay_us: u64, speed: f64) -> Duration {
    Duration::from_secs_f64(delay_us as f64 / speed / 1_000_000.0)
}

pub enum FlowControl {
    /// proceed to the next command; the caller advances `frame.idx`.
    Continue,
    /// `frame.idx` already points at the next command; used by jumps so they can
    /// land on index 0 without off-by-one arithmetic.
    Jump,
    /// finish the current frame; the parent frame resumes from its saved index.
    BreakFrame,
    /// abort playback entirely.
    Stop,
}

#[cfg(test)]
mod tests {
    use super::*;

    static KILL: AtomicBool = AtomicBool::new(false);

    /// a timeline anchored far in the past, as if a long scan had run.
    fn lagging_timeline(secs: u64) -> PlaybackTimeline {
        let mut tl = PlaybackTimeline::new();
        tl.ensure_started();
        tl.start = Some(Instant::now() - Duration::from_secs(secs));
        tl
    }

    /// a far-past schedule must not hang the next wait; the timeline re-anchors.
    #[test]
    fn past_deadline_does_not_wait() {
        let mut tl = lagging_timeline(60);
        let start = Instant::now();
        assert!(tl.wait_scheduled(0, 1.0, &KILL));
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    /// a far-behind timeline is re-anchored, so the next gap is waited in full.
    #[test]
    fn lagging_timeline_waits_gap_in_full() {
        let mut tl = lagging_timeline(60);
        let start = Instant::now();
        assert!(tl.wait_scheduled(20_000, 1.0, &KILL));
        assert!(start.elapsed() >= Duration::from_millis(16));
    }

    /// sub-tolerance lag is absorbed into the gap, ending the wait early.
    #[test]
    fn small_lag_stays_absorbed() {
        let mut tl = PlaybackTimeline::new();
        tl.start = Some(Instant::now() - Duration::from_millis(5));
        let start = Instant::now();
        assert!(tl.wait_scheduled(20_000, 1.0, &KILL));
        assert!(start.elapsed() < Duration::from_millis(20));
    }

    /// a fresh timeline waits a scheduled gap in full.
    #[test]
    fn fresh_timeline_waits_scheduled_gap() {
        let mut tl = PlaybackTimeline::new();
        let start = Instant::now();
        assert!(tl.wait_scheduled(20_000, 1.0, &KILL));
        assert!(start.elapsed() >= Duration::from_millis(16));
    }

    /// consecutive gaps share one timeline, keeping the total close to the recorded sum.
    #[test]
    fn consecutive_gaps_share_one_timeline() {
        let mut tl = PlaybackTimeline::new();
        let start = Instant::now();
        assert!(tl.wait_scheduled(20_000, 1.0, &KILL));
        assert!(tl.wait_scheduled(20_000, 1.0, &KILL));
        assert!(start.elapsed() >= Duration::from_millis(32));
    }

    /// manual delays wait in full even when the timeline is already due, and advance the schedule.
    #[test]
    fn exact_delay_waits_in_full_when_schedule_is_due() {
        let mut tl = PlaybackTimeline::new();
        tl.shift(Duration::from_secs(60));
        let start = Instant::now();
        assert!(tl.wait_exact(20_000, 1.0, &KILL));
        assert!(start.elapsed() >= Duration::from_millis(16));
    }
}
