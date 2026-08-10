//! capture request state shared between the request thread and the PipeWire
//! thread, plus the serve path that converts the retained frame into the
//! requested regions.

use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use anyhow::Result;
use image::{DynamicImage, GrayImage, RgbaImage};

use crate::image_utils::outputs::OutputInfo;

use super::pixel::{convert_region, copy_region_rgba, PixelFormat};

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
    pub(super) pending: Vec<CaptureRequest>,
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
            pending: Vec::new(),
        }
    }
}

/// bounding box of all pending region requests in frame coordinates. `None`
/// means the whole frame must be read (a `full` request, or no usable region:
/// the fallback keeps serving empty images for out-of-screen requests).
pub(super) fn request_union(s: &CaptureState, fw: u32, fh: u32) -> Option<(u32, u32, u32, u32)> {
    let output = s.output.as_ref()?;
    let sx = fw as f64 / output.size.0.max(1) as f64;
    let sy = fh as f64 / output.size.1.max(1) as f64;
    let mut x0 = fw as i64;
    let mut y0 = fh as i64;
    let mut x1 = 0i64;
    let mut y1 = 0i64;
    let mut valid = false;
    for req in &s.pending {
        if req.full {
            return None;
        }
        let rx0 = (((req.x - output.pos.0) as f64) * sx).floor() as i64;
        let ry0 = (((req.y - output.pos.1) as f64) * sy).floor() as i64;
        let rx1 = ((((req.x + req.w) - output.pos.0) as f64) * sx).ceil() as i64;
        let ry1 = ((((req.y + req.h) - output.pos.1) as f64) * sy).ceil() as i64;
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

/// whether `req` can be answered from the currently retained buffer. zero-area
/// requests (fully out of the output) are always coverable: the clamps in
/// `serve_requests` turn them into empty images.
pub(super) fn request_coverable(s: &CaptureState, req: &CaptureRequest) -> bool {
    let Some((fw, fh)) = s.frame else { return false };
    if s.retained_w == 0 || s.retained_h == 0 {
        return false;
    }
    if req.full {
        return s.retained_w == fw && s.retained_h == fh;
    }
    let Some(output) = &s.output else { return false };
    let sx = fw as f64 / output.size.0.max(1) as f64;
    let sy = fh as f64 / output.size.1.max(1) as f64;
    let x0 = (((req.x - output.pos.0) as f64) * sx).floor() as i64;
    let y0 = (((req.y - output.pos.1) as f64) * sy).floor() as i64;
    let x1 = ((((req.x + req.w) - output.pos.0) as f64) * sx).ceil() as i64;
    let y1 = ((((req.y + req.h) - output.pos.1) as f64) * sy).ceil() as i64;
    if x1 <= x0 || y1 <= y0 {
        return true;
    }
    let ox = s.retained_origin.0 as i64;
    let oy = s.retained_origin.1 as i64;
    x0 >= ox
        && x1 <= ox + s.retained_w as i64
        && y0 >= oy
        && y1 <= oy + s.retained_h as i64
}

pub(super) fn serve_requests(state: &Arc<Mutex<CaptureState>>, requests: Vec<CaptureRequest>) {
    let s = state.lock().unwrap();
    let Some(output) = &s.output else { return };
    let Some((fw, fh)) = s.frame else { return };
    let fw = fw as i64;
    let fh = fh as i64;
    let (rw, rh) = (s.retained_w as i64, s.retained_h as i64);
    if rw == 0 || rh == 0 {
        for req in requests {
            let _ = req.reply.try_send(Err("no frame data available".to_string()));
        }
        return;
    }
    let (ox, oy) = (s.retained_origin.0 as i64, s.retained_origin.1 as i64);
    let sx = fw as f64 / output.size.0.max(1) as f64;
    let sy = fh as f64 / output.size.1.max(1) as f64;
    let convert_start = Instant::now();
    let n_reqs = requests.len();

    for req in requests {
        let (bx, by, bw, bh) = if req.full {
            (0u32, 0u32, s.retained_w, s.retained_h)
        } else {
            let x0 = (((req.x - output.pos.0) as f64) * sx).floor() as i64;
            let y0 = (((req.y - output.pos.1) as f64) * sy).floor() as i64;
            let x1 = ((((req.x + req.w) - output.pos.0) as f64) * sx).ceil() as i64;
            let y1 = ((((req.y + req.h) - output.pos.1) as f64) * sy).ceil() as i64;
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
