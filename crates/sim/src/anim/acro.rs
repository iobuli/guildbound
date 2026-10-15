//! Ported from abilbox anim_acro.rs.
//! Procedural acrobatics: back step entry, back handspring and backflip, built as dense per-tick
//! keys (RotTrack-style `(tick, quat)` per bone) plus a root trajectory. Sagittal-plane model:
//! z = forward (the moves travel to -z), y = up, all joint rotations are about the lateral X axis
//! with + = flexion (forward). Flight phases are exact ballistic arcs (g = 9.81). Contacts (feet,
//! hands) are solved with 2-link IK so planted ends sit exactly on the ground.
//! Bone indices are skin-joint indices of assets/packs/hero/master.glb (skins[0].joints order).

#![allow(dead_code)]
use std::f64::consts::PI;

pub const HZ: f64 = 120.0;
pub const G: f64 = 9.81;
pub type Quat = [f32; 4];

/// Segment lengths (m), measured from master.glb rest offsets and rounded.
pub const THIGH: f64 = 0.44;
pub const SHIN: f64 = 0.44;
pub const ANKLE_H: f64 = 0.08; // ankle height when the foot is flat on the ground
pub const SPINE: f64 = 0.52; // pelvis to shoulder
pub const ARM: f64 = 0.57; // shoulder to palm, elbow straight
pub const UPPER_ARM: f64 = 0.25;
pub const PALM_H: f64 = 0.03; // wrist/palm height when the hand is planted
pub const STAND_Y: f64 = 0.94;

/// master.glb skin-joint indices and names.
pub mod bone {
    pub const PELVIS: u16 = 1; // B_Pelvis (carries the trunk pitch)
    pub const L_THIGH: u16 = 2; // B_L_Thigh
    pub const L_CALF: u16 = 3; // B_L_Calf
    pub const L_FOOT: u16 = 4; // B_L_Foot
    pub const R_THIGH: u16 = 7; // B_R_Thigh
    pub const R_CALF: u16 = 8; // B_R_Calf
    pub const R_FOOT: u16 = 9; // B_R_Foot
    pub const SPINE: u16 = 12; // B_Spine
    pub const SPINE1: u16 = 13; // B_Spine1
    pub const SPINE2: u16 = 14; // B_Spine2
    pub const L_UPPERARM: u16 = 16; // B_L_UpperArm
    pub const L_FOREARM: u16 = 17; // B_L_Forearm
    pub const NECK: u16 = 39; // B_Neck
    pub const R_UPPERARM: u16 = 42; // B_R_UpperArm
    pub const R_FOREARM: u16 = 43; // B_R_Forearm
}

/// Unit vector at angle a from +y toward +z.
fn u(a: f64) -> [f64; 2] {
    [a.sin(), a.cos()]
} // [z, y]
fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
fn sc(a: [f64; 2], s: f64) -> [f64; 2] {
    [a[0] * s, a[1] * s]
}

/// Cubic Hermite over duration `d` (values, rates per second) at local time t.
fn herm(p0: f64, m0: f64, p1: f64, m1: f64, d: f64, t: f64) -> f64 {
    let s = (t / d).clamp(0.0, 1.0);
    let (s2, s3) = (s * s, s * s * s);
    (2.0 * s3 - 3.0 * s2 + 1.0) * p0
        + (s3 - 2.0 * s2 + s) * d * m0
        + (-2.0 * s3 + 3.0 * s2) * p1
        + (s3 - s2) * d * m1
}
fn herm2(p0: [f64; 2], m0: [f64; 2], p1: [f64; 2], m1: [f64; 2], d: f64, t: f64) -> [f64; 2] {
    [
        herm(p0[0], m0[0], p1[0], m1[0], d, t),
        herm(p0[1], m0[1], p1[1], m1[1], d, t),
    ]
}
fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
fn mix(a: f64, b: f64, s: f64) -> f64 {
    a + (b - a) * s
}

