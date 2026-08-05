use wmacro_core_types::SmartPathOptions;
use super::rng::Pcg;
use super::math::min_jerk;

#[derive(Clone, Copy, PartialEq)]
pub enum MovementClass {
    Short,
    Medium,
    Long,
}

#[derive(Clone, Copy)]
pub enum VelocityProfile {
    MinJerk,
    LogNormal,
    TwoStageBallistic,
    Corrective,
}

pub fn classify_movement(dist: f32, opts: &SmartPathOptions) -> MovementClass {
    if dist < opts.short_move_threshold {
        MovementClass::Short
    } else if dist < opts.long_move_threshold {
        MovementClass::Medium
    } else {
        MovementClass::Long
    }
}

pub fn choose_velocity_profile(class: MovementClass, rng: &mut Pcg) -> VelocityProfile {
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

pub fn apply_profile(u: f32, profile: VelocityProfile, _rng: &mut Pcg) -> f32 {
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

pub fn generate_waypoints(a: (f32,f32), b: (f32,f32), class: MovementClass, rng: &mut Pcg, opts: &SmartPathOptions) -> Vec<(f32,f32)> {
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
