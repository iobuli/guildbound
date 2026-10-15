//! Procedural keyframe generators: run, jump, air recovery, spin footwork (no clips, no ML).
//! Each generator is a pure function of its inputs; targets come from sampling the generator one tick
//! either side (DT = 1/120) and `target::target_from`, so ω/α are full-angle Log derivatives
//! in the PARENT BONE frame (map to world with `target::rates_to_world_rel` and B_p = R_p·O_p).
//!
//! Conventions (shared with `acro`): pelvis frame, y up, z forward, x = LEFT (left hip at +x, a guess
//! until the `[anim_rig]` log confirms). Hinge angles about X with + = flexion, `acro::leg_ik` for legs.
//! Abduction is about Z, thigh twist about Y. Bone indices/names: `PROC_BONES` (master.glb guesses).

use super::DT;
use super::acro::{ANKLE_H, SHIN, STAND_Y, THIGH, leg_ik};
use super::target::{BoneTarget, target_from};
use crate::math::{Quat, Vec3};
use std::f64::consts::PI;

/// Bones driven by `proc`, as (skin-joint index, name). Single source of truth for this module;
/// indices match `acro::bone` / `flare::bone`, names match `trail::DEFAULT_BONES` style.
pub const PROC_BONES: [(u16, &str); 15] = [
    (1, "B_Pelvis"),
    (2, "B_L_Thigh"),
    (3, "B_L_Calf"),
    (4, "B_L_Foot"),
    (7, "B_R_Thigh"),
    (8, "B_R_Calf"),
    (9, "B_R_Foot"),
    (12, "B_Spine"),
    (13, "B_Spine1"),
    (14, "B_Spine2"),
    (16, "B_L_UpperArm"),
    (17, "B_L_Forearm"),
    (42, "B_R_UpperArm"),
    (43, "B_R_Forearm"),
    (39, "B_Neck"),
];
const PELVIS: usize = 0;
const THIGH_I: [usize; 2] = [1, 4];
const CALF_I: [usize; 2] = [2, 5];
const FOOT_I: [usize; 2] = [3, 6];
const SPINE_I: [usize; 3] = [7, 8, 9];
const UARM_I: [usize; 2] = [10, 12];
const FARM_I: [usize; 2] = [11, 13];
const NECK_I: usize = 14;

/// Hip half-width (m): matches the rig foot box at x = ±0.1.
pub const HIP_W: f64 = 0.10;
pub(super) const REACH: f64 = (THIGH + SHIN) * 0.98;
const G: f64 = 9.81;

pub(super) fn qx(a: f64) -> Quat {
    Quat::from_rotvec(Vec3::new(a, 0.0, 0.0))
}
pub(super) fn qy(a: f64) -> Quat {
    Quat::from_rotvec(Vec3::new(0.0, a, 0.0))
}
pub(super) fn qz(a: f64) -> Quat {
    Quat::from_rotvec(Vec3::new(0.0, 0.0, a))
}
pub(super) fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
pub(super) fn mix(a: f64, b: f64, s: f64) -> f64 {
    a + (b - a) * s
}
pub(super) fn herm(p0: f64, m0: f64, p1: f64, m1: f64, d: f64, t: f64) -> f64 {
    let s = (t / d).clamp(0.0, 1.0);
    let (s2, s3) = (s * s, s * s * s);
    (2.0 * s3 - 3.0 * s2 + 1.0) * p0
        + (s3 - 2.0 * s2 + s) * d * m0
        + (-2.0 * s3 + 3.0 * s2) * p1
        + (s3 - s2) * d * m1
}
pub(super) fn side_x(i: usize) -> f64 {
    if i == 0 { HIP_W } else { -HIP_W }
}

/// Joint-angle pose (rad). Index 0 = left, 1 = right.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct P {
    pub(super) pitch: f64,
    pub(super) roll: f64,
    pub(super) spine: f64,
    pub(super) neck: f64,
    pub(super) hip: [f64; 2],
    pub(super) knee: [f64; 2],
    pub(super) abd: [f64; 2],
    pub(super) twist: [f64; 2],
    pub(super) sh: [f64; 2],
    pub(super) sh_abd: [f64; 2],
    pub(super) el: [f64; 2],
    /// Pelvis yaw and extra spine yaw (about Y), used by `gait` turning.
    pub(super) yaw: f64,
    pub(super) spine_yaw: f64,
}
impl P {
    pub(super) fn lerp(a: &P, b: &P, s: f64) -> P {
        let m2 = |x: [f64; 2], y: [f64; 2]| [mix(x[0], y[0], s), mix(x[1], y[1], s)];
        P {
            pitch: mix(a.pitch, b.pitch, s),
            roll: mix(a.roll, b.roll, s),
            spine: mix(a.spine, b.spine, s),
            neck: mix(a.neck, b.neck, s),
            yaw: mix(a.yaw, b.yaw, s),
            spine_yaw: mix(a.spine_yaw, b.spine_yaw, s),
            hip: m2(a.hip, b.hip),
            knee: m2(a.knee, b.knee),
            abd: m2(a.abd, b.abd),
            twist: m2(a.twist, b.twist),
            sh: m2(a.sh, b.sh),
            sh_abd: m2(a.sh_abd, b.sh_abd),
            el: m2(a.el, b.el),
        }
    }
    /// Solve leg `i` so its ankle reaches `ank` (pelvis frame, pelvis origin = mid-hip).
    pub(super) fn leg(&mut self, i: usize, ank: Vec3, twist: f64) {
        let d = qy(-twist).rotate(ank - Vec3::new(side_x(i), 0.0, 0.0));
        let vert = (d.x * d.x + d.y * d.y).sqrt();
        let (h, k) = leg_ik([0.0, 0.0], [d.z, -vert], self.pitch);
        self.hip[i] = h;
        self.knee[i] = k;
        self.twist[i] = twist;
        self.abd[i] = d.x.atan2(-d.y) - self.roll;
    }
    fn quats(&self) -> [Quat; 15] {
        let mut q = [Quat::IDENTITY; 15];
        q[PELVIS] = (qy(self.yaw) * qz(self.roll) * qx(self.pitch)).normalize();
        for i in 0..2 {
            let sgn = if i == 0 { 1.0 } else { -1.0 };
            q[THIGH_I[i]] = (qy(self.twist[i]) * qz(self.abd[i]) * qx(self.hip[i])).normalize();
            q[CALF_I[i]] = qx(-self.knee[i]);
            q[FOOT_I[i]] = qx(self.knee[i] - self.hip[i]);
            q[UARM_I[i]] = (qz(sgn * self.sh_abd[i]) * qx(self.sh[i])).normalize();
            q[FARM_I[i]] = qx(self.el[i]);
        }
        for &s in &SPINE_I {
            q[s] = (qy(self.spine_yaw / 3.0) * qx(self.spine / 3.0)).normalize();
        }
        q[NECK_I] = qx(self.neck);
        q
    }
}