/// Leg IK: hip at `hip`, ankle at `ank`, trunk pitch `th` -> (hip flexion, knee flexion). Knee bends forward.
pub fn leg_ik(hip: [f64; 2], ank: [f64; 2], th: f64) -> (f64, f64) {
    let dz = ank[0] - hip[0];
    let dy = ank[1] - hip[1];
    let d = (dz * dz + dy * dy).sqrt().clamp(1e-6, THIGH + SHIN - 1e-9);
    let gam = dz.atan2(dy);
    let knee_in = ((THIGH * THIGH + SHIN * SHIN - d * d) / (2.0 * THIGH * SHIN))
        .clamp(-1.0, 1.0)
        .acos();
    let del = ((THIGH * THIGH + d * d - SHIN * SHIN) / (2.0 * THIGH * d))
        .clamp(-1.0, 1.0)
        .acos();
    let thigh = gam - del;
    (wrap(th + PI - thigh), PI - knee_in)
}
/// Leg FK: ankle position.
pub fn leg_fk(hip: [f64; 2], th: f64, h: f64, k: f64) -> [f64; 2] {
    let a = th + PI - h;
    add(add(hip, sc(u(a), THIGH)), sc(u(a + k), SHIN))
}
/// Arm FK (elbow flex e bends the forearm forward/up): palm position.
pub fn arm_fk(pelvis: [f64; 2], th: f64, spine: f64, phi: f64, e: f64) -> [f64; 2] {
    let sh = add(pelvis, sc(u(th + spine), SPINE));
    let a = th + spine + PI - phi;
    add(add(sh, sc(u(a), UPPER_ARM)), sc(u(a - e), ARM - UPPER_ARM))
}
fn wrap(a: f64) -> f64 {
    let mut a = a;
    while a > PI {
        a -= 2.0 * PI;
    }
    while a < -PI {
        a += 2.0 * PI;
    }
    a
}

/// Full sagittal pose at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub pelvis: [f64; 2], // [z, y]
    pub pitch: f64,       // trunk pitch, + forward; a backflip goes 0 -> -2pi
    pub spine: f64,       // extra spine bend (split over 3 spine bones)
    pub hip: [f64; 2],    // [left, right] hip flexion
    pub knee: [f64; 2],
    pub shoulder: f64,
    pub elbow: f64,
    pub feet_planted: [bool; 2],
    pub hands_planted: bool,
    pub airborne: bool,
    /// Where planted feet are held (z), used by the back step; NaN = phase default.
    pub foot_target: [f64; 2],
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            pelvis: [0.0; 2],
            pitch: 0.0,
            spine: 0.0,
            hip: [0.0; 2],
            knee: [0.0; 2],
            shoulder: 0.0,
            elbow: 0.0,
            feet_planted: [false; 2],
            hands_planted: false,
            airborne: false,
            foot_target: [f64::NAN; 2],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Phase {
    pub name: &'static str,
    pub start: u32,
    pub end: u32,
}

/// A generated clip: dense per-tick poses, bone keys and root trajectory.
#[derive(Clone, Debug)]
pub struct AcroClip {
    pub name: &'static str,
    pub poses: Vec<Pose>,
    pub phases: Vec<Phase>,
    /// Ground contact targets (z) for the feet at each tick (left, right) and the hand plant (z).
    pub feet_z: Vec<[f64; 2]>,
    pub hand_z: f64,
}

