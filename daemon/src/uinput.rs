use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use wmacro_core_types::{ClickButton, ClickType};

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const EV_ABS: u16 = 0x03;
const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const REL_HWHEEL: u16 = 0x06;
const REL_WHEEL: u16 = 0x08;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;
const SYN_REPORT: u16 = 0x00;

const UI_SET_EVBIT: u64 = 0x40045564;
const UI_SET_KEYBIT: u64 = 0x40045565;
const UI_SET_RELBIT: u64 = 0x40045566;
const UI_SET_ABSBIT: u64 = 0x40045567;
const UI_DEV_CREATE: u64 = 0x5501;
const UI_DEV_DESTROY: u64 = 0x5502;
const UI_DEV_SETUP: u64 = 0x405c5503;
const UI_ABS_SETUP: u64 = 0x401c5504;

#[repr(C)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
struct UinputSetup {
    id: InputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

#[repr(C)]
struct InputAbsinfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

#[repr(C)]
struct UinputAbsSetup {
    code: u16,
    absinfo: InputAbsinfo,
}

// TODO: tv_sec and tv_usec are strictly i64 here, which perfectly aligns
// for 64-bit Linux. on 32-bit Linux (like older Raspberry Pis), this struct
// layout will be incorrect and will corrupt kernel messages.
#[repr(C)]
struct InputEvent {
    tv_sec: i64,
    tv_usec: i64,
    kind: u16,
    code: u16,
    value: i32,
}

fn detect_screen_bounds() -> (i32, i32) {
    if let Some(b) = try_hyprctl() {
        return b;
    }
    if let Some(b) = try_xrandr() {
        return b;
    }
    if let Some(b) = try_wlr_randr() {
        return b;
    }
    if let Some(b) = try_drm_sysfs() {
        return b;
    }

    // TODO: cross-check the detected bounds against a real compositor before trusting them.
    log::warn!("Screen detection failed across all backends, falling back to 1920x1080");
    (1920, 1080)
}

fn try_hyprctl() -> Option<(i32, i32)> {
    let out = std::process::Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }

    let json = std::str::from_utf8(&out.stdout).ok()?;
    let mut max_x = 0;
    let mut max_y = 0;

    for m in split_json_objects(json) {
        let x = json_i32(&m, "x")?;
        let y = json_i32(&m, "y")?;
        let width = json_i32(&m, "width")?;
        let height = json_i32(&m, "height")?;
        let scale = json_f32(&m, "scale").unwrap_or(1.0);

        let right = x + (width as f32 / scale) as i32;
        let bottom = y + (height as f32 / scale) as i32;

        max_x = max_x.max(right);
        max_y = max_y.max(bottom);
    }

    if max_x > 0 && max_y > 0 {
        log::info!("Screen bounds from hyprctl: {}x{}", max_x, max_y);
        Some((max_x, max_y))
    } else {
        None
    }
}

fn try_xrandr() -> Option<(i32, i32)> {
    let out = std::process::Command::new("xrandr")
        .arg("--query")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }

    let text = std::str::from_utf8(&out.stdout).ok()?;
    for line in text.lines().filter(|l| l.contains("current")) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if let Some(pos) = parts.iter().position(|&w| w == "current")
            && parts.len() > pos + 3
        {
            let w: i32 = parts[pos + 1].parse().ok()?;
            let h: i32 = parts[pos + 3].trim_end_matches(',').parse().ok()?;
            log::info!("Screen bounds from xrandr: {}x{}", w, h);
            return Some((w, h));
        }
    }
    None
}

fn try_wlr_randr() -> Option<(i32, i32)> {
    let out = std::process::Command::new("wlr-randr")
        .arg("--json")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }

    let json = std::str::from_utf8(&out.stdout).ok()?;
    let mut max_x = 0;
    let mut max_y = 0;

    for obj in split_json_objects(json) {
        if let (Some(w), Some(h)) = (json_i32(&obj, "width"), json_i32(&obj, "height")) {
            max_x = max_x.max(w);
            max_y = max_y.max(h);
        }
    }

    if max_x > 0 && max_y > 0 {
        log::info!("Screen bounds from wlr-randr: {}x{}", max_x, max_y);
        Some((max_x, max_y))
    } else {
        None
    }
}

