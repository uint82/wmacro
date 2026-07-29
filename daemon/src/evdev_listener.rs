use wmacro_core_types::{
    BTN_LEFT_CODE, BTN_MIDDLE_CODE, BTN_RIGHT_CODE, HardwareEvent, HardwareEventKind, HotkeyEvent,
    MacroButton,
};
use evdev::{AbsoluteAxisType, Device, InputEventKind, Key, RelativeAxisType};
use inotify::{Inotify, WatchMask};
use log::{error, info, warn};
use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const TAP_MAX_DURATION: Duration = Duration::from_millis(200);
const TAP_MAX_DISPLACEMENT: i32 = 20;
const INOTIFY_TOKEN: u64 = u64::MAX;
const EPOLL_TIMEOUT_MS: i32 = 100;

/// TODO: migrate to the `epoll` crate to remove raw `libc` usage in the future.
struct EpollFd(i32);
impl Drop for EpollFd {
    fn drop(&mut self) {
        if self.0 >= 0 {
            unsafe { libc::close(self.0) };
        }
    }
}

#[derive(Default)]
struct TapState {
    touch_down_time: Option<SystemTime>,
    last_tap_time: Option<SystemTime>,
    start_x: Option<i32>,
    start_y: Option<i32>,
    last_x: Option<i32>,
    last_y: Option<i32>,
    is_dragging: bool,
}

impl TapState {
    fn update_abs_x(&mut self, val: i32) {
        if self.start_x.is_none() {
            self.start_x = Some(val);
        }
        self.last_x = Some(val);
    }

    fn update_abs_y(&mut self, val: i32) {
        if self.start_y.is_none() {
            self.start_y = Some(val);
        }
        self.last_y = Some(val);
    }

    fn displacement(&self) -> i32 {
        let dx = self.last_x.zip(self.start_x).map(|(l, s)| (l - s).abs()).unwrap_or(0);
        let dy = self.last_y.zip(self.start_y).map(|(l, s)| (l - s).abs()).unwrap_or(0);
        dx.max(dy)
    }

    fn cancel_drag(&mut self, tx_hotkey: &Sender<HotkeyEvent>) -> Option<HardwareEventKind> {
        if !self.is_dragging {
            return None;
        }
        self.is_dragging = false;
        let _ = tx_hotkey.send(HotkeyEvent { code: BTN_LEFT_CODE, pressed: false });
        Some(HardwareEventKind::MouseUp(MacroButton::Left))
    }

    fn handle_touch_down(
        &mut self,
        hw_time: SystemTime,
        tx_hotkey: &Sender<HotkeyEvent>,
    ) -> Option<HardwareEventKind> {
        self.touch_down_time = Some(hw_time);
        self.start_x = None;
        self.start_y = None;
        self.last_x = None;
        self.last_y = None;

        if let Some(last_tap) = self.last_tap_time {
            if hw_time.duration_since(last_tap).unwrap_or_default() <= TAP_MAX_DURATION {
                self.is_dragging = true;
                let _ = tx_hotkey.send(HotkeyEvent { code: BTN_LEFT_CODE, pressed: true });
                return Some(HardwareEventKind::MouseDown(MacroButton::Left));
            }
        }
        None
    }

    fn handle_touch_up(
        &mut self,
        hw_time: SystemTime,
        tx: &Sender<HardwareEvent>,
        tx_hotkey: &Sender<HotkeyEvent>,
    ) -> Option<HardwareEventKind> {
        if self.is_dragging {
            self.is_dragging = false;
            self.touch_down_time = None;
            let _ = tx_hotkey.send(HotkeyEvent { code: BTN_LEFT_CODE, pressed: false });
            return Some(HardwareEventKind::MouseUp(MacroButton::Left));
        }

        let elapsed = self.touch_down_time.and_then(|t| hw_time.duration_since(t).ok());
        let displacement = self.displacement();

        let mut return_event = None;

        if let Some(el) = elapsed {
            if el <= TAP_MAX_DURATION && displacement <= TAP_MAX_DISPLACEMENT {
                let _ = tx_hotkey.send(HotkeyEvent { code: BTN_LEFT_CODE, pressed: true });
                let _ = tx.send(HardwareEvent {
                    hardware_time: hw_time,
                    kind: HardwareEventKind::MouseDown(MacroButton::Left),
                });
                let _ = tx_hotkey.send(HotkeyEvent { code: BTN_LEFT_CODE, pressed: false });

                return_event = Some(HardwareEventKind::MouseUp(MacroButton::Left));
                self.last_tap_time = Some(hw_time);
            } else {
                self.last_tap_time = None;
            }
        }

        self.touch_down_time = None;
        return_event
    }
}

