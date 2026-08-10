use crate::image_utils::capture::capture_region_color_native;
use crate::state::SharedState;
use image::RgbaImage;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::env;
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixStream;

const CACHE_TTL: Duration = Duration::from_millis(100);
const CAPTURE_TILE_SIZE: u32 = 32;
const CAPTURE_TILE_MARGIN: i32 = (CAPTURE_TILE_SIZE / 2) as i32;

struct CapturedTile {
    image: RgbaImage,
    origin_x: i32,
    origin_y: i32,
}

struct ScreenCache {
    tile: Option<Arc<CapturedTile>>,
    last_captured: Option<Instant>,
}

static SCREEN_CACHE: Mutex<ScreenCache> = Mutex::new(ScreenCache {
    tile: None,
    last_captured: None,
});

fn get_cached_screen(x: i32, y: i32) -> Option<Arc<CapturedTile>> {
    let mut cache = SCREEN_CACHE.lock().ok()?;

    let cache_hit = cache.last_captured.is_some_and(|t| t.elapsed() < CACHE_TTL)
        && cache.tile.as_ref().is_some_and(|tile| {
            x >= tile.origin_x
                && y >= tile.origin_y
                && x < tile.origin_x + CAPTURE_TILE_SIZE as i32
                && y < tile.origin_y + CAPTURE_TILE_SIZE as i32
        });

    if cache_hit {
        return cache.tile.clone();
    }

    let origin_x = (x - CAPTURE_TILE_MARGIN).max(0);
    let origin_y = (y - CAPTURE_TILE_MARGIN).max(0);

    let t_start = Instant::now();
    let img = capture_region_color_native(
        origin_x,
        origin_y,
        CAPTURE_TILE_SIZE as i32,
        CAPTURE_TILE_SIZE as i32,
    )
    .ok()?;
    log::debug!("[TIMING] On-demand region capture took: {:?}", t_start.elapsed());

    let tile = Arc::new(CapturedTile {
        image: img,
        origin_x,
        origin_y,
    });
    cache.tile = Some(Arc::clone(&tile));
    cache.last_captured = Some(Instant::now());

    Some(tile)
}

pub fn spawn_cursor_tracker(state: SharedState) {
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

    stream.set_write_timeout(Some(Duration::from_millis(100))).ok()?;
    stream.set_read_timeout(Some(Duration::from_millis(100))).ok()?;
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

    stream.set_write_timeout(Some(Duration::from_millis(100))).ok()?;
    stream.set_read_timeout(Some(Duration::from_millis(100))).ok()?;

    let cmd = format!("dispatch movecursor {} {}", x, y);
    stream.write_all(cmd.as_bytes()).ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;

    Some(())
}

#[cfg(target_os = "linux")]
pub fn get_pixel_color(x: i32, y: i32) -> (u8, u8, u8) {
    // TODO(failure-mode): a failed read returns (0,0,0), which is
    // indistinguishable from an actual black pixel. IfPixelColor would treat
    // the failure as a black match. surface the failure to callers instead
    // (e.g. Result or a last-error flag) so the macro player can skip the
    // condition instead of comparing against black.
    let t_start = std::time::Instant::now();

    let tile = match get_cached_screen(x, y) {
        Some(tile) => tile,
        None => {
            log::warn!("Failed to retrieve screen tile from cache.");
            return (0, 0, 0);
        }
    };

    let local_x = x - tile.origin_x;
    let local_y = y - tile.origin_y;

    if local_x < 0 || local_y < 0 {
        log::warn!("Coordinates out of bounds.");
        return (0, 0, 0);
    }
    let (local_x, local_y) = (local_x as u32, local_y as u32);

    if local_x < tile.image.width() && local_y < tile.image.height() {
        let pixel = tile.image.get_pixel(local_x, local_y);
        log::debug!("[TIMING] get_pixel_color total execution took: {:?}", t_start.elapsed());
        (pixel[0], pixel[1], pixel[2])
    } else {
        log::warn!("Coordinates out of bounds.");
        (0, 0, 0)
    }
}
