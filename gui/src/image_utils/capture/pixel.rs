//! pixel format mapping, BGRA -> Rec.601 luma conversion, and color copies.

use gbm::Format as GbmFormat;
use image::{GrayImage, RgbaImage};
use pipewire::spa::param::video::VideoFormat;

use rayon::prelude::*;

/// memory (little-endian) byte order of the captured frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PixelFormat {
    Bgra,
    Bgrx,
    Rgba,
    Rgbx,
    Argb,
    Abgr,
}

impl PixelFormat {
    pub(super) fn from_fourcc(fourcc: u32) -> Option<Self> {
        match VideoFormat::from_raw(fourcc) {
            VideoFormat::BGRA => Some(Self::Bgra),
            VideoFormat::BGRx => Some(Self::Bgrx),
            VideoFormat::RGBA => Some(Self::Rgba),
            VideoFormat::RGBx => Some(Self::Rgbx),
            VideoFormat::ARGB => Some(Self::Argb),
            VideoFormat::ABGR => Some(Self::Abgr),
            _ => None,
        }
    }

    /// DRM fourcc matching the in-memory byte order.
    /// DRM fourcc codes name channels from MSB to LSB, so the memory layout is
    /// the reversed name (e.g. DRM_FORMAT_ARGB8888 is BGRA in memory).
    pub(super) fn fourcc(self) -> GbmFormat {
        match self {
            Self::Bgra => GbmFormat::Argb8888,
            Self::Bgrx => GbmFormat::Xrgb8888,
            Self::Rgba => GbmFormat::Abgr8888,
            Self::Rgbx => GbmFormat::Xbgr8888,
            Self::Argb => GbmFormat::Bgra8888,
            Self::Abgr => GbmFormat::Rgba8888,
        }
    }
}

/// byte index of the r/g/b channel inside a 4-byte pixel word for `fmt`.
fn channel_offsets(fmt: PixelFormat) -> (u32, u32, u32) {
    match fmt {
        PixelFormat::Bgra | PixelFormat::Bgrx => (2u32, 1u32, 0u32),
        PixelFormat::Rgba | PixelFormat::Rgbx => (0, 1, 2),
        PixelFormat::Argb => (1, 2, 3),
        PixelFormat::Abgr => (3, 2, 1),
    }
}

#[inline(always)]
fn luma_word(px: u32, ro: u32, go: u32, bo: u32) -> u8 {
    let r = (px >> (ro * 8)) & 0xff;
    let g = (px >> (go * 8)) & 0xff;
    let b = (px >> (bo * 8)) & 0xff;
    ((r * 77 + g * 150 + b * 29) >> 8) as u8
}

/// converts one row. the inner loop loads 4 bytes as one u32 word with
/// constant channel shifts, which LLVM can vectorize.
#[inline]
fn convert_row(src: &[u8], src_off: usize, w: u32, out: &mut [u8], ro: u32, go: u32, bo: u32) {
    for col in 0..w {
        let p = src_off + col as usize * 4;
        let px = (src[p] as u32)
            | ((src[p + 1] as u32) << 8)
            | ((src[p + 2] as u32) << 16)
            | ((src[p + 3] as u32) << 24);
        out[col as usize] = luma_word(px, ro, go, bo);
    }
}

/// copies one row, reordering channels into RGBA output (alpha forced to 255).
#[inline]
fn copy_row(src: &[u8], src_off: usize, w: u32, out: &mut [u8], ro: u32, go: u32, bo: u32) {
    for col in 0..w {
        let p = src_off + col as usize * 4;
        let px = (src[p] as u32)
            | ((src[p + 1] as u32) << 8)
            | ((src[p + 2] as u32) << 16)
            | ((src[p + 3] as u32) << 24);
        let o = col as usize * 4;
        out[o] = ((px >> (ro * 8)) & 0xff) as u8;
        out[o + 1] = ((px >> (go * 8)) & 0xff) as u8;
        out[o + 2] = ((px >> (bo * 8)) & 0xff) as u8;
        out[o + 3] = 255;
    }
}

/// converts `(x0, y0, w, h)` from the raw `src` frame into `dst`, applying
/// Rec.601 luma weights. rows that would read past the end of `src` are
/// skipped (defensive; the frame may be a partial last frame). large regions
/// are converted with rows split across the rayon pool.
#[allow(clippy::too_many_arguments)]
pub(super) fn convert_region(
    src: &[u8],
    stride: usize,
    fmt: PixelFormat,
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    dst: &mut GrayImage,
) {
    let (ro, go, bo) = channel_offsets(fmt);
    if (w as u64) * (h as u64) >= 512 * 512 {
        dst.as_mut()
            .par_chunks_mut(w as usize)
            .zip(0..h)
            .for_each(|(out_row, row)| {
                let src_off = (y0 + row) as usize * stride + x0 as usize * 4;
                if src_off + w as usize * 4 > src.len() {
                    return;
                }
                convert_row(src, src_off, w, out_row, ro, go, bo);
            });
    } else {
        let out = dst.as_mut();
        for row in 0..h {
            let src_off = (y0 + row) as usize * stride + x0 as usize * 4;
            if src_off + w as usize * 4 > src.len() {
                break;
            }
            let r0 = row as usize * w as usize;
            convert_row(
                src,
                src_off,
                w,
                &mut out[r0..r0 + w as usize],
                ro,
                go,
                bo,
            );
        }
    }
}

/// copies `(x0, y0, w, h)` from the raw `src` frame into `dst` as RGBA,
/// reordering channels by `fmt`. same row iteration and rayon split as
/// `convert_region`, used for color reads (if-pixel-color).
#[allow(clippy::too_many_arguments)]
pub(super) fn copy_region_rgba(
    src: &[u8],
    stride: usize,
    fmt: PixelFormat,
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    dst: &mut RgbaImage,
) {
    let (ro, go, bo) = channel_offsets(fmt);
    if (w as u64) * (h as u64) >= 512 * 512 {
        dst.as_mut()
            .par_chunks_exact_mut(w as usize * 4)
            .zip(0..h)
            .for_each(|(out_row, row)| {
                let src_off = (y0 + row) as usize * stride + x0 as usize * 4;
                if src_off + w as usize * 4 > src.len() {
                    return;
                }
                copy_row(src, src_off, w, out_row, ro, go, bo);
            });
    } else {
        let out = dst.as_mut();
        for row in 0..h {
            let src_off = (y0 + row) as usize * stride + x0 as usize * 4;
            if src_off + w as usize * 4 > src.len() {
                break;
            }
            let r0 = row as usize * w as usize * 4;
            copy_row(
                src,
                src_off,
                w,
                &mut out[r0..r0 + w as usize * 4],
                ro,
                go,
                bo,
            );
        }
    }
}
