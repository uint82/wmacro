use anyhow::Result;
use image::GrayImage;
use rayon::prelude::*;

use super::fft::fft_cross_correlate;
use super::integral::IntegralImage;
use super::math::{compute_stats, downsample_box};

#[inline(always)]
fn spatial_match_candidates(
    search: &GrayImage,
    target: &GrayImage,
    integral: &IntegralImage,
    threshold: f32,
    t_mean_norm: f64,
    t_std_unnorm: f64,
    top_n: usize,
) -> Vec<(f32, u32, u32)> {
    let sw = search.width() as usize;
    let sh = search.height() as usize;
    let tw = target.width() as usize;
    let th = target.height() as usize;

    let out_w = sw.saturating_sub(tw) + 1;
    let out_h = sh.saturating_sub(th) + 1;

    if out_w == 0 || out_h == 0 {
        return vec![];
    }

    let s_raw = search.as_raw();
    let t_raw = target.as_raw();
    let n = (tw * th) as f64;
    const NORM: f64 = 1.0 / 255.0;
    const NORM2: f64 = NORM * NORM;

    let mut all_cands: Vec<(f32, u32, u32)> = (0..out_h)
        .into_par_iter()
        .map_init(
            || vec![0u32; out_w],
            |dot_sums, y| {
                dot_sums.fill(0);

                for ty in 0..th {
                    let t_row = &t_raw[ty * tw .. ty * tw + tw];
                    let s_row = &s_raw[(y + ty) * sw .. (y + ty) * sw + sw];

                    for tx in 0..tw {
                        let t_val = t_row[tx] as u32;
                        if t_val == 0 { continue; }

                        let s_slice = &s_row[tx .. tx + out_w];
                        let d_slice = &mut dot_sums[..out_w];
                        for (d, &s) in d_slice.iter_mut().zip(s_slice.iter()) {
                            *d += (s as u32) * t_val;
                        }
                    }
                }

                let mut row_cands = Vec::new();
                for x in 0..out_w {
                    let (s_sum, s_sq_sum) = integral.query(x, y, tw, th);
                    let s_sum_n = s_sum as f64 * NORM;
                    let s_sq_n = s_sq_sum as f64 * NORM2;
                    let s_mean_n = s_sum_n / n;
                    let s_var = (s_sq_n - s_sum_n * s_mean_n).max(0.0);
                    let denom = s_var.sqrt() * t_std_unnorm;

                    if denom >= 1e-7 {
                        let dot_sum_f64 = dot_sums[x] as f64 * NORM2;
                        let num = dot_sum_f64 - t_mean_norm * s_sum_n;
                        let zncc = (num / denom).clamp(-1.0, 1.0) as f32;

                        if zncc >= threshold {
                            row_cands.push((zncc, x as u32, y as u32));
                        }
                    }
                }
                row_cands
            },
        )
        .flatten()
        .collect();

    all_cands.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    let mut nms = Vec::new();
    let nms_radius = (tw.min(th) / 2).max(1) as u32;

    for cand in all_cands {
        let mut overlap = false;
        for existing in &nms {
            let &(_, ex, ey) = existing;
            if cand.1.abs_diff(ex) < nms_radius && cand.2.abs_diff(ey) < nms_radius {
                overlap = true;
                break;
            }
        }
        if !overlap {
            nms.push(cand);
            if nms.len() >= top_n { break; }
        }
    }
    nms
}

fn find_candidates(
    cross: &[f32],
    integral: &IntegralImage,
    out_w: usize,
    out_h: usize,
    tw: usize,
    th: usize,
    t_mean_norm: f64,
    t_std_unnorm: f64,
    threshold: f32,
    top_n: usize,
) -> Vec<(f32, u32, u32)> {
    let n = (tw * th) as f64;
    const NORM: f64 = 1.0 / 255.0;
    const NORM2: f64 = NORM * NORM;

    let mut candidates: Vec<(f32, u32, u32)> = (0..out_h)
        .into_par_iter()
        .flat_map(|y| {
            let mut row_cands = Vec::new();
            for x in 0..out_w {
                let (s_sum, s_sq_sum) = integral.query(x, y, tw, th);
                let s_sum_n = s_sum as f64 * NORM;
                let s_sq_n = s_sq_sum as f64 * NORM2;
                let s_mean_n = s_sum_n / n;
                let num = (cross[y * out_w + x] as f64) - t_mean_norm * s_sum_n;
                let s_var = (s_sq_n - s_sum_n * s_mean_n).max(0.0);
                let denom = s_var.sqrt() * t_std_unnorm;
                let zncc = if denom < 1e-7 {
                    0.0
                } else {
                    (num / denom).clamp(-1.0, 1.0) as f32
                };
                if zncc >= threshold {
                    row_cands.push((zncc, x as u32, y as u32));
                }
            }
            row_cands
        })
        .collect();

    candidates.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    let mut nms = Vec::new();
    let nms_radius = (tw.min(th) / 2).max(1) as u32;
    for cand in candidates {
        let mut overlap = false;
        for existing in &nms {
            let &(_, ex, ey) = existing;
            if cand.1.abs_diff(ex) < nms_radius && cand.2.abs_diff(ey) < nms_radius {
                overlap = true;
                break;
            }
        }
        if !overlap {
            nms.push(cand);
            if nms.len() >= top_n { break; }
        }
    }
    nms
}

