use crate::image_utils::capture::capture_pixel_color;
use crate::state::SharedState;
use std::time::Duration;

#[cfg(target_os = "linux")]
use std::env;
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixStream;

pub fn spawn_cursor_tracker(state: SharedState) {
    // 60Hz polling keeps the "insert at cursor" defaults honest without wasting much CPU.
    std::thread::spawn(move || {
        loop {
            if let Some((x, y)) = get_cursor_position() {
                let mut s = state.lock().unwrap();
                s.cursor_x = x;
                s.cursor_y = y;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    });
}

#[cfg(target_os = "linux")]
fn get_cursor_position() -> Option<(i32, i32)> {
    // the hyprland socket answers instantly; hyprctl spawns a process, so it is only a fallback for setups without the socket.
    if let Some(pos) = hyprland_cursorpos_socket() {
        return Some(pos);
    }
    if let Some(pos) = hyprctl_cursorpos() {
        return Some(pos);
    }
    None
}

#[cfg(target_os = "linux")]
fn get_hyprland_socket_path() -> Option<String> {
    let signature = env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    if let Ok(xdg) = env::var("XDG_RUNTIME_DIR") {
        let path = format!("{}/hypr/{}/.socket.sock", xdg, signature);
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }
    Some(format!("/tmp/hypr/{}/.socket.sock", signature))
}

#[cfg(target_os = "linux")]
pub fn hyprland_cursorpos_socket() -> Option<(i32, i32)> {
    let socket_path = get_hyprland_socket_path()?;
    let mut stream = UnixStream::connect(&socket_path).ok()?;

    stream
        .set_write_timeout(Some(Duration::from_millis(100)))
        .ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok()?;
    stream.write_all(b"cursorpos").ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;

    let stdout = response.trim();
    let mut parts = stdout.splitn(2, ',');
    let x: i32 = parts.next()?.trim().parse().ok()?;
    let y: i32 = parts.next()?.trim().parse().ok()?;
    Some((x, y))
}

#[cfg(target_os = "linux")]
pub(crate) fn hyprctl_cursorpos() -> Option<(i32, i32)> {
    let output = std::process::Command::new("hyprctl")
        .arg("cursorpos")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = std::str::from_utf8(&output.stdout).ok()?.trim();
    let mut parts = stdout.splitn(2, ',');
    let x: i32 = parts.next()?.trim().parse().ok()?;
    let y: i32 = parts.next()?.trim().parse().ok()?;
    Some((x, y))
}

#[cfg(target_os = "linux")]
pub fn hyprland_movecursor_socket(x: i32, y: i32) -> Option<()> {
    // read the reply even though it is ignored: the socket call blocks until the daemon has processed the dispatch.
    let socket_path = get_hyprland_socket_path()?;
    let mut stream = UnixStream::connect(&socket_path).ok()?;

    stream
        .set_write_timeout(Some(Duration::from_millis(100)))
        .ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok()?;

    let cmd = format!("dispatch movecursor {} {}", x, y);
    stream.write_all(cmd.as_bytes()).ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;

    Some(())
}

#[cfg(target_os = "linux")]
pub fn get_pixel_color(x: i32, y: i32) -> (u8, u8, u8) {
    // fast path: served from the capture thread's retained frame when it is fresh and covers (x, y); otherwise a fresh 32x32 tile is captured.
    // TODO(failure-mode): a failed read returns (0,0,0), indistinguishable from an actual black pixel. surface the failure to callers instead (e.g. Result or a last-error flag).
    let t_start = std::time::Instant::now();
    let px = capture_pixel_color(x, y);
    log::debug!(
        "[TIMING] get_pixel_color total execution took: {:?}",
        t_start.elapsed()
    );
    px.unwrap_or_else(|e| {
        log::warn!("failed to read pixel at ({x}, {y}): {e}");
        (0, 0, 0)
    })
}