/// Targets at the centre sample from poses one tick before / at / after.
pub(super) fn build(m: &P, c: &P, p: &P) -> Vec<(u16, BoneTarget)> {
    let (qm, q0, qp) = (m.quats(), c.quats(), p.quats());
    (0..PROC_BONES.len())
        .map(|i| {
            (
                PROC_BONES[i].0,
                target_from(Some(qm[i]), q0[i], Some(qp[i])),
            )
        })
        .collect()
}

/// Clamp an ankle target to leg reach from its hip.
pub(super) fn reachable(i: usize, ank: Vec3) -> Vec3 {
    let hip = Vec3::new(side_x(i), 0.0, 0.0);
    hip + (ank - hip).clamp_length(REACH)
}

// ---------------------------------------------------------------- idle / depth

/// Scales gait knee/hip flexion amplitude via stride length, swing lift, pelvis drop and bob in
/// `run_pose` and `gait` (jump absorb is NOT scaled). 0.5 while the Avian motors only hold at low gains
/// (Kp 2000 / Kd 40); raise to 1.0 once the gains hold.
pub const GAIT_DEPTH: f64 = 0.5;
/// Soft idle stance: hip and knee flexion (rad), trunk upright, ankles under the hips.
pub const IDLE_HIP: f64 = 0.08;
pub const IDLE_KNEE: f64 = 0.15;

/// Soft idle joint pose (feet under hips: thigh forward lean and shin backward lean cancel).
pub(super) fn idle_p() -> P {
    P {
        hip: [IDLE_HIP; 2],
        knee: [IDLE_KNEE; 2],
        sh: [0.0; 2],
        el: [0.2; 2],
        sh_abd: [0.1; 2],
        ..P::default()
    }
}
/// Pelvis height (m) of the soft idle stance.
pub fn idle_pelvis_y() -> f64 {
    ANKLE_H + THIGH * IDLE_HIP.cos() + SHIN * (IDLE_KNEE - IDLE_HIP).cos()
}
/// Static soft idle targets (zero rates).
pub fn idle_pose() -> Vec<(u16, BoneTarget)> {
    let p = idle_p();
    build(&p, &p, &p)
}

// ---------------------------------------------------------------- run

/// Run layout: pose plus both ankle positions (pelvis frame; ground at y = ankle - ANKLE_H).
struct RunLayout {
    pose: P,
    ankles: [Vec3; 2],
    pelvis_y: f64,
    stance: [bool; 2],
}

fn run_layout(phase: f64, speed: f64) -> RunLayout {
    let v = speed.max(0.0);
    let stride = GAIT_DEPTH * (0.6 + 0.37 * v).min(3.2); // full cycle length (m), `ability::default_stride` × depth
    let base_y = STAND_Y - GAIT_DEPTH * (0.03 + 0.008 * v.min(8.0));
    let bob = GAIT_DEPTH * (0.012 + 0.006 * v.min(6.0)).min(0.05);
    let dy_hi = base_y + bob - ANKLE_H;
    let zmax = (REACH * REACH - dy_hi * dy_hi).max(0.0).sqrt();
    let beta = (0.62 - 0.035 * v)
        .clamp(0.35, 0.62)
        .min(2.0 * zmax / stride.max(1e-6));
    let pelvis_y = base_y - bob * (4.0 * PI * (phase - 0.5 * beta)).cos();
    let lift = GAIT_DEPTH * (0.06 + 0.04 * v).min(0.35);
    let mut pose = P {
        pitch: (0.04 + 0.035 * v).min(0.3),
        ..P::default()
    };
    pose.spine = 0.3 * pose.pitch;
    pose.neck = -0.5 * pose.pitch;
    let mut ankles = [Vec3::ZERO; 2];
    let mut stance = [false; 2];
    for i in 0..2 {
        let s = (phase + 0.5 * i as f64).rem_euclid(1.0);
        let half = 0.5 * beta * stride;
        let (z, y) = if s < beta {
            stance[i] = true;
            (half - s * stride, ANKLE_H)
        } else {
            let u = (s - beta) / (1.0 - beta);
            (
                herm(-half, -stride, half, -stride, 1.0 - beta, s - beta),
                ANKLE_H + lift * (PI * u).sin().powi(3),
            )
        };
        ankles[i] = Vec3::new(side_x(i), y - pelvis_y, z);
        pose.leg(i, reachable(i, ankles[i]), 0.0);
    }
    let a = (0.25 + 0.08 * v).min(0.9);
    let c = (2.0 * PI * phase).cos();
    pose.sh = [-a * c, a * c];
    pose.el = [(0.4 + 0.12 * v).min(1.6); 2];
    pose.sh_abd = [0.08; 2];
    RunLayout {
        pose,
        ankles,
        pelvis_y,
        stance,
    }
}

