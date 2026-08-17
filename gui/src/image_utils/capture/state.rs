use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use anyhow::Result;
use image::{DynamicImage, GrayImage, RgbaImage};

use crate::image_utils::outputs::OutputInfo;

use super::pixel::{PixelFormat, channel_offsets, convert_region, copy_region_rgba};

pub(super) struct CaptureRequest {
    /// region in global logical coordinates.
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) w: i32,
    pub(super) h: i32,

    /// request the whole captured output instead of a region.
    pub(super) full: bool,

    /// serve an RGBA copy of the region instead of a luma conversion.
    pub(super) color: bool,
    pub(super) reply: mpsc::SyncSender<Result<DynamicImage, String>>,
}

pub(super) struct CaptureState {
    pub(super) output: Option<OutputInfo>,
    pub(super) frame: Option<(u32, u32)>,
    pub(super) format: Option<PixelFormat>,
    pub(super) modifier: u64,
    pub(super) stream_error: Option<String>,
    pub(super) seq: u64,
    pub(super) retained_seq: u64,
    pub(super) last_served_seq: u64,
    pub(super) retained: Vec<u8>,
    pub(super) retained_stride: usize,
    pub(super) retained_w: u32,
    pub(super) retained_h: u32,
    pub(super) retained_origin: (u32, u32),
    pub(super) retained_fmt: PixelFormat,
    /// when the retained buffer was last read, bounding how stale a fast-path pixel read may be.
    pub(super) retained_at: Option<Instant>,
    pub(super) pending: Vec<CaptureRequest>,

    /// while set, every arriving frame is read into the retained buffer
    /// (bounded to the last used region when nothing is pending) so the
    /// pixel fast path stays hot; enabled by the macro player.
    pub(super) continuous: bool,
}

impl Default for CaptureState {
    fn default() -> Self {
        Self {
            output: None,
            frame: None,
            format: None,
            modifier: 0,
            stream_error: None,
            seq: 0,
            retained_seq: 0,
            last_served_seq: 0,
            retained: Vec::new(),
            retained_stride: 0,
            retained_w: 0,
            retained_h: 0,
            retained_origin: (0, 0),
            retained_fmt: PixelFormat::Bgra,
            retained_at: None,
            pending: Vec::new(),
            continuous: false,
        }
    }
}

fn frame_point(s: &CaptureState, x: i32, y: i32) -> Option<(i64, i64)> {
    let output = s.output.as_ref()?;
    let (fw, fh) = s.frame?;
    let sx = fw as f64 / output.size.0.max(1) as f64;
    let sy = fh as f64 / output.size.1.max(1) as f64;
    Some((
        (((x - output.pos.0) as f64) * sx).floor() as i64,
        (((y - output.pos.1) as f64) * sy).floor() as i64,
    ))
}

fn frame_rect(s: &CaptureState, x: i32, y: i32, w: i32, h: i32) -> Option<(i64, i64, i64, i64)> {
    let output = s.output.as_ref()?;
    let (fw, fh) = s.frame?;
    let sx = fw as f64 / output.size.0.max(1) as f64;
    let sy = fh as f64 / output.size.1.max(1) as f64;
    Some((
        (((x - output.pos.0) as f64) * sx).floor() as i64,
        (((y - output.pos.1) as f64) * sy).floor() as i64,
        ((((x + w) - output.pos.0) as f64) * sx).ceil() as i64,
        ((((y + h) - output.pos.1) as f64) * sy).ceil() as i64,
    ))
}

/// bounding box of all pending region requests in frame coordinates; `None`
/// means the whole frame must be read (a `full` request, or no usable region).
pub(super) fn request_union(s: &CaptureState, fw: u32, fh: u32) -> Option<(u32, u32, u32, u32)> {
    let mut x0 = fw as i64;
    let mut y0 = fh as i64;
    let mut x1 = 0i64;
    let mut y1 = 0i64;
    let mut valid = false;
    for req in &s.pending {
        if req.full {
            return None;
        }
        let Some((rx0, ry0, rx1, ry1)) = frame_rect(s, req.x, req.y, req.w, req.h) else {
            continue;
        };
        let rx0 = rx0.clamp(0, fw as i64);
        let rx1 = rx1.clamp(0, fw as i64);
        let ry0 = ry0.clamp(0, fh as i64);
        let ry1 = ry1.clamp(0, fh as i64);
        if rx1 > rx0 && ry1 > ry0 {
            valid = true;
            x0 = x0.min(rx0);
            y0 = y0.min(ry0);
            x1 = x1.max(rx1);
            y1 = y1.max(ry1);
        }
    }
    if !valid {
        return None;
    }
    Some((x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32))
}

/// whether `req` can be answered from the retained buffer. zero-area requests
/// (fully out of the output) are always coverable: `serve_requests` clamps turn them into empty images.
pub(super) fn request_coverable(s: &CaptureState, req: &CaptureRequest) -> bool {
    let Some((fw, fh)) = s.frame else {
        return false;
    };
    if s.retained_w == 0 || s.retained_h == 0 {
        return false;
    }
    if req.full {
        return s.retained_w == fw && s.retained_h == fh;
    }
    let Some((x0, y0, x1, y1)) = frame_rect(s, req.x, req.y, req.w, req.h) else {
        return false;
    };
    if x1 <= x0 || y1 <= y0 {
        return true;
    }
    let ox = s.retained_origin.0 as i64;
    let oy = s.retained_origin.1 as i64;
    x0 >= ox && x1 <= ox + s.retained_w as i64 && y0 >= oy && y1 <= oy + s.retained_h as i64
}

