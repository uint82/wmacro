use super::pixel::{PixelFormat, channel_offsets};

/// result of a color-region scan over a region of native-format bytes.
pub(super) struct ColorRegion {
    /// number of matching pixels (used only to pick the largest one).
    pub(super) area: u32,
    /// mass center in local pixel coordinates of the scanned region.
    pub(super) cx: f64,
    pub(super) cy: f64,
    /// inclusive bounding box of the color, in local pixel coordinates.
    pub(super) x0: i32,
    pub(super) y0: i32,
    pub(super) x1: i32,
    pub(super) y1: i32,
}

impl ColorRegion {
    pub(super) fn width(&self) -> u32 {
        (self.x1 - self.x0 + 1) as u32
    }

    pub(super) fn height(&self) -> u32 {
        (self.y1 - self.y0 + 1) as u32
    }
}

/// largest 8-connected region of pixels within `tolerance` (0-100, Euclidean
/// RGB distance) of (r, g, b), ignoring regions narrower than `min_width` or
/// shorter than `min_height`; returns the region's center and bounding box.
#[allow(clippy::too_many_arguments)]
pub(super) fn largest_color_region(
    buf: &[u8],
    stride: usize,
    w: u32,
    h: u32,
    fmt: PixelFormat,
    r: u8,
    g: u8,
    b: u8,
    tolerance: u8,
    min_width: u32,
    min_height: u32,
) -> Option<ColorRegion> {
    let n = w as usize * h as usize;
    if n == 0 {
        return None;
    }
    let (ro, go, bo) = channel_offsets(fmt);
    let max = 441.673_f64 * (f64::from(tolerance) / 100.0);
    let max_sq = max * max;

    let matches = |x: usize, y: usize| -> bool {
        let off = y * stride + x * 4;
        if off + 4 > buf.len() {
            return false;
        }
        let px = u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
        let pr = ((px >> (ro * 8)) & 0xff) as i64;
        let pg = ((px >> (go * 8)) & 0xff) as i64;
        let pb = ((px >> (bo * 8)) & 0xff) as i64;
        let dr = pr - i64::from(r);
        let dg = pg - i64::from(g);
        let db = pb - i64::from(b);
        let dist_sq = (dr * dr + dg * dg + db * db) as f64;
        dist_sq <= max_sq
    };

    // one pass to build the match mask, then flood-fill only matching pixels, so sparse regions cost proportional to the region.
    let mut mask = vec![false; n];
    let mut any = false;
    for y in 0..h as usize {
        for x in 0..w as usize {
            if matches(x, y) {
                mask[y * w as usize + x] = true;
                any = true;
            }
        }
    }
    if !any {
        return None;
    }

    let mut seen = vec![false; n];
    let mut stack = Vec::with_capacity(1024);
    let mut best: Option<ColorRegion> = None;

    for y0 in 0..h as usize {
        for x0 in 0..w as usize {
            let start = y0 * w as usize + x0;
            if seen[start] || !mask[start] {
                continue;
            }
            let mut count = 0u32;
            let mut sum_x = 0i64;
            let mut sum_y = 0i64;
            let mut min_x = x0 as i32;
            let mut min_y = y0 as i32;
            let mut max_x = x0 as i32;
            let mut max_y = y0 as i32;
            stack.push(start);
            seen[start] = true;
            while let Some(cur) = stack.pop() {
                let cx = cur % w as usize;
                let cy = cur / w as usize;
                count += 1;
                sum_x += cx as i64;
                sum_y += cy as i64;
                min_x = min_x.min(cx as i32);
                min_y = min_y.min(cy as i32);
                max_x = max_x.max(cx as i32);
                max_y = max_y.max(cy as i32);
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = cx as i64 + i64::from(dx);
                        let ny = cy as i64 + i64::from(dy);
                        if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                            continue;
                        }
                        let ni = ny as usize * w as usize + nx as usize;
                        if seen[ni] {
                            continue;
                        }
                        seen[ni] = true;
                        if mask[ni] {
                            stack.push(ni);
                        }
                    }
                }
            }
            let region = ColorRegion {
                area: count,
                cx: sum_x as f64 / f64::from(count),
                cy: sum_y as f64 / f64::from(count),
                x0: min_x,
                y0: min_y,
                x1: max_x,
                y1: max_y,
            };
            if region.width() >= min_width
                && region.height() >= min_height
                && best.as_ref().is_none_or(|b| region.area > b.area)
            {
                best = Some(region);
            }
        }
    }
    best
}