/// Run cycle period (s) at `speed` (m/s); `f64::INFINITY` when standing.
pub fn run_period(speed: f64) -> f64 {
    if speed > 1e-3 {
        GAIT_DEPTH * (0.6 + 0.37 * speed).min(3.2) / speed
    } else {
        f64::INFINITY
    }
}

/// Procedural run at cycle `phase` (0..1, left foot touches down at 0) and `speed` (m/s, ≥ 0):
/// speed-scaled stride, stance foot planted (moves back at exactly `speed` in the pelvis frame),
/// swing foot on a Hermite/sin³ arc, counter arm swing, pelvis bob (lowest at mid-stance).
/// Advance with `phase += DT / run_period(speed)`.
pub fn run_pose(phase: f64, speed: f64) -> Vec<(u16, BoneTarget)> {
    let dp = if speed > 1e-3 {
        DT / run_period(speed)
    } else {
        0.0
    };
    build(
        &run_layout(phase - dp, speed).pose,
        &run_layout(phase, speed).pose,
        &run_layout(phase + dp, speed).pose,
    )
}

/// Run foot placement for the game / checks: (ankle positions in pelvis frame, stance flags, pelvis height).
pub fn run_feet(phase: f64, speed: f64) -> ([Vec3; 2], [bool; 2], f64) {
    let l = run_layout(phase, speed);
    (l.ankles, l.stance, l.pelvis_y)
}

// ---------------------------------------------------------------- jump

pub const JUMP_CROUCH_TICKS: u64 = 18;
pub const JUMP_PUSH_TICKS: u64 = 9;
pub const JUMP_LAND_TICKS: u64 = 36;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpPhase {
    Crouch,
    Pushoff,
    Airborne,
    Land,
    Done,
}

/// Jump timeline. Airborne lasts until the game reports touchdown with `land`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpState {
    pub start: u64,
    pub land: Option<(u64, f64)>,
}

impl JumpState {
    pub fn new(start: u64) -> Self {
        Self { start, land: None }
    }
    /// Touchdown at `tick` with vertical speed `impact_vy` (m/s, sign ignored). First call wins.
    pub fn land(&mut self, tick: u64, impact_vy: f64) {
        if self.land.is_none() {
            self.land = Some((tick, impact_vy.abs()));
        }
    }
    pub fn takeoff_tick(&self) -> u64 {
        self.start + JUMP_CROUCH_TICKS + JUMP_PUSH_TICKS
    }
    pub fn phase(&self, tick: u64) -> JumpPhase {
        if let Some((lt, _)) = self.land {
            if tick >= lt {
                return if tick >= lt + JUMP_LAND_TICKS {
                    JumpPhase::Done
                } else {
                    JumpPhase::Land
                };
            }
        }
        let t = tick.saturating_sub(self.start);
        if t < JUMP_CROUCH_TICKS {
            JumpPhase::Crouch
        } else if t < JUMP_CROUCH_TICKS + JUMP_PUSH_TICKS {
            JumpPhase::Pushoff
        } else {
            JumpPhase::Airborne
        }
    }
    /// Landing absorb depth (m) for an impact speed: deeper for harder landings.
    pub fn absorb_depth(impact_vy: f64) -> f64 {
        (0.05 + 0.07 * impact_vy.abs()).clamp(0.05, 0.38)
    }
}

pub(super) fn planted(pelvis_y: f64, pitch: f64, sh: f64, el: f64) -> P {
    let mut p = P {
        pitch,
        spine: 0.3 * pitch,
        neck: -0.4 * pitch,
        sh: [sh; 2],
        el: [el; 2],
        sh_abd: [0.1; 2],
        ..P::default()
    };
    for i in 0..2 {
        p.leg(
            i,
            reachable(i, Vec3::new(side_x(i), ANKLE_H - pelvis_y, 0.0)),
            0.0,
        );
    }
    p
}

fn jump_air(t_air: f64) -> P {
    let push_end = planted(STAND_Y + 0.03, 0.1, 1.2, 0.3);
    let tuck = P {
        pitch: 0.15,
        spine: 0.25,
        hip: [1.2; 2],
        knee: [1.9; 2],
        sh: [0.9; 2],
        el: [0.9; 2],
        sh_abd: [0.3; 2],
        ..P::default()
    };
    let ready = P {
        pitch: 0.1,
        spine: 0.1,
        hip: [0.35; 2],
        knee: [0.55; 2],
        sh: [0.5; 2],
        el: [0.4; 2],
        sh_abd: [0.5; 2],
        ..P::default()
    };
    let a = P::lerp(&push_end, &tuck, smooth(t_air / 14.0));
    P::lerp(&a, &ready, smooth((t_air - 28.0) / 16.0))
}