/// reads a single pixel out of the retained buffer; `None` when it has no data or does not cover the point. freshness is the caller's job.
pub(super) fn retained_pixel(s: &CaptureState, x: i32, y: i32) -> Option<(u8, u8, u8)> {
    if s.retained_w == 0 || s.retained_h == 0 {
        return None;
    }
    let (fx, fy) = frame_point(s, x, y)?;
    let (ox, oy) = (s.retained_origin.0 as i64, s.retained_origin.1 as i64);
    let (rw, rh) = (s.retained_w as i64, s.retained_h as i64);
    if fx < ox || fx >= ox + rw || fy < oy || fy >= oy + rh {
        return None;
    }
    let off = (fy - oy) as usize * s.retained_stride + (fx - ox) as usize * 4;
    if off + 4 > s.retained.len() {
        return None;
    }
    let px = u32::from_le_bytes([
        s.retained[off],
        s.retained[off + 1],
        s.retained[off + 2],
        s.retained[off + 3],
    ]);
    let (ro, go, bo) = channel_offsets(s.retained_fmt);
    Some((
        ((px >> (ro * 8)) & 0xff) as u8,
        ((px >> (go * 8)) & 0xff) as u8,
        ((px >> (bo * 8)) & 0xff) as u8,
    ))
}

/// global logical region -> exclusive frame rectangle `(x0, y0, x1, y1)`,
/// clipped to the output frame. `None` when the region has no visible area.
pub(super) fn visible_region_rect(
    s: &CaptureState,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Option<(i64, i64, i64, i64)> {
    let (fw, fh) = s.frame?;
    let (x0, y0, x1, y1) = frame_rect(s, x, y, w, h)?;
    let x0 = x0.clamp(0, fw as i64);
    let x1 = x1.clamp(0, fw as i64);
    let y0 = y0.clamp(0, fh as i64);
    let y1 = y1.clamp(0, fh as i64);
    if x1 > x0 && y1 > y0 {
        Some((x0, y0, x1, y1))
    } else {
        None
    }
}

pub(super) fn retained_rect_covered(s: &CaptureState, x0: i64, y0: i64, x1: i64, y1: i64) -> bool {
    if s.retained_w == 0 || s.retained_h == 0 {
        return false;
    }
    let ox = s.retained_origin.0 as i64;
    let oy = s.retained_origin.1 as i64;
    x0 >= ox && y0 >= oy && x1 <= ox + s.retained_w as i64 && y1 <= oy + s.retained_h as i64
}

/// copies rows `[y0, y1)` of columns `[x0, x1)` out of the retained buffer in
/// its native format; rows past the end are zero-filled so geometry is preserved.
pub(super) fn retained_snapshot(
    s: &CaptureState,
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
) -> (Vec<u8>, u32, u32) {
    let w = (x1 - x0) as usize;
    let h = (y1 - y0) as usize;
    let mut out = Vec::with_capacity(w * h * 4);
    let ox = s.retained_origin.0 as i64;
    let oy = s.retained_origin.1 as i64;
    for fy in y0..y1 {
        let off = (fy - oy) as usize * s.retained_stride + (x0 - ox) as usize * 4;
        let need = w * 4;
        if off + need <= s.retained.len() {
            out.extend_from_slice(&s.retained[off..off + need]);
        } else {
            out.extend(std::iter::repeat_n(0u8, need));
        }
    }
    (out, w as u32, h as u32)
}

pub(super) fn frame_to_logical(s: &CaptureState, fx: i64, fy: i64) -> Option<(i32, i32)> {
    let output = s.output.as_ref()?;
    let (fw, fh) = s.frame?;
    let sx = fw as f64 / output.size.0.max(1) as f64;
    let sy = fh as f64 / output.size.1.max(1) as f64;
    Some((
        (fx as f64 / sx).round() as i32 + output.pos.0,
        (fy as f64 / sy).round() as i32 + output.pos.1,
    ))
}

pub(super) fn serve_requests(state: &Arc<Mutex<CaptureState>>, requests: Vec<CaptureRequest>) {
    let s = state.lock().unwrap();
    let (rw, rh) = (s.retained_w as i64, s.retained_h as i64);
    if rw == 0 || rh == 0 {
        for req in requests {
            let _ = req
                .reply
                .try_send(Err("no frame data available".to_string()));
        }
        return;
    }
    let (ox, oy) = (s.retained_origin.0 as i64, s.retained_origin.1 as i64);
    let convert_start = Instant::now();
    let n_reqs = requests.len();

    for req in requests {
        let (bx, by, bw, bh) = if req.full {
            (0u32, 0u32, s.retained_w, s.retained_h)
        } else {
            let Some((x0, y0, x1, y1)) = frame_rect(&s, req.x, req.y, req.w, req.h) else {
                continue;
            };
            let x0 = (x0 - ox).clamp(0, rw);
            let x1 = (x1 - ox).clamp(0, rw);
            let y0 = (y0 - oy).clamp(0, rh);
            let y1 = (y1 - oy).clamp(0, rh);
            if x1 <= x0 || y1 <= y0 {
                (0, 0, 0, 0)
            } else {
                (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32)
            }
        };
        let img = if req.color {
            let mut img = RgbaImage::new(bw, bh);
            copy_region_rgba(
                &s.retained,
                s.retained_stride,
                s.retained_fmt,
                bx,
                by,
                bw,
                bh,
                &mut img,
            );
            DynamicImage::ImageRgba8(img)
        } else {
            let mut img = GrayImage::new(bw, bh);
            convert_region(
                &s.retained,
                s.retained_stride,
                s.retained_fmt,
                bx,
                by,
                bw,
                bh,
                &mut img,
            );
            DynamicImage::ImageLuma8(img)
        };
        let _ = req.reply.try_send(Ok(img));
    }
    log::debug!(
        "served {n_reqs} request(s), conversion took {:.3}ms",
        convert_start.elapsed().as_secs_f64() * 1000.0
    );
}
