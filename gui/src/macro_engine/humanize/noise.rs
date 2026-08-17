//! low-frequency noise source that makes humanized pointer paths feel organic.

use super::constants::NOISE_EMA_ALPHA;
use super::rng::Pcg;

pub struct LowFrequencyNoise {
    white_values: [f32; 5],
    counter: u32,
    current_value: f32,
}

impl LowFrequencyNoise {
    pub fn new(rng: &mut Pcg) -> Self {
        let mut n = LowFrequencyNoise {
            white_values: [0.0; 5],
            counter: 0,
            current_value: 0.0,
        };
        for i in 0..5 {
            n.white_values[i] = rng.next_range(-1.0, 1.0);
        }
        n
    }

    pub fn next_value(&mut self, rng: &mut Pcg) -> f32 {
        self.counter = self.counter.wrapping_add(1);
        let trailing_zeros = self.counter.trailing_zeros();
        let idx = (trailing_zeros as usize).min(4);
        self.white_values[idx] = rng.next_range(-1.0, 1.0);

        let sum: f32 = self.white_values.iter().sum();
        let target = sum / 5.0;
        self.current_value += (target - self.current_value) * NOISE_EMA_ALPHA;
        self.current_value
    }
}
