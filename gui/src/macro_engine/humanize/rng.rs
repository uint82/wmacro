//! the `Pcg` random number generator and clock-based seeding used by the humanizer.

use std::f32::consts::PI;

/// seeds a [`Pcg`] from the system clock.
pub fn time_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

pub struct Pcg {
    state: u64,
    inc: u64,
    next_gaussian: Option<f32>,
}

impl Pcg {
    pub fn new(seed: u64) -> Self {
        let mut pcg = Pcg {
            state: 0,
            inc: (seed << 1) | 1,
            next_gaussian: None,
        };
        pcg.next_u32();
        pcg.state = pcg.state.wrapping_add(seed);
        pcg.next_u32();
        pcg
    }

    pub fn next_u32(&mut self) -> u32 {
        let oldstate = self.state;
        self.state = oldstate
            .wrapping_mul(6364136223846793005)
            .wrapping_add(self.inc);
        let xorshifted = (((oldstate >> 18) ^ oldstate) >> 27) as u32;
        let rot = (oldstate >> 59) as u32;
        (xorshifted >> rot) | (xorshifted << ((rot.wrapping_neg()) & 31))
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16777216.0
    }

    pub fn next_range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    /// a uniform integer in `[min, max]` (inclusive); `min` when the range is
    /// empty. the full i64 range is supported.
    pub fn next_range_i64(&mut self, min: i64, max: i64) -> i64 {
        if min >= max {
            return min;
        }
        let span = (max as u64).wrapping_sub(min as u64).wrapping_add(1);
        let draw = if span == 0 {
            u64::from(self.next_u32()) | (u64::from(self.next_u32()) << 32)
        } else {
            u64::from(self.next_u32()) % span
        };
        min.wrapping_add(draw as i64)
    }

    pub fn next_normal(&mut self, mean: f32, stddev: f32) -> f32 {
        if let Some(z1) = self.next_gaussian.take() {
            return mean + z1 * stddev;
        }
        let u1 = self.next_f32().max(f32::EPSILON);
        let u2 = self.next_f32();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * PI * u2;
        let z0 = r * theta.cos();
        let z1 = r * theta.sin();
        self.next_gaussian = Some(z1);
        mean + z0 * stddev
    }
}
