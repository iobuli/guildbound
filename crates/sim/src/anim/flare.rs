//! Additive "flare" layers on top of the clip pose: hip-leads-shoulder strike twist, lean from
//! acceleration, follow-through overshoot+settle, and speed-scaled run arm swing / torso twist.
//! Pure functions of (tick, inputs) -> per-bone additive quaternions, keyed by skin-joint index of
//! assets/packs/hero/master.glb. Offsets are in MODEL space (Y up, X lateral); apply as
//! `local = parent_world^-1 * offset * parent_world * local` (or pre-multiply in world).

pub const DT: f32 = 1.0 / 120.0;
pub type Quat = [f32; 4]; // x y z w

/// Skin-joint indices in master.glb (order of skins[0].joints).
pub mod bone {
    pub const PELVIS: u16 = 1;
    pub const SPINE: u16 = 12;
    pub const SPINE1: u16 = 13;
    pub const SPINE2: u16 = 14;
    pub const L_UPPERARM: u16 = 16;
    pub const L_FOREARM: u16 = 17;
    pub const R_UPPERARM: u16 = 42;
    pub const R_FOREARM: u16 = 43;
}

pub fn axis_angle(axis: [f32; 3], angle: f32) -> Quat {
    let (s, c) = (0.5 * angle).sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}
pub fn qmul(a: Quat, b: Quat) -> Quat {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}
const YAW: [f32; 3] = [0.0, 1.0, 0.0];
const PITCH: [f32; 3] = [1.0, 0.0, 0.0];

/// Additive pose: (bone index, offset), always sorted by bone index, at most one entry per bone.
pub type Additive = Vec<(u16, Quat)>;

/// Merge layers deterministically (bone order; later layer pre-multiplies).
pub fn compose(layers: &[Additive]) -> Additive {
    let mut out: Additive = Vec::new();
    for l in layers {
        for &(b, q) in l {
            match out.binary_search_by_key(&b, |e| e.0) {
                Ok(i) => out[i].1 = qmul(q, out[i].1),
                Err(i) => out.insert(i, (b, q)),
            }
        }
    }
    out
}

fn smoother(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * x * (x * (6.0 * x - 15.0) + 10.0)
}

/// Strike twist profile: wind back by `windup_frac` of the amplitude, then whip through to +1 at
/// `strike` and ease back to 0 over `recover` ticks. Unit amplitude.
pub fn twist_profile(tick: f32, strike: f32, recover: f32, windup_frac: f32) -> f32 {
    let wind = smoother(tick / (strike * 0.6).max(1.0));
    let whip = smoother((tick - strike * 0.6) / (strike * 0.4).max(1.0));
    let back = smoother((tick - strike) / recover.max(1.0));
    -windup_frac * wind * (1.0 - whip) + whip * (1.0 - back)
}

#[derive(Clone, Copy, Debug)]
pub struct StrikeFlare {
    pub strike_tick: u32,
    pub recover_ticks: u32,
    /// Peak yaw of the hips (rad); shoulders get `shoulder_gain` times this, `lead_ticks` later.
    pub hip_yaw: f32,
    pub shoulder_gain: f32,
    pub lead_ticks: u32,
    pub windup_frac: f32,
    /// +1 for a right-hand strike (turn left), -1 for left.
    pub side: f32,
}

impl Default for StrikeFlare {
    fn default() -> Self {
        Self {
            strike_tick: 24,
            recover_ticks: 30,
            hip_yaw: 0.18,
            shoulder_gain: 1.4,
            lead_ticks: 4,
            windup_frac: 0.35,
            side: 1.0,
        }
    }
}

/// Hip and shoulder yaw at `tick` (hips lead; shoulders lag `lead_ticks`, so torso stretches then snaps).
pub fn strike_yaws(f: &StrikeFlare, tick: u32) -> (f32, f32) {
    let t = tick as f32;
    let hips = f.side
        * f.hip_yaw
        * twist_profile(
            t + f.lead_ticks as f32,
            f.strike_tick as f32,
            f.recover_ticks as f32,
            f.windup_frac,
        );
    let sh = f.side
        * f.hip_yaw
        * f.shoulder_gain
        * twist_profile(
            t,
            f.strike_tick as f32,
            f.recover_ticks as f32,
            f.windup_frac,
        );
    (hips, sh)
}

/// Strike layer: pelvis carries hip yaw, the spine chain shares the extra shoulder yaw.
pub fn strike_layer(f: &StrikeFlare, tick: u32) -> Additive {
    let (h, s) = strike_yaws(f, tick);
    let extra = (s - h) / 3.0;
    vec![
        (bone::PELVIS, axis_angle(YAW, h)),
        (bone::SPINE, axis_angle(YAW, extra)),
        (bone::SPINE1, axis_angle(YAW, extra)),
        (bone::SPINE2, axis_angle(YAW, extra)),
    ]
}