impl AcroClip {
    pub fn ticks(&self) -> u32 {
        self.poses.len() as u32
    }
    /// Root (pelvis) translation per tick as [x, y, z] (f32), x = 0.
    pub fn root_track(&self) -> Vec<(u32, [f32; 3])> {
        self.poses
            .iter()
            .enumerate()
            .map(|(i, p)| (i as u32, [0.0, p.pelvis[1] as f32, p.pelvis[0] as f32]))
            .collect()
    }
    /// Per-bone local hinge angles (rad about lateral X), sorted by bone index.
    pub fn bone_angles(p: &Pose) -> Vec<(u16, f64)> {
        vec![
            (bone::PELVIS, p.pitch),
            (bone::L_THIGH, p.hip[0]),
            (bone::L_CALF, -p.knee[0]),
            (bone::L_FOOT, p.knee[0] - p.hip[0]),
            (bone::R_THIGH, p.hip[1]),
            (bone::R_CALF, -p.knee[1]),
            (bone::R_FOOT, p.knee[1] - p.hip[1]),
            (bone::SPINE, p.spine / 3.0),
            (bone::SPINE1, p.spine / 3.0),
            (bone::SPINE2, p.spine / 3.0),
            (bone::L_UPPERARM, p.shoulder),
            (bone::L_FOREARM, p.elbow),
            (bone::R_UPPERARM, p.shoulder),
            (bone::R_FOREARM, p.elbow),
        ]
    }
    /// RotTrack-style keys: for each bone, `(tick, quat)` about lateral X (x,y,z,w).
    pub fn rot_tracks(&self) -> Vec<(u16, Vec<(u32, Quat)>)> {
        let mut out: Vec<(u16, Vec<(u32, Quat)>)> = Self::bone_angles(&Pose::default())
            .iter()
            .map(|(b, _)| (*b, Vec::new()))
            .collect();
        for (t, p) in self.poses.iter().enumerate() {
            for (i, (_, a)) in Self::bone_angles(p).iter().enumerate() {
                let (s, c) = (0.5 * a).sin_cos();
                out[i].1.push((t as u32, [s as f32, 0.0, 0.0, c as f32]));
            }
        }
        out
    }
    pub fn ankle(&self, t: usize, side: usize) -> [f64; 2] {
        let p = &self.poses[t];
        leg_fk(p.pelvis, p.pitch, p.hip[side], p.knee[side])
    }
    pub fn palm(&self, t: usize) -> [f64; 2] {
        let p = &self.poses[t];
        arm_fk(p.pelvis, p.pitch, p.spine, p.shoulder, p.elbow)
    }
}

/// Builder: phases are closures of local time (s) -> Pose, sampled at 120 Hz.
struct Seq {
    poses: Vec<Pose>,
    phases: Vec<Phase>,
    feet: Vec<[f64; 2]>,
    t0: f64,
}
impl Seq {
    fn push(&mut self, name: &'static str, dur: f64, feet: [f64; 2], f: &dyn Fn(f64) -> Pose) {
        let start = self.poses.len() as u32;
        let end_t = self.t0 + dur;
        // sample every tick whose time lies in [t0, end_t)
        let mut i = self.poses.len();
        while (i as f64) / HZ < end_t - 1e-9 {
            let lt = i as f64 / HZ - self.t0;
            let p = f(lt);
            self.feet.push(if p.foot_target[0].is_nan() {
                feet
            } else {
                p.foot_target
            });
            self.poses.push(p);
            i += 1;
        }
        self.t0 = end_t;
        self.phases.push(Phase {
            name,
            start,
            end: self.poses.len() as u32,
        });
    }
}

/// Inputs carried from one phase to the next.
#[derive(Clone, Copy, Debug)]
struct Carry {
    p: [f64; 2],
    v: [f64; 2],
    th: f64,
    w: f64,
    phi: f64,
    phi_rate: f64,
}

/// Back step entry: start with feet `dist` in front of z=0, step left then right back to z=0.
fn back_step(seq: &mut Seq, dist: f64) {
    let d = 0.40;
    seq.push("back_step", d, [0.0, 0.0], &|t| {
        let pel_z = herm(dist, 0.0, 0.0, -0.6, d, t);
        let pel_y = STAND_Y - 0.03 * (PI * t / d).sin();
        let th = 0.05 * (PI * t / d).sin();
        let pel = [pel_z, pel_y];
        let mut p = Pose {
            pelvis: pel,
            pitch: th,
            elbow: 0.3,
            ..Default::default()
        };
        for side in 0..2 {
            let (a, b) = if side == 0 { (0.0, 0.5) } else { (0.5, 1.0) };
            let s = ((t / d - a) / (b - a)).clamp(0.0, 1.0);
            let swing = s > 0.0 && s < 1.0;
            let z = mix(dist, 0.0, smooth(s));
            let y = ANKLE_H + if swing { 0.07 * (PI * s).sin() } else { 0.0 };
            let (h, k) = leg_ik(pel, [z, y], th);
            p.hip[side] = h;
            p.knee[side] = k;
            p.feet_planted[side] = !swing;
            p.foot_target[side] = z;
        }
        p.shoulder = 0.15 * (2.0 * PI * t / d).sin();
        p
    });
}

