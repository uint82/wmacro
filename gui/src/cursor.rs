use crate::state::SharedState;
use std::time::Duration;

#[cfg(target_os = "linux")]
use std::env;
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixStream;

pub fn spawn_cursor_tracker(state: SharedState) {
    std::thread::spawn(move || {
        loop {
            if let Some((x, y)) = get_cursor_position() {
                let mut s = state.lock().unwrap();
                s.cursor_x = x;
                s.cursor_y = y;
            }
            // TODO: measure idle CPU usage and adjust the polling interval if needed.
            std::thread::sleep(Duration::from_millis(16));
        }
    });
}

#[cfg(target_os = "linux")]
fn get_cursor_position() -> Option<(i32, i32)> {
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
    let output = std::process::Command::new("grim")
        .arg("-g")
        .arg(format!("{},{} 1x1", x, y))
        .arg("-t")
        .arg("ppm")
        .arg("-")
        .output();

    match output {
        Ok(out) if out.status.success() && out.stdout.len() >= 3 => {
            let len = out.stdout.len();
            let r = out.stdout[len - 3];
            let g = out.stdout[len - 2];
            let b = out.stdout[len - 1];
            (r, g, b)
        }
        Ok(out) => {
            log::warn!("grim succeeded but returned unexpected output. status: {}", out.status);
            (0, 0, 0)
        }
        Err(e) => {
            log::warn!("Failed to execute grim to get pixel color: {e}");
            (0, 0, 0)
        }
    }
}

