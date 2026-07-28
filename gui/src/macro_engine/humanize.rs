use core_types::{MacroCommand, MacroEvent, MousePosition, SmartPathOptions};
use std::f32::consts::PI;
use std::time::SystemTime;

const OVERSHOOT_PROBABILITY_MEDIUM: f32 = 0.20;
const OVERSHOOT_PROBABILITY_LONG: f32 = 0.35;
const OVERSHOOT_MAX_DIST: f32 = 12.0;
const OVERSHOOT_DIST_SCALAR: f32 = 0.05;
const SPLIT_PCT_MIN: f32 = 0.80;
const SPLIT_PCT_MAX: f32 = 0.92;
const POLLING_MEAN: f32 = 11.0;
const POLLING_STDDEV: f32 = 1.5;
const NOISE_EMA_ALPHA: f32 = 0.15;
const RECORDED_STYLE: f32 = 0.25;

#[derive(Clone, Copy, PartialEq)]
enum MovementClass {
    Short,
    Medium,
    Long,
}

#[derive(Clone, Copy)]
enum VelocityProfile {
    MinJerk,
    LogNormal,
    TwoStageBallistic,
    Corrective,
}

struct Pcg {
    state: u64,
    inc: u64,
    next_gaussian: Option<f32>,
}

impl Pcg {
    fn new(seed: u64) -> Self {
        let mut pcg = Pcg { state: 0, inc: (seed << 1) | 1, next_gaussian: None };
        pcg.next_u32();
        pcg.state = pcg.state.wrapping_add(seed);
        pcg.next_u32();
        pcg
    }

    fn next_u32(&mut self) -> u32 {
        let oldstate = self.state;
        self.state = oldstate.wrapping_mul(6364136223846793005).wrapping_add(self.inc);
        let xorshifted = (((oldstate >> 18) ^ oldstate) >> 27) as u32;
        let rot = (oldstate >> 59) as u32;
        (xorshifted >> rot) | (xorshifted << ((rot.wrapping_neg()) & 31))
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16777216.0
    }

