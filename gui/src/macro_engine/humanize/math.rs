//! movement math: min-jerk easing, cubic bezier sampling and arc lookup tables.

pub fn min_jerk(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    10.0 * u.powi(3) - 15.0 * u.powi(4) + 6.0 * u.powi(5)
}

pub fn eval_bezier(
    t: f32,
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
) -> (f32, f32) {
    let u = 1.0 - t;
    let x = u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0;
    let y = u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1;
    (x, y)
}

pub fn build_arc_lut(
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
    curve: f32,
) -> Vec<(f32, f32)> {
    let steps = if curve < 0.05 {
        16
    } else if curve < 0.10 {
        24
    } else if curve < 0.15 {
        32
    } else {
        40
    };

    let mut points = Vec::with_capacity(steps + 1);
    let mut total_len = 0.0;
    points.push((0.0, 0.0));

    let mut last_p = p0;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let p = eval_bezier(t, p0, p1, p2, p3);
        let dx = p.0 - last_p.0;
        let dy = p.1 - last_p.1;
        total_len += (dx * dx + dy * dy).sqrt();
        points.push((total_len, t));
        last_p = p;
    }

    if total_len == 0.0 {
        return vec![(0.0, 0.0), (1.0, 1.0)];
    }

    for pt in &mut points {
        pt.0 /= total_len;
    }
    points
}

pub fn arc_to_t(s: f32, lut: &[(f32, f32)]) -> f32 {
    let s = s.clamp(0.0, 1.0);
    if s <= 0.0 {
        return 0.0;
    }
    if s >= 1.0 {
        return 1.0;
    }

    let mut idx = 1;
    while idx < lut.len() && lut[idx].0 < s {
        idx += 1;
    }

    let p_prev = lut[idx - 1];
    let p_next = lut[idx];

    let range = p_next.0 - p_prev.0;
    if range == 0.0 {
        return p_prev.1;
    }

    let factor = (s - p_prev.0) / range;
    p_prev.1 + factor * (p_next.1 - p_prev.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_jerk() {
        assert_eq!(min_jerk(0.0), 0.0);
        assert_eq!(min_jerk(1.0), 1.0);
        assert!(min_jerk(0.5) > 0.49 && min_jerk(0.5) < 0.51);
    }

    #[test]
    fn test_lut() {
        let p0 = (0.0, 0.0);
        let p1 = (100.0, 0.0);
        let p2 = (200.0, 0.0);
        let p3 = (300.0, 0.0);
        let lut = build_arc_lut(p0, p1, p2, p3, 0.0);
        assert_eq!(lut.first().unwrap(), &(0.0, 0.0));
        assert_eq!(lut.last().unwrap(), &(1.0, 1.0));

        let t_mid = arc_to_t(0.5, &lut);
        assert!(t_mid > 0.49 && t_mid < 0.51);
    }
}