pub(crate) fn match_in_memory(
    search: &GrayImage,
    target: &GrayImage,
    offset_x: u32,
    offset_y: u32,
    threshold: f32,
) -> Result<Option<(u32, u32)>> {
    let threshold = threshold.min(0.999);
    let t_total = std::time::Instant::now();
    if target.width() > search.width() || target.height() > search.height() {
        return Ok(None);
    }

    let (t_mean_norm, t_std_unnorm) = compute_stats(target);
    let tw = target.width();
    let th = target.height();
    let sw = search.width();
    let sh = search.height();

    if t_std_unnorm < 1e-7 {
        let integral = IntegralImage::build(search);
        let t_mean_255 = (t_mean_norm * 255.0).round() as i64;
        let n = (tw * th) as f64;
        for y in 0..=(sh - th) {
            for x in 0..=(sw - tw) {
                let (s, _) = integral.query(x as usize, y as usize, tw as usize, th as usize);
                if (s / n as i64) == t_mean_255 {
                    return Ok(Some((x + offset_x, y + offset_y)));
                }
            }
        }
        return Ok(None);
    }

    // allow much smaller templates to benefit from pyramids (12x12)
    let factor = if tw >= 32 && th >= 32 {
        4
    } else if tw >= 12 && th >= 12 {
        2
    } else {
        1
    };

    log::debug!("[TIMING] Using Pyramid matching with factor {}", factor);

    let s_coarse = if factor > 1 { downsample_box(search, factor) } else { search.clone() };
    let t_coarse = if factor > 1 { downsample_box(target, factor) } else { target.clone() };

    let t1 = std::time::Instant::now();
    let integral_coarse = IntegralImage::build(&s_coarse);
    log::debug!("[TIMING] IntegralImage::build (coarse) took: {:?}", t1.elapsed());

    let coarse_thresh = if factor > 1 {
        (threshold - 0.20).clamp(0.4, 0.90)
    } else {
        threshold
    };
    let (tc_mean_norm, tc_std_unnorm) = compute_stats(&t_coarse);
    let coarse_area = t_coarse.width() * t_coarse.height();

    let t2 = std::time::Instant::now();
    let cands = if coarse_area <= 1500 {
        // simd spatial is significantly faster than fft for small coarse regions
        spatial_match_candidates(
            &s_coarse, &t_coarse, &integral_coarse, coarse_thresh, tc_mean_norm, tc_std_unnorm, 10
        )
    } else {
        // fft scales better for massive coarse regions
        let (cross, out_w, out_h) = fft_cross_correlate(&s_coarse, &t_coarse);
        find_candidates(
            &cross, &integral_coarse, out_w, out_h, t_coarse.width() as usize, t_coarse.height() as usize, tc_mean_norm, tc_std_unnorm, coarse_thresh, 10
        )
    };
    log::debug!("[TIMING] Coarse search pass took: {:?}", t2.elapsed());

    if factor == 1 {
        log::debug!("[TIMING] Total match_in_memory took: {:?}", t_total.elapsed());
        return Ok(cands.first().map(|&(_, x, y)| (x + offset_x, y + offset_y)));
    }

    let t3 = std::time::Instant::now();
    let radius = factor * 2;
    let mut best_overall: Option<(f32, u32, u32)> = None;

    for (_coarse_score, cx, cy) in cands {
        let fine_x = cx * factor;
        let fine_y = cy * factor;

        let min_x = fine_x.saturating_sub(radius);
        let min_y = fine_y.saturating_sub(radius);
        let max_x = (fine_x + radius + tw).min(sw);
        let max_y = (fine_y + radius + th).min(sh);
        let crop_w = max_x - min_x;
        let crop_h = max_y - min_y;

        if crop_w < tw || crop_h < th {
            continue;
        }

        let crop = image::imageops::crop_imm(search, min_x, min_y, crop_w, crop_h).to_image();

        // use fast spatial search instead of slow fft for the fine-refine crop
        let integral_crop = IntegralImage::build(&crop);
        let crop_cands = spatial_match_candidates(
            &crop, target, &integral_crop, threshold, t_mean_norm, t_std_unnorm, 1
        );

        if let Some(&(score, lx, ly)) = crop_cands.first() {
            let abs_x = lx + min_x;
            let abs_y = ly + min_y;
            if best_overall.map_or(true, |(best_score, _, _)| score > best_score) {
                best_overall = Some((score, abs_x, abs_y));
            }
        }
    }

    log::debug!("[TIMING] Fine spatial refine took: {:?}", t3.elapsed());
    log::debug!("[TIMING] Total match_in_memory took: {:?}", t_total.elapsed());

    Ok(best_overall.map(|(_, x, y)| (x + offset_x, y + offset_y)))
}