fn is_supported_device(d: &Device) -> bool {
    d.supported_keys().map_or(false, |keys| {
        keys.contains(Key::KEY_A) || keys.contains(Key::BTN_LEFT)
    }) || d.supported_relative_axes()
        .map_or(false, |axes| axes.contains(RelativeAxisType::REL_X))
       || d.supported_absolute_axes()
        .map_or(false, |axes| axes.contains(AbsoluteAxisType::ABS_X))
}

fn register_epoll_fd(epfd: i32, fd: i32, token: u64) -> std::io::Result<()> {
    let mut event = libc::epoll_event {
        events: libc::EPOLLIN as u32,
        u64: token,
    };
    if unsafe { libc::epoll_ctl(epfd, libc::EPOLL_CTL_ADD, fd, &mut event) } < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn spawn_evdev_listener(
    tx: Sender<HardwareEvent>,
    tx_hotkey: Sender<HotkeyEvent>,
    stop_flag: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        if let Err(e) = run_listener_loop(tx, tx_hotkey, stop_flag) {
            error!("Evdev listener thread aborted due to error: {}", e);
        } else {
            info!("Listener thread stopped (stop_flag set or channel closed).");
        }
    });
}

fn run_listener_loop(
    tx: Sender<HardwareEvent>,
    tx_hotkey: Sender<HotkeyEvent>,
    stop_flag: Arc<AtomicBool>,
) -> std::io::Result<()> {
    let raw_epfd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
    if raw_epfd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let epfd = EpollFd(raw_epfd);

    let mut devices: HashMap<i32, Device> = HashMap::new();
    let mut tap_states: HashMap<i32, TapState> = HashMap::new();

    for (_, d) in evdev::enumerate() {
        if is_supported_device(&d) {
            let fd = d.as_raw_fd();
            if register_epoll_fd(epfd.0, fd, fd as u64).is_ok() {
                devices.insert(fd, d);
            }
        }
    }
    info!("Lightweight kernel listener active on {} devices.", devices.len());

    let mut inotify = Inotify::init()?;
    inotify.watches().add("/dev/input", WatchMask::CREATE)?;
    register_epoll_fd(epfd.0, inotify.as_raw_fd(), INOTIFY_TOKEN)?;

    let mut events = vec![libc::epoll_event { events: 0, u64: 0 }; 64];
    let mut inotify_buf = [0u8];

    while !stop_flag.load(Ordering::SeqCst) {
        let n = unsafe {
            libc::epoll_wait(
                epfd.0,
                events.as_mut_ptr(),
                events.len() as i32,
                EPOLL_TIMEOUT_MS,
            )
        };

        if n < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted { continue; }
            return Err(err);
        }

        for i in 0..(n as usize) {
            let token = events[i].u64;

            if token == INOTIFY_TOKEN {
                handle_inotify_events(&mut inotify, &mut inotify_buf[..], epfd.0, &mut devices);
            } else {
                let fd = token as i32;
                if !process_device_events(fd, epfd.0, &mut devices, &mut tap_states, &tx, &tx_hotkey) {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

fn handle_inotify_events(
    inotify: &mut Inotify,
    buf: &mut [u8],
    epfd: i32,
    devices: &mut HashMap<i32, Device>,
) {
    let Ok(inotify_events) = inotify.read_events(buf) else { return };

    for event in inotify_events {
        if let Some(name) = event.name {
            let path = format!("/dev/input/{}", name.to_string_lossy());
            if let Ok(new_device) = evdev::Device::open(&path) {
                if is_supported_device(&new_device) {
                    let fd = new_device.as_raw_fd();
                    if register_epoll_fd(epfd, fd, fd as u64).is_ok() {
                        devices.insert(fd, new_device);
                        info!("Hotplug detected & registered: {}", path);
                    }
                }
            }
        }
    }
}

fn process_device_events(
    fd: i32,
    epfd: i32,
    devices: &mut HashMap<i32, Device>,
    tap_states: &mut HashMap<i32, TapState>,
    tx: &Sender<HardwareEvent>,
    tx_hotkey: &Sender<HotkeyEvent>,
) -> bool {
    let mut disconnected = false;

    if let Some(device) = devices.get_mut(&fd) {
        match device.fetch_events() {
            Ok(ev_iter) => {
                let ts = tap_states.entry(fd).or_default();
                for raw_ev in ev_iter {
                    let hw_time = raw_ev.timestamp();
                    let val = raw_ev.value();

                    let kind = match raw_ev.kind() {
                        InputEventKind::Key(k) => handle_key_event(k, val, hw_time, ts, tx, tx_hotkey),
                        InputEventKind::RelAxis(axis) => handle_rel_axis(axis, val),
                        InputEventKind::AbsAxis(axis) => handle_abs_axis(axis, val, ts, tx_hotkey),
                        _ => None,
                    };

                    if let Some(k) = kind {
                        if tx.send(HardwareEvent { hardware_time: hw_time, kind: k }).is_err() {
                            return false;
                        }
                    }
                }
            }
            Err(_) => {
                disconnected = true;
            }
        }
    }

    if disconnected {
        unsafe {
            libc::epoll_ctl(epfd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut());
        }
        devices.remove(&fd);
        tap_states.remove(&fd);
        warn!("Device disconnected (fd: {})", fd);
    }

    true
}

fn handle_key_event(
    k: Key,
    val: i32,
    hw_time: SystemTime,
    ts: &mut TapState,
    tx: &Sender<HardwareEvent>,
    tx_hotkey: &Sender<HotkeyEvent>,
) -> Option<HardwareEventKind> {
    match k {
        Key::BTN_LEFT => dispatch_mouse_btn(val, BTN_LEFT_CODE, MacroButton::Left, tx_hotkey),
        Key::BTN_RIGHT => dispatch_mouse_btn(val, BTN_RIGHT_CODE, MacroButton::Right, tx_hotkey),
        Key::BTN_MIDDLE => dispatch_mouse_btn(val, BTN_MIDDLE_CODE, MacroButton::Middle, tx_hotkey),
        Key::BTN_TOUCH => match val {
            1 => ts.handle_touch_down(hw_time, tx_hotkey),
            0 => ts.handle_touch_up(hw_time, tx, tx_hotkey),
            _ => None,
        },
        Key::BTN_TOOL_FINGER => {
            if val == 0 {
                ts.cancel_drag(tx_hotkey)
            } else {
                None
            }
        }
        _ => {
            let code = k.code();
            match val {
                1 => {
                    let _ = tx_hotkey.send(HotkeyEvent { code, pressed: true });
                    Some(HardwareEventKind::KeyDown(format!("{:?}", k), code))
                }
                0 => {
                    let _ = tx_hotkey.send(HotkeyEvent { code, pressed: false });
                    Some(HardwareEventKind::KeyUp(format!("{:?}", k), code))
                }
                _ => None,
            }
        }
    }
}

fn handle_rel_axis(axis: RelativeAxisType, val: i32) -> Option<HardwareEventKind> {
    match axis {
        RelativeAxisType::REL_X | RelativeAxisType::REL_Y => Some(HardwareEventKind::MouseMove),
        RelativeAxisType::REL_WHEEL => Some(HardwareEventKind::Scroll { dx: 0, dy: -val }),
        RelativeAxisType::REL_HWHEEL => Some(HardwareEventKind::Scroll { dx: val, dy: 0 }),
        _ => None,
    }
}

fn handle_abs_axis(
    axis: AbsoluteAxisType,
    val: i32,
    ts: &mut TapState,
    tx_hotkey: &Sender<HotkeyEvent>,
) -> Option<HardwareEventKind> {
    match axis {
        AbsoluteAxisType::ABS_X | AbsoluteAxisType::ABS_MT_POSITION_X => {
            ts.update_abs_x(val);
            Some(HardwareEventKind::MouseMove)
        }
        AbsoluteAxisType::ABS_Y | AbsoluteAxisType::ABS_MT_POSITION_Y => {
            ts.update_abs_y(val);
            Some(HardwareEventKind::MouseMove)
        }
        AbsoluteAxisType::ABS_MT_TRACKING_ID => {
            if val == -1 {
                ts.cancel_drag(tx_hotkey)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn dispatch_mouse_btn(
    val: i32,
    code: u16,
    btn: MacroButton,
    tx_hotkey: &Sender<HotkeyEvent>,
) -> Option<HardwareEventKind> {
    if val == 1 || val == 0 {
        let _ = tx_hotkey.send(HotkeyEvent {
            code,
            pressed: val == 1,
        });
    }

    match val {
        1 => Some(HardwareEventKind::MouseDown(btn)),
        0 => Some(HardwareEventKind::MouseUp(btn)),
        _ => None,
    }
}
