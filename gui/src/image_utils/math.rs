//! math helpers: 5-smooth padding sizes, downsampling, and window statistics.

use image::GrayImage;
use rayon::prelude::*;

pub(crate) fn next_5smooth(n: usize) -> usize {
    if n <= 1 {
        return 1;
    }
    let mut best = n.next_power_of_two();
    let mut a = 1usize;
    'outer: loop {
        let mut b = a;
        loop {
            let mut c = b;
            loop {
                if c >= n && c < best {
                    best = c;
                }
                match c.checked_mul(5) {
                    Some(v) if v <= best * 2 => c = v,
                    _ => break,
                }
            }
            match b.checked_mul(3) {
                Some(v) if v <= best * 2 => b = v,
                _ => break,
            }
        }
        match a.checked_mul(2) {
            Some(v) if v <= best * 2 => a = v,
            _ => break 'outer,
        }
    }
    best
}

pub(crate) fn compute_stats(img: &GrayImage) -> (f64, f64) {
    let raw = img.as_raw();
    let n = raw.len() as f64;
    const NORM: f64 = 1.0 / 255.0;
    let sum: f64 = raw.iter().map(|&p| p as f64).sum();
    let mean: f64 = (sum * NORM) / n;
    let std = raw
        .iter()
        .map(|&p| {
            let d = p as f64 * NORM - mean;
            d * d
        })
        .sum::<f64>()
        .sqrt();
    (mean, std)
}

pub(crate) fn downsample_box(img: &GrayImage, factor: u32) -> GrayImage {
    let w = img.width();
    let h = img.height();
    let nw = w / factor;
    let nh = h / factor;
    let mut out = GrayImage::new(nw, nh);
    let raw = img.as_raw();
    let out_raw = &mut *out;
    let area = factor * factor;

    out_raw
        .par_chunks_mut(nw as usize)
        .enumerate()
        .for_each(|(y, out_row)| {
            let base_y = (y as u32) * factor;
            for x in 0..nw {
                let base_x = x * factor;
                let mut sum = 0u32;
                for dy in 0..factor {
                    let row_idx = (base_y + dy) * w;
                    for dx in 0..factor {
                        sum += raw[(row_idx + base_x + dx) as usize] as u32;
                    }
                }
                out_row[x as usize] = (sum / area) as u8;
            }
        });
    out
}