fn try_drm_sysfs() -> Option<(i32, i32)> {
    let mut max_x = 0;
    let mut max_y = 0;
    let dir = std::fs::read_dir("/sys/class/drm").ok()?;

    for entry in dir.flatten() {
        let path = entry.path().join("modes");
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                if let Some((ws, hs)) = line.trim().split_once('x')
                    && let (Ok(w), Ok(h)) = (ws.parse::<i32>(), hs.parse::<i32>())
                {
                    max_x = max_x.max(w);
                    max_y = max_y.max(h);
                }
            }
        }
    }

    if max_x > 0 && max_y > 0 {
        log::info!("Screen bounds from drm sysfs: {}x{}", max_x, max_y);
        Some((max_x, max_y))
    } else {
        None
    }
}

fn split_json_objects(json: &str) -> Vec<String> {
    json.split("},{").map(|s| s.to_string()).collect()
}

fn json_i32(obj: &str, key: &str) -> Option<i32> {
    let needle = format!("\"{}\":", key);
    let start = obj.find(&needle)? + needle.len();
    let rest = obj[start..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn json_f32(obj: &str, key: &str) -> Option<f32> {
    let needle = format!("\"{}\":", key);
    let start = obj.find(&needle)? + needle.len();
    let rest = obj[start..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

pub struct UinputBackend {
    file: std::fs::File,
    last_x: i32,
    last_y: i32,
}

impl UinputBackend {
    pub fn new() -> Result<Self, String> {
        let (max_x, max_y) = detect_screen_bounds();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uinput")
            .map_err(|e| format!("Failed to open /dev/uinput: {e} (do you have permissions?)"))?;

        let fd = file.as_raw_fd();

        unsafe {
            setup_device_capabilities(fd, max_x, max_y)?;
            create_uinput_device(fd, &b"wmacro-tablet"[..])?;
        }

        log::info!("Absolute tablet device created ({}x{})", max_x, max_y);
        wait_for_device_registration("wmacro-tablet");

        Ok(Self {
            file,
            last_x: 0,
            last_y: 0,
        })
    }

    fn btn_code(button: &ClickButton) -> u16 {
        match button {
            ClickButton::Left => BTN_LEFT,
            ClickButton::Right => BTN_RIGHT,
            ClickButton::Middle => BTN_MIDDLE,
        }
    }

    fn write_event(&mut self, kind: u16, code: u16, value: i32) -> Result<(), String> {
        let duration = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default();

        let ev = InputEvent {
            tv_sec: duration.as_secs() as i64,
            tv_usec: duration.subsec_micros() as i64,
            kind,
            code,
            value,
        };

        let bytes = unsafe {
            std::slice::from_raw_parts(
                &ev as *const _ as *const u8,
                std::mem::size_of::<InputEvent>(),
            )
        };

        self.file
            .write_all(bytes)
            .map_err(|e| format!("Failed to write event ({kind}, {code}, {value}): {e}"))
    }

    fn syn(&mut self) -> Result<(), String> {
        self.write_event(EV_SYN, SYN_REPORT, 0)
    }

    fn single_click(
        &mut self,
        code: u16,
        hold_duration_ms: u64,
        target_x: i32,
        target_y: i32,
        button: &ClickButton,
    ) -> Result<(), String> {
        self.write_event(EV_KEY, code, 1)?;
        self.syn()?;
        log::info!("press({:?}) at ({},{})", button, target_x, target_y);

        if hold_duration_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(hold_duration_ms));
        }

        self.write_event(EV_KEY, code, 0)?;
        self.syn()?;
        log::info!("release({:?})", button);

        Ok(())
    }

    pub fn move_to(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.write_event(EV_ABS, ABS_X, x)?;
        self.write_event(EV_ABS, ABS_Y, y)?;
        self.syn()?;

        self.write_event(EV_REL, REL_X, 1)?;
        self.syn()?;
        self.write_event(EV_REL, REL_X, -1)?;
        self.syn()?;

        self.last_x = x;
        self.last_y = y;
        Ok(())
    }

    pub fn press(&mut self, button: &ClickButton) -> Result<(), String> {
        self.write_event(EV_KEY, Self::btn_code(button), 1)?;
        self.syn()
    }

    pub fn release(&mut self, button: &ClickButton) -> Result<(), String> {
        self.write_event(EV_KEY, Self::btn_code(button), 0)?;
        self.syn()
    }

    pub fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        if dy != 0 {
            self.write_event(EV_REL, REL_WHEEL, -dy)?;
            self.syn()?;
        }
        if dx != 0 {
            self.write_event(EV_REL, REL_HWHEEL, dx)?;
        }
        if dx != 0 || dy != 0 {
            self.syn()?;
        }
        Ok(())
    }

    pub fn key_down(&mut self, key: &str, code: u16) -> Result<(), String> {
        if code == 0 {
            log::warn!(
                "Key '{}' recorded with code 0 (KEY_RESERVED), skipping",
                key
            );
            return Ok(());
        }
        self.write_event(EV_KEY, code, 1)?;
        self.syn()
    }

    pub fn key_up(&mut self, key: &str, code: u16) -> Result<(), String> {
        if code == 0 {
            log::warn!(
                "Key '{}' recorded with code 0 (KEY_RESERVED), skipping",
                key
            );
            return Ok(());
        }
        self.write_event(EV_KEY, code, 0)?;
        self.syn()
    }

    pub fn click(
        &mut self,
        target_x: i32,
        target_y: i32,
        button: &ClickButton,
        click_type: &ClickType,
        hold_duration_ms: u64,
        move_cursor: bool,
    ) -> Result<(), String> {
        if move_cursor {
            self.move_to(target_x, target_y)?;
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let code = Self::btn_code(button);
        match click_type {
            ClickType::Single => {
                self.single_click(code, hold_duration_ms, target_x, target_y, button)?;
            }
            ClickType::Double => {
                self.single_click(code, hold_duration_ms, target_x, target_y, button)?;
                std::thread::sleep(std::time::Duration::from_millis(40));
                self.single_click(code, hold_duration_ms, target_x, target_y, button)?;
            }
        }
        Ok(())
    }
}

impl Drop for UinputBackend {
    fn drop(&mut self) {
        unsafe {
            let _ = ioctl_val(self.file.as_raw_fd(), UI_DEV_DESTROY, 0);
        }
        log::info!("Virtual uinput device destroyed");
    }
}

unsafe fn setup_device_capabilities(fd: i32, max_x: i32, max_y: i32) -> Result<(), String> {
    unsafe {
        ioctl_val(fd, UI_SET_EVBIT, EV_KEY as i32)?;
        ioctl_val(fd, UI_SET_KEYBIT, BTN_LEFT as i32)?;
        ioctl_val(fd, UI_SET_KEYBIT, BTN_RIGHT as i32)?;
        ioctl_val(fd, UI_SET_KEYBIT, BTN_MIDDLE as i32)?;

        enable_keyboard_keys(fd)?;

        ioctl_val(fd, UI_SET_EVBIT, EV_REL as i32)?;
        ioctl_val(fd, UI_SET_RELBIT, REL_X as i32)?;
        ioctl_val(fd, UI_SET_RELBIT, REL_Y as i32)?;
        ioctl_val(fd, UI_SET_RELBIT, REL_WHEEL as i32)?;
        ioctl_val(fd, UI_SET_RELBIT, REL_HWHEEL as i32)?;

        ioctl_val(fd, UI_SET_EVBIT, EV_ABS as i32)?;
        ioctl_val(fd, UI_SET_ABSBIT, ABS_X as i32)?;
        ioctl_val(fd, UI_SET_ABSBIT, ABS_Y as i32)?;

        ioctl_val(fd, UI_SET_EVBIT, EV_SYN as i32)?;
    }

    let setup_abs = |code: u16, max_val: i32| -> Result<(), String> {
        let abs = UinputAbsSetup {
            code,
            absinfo: InputAbsinfo {
                value: 0,
                minimum: 0,
                maximum: max_val - 1,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            },
        };
        unsafe { ioctl_ptr(fd, UI_ABS_SETUP, &abs) }
    };

    setup_abs(ABS_X, max_x)?;
    setup_abs(ABS_Y, max_y)?;

    Ok(())
}

unsafe fn enable_keyboard_keys(fd: i32) -> Result<(), String> {
    let keys = [
        1u32, 14, 15, 28, 29, 42, 54, 56, 57, 58, 69, 70, 97, 99, 100, 102, 103, 104, 105, 106,
        107, 108, 109, 110, 111, 114, 119, 125, 126, 12, 13, 26, 27, 39, 40, 41, 43, 51, 52, 53,
        55, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 98, 30, 48, 46, 32, 18, 33, 34, 35,
        23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44,
    ];

    unsafe {
        for kc in keys {
            ioctl_val(fd, UI_SET_KEYBIT, kc as i32)?;
        }
        for kc in 2u32..=11 {
            ioctl_val(fd, UI_SET_KEYBIT, kc as i32)?;
        }
    }
    Ok(())
}

unsafe fn create_uinput_device(fd: i32, device_name: &[u8]) -> Result<(), String> {
    unsafe {
        let mut setup: UinputSetup = std::mem::zeroed();
        setup.id.bustype = 0x03; // BUS_USB, so the compositor recognizes us as a plain USB peripheral.
        setup.id.vendor = 0x1234;
        setup.id.product = 0x5678;
        setup.id.version = 1;

        let len = device_name.len().min(setup.name.len());
        setup.name[..len].copy_from_slice(&device_name[..len]);

        ioctl_ptr(fd, UI_DEV_SETUP, &setup)?;
        ioctl_val(fd, UI_DEV_CREATE, 0)?;
    }
    Ok(())
}

fn wait_for_device_registration(device_name: &str) {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(1000);
    let poll_interval = std::time::Duration::from_millis(10);
    // TODO: retry with exponential backoff instead of a fixed poll interval.

    loop {
        if start.elapsed() > timeout {
            log::warn!(
                "Timed out waiting for uinput device '{}'. Early events may be dropped.",
                device_name
            );
            break;
        }

        if let Ok(output) = std::process::Command::new("hyprctl")
            .args(["devices", "-j"])
            .output()
        {
            if let Ok(json) = std::str::from_utf8(&output.stdout)
                && json.contains(device_name)
            {
                log::info!(
                    "Compositor registered device in {}ms",
                    start.elapsed().as_millis()
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
                return;
            }
        } else {
            log::warn!(
                "hyprctl not available to check device registration, falling back to 200ms sleep."
            );
            std::thread::sleep(std::time::Duration::from_millis(200));
            return;
        }

        std::thread::sleep(poll_interval);
    }
}

unsafe fn ioctl_val(fd: i32, request: u64, val: i32) -> Result<(), String> {
    let ret = unsafe { libc::ioctl(fd, request as std::os::raw::c_ulong, val) };
    if ret < 0 {
        Err(format!(
            "ioctl_val({:#x}): {}",
            request,
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

unsafe fn ioctl_ptr<T>(fd: i32, request: u64, ptr: *const T) -> Result<(), String> {
    let ret = unsafe { libc::ioctl(fd, request as std::os::raw::c_ulong, ptr) };
    if ret < 0 {
        Err(format!(
            "ioctl_ptr({:#x}): {}",
            request,
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}
