use image::GrayImage;
use rayon::prelude::*;
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::sync::Arc;
use transpose::transpose;

use super::math::next_5smooth;

fn fft_rows_par(data: &mut [Complex<f32>], width: usize, fft: &Arc<dyn Fft<f32>>) {
    let slen = fft.get_inplace_scratch_len();
    const BATCH: usize = 32;
    data.par_chunks_mut(width * BATCH).for_each(|group| {
        let mut scratch = vec![Complex::default(); slen];
        for row in group.chunks_mut(width) {
            fft.process_with_scratch(row, &mut scratch);
        }
    });
}

fn fft_cols_par(data: &mut [Complex<f32>], width: usize, height: usize, fft: &Arc<dyn Fft<f32>>) {
    let mut transposed = vec![Complex::<f32>::default(); width * height];
    transpose(data, &mut transposed, width, height);
    fft_rows_par(&mut transposed, height, fft);
    transpose(&transposed, data, height, width);
}

fn fft2d_par(
    data: &mut [Complex<f32>],
    width: usize,
    height: usize,
    planner: &mut FftPlanner<f32>,
    inverse: bool,
) {
    let fft_w: Arc<dyn Fft<f32>> = if inverse {
        planner.plan_fft_inverse(width)
    } else {
        planner.plan_fft_forward(width)
    };
    let fft_h: Arc<dyn Fft<f32>> = if inverse {
        planner.plan_fft_inverse(height)
    } else {
        planner.plan_fft_forward(height)
    };
    fft_rows_par(data, width, &fft_w);
    fft_cols_par(data, width, height, &fft_h);
}

pub(crate) fn fft_cross_correlate(search: &GrayImage, template: &GrayImage) -> (Vec<f32>, usize, usize) {
    let t_start = std::time::Instant::now();
    let sw = search.width() as usize;
    let sh = search.height() as usize;
    let tw = template.width() as usize;
    let th = template.height() as usize;

    let out_w = sw - tw + 1;
    let out_h = sh - th + 1;

    let fft_w = next_5smooth(sw + tw - 1);
    let fft_h = next_5smooth(sh + th - 1);
    let fft_size = fft_w * fft_h;

    let mut s_buf = vec![Complex::<f32>::default(); fft_size];
    let mut t_buf = vec![Complex::<f32>::default(); fft_size];

    let s_raw = search.as_raw();
    let t_raw = template.as_raw();

    const NORM: f32 = 1.0 / 255.0;
    s_buf.par_chunks_mut(fft_w).enumerate().for_each(|(y, row)| {
        if y < sh {
            for x in 0..sw {
                row[x].re = s_raw[y * sw + x] as f32 * NORM;
            }
        }
    });
    t_buf.par_chunks_mut(fft_w).enumerate().for_each(|(y, row)| {
        if y < th {
            for x in 0..tw {
                row[x].re = t_raw[y * tw + x] as f32 * NORM;
            }
        }
    });
    log::debug!("[TIMING] FFT array allocation & init took: {:?}", t_start.elapsed());

    let t1 = std::time::Instant::now();
    let mut planner = FftPlanner::<f32>::new();
    fft2d_par(&mut s_buf, fft_w, fft_h, &mut planner, false);
    fft2d_par(&mut t_buf, fft_w, fft_h, &mut planner, false);
    log::debug!("[TIMING] FFT forward passes took: {:?}", t1.elapsed());

    let t2 = std::time::Instant::now();
    s_buf.par_iter_mut().zip(t_buf.par_iter()).for_each(|(s, t)| {
        *s = *s * t.conj();
    });
    log::debug!("[TIMING] FFT complex multiplication took: {:?}", t2.elapsed());

    let t3 = std::time::Instant::now();
    fft2d_par(&mut s_buf, fft_w, fft_h, &mut planner, true);
    log::debug!("[TIMING] FFT inverse pass took: {:?}", t3.elapsed());

    let t4 = std::time::Instant::now();
    let scale = 1.0 / fft_size as f32;
    let mut cross = vec![0f32; out_w * out_h];
    cross.par_chunks_mut(out_w).enumerate().for_each(|(y, row)| {
        for x in 0..out_w {
            row[x] = s_buf[y * fft_w + x].re * scale;
        }
    });
    log::debug!("[TIMING] FFT output scaling & extract took: {:?}", t4.elapsed());

    (cross, out_w, out_h)
}