fn jump_layout(st: &JumpState, tick: f64) -> P {
    let (c, pu) = (JUMP_CROUCH_TICKS as f64, JUMP_PUSH_TICKS as f64);
    let t = tick - st.start as f64;
    if let Some((lt, vy)) = st.land {
        let tl = tick - lt as f64;
        if tl >= 0.0 {
            let d = JumpState::absorb_depth(vy);
            let tau = 6.0 + 2.0 * vy.min(8.0);
            let shape = (tl / tau) * (1.0 - tl / tau).exp();
            let pl = planted(
                STAND_Y - d * shape,
                0.1 + 1.2 * d * shape,
                0.5 * (1.0 - smooth(tl / JUMP_LAND_TICKS as f64)),
                0.4,
            );
            let pl = P::lerp(
                &pl,
                &idle_p(),
                smooth((tl - 24.0) / (JUMP_LAND_TICKS as f64 - 24.0)),
            );
            return P::lerp(
                &jump_air(lt as f64 - st.takeoff_tick() as f64),
                &pl,
                smooth(tl / 4.0),
            );
        }
    }
    if t < c {
        let s = smooth(t / c);
        P::lerp(
            &idle_p(),
            &planted(
                herm(STAND_Y, 0.0, STAND_Y - 0.20, 0.0, c, t.max(0.0)),
                0.35 * s,
                -0.8 * s,
                0.3,
            ),
            s,
        )
    } else if t < c + pu {
        let s = smooth((t - c) / pu);
        planted(
            herm(STAND_Y - 0.20, 0.0, STAND_Y + 0.03, 0.04, pu, t - c),
            mix(0.35, 0.1, s),
            mix(-0.8, 1.2, s),
            0.3,
        )
    } else {
        jump_air(t - c - pu)
    }
}

/// Jump targets at `tick`: crouch -> pushoff -> airborne tuck (then legs ready) -> landing absorb whose
/// depth scales with the impact vertical speed given to `JumpState::land`.
pub fn jump_pose(state: &JumpState, tick: u64) -> Vec<(u16, BoneTarget)> {
    let t = tick as f64;
    build(
        &jump_layout(state, t - 1.0),
        &jump_layout(state, t),
        &jump_layout(state, t + 1.0),
    )
}

// ---------------------------------------------------------------- air recovery

fn air_layout(body_up: Vec3, feet: [Vec3; 2], ttl: f64) -> P {
    let l = body_up.length();
    let up = if l > 1e-9 && l.is_finite() {
        body_up * (1.0 / l)
    } else {
        Vec3::Y
    };
    // r = rotation (pelvis frame) that would bring the body upright.
    let ax = Vec3::Y.cross(up);
    let s = ax.length();
    let ang = s.atan2(up.y);
    let r = if s > 1e-9 {
        ax * (ang / s)
    } else if up.y < 0.0 {
        Vec3::new(PI, 0.0, 0.0)
    } else {
        Vec3::ZERO
    };
    // Trunk bends toward upright, limbs windmill the opposite way (angular momentum stays ~0).
    let k = 0.6;
    let tuck = P {
        hip: [1.0; 2],
        knee: [1.5; 2],
        ..P::default()
    };
    let mut reach = P::default();
    for i in 0..2 {
        let foot = if feet[i].is_finite() {
            feet[i]
        } else {
            Vec3::new(side_x(i), ANKLE_H - STAND_Y, 0.0)
        };
        let hip = Vec3::new(side_x(i), 0.0, 0.0);
        let ank = hip + (foot - hip).clamp_length(REACH * 0.95);
        reach.leg(i, ank, 0.0);
    }
    let w = 1.0 - smooth((ttl - 0.15) / 0.45);
    let mut p = P::lerp(&tuck, &reach, w);
    p.spine = (0.5 * r.x).clamp(-0.6, 0.6);
    p.neck = (0.4 * r.x).clamp(-0.5, 0.5);
    for i in 0..2 {
        p.sh[i] = (-k * 2.0 * r.x).clamp(-2.5, 2.5);
        let sgn = if i == 0 { 1.0 } else { -1.0 };
        p.sh_abd[i] = (0.8 + sgn * k * 2.0 * r.z).clamp(0.0, 2.6);
        p.el[i] = 0.3;
    }
    p
}

/// In-air recovery. `body_up` = world up expressed in the pelvis frame; `foot_targets` = ankle targets
/// (pelvis frame, left then right) from Physics' `phys_air::foot_targets`; `time_to_land` (s).
/// Legs tuck while far and reach the foot targets over the last ~0.6 s; trunk and arms counter-rotate
/// toward upright. No fade output: the api owns the motor weight.
pub fn air_recover_pose(
    body_up: Vec3,
    foot_targets: [Vec3; 2],
    time_to_land: f64,
) -> Vec<(u16, BoneTarget)> {
    let f = |t: f64| air_layout(body_up, foot_targets, t.max(0.0));
    build(
        &f(time_to_land + DT),
        &f(time_to_land),
        &f(time_to_land - DT),
    )
}

// ---------------------------------------------------------------- spin footwork

/// Body yaw turned per foot step while spinning (rad).
pub const SPIN_STEP: f64 = PI / 2.0;
const SPIN_FOOT_X: f64 = 0.16;

struct SpinLayout {
    pose: P,
    ankles: [Vec3; 2],
    planted: [bool; 2],
    plant_yaw: [f64; 2],
}