/// Crouch + arm swing into takeoff with given end state. Feet planted at z=0.
fn crouch(seq: &mut Seq, d: f64, start: Carry, to: Carry, spine_to: f64) {
    seq.push("crouch_swing", d, [0.0, 0.0], &|t| {
        let pel = herm2(start.p, start.v, to.p, to.v, d, t);
        let th = herm(start.th, start.w, to.th, to.w, d, t);
        // arms: swing down/back first, then throw forward-up
        let split = 0.45 * d;
        let phi = if t < split {
            herm(start.phi, start.phi_rate, -0.7, 0.0, split, t)
        } else {
            herm(-0.7, 0.0, to.phi, to.phi_rate, d - split, t - split)
        };
        let (h, k) = leg_ik(pel, [0.0, ANKLE_H], th);
        Pose {
            pelvis: pel,
            pitch: th,
            spine: mix(0.0, spine_to, smooth(t / d)),
            hip: [h, h],
            knee: [k, k],
            shoulder: phi,
            elbow: mix(0.3, 0.05, smooth(t / d)),
            feet_planted: [true, true],
            ..Default::default()
        }
    });
}

/// Ballistic flight from `c` until the pelvis falls to `y_end` (or for `dur`), returning duration.
fn flight_time(c: &Carry, y_end: f64) -> f64 {
    // y0 + vy t - g t^2/2 = y_end, descending root
    let (a, b, cc) = (0.5 * G, -c.v[1], y_end - c.p[1]);
    (-b + (b * b - 4.0 * a * cc).sqrt()) / (2.0 * a)
}
fn ballistic(c: &Carry, t: f64) -> ([f64; 2], [f64; 2]) {
    (
        [c.p[0] + c.v[0] * t, c.p[1] + c.v[1] * t - 0.5 * G * t * t],
        [c.v[0], c.v[1] - G * t],
    )
}

/// Land at `land` (pelvis, velocity, pitch, pitch rate), absorb and stand up. Feet at `foot_z`.
fn land_and_recover(seq: &mut Seq, land: Carry, foot_z: f64) {
    let da = 0.22;
    let low = [foot_z + 0.06, 0.74];
    let th_low = -2.0 * PI + 0.12;
    seq.push("land_absorb", da, [foot_z, foot_z], &|t| {
        let pel = herm2(land.p, land.v, low, [-0.4, 0.0], da, t);
        let th = herm(land.th, land.w, th_low, -0.3, da, t);
        let (h, k) = leg_ik(pel, [foot_z, ANKLE_H], th);
        Pose {
            pelvis: pel,
            pitch: th,
            spine: mix(0.1, 0.15, t / da),
            hip: [h, h],
            knee: [k, k],
            shoulder: herm(land.phi, land.phi_rate, 1.3, 0.0, da, t),
            elbow: 0.4,
            feet_planted: [true, true],
            ..Default::default()
        }
    });
    let dr = 0.30;
    seq.push("stand", dr, [foot_z, foot_z], &|t| {
        let pel = herm2(low, [-0.4, 0.0], [foot_z, STAND_Y], [0.0, 0.0], dr, t);
        let th = herm(th_low, -0.3, -2.0 * PI, 0.0, dr, t);
        let (h, k) = leg_ik(pel, [foot_z, ANKLE_H], th);
        Pose {
            pelvis: pel,
            pitch: th,
            spine: mix(0.15, 0.0, smooth(t / dr)),
            hip: [h, h],
            knee: [k, k],
            shoulder: herm(1.3, 0.0, 0.1, 0.0, dr, t),
            elbow: mix(0.4, 0.25, smooth(t / dr)),
            feet_planted: [true, true],
            ..Default::default()
        }
    });
}

const Y_LAND: f64 = 0.80;
const TH_S: f64 = -3.30;
const W_S: f64 = -8.0;
const PHI_S: f64 = 1.8;
const PHI_S_RATE: f64 = -3.0;
const FOOT_AHEAD: f64 = 0.30; // feet land this far beyond the pelvis in the travel direction