    fn next_range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    fn next_normal(&mut self, mean: f32, stddev: f32) -> f32 {
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

struct LowFrequencyNoise {
    white_values: [f32; 5],
    counter: u32,
    current_value: f32,
}

impl LowFrequencyNoise {
    fn new(rng: &mut Pcg) -> Self {
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

    fn next_value(&mut self, rng: &mut Pcg) -> f32 {
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

fn min_jerk(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    10.0 * u.powi(3) - 15.0 * u.powi(4) + 6.0 * u.powi(5)
}

fn classify_movement(dist: f32, opts: &SmartPathOptions) -> MovementClass {
    if dist < opts.short_move_threshold {
        MovementClass::Short
    } else if dist < opts.long_move_threshold {
        MovementClass::Medium
    } else {
        MovementClass::Long
    }
}

fn choose_velocity_profile(class: MovementClass, rng: &mut Pcg) -> VelocityProfile {
    let r = rng.next_f32();
    match class {
        MovementClass::Short => VelocityProfile::MinJerk,
        MovementClass::Medium => {
            if r < 0.55 { VelocityProfile::MinJerk }
            else if r < 0.80 { VelocityProfile::LogNormal }
            else if r < 0.95 { VelocityProfile::TwoStageBallistic }
            else { VelocityProfile::Corrective }
        },
        MovementClass::Long => {
            if r < 0.40 { VelocityProfile::MinJerk }
            else if r < 0.65 { VelocityProfile::LogNormal }
            else if r < 0.85 { VelocityProfile::TwoStageBallistic }
            else { VelocityProfile::Corrective }
        }
    }
}

fn apply_profile(u: f32, profile: VelocityProfile, _rng: &mut Pcg) -> f32 {
    match profile {
        VelocityProfile::MinJerk => min_jerk(u),
        VelocityProfile::LogNormal => min_jerk(u.powf(0.75)),
        VelocityProfile::TwoStageBallistic => {
            if u <= 0.85 {
                min_jerk(u / 0.85) * 0.92
            } else {
                0.92 + min_jerk((u - 0.85) / 0.15) * 0.08
            }
        },
        VelocityProfile::Corrective => {
            let hesitate_at = 0.45;
            if u < hesitate_at {
                min_jerk(u / hesitate_at) * hesitate_at * 0.9
            } else {
                hesitate_at * 0.9 + min_jerk((u - hesitate_at)/(1.0 - hesitate_at)) * (1.0 - hesitate_at * 0.9)
            }
        }
    }
}

fn eval_bezier(t: f32, p0: (f32,f32), p1: (f32,f32), p2: (f32,f32), p3: (f32,f32)) -> (f32,f32) {
    let u = 1.0 - t;
    let x = u*u*u*p0.0 + 3.0*u*u*t*p1.0 + 3.0*u*t*t*p2.0 + t*t*t*p3.0;
    let y = u*u*u*p0.1 + 3.0*u*u*t*p1.1 + 3.0*u*t*t*p2.1 + t*t*t*p3.1;
    (x, y)
}

fn build_arc_lut(p0: (f32,f32), p1: (f32,f32), p2: (f32,f32), p3: (f32,f32), curve: f32) -> Vec<(f32, f32)> {
    let steps = if curve < 0.05 { 16 }
    else if curve < 0.10 { 24 }
    else if curve < 0.15 { 32 }
    else { 40 };

    let mut points = Vec::with_capacity(steps + 1);
    let mut total_len = 0.0;
    points.push((0.0, 0.0));

    let mut last_p = p0;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let p = eval_bezier(t, p0, p1, p2, p3);
        let dx = p.0 - last_p.0;
        let dy = p.1 - last_p.1;
        total_len += (dx*dx + dy*dy).sqrt();
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

fn arc_to_t(s: f32, lut: &[(f32, f32)]) -> f32 {
    let s = s.clamp(0.0, 1.0);
    if s <= 0.0 { return 0.0; }
    if s >= 1.0 { return 1.0; }

    let mut idx = 1;
    while idx < lut.len() && lut[idx].0 < s {
        idx += 1;
    }

    let p_prev = lut[idx - 1];
    let p_next = lut[idx];

    let range = p_next.0 - p_prev.0;
    if range == 0.0 { return p_prev.1; }

    let factor = (s - p_prev.0) / range;
    p_prev.1 + factor * (p_next.1 - p_prev.1)
}

fn generate_waypoints(a: (f32,f32), b: (f32,f32), class: MovementClass, rng: &mut Pcg, opts: &SmartPathOptions) -> Vec<(f32,f32)> {
    if !opts.submovement_enabled {
        return vec![a, b];
    }

    let mut n_waypoints = 0;
    match class {
        MovementClass::Short => n_waypoints = 0,
        MovementClass::Medium => {
            if rng.next_f32() < 0.5 { n_waypoints = 1; }
        },
        MovementClass::Long => {
            let r = rng.next_f32();
            if r < 0.35 { n_waypoints = 1; }
            else { n_waypoints = 2; }
        }
    }

    let mut wpts = Vec::new();
    wpts.push(a);

    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let dist = (dx*dx + dy*dy).sqrt();

    if dist > 0.0 && n_waypoints > 0 {
        let ux = dx / dist;
        let uy = dy / dist;
        let perp_x = -uy;
        let perp_y = ux;

        for i in 1..=n_waypoints {
            let base_t = i as f32 / (n_waypoints + 1) as f32;
            let t = (base_t + rng.next_normal(0.0, 0.06)).clamp(0.1, 0.9);

            let base_x = a.0 + t * dx;
            let base_y = a.1 + t * dy;

            let perp_d = rng.next_normal(0.0, dist * 0.04).clamp(-dist * 0.10, dist * 0.10);

            wpts.push((base_x + perp_d * perp_x, base_y + perp_d * perp_y));
        }
    }
    wpts.push(b);
    wpts
}

struct MovementProfile {
    curve: f32,
    tremor: f32,
    endpoint_jitter: f32,
}

fn next_poll_dt(rng: &mut Pcg) -> u64 {
    if rng.next_f32() < 0.05 {
        rng.next_range(17.0, 25.0) as u64 * 1000
    } else {
        rng.next_normal(POLLING_MEAN, POLLING_STDDEV).clamp(7.0, 15.0) as u64 * 1000
    }
}

pub fn humanize_commands(commands: &mut Vec<MacroCommand>, options: &SmartPathOptions) {
    if !options.enabled {
        return;
    }

    let seed = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    let mut rng = Pcg::new(seed);

    let mut new_commands = Vec::with_capacity(commands.len() * 2);
    let mut current_segment = Vec::new();
    let mut prev_end_offset = (0.0, 0.0);
    let mut is_first_path = true;

    let mut current_threshold_us = rng.next_range(10.0, options.segment_delay_threshold_ms as f32) as u64 * 1000;

    for cmd in commands.iter() {
        let is_movement = match cmd {
            MacroCommand::Action(MacroEvent::MouseMove { .. }) => true,
            MacroCommand::Action(MacroEvent::Delay(us)) if *us < current_threshold_us => true,
            _ => false,
        };

        if is_movement {
            current_segment.push(cmd.clone());
        } else {
            if !current_segment.is_empty() {
                let profile = MovementProfile {
                    curve: rng.next_range(-options.path_curve, options.path_curve),
                    tremor: rng.next_range(0.0, options.path_wobble),
                    endpoint_jitter: rng.next_range(0.0, options.endpoint_jitter),
                };
                let processed = process_segment(&current_segment, &mut prev_end_offset, options, &profile, &mut rng, &mut is_first_path);
                new_commands.extend(processed);
                current_segment.clear();
                current_threshold_us = rng.next_range(10.0, options.segment_delay_threshold_ms as f32) as u64 * 1000;
            }

            let mut boundary_cmd = cmd.clone();
            match &mut boundary_cmd {
                MacroCommand::Action(MacroEvent::MouseDown { position: MousePosition::Absolute { x, y }, .. }) |
                MacroCommand::Action(MacroEvent::MouseUp { position: MousePosition::Absolute { x, y }, .. }) |
                MacroCommand::Action(MacroEvent::Click { position: MousePosition::Absolute { x, y }, .. }) => {
                    *x = (*x as f32 + prev_end_offset.0).round() as i32;
                    *y = (*y as f32 + prev_end_offset.1).round() as i32;
                }
                _ => {}
            }
            new_commands.push(boundary_cmd);
        }
    }

    if !current_segment.is_empty() {
        let profile = MovementProfile {
            curve: rng.next_range(-options.path_curve, options.path_curve),
            tremor: rng.next_range(0.0, options.path_wobble),
            endpoint_jitter: rng.next_range(0.0, options.endpoint_jitter),
        };
        let processed = process_segment(&current_segment, &mut prev_end_offset, options, &profile, &mut rng, &mut is_first_path);
        new_commands.extend(processed);
    }

    *commands = new_commands;
}

fn process_segment(
    seg: &[MacroCommand],
    prev_end_offset: &mut (f32, f32),
    options: &SmartPathOptions,
    profile: &MovementProfile,
    rng: &mut Pcg,
    is_first_path: &mut bool
) -> Vec<MacroCommand> {
    let mut noise = LowFrequencyNoise::new(rng);
    let mut pts = Vec::new();
    let mut total_delay_us: u64 = 0;

    for cmd in seg {
        match cmd {
            MacroCommand::Action(MacroEvent::MouseMove { x, y }) => pts.push((*x as f32, *y as f32)),
            MacroCommand::Action(MacroEvent::Delay(us)) => total_delay_us += *us,
            _ => {}
        }
    }

    if pts.is_empty() {
        return seg.to_vec();
    }

    let start_offset = if *is_first_path { (0.0, 0.0) } else { *prev_end_offset };
    *is_first_path = false;

    let a = pts[0];
    let b = *pts.last().unwrap();
    let dx_total = b.0 - a.0;
    let dy_total = b.1 - a.1;
    let total_dist = (dx_total*dx_total + dy_total*dy_total).sqrt();

    if total_dist < 1.0 || total_delay_us < 10_000 {
        *prev_end_offset = start_offset;
        let mut out = Vec::new();
        for cmd in seg {
            let mut c = cmd.clone();
            if let MacroCommand::Action(MacroEvent::MouseMove { x, y }) = &mut c {
                *x = (*x as f32 + start_offset.0).round() as i32;
                *y = (*y as f32 + start_offset.1).round() as i32;
            }
            out.push(c);
        }
        return out;
    }

    let class = classify_movement(total_dist, options);

    let mut recorded_lateral = 0.0;
    if total_dist > 0.0 && pts.len() >= 3 {
        let ux = dx_total / total_dist;
        let uy = dy_total / total_dist;
        let perp_x = -uy;
        let perp_y = ux;

        let recorded_mid = pts[pts.len() / 2];
        let chord_mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        recorded_lateral = (recorded_mid.0 - chord_mid.0) * perp_x + (recorded_mid.1 - chord_mid.1) * perp_y;
    }

    let effective_endpoint_jitter = (total_dist.sqrt() * 0.15).min(profile.endpoint_jitter);
    let r_jitter = effective_endpoint_jitter * rng.next_f32().sqrt();
    let theta_jitter = rng.next_f32() * 2.0 * PI;
    let end_offset = (r_jitter * theta_jitter.cos(), r_jitter * theta_jitter.sin());
    *prev_end_offset = end_offset;

    let overshoot_prob = match class {
        MovementClass::Short => 0.0,
        MovementClass::Medium => OVERSHOOT_PROBABILITY_MEDIUM,
        MovementClass::Long => OVERSHOOT_PROBABILITY_LONG,
    };

    let overshoot_mag = if rng.next_f32() < overshoot_prob {
        rng.next_range(1.0, (total_dist * OVERSHOOT_DIST_SCALAR).min(OVERSHOOT_MAX_DIST))
    } else {
        0.0
    };

    let overshoot_vec = if total_dist > 0.0 {
        let ux = dx_total / total_dist;
        let uy = dy_total / total_dist;
        (ux * overshoot_mag, uy * overshoot_mag)
    } else { (0.0, 0.0) };

    let split_pct = rng.next_range(SPLIT_PCT_MIN, SPLIT_PCT_MAX);

    let waypoints = generate_waypoints(a, b, class, rng, options);

    struct SubMovement {
        p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32),
        lut: Vec<(f32, f32)>,
        delay_share_us: u64,
        profile: VelocityProfile,
    }

    let mut subs = Vec::new();
    let num_subs = waypoints.len() - 1;

    let mut sub_dists = Vec::new();
    let mut sum_sub_dist = 0.0;
    for i in 0..num_subs {
        let wp0 = waypoints[i];
        let wp1 = waypoints[i+1];
        let d = ((wp1.0-wp0.0).powi(2) + (wp1.1-wp0.1).powi(2)).sqrt();
        sub_dists.push(d);
        sum_sub_dist += d;
    }

    let mut delay_left = total_delay_us;
    for i in 0..num_subs {
        let wp0 = waypoints[i];
        let mut wp1 = waypoints[i+1];

        let share = if i == num_subs - 1 || sum_sub_dist == 0.0 {
            delay_left
        } else {
            let frac = sub_dists[i] / sum_sub_dist;
            let mut s = (total_delay_us as f64 * frac as f64) as u64;
            let jitter = rng.next_range(0.9, 1.1);
            s = (s as f32 * jitter) as u64;
            s = s.min(delay_left);
            s
        };
        delay_left -= share;

        if i == num_subs - 1 {
            wp1.0 += overshoot_vec.0;
            wp1.1 += overshoot_vec.1;
        }

        let d = ((wp1.0-wp0.0).powi(2) + (wp1.1-wp0.1).powi(2)).sqrt();
        let (ux, uy) = if d > 0.0 { ((wp1.0-wp0.0)/d, (wp1.1-wp0.1)/d) } else { (0.0, 0.0) };
        let perp = (-uy, ux);

        let alpha1 = rng.next_normal(0.35, 0.07).clamp(0.20, 0.50);
        let beta1_synth = profile.curve * d;

        let recorded_lateral_scaled = if total_dist > 0.0 { recorded_lateral * (d / total_dist) } else { 0.0 };

        let mut beta1 = 0.75 * beta1_synth + RECORDED_STYLE * recorded_lateral_scaled;
        beta1 = beta1.clamp(-profile.curve.abs() * d * 1.5, profile.curve.abs() * d * 1.5);
        let p1 = (wp0.0 + alpha1 * d * ux + beta1 * perp.0, wp0.1 + alpha1 * d * uy + beta1 * perp.1);

        let alpha2 = rng.next_normal(0.65, 0.07).clamp(0.50, 0.80);
        let s_curve_prob = if class == MovementClass::Long { 0.15 } else { 0.30 };
        let s_curve = rng.next_f32() < s_curve_prob;
        let beta2_sign = if s_curve { -beta1.signum() } else { beta1.signum() };
        let beta2_mag = (beta1.abs() * 0.8).clamp(0.0, profile.curve.abs() * d * 1.5);
        let beta2 = beta2_sign * beta2_mag;
        let p2 = (wp0.0 + alpha2 * d * ux + beta2 * perp.0, wp0.1 + alpha2 * d * uy + beta2 * perp.1);

        let lut = build_arc_lut(wp0, p1, p2, wp1, profile.curve.abs());
        let vel_profile = choose_velocity_profile(class, rng);

        subs.push(SubMovement {
            p0: wp0, p1, p2, p3: wp1, lut, delay_share_us: share, profile: vel_profile
        });
    }

    let mut seq = Vec::new();

    let mut last_emitted = (
        (a.0 + start_offset.0).round() as i32,
        (a.1 + start_offset.1).round() as i32
    );
    let mut last_emit_t = 0;

    seq.push(MacroCommand::Action(MacroEvent::MouseMove {
        x: last_emitted.0,
        y: last_emitted.1
    }));

    let mut global_t_us: u64 = 0;
    let mut last_base = a;

    for sub in &subs {
        let mut sub_t_us: u64 = 0;

        let mut hesitate_event: Option<u64> = None;
        if class == MovementClass::Long && rng.next_f32() < 0.25 {
            let h_at = rng.next_range(0.30, 0.65);
            hesitate_event = Some((sub.delay_share_us as f32 * h_at) as u64);
        }

        let wobble_amplitude = profile.tremor;

        while sub_t_us < sub.delay_share_us {
            let mut dt = next_poll_dt(rng);

            if let Some(h_time) = hesitate_event {
                if sub_t_us <= h_time && sub_t_us + dt > h_time {
                    let h_dur = rng.next_range(6.0, 20.0) as u64 * 1000;
                    dt += h_dur;
                    hesitate_event = None;
                }
            }

            let next_t = (sub_t_us + dt).min(sub.delay_share_us);
            let global_next_t = global_t_us + next_t;

            let u = next_t as f32 / sub.delay_share_us as f32;

            let base_pos = if global_next_t as f32 / total_delay_us as f32 <= split_pct {
                let s = apply_profile(u, sub.profile, rng);
                let t_bezier = arc_to_t(s, &sub.lut);
                eval_bezier(t_bezier, sub.p0, sub.p1, sub.p2, sub.p3)
            } else {
                let u_global = global_next_t as f32 / total_delay_us as f32;
                let u_corr = (u_global - split_pct) / (1.0 - split_pct);
                let s_corr = min_jerk(u_corr);

                let tgt = b;
                let sx = tgt.0 + overshoot_vec.0;
                let sy = tgt.1 + overshoot_vec.1;
                (sx + (tgt.0 - sx) * s_corr, sy + (tgt.1 - sy) * s_corr)
            };

            let u_global = global_next_t as f32 / total_delay_us as f32;
            let drift_x = start_offset.0 + (end_offset.0 - start_offset.0) * u_global;
            let drift_y = start_offset.1 + (end_offset.1 - start_offset.1) * u_global;

            let dx = base_pos.0 - last_base.0;
            let dy = base_pos.1 - last_base.1;
            let len = (dx*dx + dy*dy).sqrt();
            let (perp_x, perp_y) = if len > 0.0 { (-dy/len, dx/len) } else { (0.0, 0.0) };

            let wobble_scalar = if u_global <= split_pct {
                let wobble_amp = (PI * (u_global / split_pct)).sin();
                let pn = noise.next_value(rng);
                pn * wobble_amplitude * wobble_amp
            } else { 0.0 };

            let wobble_x = perp_x * wobble_scalar;
            let wobble_y = perp_y * wobble_scalar;

            let final_x = base_pos.0 + drift_x + wobble_x;
            let final_y = base_pos.1 + drift_y + wobble_y;

            let final_x_i32 = final_x.round() as i32;
            let final_y_i32 = final_y.round() as i32;

            let moved = final_x_i32 != last_emitted.0 || final_y_i32 != last_emitted.1;

            if moved {
                let delay_since_last = global_next_t - last_emit_t;
                if delay_since_last > 0 {
                    seq.push(MacroCommand::Action(MacroEvent::Delay(delay_since_last)));
                }
                seq.push(MacroCommand::Action(MacroEvent::MouseMove {
                    x: final_x_i32,
                    y: final_y_i32
                }));
                last_emitted = (final_x_i32, final_y_i32);
                last_emit_t = global_next_t;
            }

            last_base = base_pos;
            sub_t_us = next_t;
        }
        global_t_us += sub.delay_share_us;
    }

    if total_delay_us > last_emit_t {
        let delay_since_last = total_delay_us - last_emit_t;
        seq.push(MacroCommand::Action(MacroEvent::Delay(delay_since_last)));
    }

    let final_target = (
        (b.0 + end_offset.0).round() as i32,
        (b.1 + end_offset.1).round() as i32,
    );
    if last_emitted != final_target {
        seq.push(MacroCommand::Action(MacroEvent::MouseMove {
            x: final_target.0,
            y: final_target.1
        }));
    }

    seq
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