fn spin_layout(center_rel: Vec3, theta: f64, lean: f64) -> SpinLayout {
    let rad = (center_rel.x * center_rel.x + center_rel.z * center_rel.z).sqrt();
    let lean = if lean.is_finite() {
        lean.clamp(0.0, 0.45)
    } else {
        0.0
    };
    let lf = lean / 0.45;
    let dir = if rad > 1e-6 {
        Vec3::new(center_rel.x / rad, 0.0, center_rel.z / rad)
    } else {
        Vec3::ZERO
    };
    // Tilt the top away from the centre: rotation taking +y toward -dir.
    let tilt = Vec3::Y.cross(-dir) * lean;
    let pelvis_y = STAND_Y - 0.06 - 0.12 * lf;
    let mut pose = P {
        pitch: tilt.x,
        roll: tilt.z,
        ..P::default()
    };
    pose.spine = -0.3 * tilt.x;
    pose.neck = -0.3 * tilt.x;
    let kf = (theta / SPIN_STEP).floor();
    let u = theta / SPIN_STEP - kf;
    let k = kf as i64;
    let swing = k.rem_euclid(2) as usize; // 0: left steps on even steps
    let mut ankles = [Vec3::ZERO; 2];
    let mut planted = [true; 2];
    let mut plant_yaw = [0.0; 2];
    for i in 0..2 {
        let (yaw_w, lift) = if i == swing {
            planted[i] = false;
            let (a, b) = ((kf - 0.5) * SPIN_STEP, (kf + 1.5) * SPIN_STEP);
            (mix(a, b, smooth(u)), 0.07 * (PI * u).sin().powi(3))
        } else {
            ((kf + 0.5) * SPIN_STEP, 0.0)
        };
        plant_yaw[i] = yaw_w;
        let rel_yaw = yaw_w - theta;
        let n = Vec3::new(if i == 0 { SPIN_FOOT_X } else { -SPIN_FOOT_X }, 0.0, 0.0);
        // Feet shift toward the centre (hips away) with the lean, in the plant frame so planted feet stay fixed.
        let foot = qy(rel_yaw).rotate(n + dir * (0.10 * lf))
            + Vec3::new(0.0, ANKLE_H + lift - pelvis_y, 0.0);
        ankles[i] = foot;
        pose.leg(i, reachable(i, foot), rel_yaw);
    }
    pose.sh = [1.2; 2];
    pose.el = [0.3; 2];
    pose.sh_abd = [0.2; 2];
    SpinLayout {
        pose,
        ankles,
        planted,
        plant_yaw,
    }
}

/// Lean angle from grab tension `tension` (N) and body mass `mass` (kg): atan(T/(m·g)), the same
/// formula as `phys_grab::spin_lean` (not imported). Clamped to [0, 0.45] rad by `spin_footwork`.
pub fn spin_lean(tension: f64, mass: f64) -> f64 {
    if mass > 1e-6 {
        (tension.max(0.0) / (mass * G)).atan()
    } else {
        0.0
    }
}

/// Footwork while spinning a grabbed enemy around `spin_center_rel` (pelvis frame, xz used; from
/// read_state). `spin_angle` = accumulated body yaw (rad), `spin_rate` = yaw rate (rad/s, only for the
/// ±1 tick samples), `lean` = lean angle (rad), e.g. `spin_lean(T, m)`. One foot is always planted
/// (fixed in world while the body turns); the other side-steps on a smooth arc every `SPIN_STEP`.
/// The top leans away from the centre, hips shift away.
pub fn spin_footwork(
    spin_center_rel: Vec3,
    spin_angle: f64,
    spin_rate: f64,
    lean: f64,
) -> Vec<(u16, BoneTarget)> {
    let d = spin_rate * DT;
    let f = |a: f64| spin_layout(spin_center_rel, a, lean).pose;
    build(&f(spin_angle - d), &f(spin_angle), &f(spin_angle + d))
}

/// Spin foot data for the game / checks: (ankles in pelvis frame, planted flags, world yaw of each foot).
pub fn spin_feet(
    spin_center_rel: Vec3,
    spin_angle: f64,
    lean: f64,
) -> ([Vec3; 2], [bool; 2], [f64; 2]) {
    let l = spin_layout(spin_center_rel, spin_angle, lean);
    (l.ankles, l.planted, l.plant_yaw)
}

// ---------------------------------------------------------------- getup

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lying {
    Prone,
    Supine,
}

/// Stand-up timeline. `from` = measured local pose at the start (PROC_BONES order; None = keyframe 0),
/// blended into keyframe 1 over `lead` ticks (≥ 40, longer for big start rotations so ω stays < 10 rad/s).
/// Later keys follow GETUP_KEYS shifted by `lead − 40`. Build with `GetupState::new`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GetupState {
    pub start: u64,
    pub kind: Lying,
    pub from: Option<[Quat; 15]>,
    pub lead: u64,
}

/// Peak joint speed (rad/s) allowed when blending from the measured start pose.
pub const GETUP_MAX_W: f64 = 8.0;