/// Back handspring (optionally entered with a back step).
pub fn back_handspring(with_back_step: bool) -> AcroClip {
    let mut seq = Seq {
        poses: vec![],
        phases: vec![],
        feet: vec![],
        t0: 0.0,
    };
    let start_v = if with_back_step {
        back_step(&mut seq, 0.35);
        [-0.6, 0.0]
    } else {
        [0.0, 0.0]
    };
    let s0 = Carry {
        p: [0.0, STAND_Y],
        v: start_v,
        th: 0.0,
        w: 0.0,
        phi: 0.0,
        phi_rate: 0.0,
    };

    // ---- plan the key states backwards from the hand plant ----
    let (tc, t1, ts) = (0.36, 0.26, 0.15);
    let (th_to, w_to) = (-0.75, -6.0);
    let (th_c, w_c, phi_c) = (-2.55, -6.5, 2.95);
    let (th_s, w_s, phi_s, phi_s_rate) = (TH_S, W_S, PHI_S, PHI_S_RATE);
    let p_to = [-0.32, 0.82];
    let hand_z = -1.15;
    let hand = [hand_z, PALM_H];
    let pel_at = |th: f64, phi: f64, spine: f64| {
        let o = arm_fk([0.0, 0.0], th, spine, phi, 0.05);
        [hand[0] - o[0], hand[1] - o[1]]
    };
    let p_c = pel_at(th_c, phi_c, -0.35);
    let v_to = [
        (p_c[0] - p_to[0]) / t1,
        (p_c[1] - p_to[1] + 0.5 * G * t1 * t1) / t1,
    ];
    let to = Carry {
        p: p_to,
        v: v_to,
        th: th_to,
        w: w_to,
        phi: 2.85,
        phi_rate: 3.0,
    };
    crouch(&mut seq, tc, s0, to, -0.2);

    // flight 1: back to the hands, legs extend and arch
    let leg0 = {
        let (h, k) = leg_ik(p_to, [0.0, ANKLE_H], th_to);
        (h, k)
    };
    seq.push("flight_to_hands", t1, [0.0, 0.0], &|t| {
        let (p, _) = ballistic(&to, t);
        let s = smooth(t / t1);
        Pose {
            pelvis: p,
            pitch: herm(th_to, w_to, th_c, w_c, t1, t),
            spine: mix(-0.2, -0.35, s),
            hip: [mix(leg0.0, -0.3, s); 2],
            knee: [mix(leg0.1, 0.1, s); 2],
            shoulder: herm(to.phi, to.phi_rate, phi_c, 0.0, t1, t),
            elbow: 0.05,
            airborne: true,
            ..Default::default()
        }
    });
    // hand support: rotate over the hands, shoulders block (extend) to pop the hips up
    let phi_t = |t: f64| herm(phi_c, 0.0, phi_s, phi_s_rate, ts, t);
    let th_t = |t: f64| herm(th_c, w_c, th_s, w_s, ts, t);
    let sp_t = |t: f64| mix(-0.35, -0.1, smooth(t / ts));
    seq.push("hand_support", ts, [0.0, 0.0], &|t| {
        let s = smooth(t / ts);
        Pose {
            pelvis: pel_at(th_t(t), phi_t(t), sp_t(t)),
            pitch: th_t(t),
            spine: sp_t(t),
            hip: [mix(-0.3, 0.1, s); 2],
            knee: [0.1; 2],
            shoulder: phi_t(t),
            elbow: 0.05,
            hands_planted: true,
            ..Default::default()
        }
    });
    let h = 1e-6;
    let ps = pel_at(th_t(ts), phi_t(ts), sp_t(ts));
    let pm = pel_at(th_t(ts - h), phi_t(ts - h), sp_t(ts - h));
    let off = Carry {
        p: ps,
        v: [(ps[0] - pm[0]) / h, (ps[1] - pm[1]) / h],
        th: th_s,
        w: w_s,
        phi: phi_s,
        phi_rate: phi_s_rate,
    };
    // flight 2: snap-down to the feet
    let t2 = flight_time(&off, Y_LAND);
    let th_l = -2.0 * PI + 0.30;
    let w_l = -3.0;
    let (pl, vl) = ballistic(&off, t2);
    let foot_z = pl[0] - FOOT_AHEAD;
    let leg_l = leg_ik(pl, [foot_z, ANKLE_H], th_l);
    seq.push("snap_down", t2, [foot_z, foot_z], &|t| {
        let (p, _) = ballistic(&off, t);
        let s = smooth(t / t2);
        let th = herm(th_s, w_s, th_l, w_l, t2, t);
        Pose {
            pelvis: p,
            pitch: th,
            spine: mix(-0.1, 0.1, s),
            hip: [mix(0.1, leg_l.0, s); 2],
            knee: [mix(0.1, leg_l.1, s); 2],
            shoulder: herm(phi_s, phi_s_rate, 1.6, 0.0, t2, t),
            elbow: mix(0.05, 0.4, s),
            airborne: true,
            ..Default::default()
        }
    });
    land_and_recover(
        &mut seq,
        Carry {
            p: pl,
            v: vl,
            th: th_l,
            w: w_l,
            phi: 1.6,
            phi_rate: 0.0,
        },
        foot_z,
    );
    AcroClip {
        name: if with_back_step {
            "back_step_handspring"
        } else {
            "back_handspring"
        },
        poses: seq.poses,
        phases: seq.phases,
        feet_z: seq.feet,
        hand_z,
    }
}

