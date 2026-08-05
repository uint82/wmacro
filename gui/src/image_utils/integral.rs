use image::GrayImage;
use rayon::prelude::*;

pub(crate) struct IntegralImage {
    pub(crate) stride: usize,
    pub(crate) sum: Vec<i64>,
    pub(crate) sum_sq: Vec<i64>,
}

impl IntegralImage {
    pub(crate) fn build(img: &GrayImage) -> Self {
        let w = img.width() as usize;
        let h = img.height() as usize;
        let stride = w + 1;
        let mut sum = vec![0i64; (h + 1) * stride];
        let mut sum_sq = vec![0i64; (h + 1) * stride];
        let raw = img.as_raw();

        // row-wise prefix sum (parallel across rows)
        sum.par_chunks_mut(stride)
            .zip(sum_sq.par_chunks_mut(stride))
            .enumerate()
            .for_each(|(y, (s_row, sq_row))| {
                if y > 0 && y <= h {
                    let raw_row = &raw[(y - 1) * w..y * w];
                    let mut run_s = 0i64;
                    let mut run_sq = 0i64;
                    for x in 0..w {
                        let p = raw_row[x] as i64;
                        run_s += p;
                        run_sq += p * p;
                        s_row[x + 1] = run_s;
                        sq_row[x + 1] = run_sq;
                    }
                }
            });

        // column-wise accumulation (cache-friendly linear pass)
        for y in 1..=h {
            let (prev_s, curr_s) = sum.split_at_mut(y * stride);
            let (prev_sq, curr_sq) = sum_sq.split_at_mut(y * stride);

            let prev_s_row = &prev_s[(y - 1) * stride..];
            let curr_s_row = &mut curr_s[0..stride];

            let prev_sq_row = &prev_sq[(y - 1) * stride..];
            let curr_sq_row = &mut curr_sq[0..stride];

            for x in 1..=w {
                curr_s_row[x] += prev_s_row[x];
                curr_sq_row[x] += prev_sq_row[x];
            }
        }

        IntegralImage {
            stride,
            sum,
            sum_sq,
        }
    }

    #[inline(always)]
    pub(crate) fn query(&self, x: usize, y: usize, w: usize, h: usize) -> (i64, i64) {
        let s = self.stride;
        let tl = y * s + x;
        let tr = y * s + (x + w);
        let bl = (y + h) * s + x;
        let br = (y + h) * s + (x + w);
        (
            self.sum[br] - self.sum[tr] - self.sum[bl] + self.sum[tl],
            self.sum_sq[br] - self.sum_sq[tr] - self.sum_sq[bl] + self.sum_sq[tl],
        )
    }
}