impl GetupState {
    /// Start a getup at `tick` from `measured_local` (PROC skin id, local rotation); missing bones use key 0.
    pub fn new(tick: u64, kind: Lying, measured_local: &[(u16, Quat)]) -> Self {
        let k1 = getup_keys(kind)[1].quats();
        let mut from = getup_keys(kind)[0].quats();
        let mut big: f64 = 0.0;
        for (i, b) in PROC_BONES.iter().enumerate() {
            if let Some(&(_, q)) = measured_local.iter().find(|m| m.0 == b.0) {
                if q.w.is_finite() && q.x.is_finite() && q.y.is_finite() && q.z.is_finite() {
                    from[i] = q.normalize();
                }
            }
            big = big.max((from[i].conj() * k1[i]).to_rotvec().length());
        }
        // smoothstep peak rate = 1.5·Δ/T  ->  T ≥ 1.5·Δ/w_max.
        let lead = ((1.5 * big / GETUP_MAX_W) / DT)
            .ceil()
            .max(GETUP_KEYS[1] as f64) as u64;
        Self {
            start: tick,
            kind,
            from: Some(from),
            lead,
        }
    }
    /// Tick at which the getup reaches idle.
    pub fn end_tick(&self) -> u64 {
        self.start + self.lead.max(GETUP_KEYS[1]) - GETUP_KEYS[1] + GETUP_KEYS[4]
    }
}

/// Measured physics-joint local rotations (index = JointId of the 13-joint rig, e.g. from
/// `BodyState::joints[j].q_local`) -> PROC bones for `GetupState::new`. JointId 1 goes to B_Spine2
/// (Spine/Spine1 identity, matching `proc_to_joints`' composition); pelvis is skipped.
pub fn measured_from_joints(q_local: &[Quat]) -> Vec<(u16, Quat)> {
    let mut out = Vec::new();
    for (name, j) in JOINT_OF.iter() {
        let skin = match PROC_BONES.iter().find(|b| b.1 == *name) {
            Some(b) => b.0,
            None => continue,
        };
        let q = match *name {
            "B_Pelvis" => continue,
            "B_Spine" | "B_Spine1" => Quat::IDENTITY,
            _ => match q_local.get(*j as usize) {
                Some(q) => *q,
                None => continue,
            },
        };
        out.push((skin, q));
    }
    out
}

/// Keyframe ticks (relative to start); the last key is the soft idle pose.
pub const GETUP_KEYS: [u64; 5] = [0, 40, 85, 130, 180];

/// Prone vs supine from the pelvis world rotation: pelvis forward (+z) pointing down = prone, up = supine;
/// None while the pelvis is more upright than lying (|forward.y| < 0.5 or up.y > 0.5).
pub fn lying_from_up(q_pelvis: Quat) -> Option<Lying> {
    let q = q_pelvis.normalize();
    let (fwd, up) = (q.rotate(Vec3::new(0.0, 0.0, 1.0)), q.rotate(Vec3::Y));
    if !fwd.is_finite() || up.y > 0.5 {
        return None;
    }
    if fwd.y < -0.5 {
        Some(Lying::Prone)
    } else if fwd.y > 0.5 {
        Some(Lying::Supine)
    } else {
        None
    }
}

fn getup_keys(kind: Lying) -> [P; 5] {
    let k = |pitch: f64, spine: f64, hip: f64, knee: f64, sh: f64, el: f64, sh_abd: f64| P {
        pitch,
        spine,
        neck: -0.3 * spine,
        hip: [hip; 2],
        knee: [knee; 2],
        sh: [sh; 2],
        el: [el; 2],
        sh_abd: [sh_abd; 2],
        ..P::default()
    };
    match kind {
        // Face down: hands under shoulders, push up, knees tuck under, rise.
        Lying::Prone => [
            k(0.0, 0.0, 0.0, 0.0, 0.0, 0.2, 0.3),
            k(0.0, -0.2, 0.2, 0.3, 1.3, 1.7, 0.25),
            k(0.0, 0.25, 0.6, 0.8, 1.4, 0.2, 0.2),
            k(0.0, 0.3, 1.9, 2.2, 1.1, 0.3, 0.15),
            idle_p(),
        ],
        // Face up: sit up, plant the feet, rise.
        Lying::Supine => [
            k(0.0, 0.0, 0.0, 0.0, 0.0, 0.2, 0.3),
            k(0.0, 0.6, 1.2, 0.4, 0.9, 0.3, 0.2),
            k(0.0, 0.5, 1.8, 2.0, 1.0, 0.3, 0.2),
            k(0.0, 0.3, 1.9, 2.2, 1.2, 0.3, 0.15),
            idle_p(),
        ],
    }
}

fn slerp(a: Quat, b: Quat, s: f64) -> Quat {
    (a * Quat::from_rotvec((a.conj() * b).to_rotvec() * s)).normalize()
}

fn getup_quats(gs: &GetupState, t: f64) -> [Quat; 15] {
    let keys = getup_keys(gs.kind);
    let lead = gs.lead.max(GETUP_KEYS[1]) as f64;
    let shift = lead - GETUP_KEYS[1] as f64;
    let t = (t - gs.start as f64).max(0.0);
    if t < lead {
        let s = smooth(t / lead);
        let (a, b) = (gs.from.unwrap_or_else(|| keys[0].quats()), keys[1].quats());
        return std::array::from_fn(|i| slerp(a[i], b[i], s));
    }
    for i in 1..4 {
        let (a, b) = (
            GETUP_KEYS[i] as f64 + shift,
            GETUP_KEYS[i + 1] as f64 + shift,
        );
        if t < b {
            return P::lerp(&keys[i], &keys[i + 1], smooth((t - a) / (b - a))).quats();
        }
    }
    keys[4].quats()
}

