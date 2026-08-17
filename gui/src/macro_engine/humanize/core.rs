use std::f32::consts::PI;
use wmacro_core_types::{Coord, MacroCommand, MacroEvent, MousePosition, SmartPathOptions};

use super::constants::*;
use super::math::*;
use super::movement::*;
use super::noise::LowFrequencyNoise;
use super::rng::{Pcg, time_seed};

pub struct MovementProfile {
    pub curve: f32,
    pub tremor: f32,
    pub endpoint_jitter: f32,
}

pub fn next_poll_dt(rng: &mut Pcg) -> u64 {
    if rng.next_f32() < 0.05 {
        rng.next_range(17.0, 25.0) as u64 * 1000
    } else {
        rng.next_normal(POLLING_MEAN, POLLING_STDDEV)
            .clamp(7.0, 15.0) as u64
            * 1000
    }
}

pub fn humanize_commands(commands: &mut Vec<MacroCommand>, options: &SmartPathOptions) {
    if !options.enabled {
        return;
    }

    let mut rng = Pcg::new(time_seed());

    let mut new_commands = Vec::with_capacity(commands.len() * 2);
    let mut current_segment = Vec::new();
    let mut prev_end_offset = (0.0, 0.0);
    let mut is_first_path = true;

    let mut current_threshold_us =
        rng.next_range(10.0, options.segment_delay_threshold_ms as f32) as u64 * 1000;

    for cmd in commands.iter() {
        // variable-driven moves are resolved at dispatch time and cannot be
        // precomputed here, so they pass through unchanged.
        let is_movement = match cmd {
            MacroCommand::Action(MacroEvent::MouseMove {
                x: Coord::Const(_),
                y: Coord::Const(_),
            }) => true,
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
                let processed = process_segment(
                    &current_segment,
                    &mut prev_end_offset,
                    options,
                    &profile,
                    &mut rng,
                    &mut is_first_path,
                );
                new_commands.extend(processed);
                current_segment.clear();
                current_threshold_us =
                    rng.next_range(10.0, options.segment_delay_threshold_ms as f32) as u64 * 1000;
            }

            let mut boundary_cmd = cmd.clone();
            match &mut boundary_cmd {
                MacroCommand::Action(MacroEvent::MouseDown {
                    position:
                        MousePosition::Absolute {
                            x: Coord::Const(x),
                            y: Coord::Const(y),
                        },
                    ..
                })
                | MacroCommand::Action(MacroEvent::MouseUp {
                    position:
                        MousePosition::Absolute {
                            x: Coord::Const(x),
                            y: Coord::Const(y),
                        },
                    ..
                })
                | MacroCommand::Action(MacroEvent::Click {
                    position:
                        MousePosition::Absolute {
                            x: Coord::Const(x),
                            y: Coord::Const(y),
                        },
                    ..
                }) => {
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
        let processed = process_segment(
            &current_segment,
            &mut prev_end_offset,
            options,
            &profile,
            &mut rng,
            &mut is_first_path,
        );
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
    is_first_path: &mut bool,
) -> Vec<MacroCommand> {
    let mut noise = LowFrequencyNoise::new(rng);
    let mut pts = Vec::new();
    let mut total_delay_us: u64 = 0;

    for cmd in seg {
        match cmd {
            MacroCommand::Action(MacroEvent::MouseMove {
                x: Coord::Const(x),
                y: Coord::Const(y),
            }) => pts.push((*x as f32, *y as f32)),
            MacroCommand::Action(MacroEvent::Delay(us)) => total_delay_us += *us,
            _ => {}
        }
    }

    if pts.is_empty() {
        return seg.to_vec();
    }

    let start_offset = if *is_first_path {
        (0.0, 0.0)
    } else {
        *prev_end_offset
    };
    *is_first_path = false;

    let a = pts[0];
    let b = *pts.last().unwrap();
    let dx_total = b.0 - a.0;
    let dy_total = b.1 - a.1;
    let total_dist = (dx_total * dx_total + dy_total * dy_total).sqrt();

    if total_dist < 1.0 || total_delay_us < 10_000 {
        *prev_end_offset = start_offset;
        let mut out = Vec::new();
        for cmd in seg {
            let mut c = cmd.clone();
            if let MacroCommand::Action(MacroEvent::MouseMove {
                x: Coord::Const(x),
                y: Coord::Const(y),
            }) = &mut c
            {
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
        recorded_lateral =
            (recorded_mid.0 - chord_mid.0) * perp_x + (recorded_mid.1 - chord_mid.1) * perp_y;
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
        rng.next_range(
            1.0,
            (total_dist * OVERSHOOT_DIST_SCALAR).min(OVERSHOOT_MAX_DIST),
        )
    } else {
        0.0
    };

    let overshoot_vec = if total_dist > 0.0 {
        let ux = dx_total / total_dist;
        let uy = dy_total / total_dist;
        (ux * overshoot_mag, uy * overshoot_mag)
    } else {
        (0.0, 0.0)
    };

    let split_pct = rng.next_range(SPLIT_PCT_MIN, SPLIT_PCT_MAX);

    let waypoints = generate_waypoints(a, b, class, rng, options);

    struct SubMovement {
        p0: (f32, f32),
        p1: (f32, f32),
        p2: (f32, f32),
        p3: (f32, f32),
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
        let wp1 = waypoints[i + 1];
        let d = ((wp1.0 - wp0.0).powi(2) + (wp1.1 - wp0.1).powi(2)).sqrt();
        sub_dists.push(d);
        sum_sub_dist += d;
    }

    let mut delay_left = total_delay_us;
    for i in 0..num_subs {
        let wp0 = waypoints[i];
        let mut wp1 = waypoints[i + 1];

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

        let d = ((wp1.0 - wp0.0).powi(2) + (wp1.1 - wp0.1).powi(2)).sqrt();
        let (ux, uy) = if d > 0.0 {
            ((wp1.0 - wp0.0) / d, (wp1.1 - wp0.1) / d)
        } else {
            (0.0, 0.0)
        };
        let perp = (-uy, ux);

        let alpha1 = rng.next_normal(0.35, 0.07).clamp(0.20, 0.50);
        let beta1_synth = profile.curve * d;

        let recorded_lateral_scaled = if total_dist > 0.0 {
            recorded_lateral * (d / total_dist)
        } else {
            0.0
        };

        let mut beta1 = 0.75 * beta1_synth + RECORDED_STYLE * recorded_lateral_scaled;
        beta1 = beta1.clamp(
            -profile.curve.abs() * d * 1.5,
            profile.curve.abs() * d * 1.5,
        );
        let p1 = (
            wp0.0 + alpha1 * d * ux + beta1 * perp.0,
            wp0.1 + alpha1 * d * uy + beta1 * perp.1,
        );

        let alpha2 = rng.next_normal(0.65, 0.07).clamp(0.50, 0.80);
        let s_curve_prob = if class == MovementClass::Long {
            0.15
        } else {
            0.30
        };
        let s_curve = rng.next_f32() < s_curve_prob;
        let beta2_sign = if s_curve {
            -beta1.signum()
        } else {
            beta1.signum()
        };
        let beta2_mag = (beta1.abs() * 0.8).clamp(0.0, profile.curve.abs() * d * 1.5);
        let beta2 = beta2_sign * beta2_mag;
        let p2 = (
            wp0.0 + alpha2 * d * ux + beta2 * perp.0,
            wp0.1 + alpha2 * d * uy + beta2 * perp.1,
        );

        let lut = build_arc_lut(wp0, p1, p2, wp1, profile.curve.abs());
        let vel_profile = choose_velocity_profile(class, rng);

        subs.push(SubMovement {
            p0: wp0,
            p1,
            p2,
            p3: wp1,
            lut,
            delay_share_us: share,
            profile: vel_profile,
        });
    }

    let mut seq = Vec::new();

    let mut last_emitted = (
        (a.0 + start_offset.0).round() as i32,
        (a.1 + start_offset.1).round() as i32,
    );
    let mut last_emit_t = 0;

    seq.push(MacroCommand::Action(MacroEvent::MouseMove {
        x: Coord::Const(last_emitted.0),
        y: Coord::Const(last_emitted.1),
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

            if let Some(h_time) = hesitate_event
                && sub_t_us <= h_time
                && sub_t_us + dt > h_time
            {
                let h_dur = rng.next_range(6.0, 20.0) as u64 * 1000;
                dt += h_dur;
                hesitate_event = None;
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
            let len = (dx * dx + dy * dy).sqrt();
            let (perp_x, perp_y) = if len > 0.0 {
                (-dy / len, dx / len)
            } else {
                (0.0, 0.0)
            };

            let wobble_scalar = if u_global <= split_pct {
                let wobble_amp = (PI * (u_global / split_pct)).sin();
                let pn = noise.next_value(rng);
                pn * wobble_amplitude * wobble_amp
            } else {
                0.0
            };

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
                    x: Coord::Const(final_x_i32),
                    y: Coord::Const(final_y_i32),
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
            x: Coord::Const(final_target.0),
            y: Coord::Const(final_target.1),
        }));
    }

    seq
}