/// Standing tuck backflip (optionally entered with a back step).
pub fn backflip(with_back_step: bool) -> AcroClip {
    let mut seq = Seq {
        poses: vec![],
        phases: vec![],
        feet: vec![],
        t0: 0.0,
    };
    let start_v = if with_back_step {
        back_step(&mut seq, 0.35);
        [-0.6, 0.0]
    } else {
        [0.0, 0.0]
    };
    let s0 = Carry {
        p: [0.0, STAND_Y],
        v: start_v,
        th: 0.0,
        w: 0.0,
        phi: 0.0,
        phi_rate: 0.0,
    };
    let to = Carry {
        p: [-0.12, 0.92],
        v: [-0.9, 3.7],
        th: -0.30,
        w: -5.0,
        phi: 3.0,
        phi_rate: 2.0,
    };
    crouch(&mut seq, 0.34, s0, to, -0.1);
    let tf = flight_time(&to, Y_LAND);
    let (pl, vl) = ballistic(&to, tf);
    let foot_z = pl[0] - 0.15;
    let th_l = -2.0 * PI + 0.30;
    let w_l = -3.0;
    let th_m = -PI - 0.2;
    let w_m = -11.0;
    let tm = 0.5 * tf;
    let leg0 = leg_ik(to.p, [0.0, ANKLE_H], to.th);
    let leg_l = leg_ik(pl, [foot_z, ANKLE_H], th_l);
    seq.push("tuck_flight", tf, [foot_z, foot_z], &|t| {
        let (p, _) = ballistic(&to, t);
        let th = if t < tm {
            herm(to.th, to.w, th_m, w_m, tm, t)
        } else {
            herm(th_m, w_m, th_l, w_l, tf - tm, t - tm)
        };
        // tuck in, hold, open for landing
        let tuck = smooth(t / (0.35 * tf)) * (1.0 - smooth((t - 0.6 * tf) / (0.3 * tf)));
        let open = smooth((t - 0.6 * tf) / (0.4 * tf));
        let hip = mix(mix(leg0.0, 2.0, smooth(t / (0.35 * tf))), leg_l.0, open);
        let knee = mix(mix(leg0.1, 2.2, tuck.max(open * 0.0)), leg_l.1, open);
        Pose {
            pelvis: p,
            pitch: th,
            spine: mix(-0.1, 0.4, tuck),
            hip: [hip; 2],
            knee: [knee; 2],
            shoulder: if t < tm {
                herm(to.phi, to.phi_rate, 1.2, 0.0, tm, t)
            } else {
                herm(1.2, 0.0, 1.6, 0.0, tf - tm, t - tm)
            },
            elbow: mix(0.05, 1.2, tuck),
            airborne: true,
            ..Default::default()
        }
    });
    land_and_recover(
        &mut seq,
        Carry {
            p: pl,
            v: vl,
            th: th_l,
            w: w_l,
            phi: 1.6,
            phi_rate: 0.0,
        },
        foot_z,
    );
    AcroClip {
        name: if with_back_step {
            "back_step_backflip"
        } else {
            "backflip"
        },
        poses: seq.poses,
        phases: seq.phases,
        feet_z: seq.feet,
        hand_z: f64::NAN,
    }
}