fn build_q(m: &[Quat; 15], c: &[Quat; 15], p: &[Quat; 15]) -> Vec<(u16, BoneTarget)> {
    (0..PROC_BONES.len())
        .map(|i| (PROC_BONES[i].0, target_from(Some(m[i]), c[i], Some(p[i]))))
        .collect()
}

/// Stand-up targets at `tick`: smoothstep keyframes (zero rate at each key), ends exactly on soft idle.
pub fn getup_pose(gs: &GetupState, tick: u64) -> Vec<(u16, BoneTarget)> {
    let t = tick as f64;
    build_q(
        &getup_quats(gs, t - 1.0),
        &getup_quats(gs, t),
        &getup_quats(gs, t + 1.0),
    )
}
/// True once the stand-up has reached the idle pose.
pub fn getup_done(gs: &GetupState, tick: u64) -> bool {
    tick >= gs.end_tick()
}
/// `getup_pose` mapped to the 13-joint physics rig.
pub fn getup_joints(gs: &GetupState, tick: u64) -> Vec<crate::api::Target> {
    proc_to_joints(&getup_pose(gs, tick))
}

/// Settle detector counter (one per body, game-owned).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SettleState {
    pub still_ticks: u32,
}

pub const SETTLE_W: f64 = 0.3;
pub const SETTLE_V: f64 = 0.1;
pub const SETTLE_TICKS: u32 = 30;

/// True once every joint's angular speed (`BodyState::joints[j].w`) stayed < SETTLE_W rad/s and the
/// body linear speed (`BodyState::com_v`; BodyState has no separate pelvis velocity) < SETTLE_V m/s for
/// `n` consecutive ticks (e.g. SETTLE_TICKS). Call once per tick; any motion resets the count.
pub fn body_settled(st: &mut SettleState, s: &crate::api::BodyState, n: u32) -> bool {
    let w = s.joints.iter().map(|j| j.w.length()).fold(0.0, f64::max);
    let v = s.com_v.length();
    if w.is_finite() && v.is_finite() && w < SETTLE_W && v < SETTLE_V {
        st.still_ticks = st.still_ticks.saturating_add(1);
    } else {
        st.still_ticks = 0;
    }
    st.still_ticks >= n
}

/// Holding limp while waiting to settle: send NO targets and call
/// `sim.set_motor_weight(body, None, 0.0)` (all joints weight 0). When `body_settled` is true, the game
/// calls `recover` only; the motor's own 36-tick smoothstep ramp brings the weight from 0 to 1. Never set
/// weight 1.0 directly, since that full-strength jump is what made the body flail. This helper only goes limp.
pub fn limp_joints(sim: &mut crate::api::Sim, b: crate::api::BodyId) {
    sim.set_motor_weight(b, None, 0.0);
}

// ---------------------------------------------------------------- api glue

/// Ankle height above the sole (m) added to world floor targets before conversion.
pub const SOLE_TO_ANKLE: f64 = 0.08;

/// World floor foot targets (`BodyState::air_feet`, phys_air::foot_targets) -> ankle targets in the
/// pelvis frame: q⁻¹·(p + 0.08·ŷ − pelvis_pos).
pub fn air_feet_to_pelvis(feet_world: [Vec3; 2], pelvis_pos: Vec3, q_pelvis: Quat) -> [Vec3; 2] {
    let qi = q_pelvis.normalize().conj();
    feet_world.map(|p| qi.rotate(p + Vec3::new(0.0, SOLE_TO_ANKLE, 0.0) - pelvis_pos))
}

/// Small persistent anim state, owned by the game (one per body).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProcState {
    /// Run cycle phase (0..1).
    pub run_phase: f64,
    /// Active jump, if any (start one with `JumpState::new(tick)`; cleared when Done).
    pub jump: Option<JumpState>,
    /// Accumulated (unwrapped) pelvis yaw while grabbing (rad).
    pub spin_angle: f64,
    last_yaw: Option<f64>,
}

fn yaw_of(q: Quat) -> f64 {
    let f = q.rotate(Vec3::new(0.0, 0.0, 1.0));
    f.x.atan2(f.z)
}