/// Forward lean (rad, + = forward) from horizontal acceleration along facing: atan(a/g)*gain, clamped.
pub fn lean_from_accel(accel: f32, gain: f32, max: f32) -> f32 {
    ((accel / 9.81).atan() * gain).clamp(-max, max)
}
pub fn lean_layer(accel: f32, gain: f32, max: f32) -> Additive {
    let a = lean_from_accel(accel, gain, max);
    vec![
        (bone::PELVIS, axis_angle(PITCH, 0.4 * a)),
        (bone::SPINE1, axis_angle(PITCH, 0.6 * a)),
    ]
}

/// Damped spring parameters from a target overshoot (fraction, 0..1) and a 2% settle time (s).
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub zeta: f32,
    pub omega: f32,
}
impl Spring {
    pub fn from_overshoot(overshoot: f32, settle_s: f32) -> Self {
        let settle_s = settle_s.max(1e-3);
        let overshoot = overshoot.min(0.9);
        let zeta = if overshoot <= 0.0 {
            1.0
        } else {
            let l = overshoot.ln();
            -l / (std::f32::consts::PI * std::f32::consts::PI + l * l).sqrt()
        };
        // 2% settle ~ 4 / (zeta * omega) (underdamped), ~5.8/omega critically damped
        let omega = if zeta < 0.999 {
            4.0 / (zeta * settle_s)
        } else {
            5.8 / settle_s
        };
        Self { zeta, omega }
    }
    /// Unit step response at time t (analytic, so it is a pure function of t).
    pub fn step(&self, t: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        }
        let (z, w) = (self.zeta, self.omega);
        if z >= 0.999 {
            return 1.0 - (1.0 + w * t) * (-w * t).exp();
        }
        let wd = w * (1.0 - z * z).sqrt();
        1.0 - (-z * w * t).exp() * ((wd * t).cos() + z / (1.0 - z * z).sqrt() * (wd * t).sin())
    }
}

/// Follow-through additive angle `ticks` after the strike lands: the bone runs past the clip's end
/// pose by `overshoot` of `swing` and settles. Expressed as (underdamped - critically damped) so it
/// starts and ends at 0 and only adds the overshoot.
pub fn follow_through(ticks: u32, swing: f32, overshoot: f32, settle_s: f32) -> f32 {
    let t = ticks as f32 * DT;
    let u = Spring::from_overshoot(overshoot, settle_s);
    let c = Spring {
        zeta: 1.0,
        omega: u.omega,
    };
    swing * (u.step(t) - c.step(t))
}
pub fn follow_through_layer(
    ticks: u32,
    swing: f32,
    overshoot: f32,
    settle_s: f32,
    side: f32,
) -> Additive {
    let a = follow_through(ticks, swing, overshoot, settle_s) * side;
    let (up, fore) = if side >= 0.0 {
        (bone::R_UPPERARM, bone::R_FOREARM)
    } else {
        (bone::L_UPPERARM, bone::L_FOREARM)
    };
    vec![
        (bone::SPINE2, axis_angle(YAW, 0.5 * a)),
        (up, axis_angle(YAW, 0.35 * a)),
        (fore, axis_angle(YAW, 0.15 * a)),
    ]
}

/// Run swing: torso counter-twist and arm pitch swing, scaled by speed (m/s), driven by gait phase
/// (0..1, 0 = left foot strike). Amplitudes grow linearly with speed up to `max_speed`.
pub fn run_swing(phase: f32, speed: f32, max_speed: f32) -> (f32, f32) {
    let k = (speed / max_speed).clamp(0.0, 1.0);
    let s = (std::f32::consts::TAU * phase).sin();
    let twist = 0.22 * k * s; // shoulders oppose hips
    let arm = (0.25 + 0.65 * k) * k * s;
    (twist, arm)
}
pub fn run_layer(phase: f32, speed: f32, max_speed: f32) -> Additive {
    let (tw, arm) = run_swing(phase, speed, max_speed);
    vec![
        (bone::PELVIS, axis_angle(YAW, -0.4 * tw)),
        (bone::SPINE1, axis_angle(YAW, 0.5 * tw)),
        (bone::SPINE2, axis_angle(YAW, 0.5 * tw)),
        (bone::L_UPPERARM, axis_angle(PITCH, arm)),
        (bone::L_FOREARM, axis_angle(PITCH, 0.4 * arm.max(0.0))),
        (bone::R_UPPERARM, axis_angle(PITCH, -arm)),
        (bone::R_FOREARM, axis_angle(PITCH, 0.4 * (-arm).max(0.0))),
    ]
}