/// Targets for the body's current mode. `mass` = total body mass (kg; BodyState carries none),
/// `tick` = current fixed tick. Grounded: jump if `ps.jump` is active, else run (idle at speed < 0.05).
/// Airborne/Recovering: air recovery toward `air_feet`. Grabbing: spin footwork with
/// lean = spin_lean(grab_tension, mass). Ragdoll/Grabbed: empty.
pub fn proc_from_state(
    ps: &mut ProcState,
    s: &crate::api::BodyState,
    mass: f64,
    tick: u64,
) -> Vec<(u16, BoneTarget)> {
    use crate::api::BodyMode;
    let pel = s.joints.first().copied().unwrap_or_default();
    let qp = pel.q_world.normalize();
    if s.mode != BodyMode::Grabbing {
        ps.last_yaw = None;
    }
    match s.mode {
        BodyMode::Grounded => {
            if let Some(mut j) = ps.jump {
                if j.phase(tick) == JumpPhase::Airborne
                    && j.land.is_none()
                    && (s.feet_down[0] || s.feet_down[1])
                    && tick > j.takeoff_tick() + 2
                {
                    j.land(tick, s.com_v.y);
                }
                if j.phase(tick) != JumpPhase::Done {
                    ps.jump = Some(j);
                    return jump_pose(&j, tick);
                }
                ps.jump = None;
            }
            let v = (s.com_v.x * s.com_v.x + s.com_v.z * s.com_v.z).sqrt();
            if v < 0.05 {
                ps.run_phase = 0.0;
                return idle_pose();
            }
            ps.run_phase = (ps.run_phase + DT / run_period(v)).rem_euclid(1.0);
            run_pose(ps.run_phase, v)
        }
        BodyMode::Airborne | BodyMode::Recovering => {
            if let Some(j) = ps.jump {
                if j.land.is_none() && j.phase(tick) == JumpPhase::Airborne {
                    return jump_pose(&j, tick);
                }
                ps.jump = None;
            }
            let up = qp.conj().rotate(Vec3::Y);
            let feet = match s.air_feet {
                Some(f) => air_feet_to_pelvis(f, pel.pos, qp),
                None => [
                    Vec3::new(HIP_W, ANKLE_H - STAND_Y, 0.0),
                    Vec3::new(-HIP_W, ANKLE_H - STAND_Y, 0.0),
                ],
            };
            air_recover_pose(up, feet, s.t_land.unwrap_or(1.0))
        }
        BodyMode::Grabbing => {
            let yaw = yaw_of(qp);
            if let Some(last) = ps.last_yaw {
                let mut d = yaw - last;
                while d > PI {
                    d -= 2.0 * PI;
                }
                while d < -PI {
                    d += 2.0 * PI;
                }
                ps.spin_angle += d;
            }
            ps.last_yaw = Some(yaw);
            let center = s
                .spin_center
                .map(|c| qp.conj().rotate(c - pel.pos))
                .unwrap_or(Vec3::ZERO);
            spin_footwork(
                center,
                ps.spin_angle,
                pel.w.y,
                spin_lean(s.grab_tension, mass),
            )
        }
        BodyMode::Ragdoll | BodyMode::Grabbed => Vec::new(),
    }
}

// ---------------------------------------------------------------- physics rig mapping

/// PROC bone name -> game physics rig JointId (13 segments, hive/RIG_BONES.md). Single table.
/// B_Pelvis is the root (no motor target). B_Spine/B_Spine1/B_Spine2 all fold into JointId 1;
/// B_Neck drives JointId 2 (B_Head).
pub const JOINT_OF: [(&str, u16); 15] = [
    ("B_Pelvis", 0),
    ("B_Spine", 1),
    ("B_Spine1", 1),
    ("B_Spine2", 1),
    ("B_Neck", 2),
    ("B_L_UpperArm", 3),
    ("B_L_Forearm", 4),
    ("B_R_UpperArm", 5),
    ("B_R_Forearm", 6),
    ("B_L_Thigh", 7),
    ("B_L_Calf", 8),
    ("B_R_Thigh", 9),
    ("B_R_Calf", 10),
    ("B_L_Foot", 11),
    ("B_R_Foot", 12),
];
const SPINE_CHAIN: [&str; 3] = ["B_Spine", "B_Spine1", "B_Spine2"];

fn proc_name(skin: u16) -> Option<&'static str> {
    PROC_BONES.iter().find(|b| b.0 == skin).map(|b| b.1)
}
fn joint_of(name: &str) -> Option<u16> {
    JOINT_OF.iter().find(|j| j.0 == name).map(|j| j.1)
}

/// Map `proc` targets (PROC_BONES skin ids) to api motor targets on the 13-joint physics rig.
/// Pelvis is dropped (root). Spine, Spine1, Spine2 compose into JointId 1:
///   rot = q_S·q_S1·q_S2;  rates are summed after mapping each into the Spine parent (pelvis) frame,
///   ω = ω_S + q_S·ω_S1 + (q_S q_S1)·ω_S2,  α = Σ R_i·α_i + Ω_i × (R_i·ω_i)  (Ω_i = rate of the frames
///   above bone i). This is the exact relative-rate chain rule, not just a plain sum. Missing spine
///   bones count as identity at rest; if none are present there is no JointId 1 target. Missing bone = no target.
pub fn proc_to_joints(targets: &[(u16, BoneTarget)]) -> Vec<crate::api::Target> {
    let mut out = Vec::new();
    let mut spine: [Option<BoneTarget>; 3] = [None; 3];
    for &(skin, bt) in targets {
        let Some(name) = proc_name(skin) else {
            continue;
        };
        if let Some(k) = SPINE_CHAIN.iter().position(|s| *s == name) {
            spine[k] = Some(bt);
            continue;
        }
        match joint_of(name) {
            Some(0) | None => {}
            Some(j) => out.push(bt.to_api(j)),
        }
    }
    if spine.iter().any(|s| s.is_some()) {
        let (mut r, mut w, mut a) = (Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO);
        for s in spine.iter().flatten() {
            let wi = r.rotate(s.omega);
            a = a + r.rotate(s.alpha) + w.cross(wi);
            w = w + wi;
            r = (r * s.rot).normalize();
        }
        out.push(
            BoneTarget {
                rot: r,
                omega: w,
                alpha: a,
            }
            .to_api(1),
        );
    }
    out.sort_by_key(|t| t.joint.0);
    out
}

/// `proc_from_state` followed by `proc_to_joints`: ready for `Sim::set_target`.
pub fn proc_from_state_joints(
    ps: &mut ProcState,
    s: &crate::api::BodyState,
    mass: f64,
    tick: u64,
) -> Vec<crate::api::Target> {
    proc_to_joints(&proc_from_state(ps, s, mass, tick))
}
