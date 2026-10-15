//! AnimAbility implementations for the Animation modules (contract: anim_ability.rs, Rust Expert).
//! Gait (Effort), strike flow (Style, marks authored strike frames), flare (Additive, Style),
//! acrobatics (Replace, Style). Pure: same ctx + tick -> same output.
//! Rotations are model-space hinge deltas unless a rest rig is attached (`with_rig`), in which
//! case they are converted with anim_rig::Rig::to_local_post (`local' = local * q`).
//! omega/alpha: central differences of the emitted rotations over +-1 tick (rad/s, rad/s^2,
//! expressed in the same frame as `rot`).

use crate::anim_ability::*;
use crate::{anim_acro, anim_flare, anim_flow, anim_rig};
use std::sync::Arc;

type Q4 = [f32; 4];

fn qmul(a: Q4, b: Q4) -> Q4 {
    [a[3]*b[0]+a[0]*b[3]+a[1]*b[2]-a[2]*b[1], a[3]*b[1]-a[0]*b[2]+a[1]*b[3]+a[2]*b[0],
     a[3]*b[2]+a[0]*b[1]-a[1]*b[0]+a[2]*b[3], a[3]*b[3]-a[0]*b[0]-a[1]*b[1]-a[2]*b[2]]
}
fn qconj(q: Q4) -> Q4 { [-q[0], -q[1], -q[2], q[3]] }
/// Fixed sim step; derivatives never use a variable dt.
const DT: f64 = 1.0 / 120.0;
type Q64 = [f64; 4];
fn up(q: Q4) -> Q64 { q.map(|x| x as f64) }
fn qmul64(a: Q64, b: Q64) -> Q64 {
    [a[3]*b[0]+a[0]*b[3]+a[1]*b[2]-a[2]*b[1], a[3]*b[1]-a[0]*b[2]+a[1]*b[3]+a[2]*b[0],
     a[3]*b[2]+a[0]*b[1]-a[1]*b[0]+a[2]*b[3], a[3]*b[3]-a[0]*b[0]-a[1]*b[1]-a[2]*b[2]]
}
fn qconj64(q: Q64) -> Q64 { [-q[0], -q[1], -q[2], q[3]] }
/// Rotation vector of q (axis * angle), shortest arc, in f64.
fn qlog64(q: Q64) -> [f64; 3] {
    let q = if q[3] < 0.0 { q.map(|x| -x) } else { q };
    let s = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
    if s < 1e-12 { return [2.0 * q[0], 2.0 * q[1], 2.0 * q[2]]; }
    let a = 2.0 * s.atan2(q[3]) / s;
    [q[0] * a, q[1] * a, q[2] * a]
}
/// omega(t) = log(q(t+1) q(t-1)^-1) / (2 dt);  alpha = (log(q(t+1)q(t)^-1) - log(q(t)q(t-1)^-1)) / dt^2.
/// Computed in f64 at the fixed 1/120 s step, rounded to f32 once at the end.
fn derivs(qm: Q4, q0: Q4, qp: Q4) -> (Vec3, Vec3) {
    let (qm, q0, qp) = (up(qm), up(q0), up(qp));
    let w = qlog64(qmul64(qp, qconj64(qm)));
    let (a, b) = (qlog64(qmul64(qp, qconj64(q0))), qlog64(qmul64(q0, qconj64(qm))));
    (Vec3([(w[0] / (2.0 * DT)) as f32, (w[1] / (2.0 * DT)) as f32, (w[2] / (2.0 * DT)) as f32]),
     Vec3([((a[0] - b[0]) / (DT * DT)) as f32, ((a[1] - b[1]) / (DT * DT)) as f32, ((a[2] - b[2]) / (DT * DT)) as f32]))
}

/// Optional rest-rig conversion shared by all abilities.
#[derive(Clone, Default)]
pub struct Frame { pub rig: Option<Arc<anim_rig::Rig>> }
impl Frame {
    fn conv(&self, bone: BoneIdx, q: Q4) -> Q4 {
        match &self.rig {
            Some(r) if (bone as usize) < r.joints.len() && r.joints[bone as usize] != usize::MAX => {
                let o = r.to_local_post(bone as usize, [q[0] as f64, q[1] as f64, q[2] as f64, q[3] as f64]);
                [o[0] as f32, o[1] as f32, o[2] as f32, o[3] as f32]
            }
            _ => q,
        }
    }
    /// Sample `raw` at t-1, t, t+1 and build BoneTargets. Neighbours are matched by bone id;
    /// a bone missing on one side (layer appearing/ending, clip edge) gets a one-sided
    /// derivative (mirror of the other side: alpha = 0), never a silent zero omega.
    fn targets(&self, raw: &dyn Fn(i64) -> Vec<(BoneIdx, Q4)>, tick: Tick, _dt: f32) -> Vec<(BoneIdx, BoneTarget)> {
        let t = tick as i64;
        let (m, c, p) = (raw(t - 1), raw(t), raw(t + 1));
        let find = |v: &Vec<(BoneIdx, Q4)>, b: BoneIdx| v.iter().find(|e| e.0 == b).map(|e| e.1);
        c.iter().map(|&(b, q)| {
            let q0 = self.conv(b, q);
            let qm = find(&m, b).map(|x| self.conv(b, x));
            let qp = find(&p, b).map(|x| self.conv(b, x));
            let mirror = |o: Q4| qmul(q0, qmul(qconj(o), q0));
            let (qm, qp) = match (qm, qp) {
                (Some(a), Some(z)) => (a, z),
                (Some(a), None) => (a, mirror(a)),
                (None, Some(z)) => (mirror(z), z),
                (None, None) => (q0, q0),
            };
            let (omega, alpha) = derivs(qm, q0, qp);
            (b, BoneTarget { rot: Quat(q0), omega, alpha })
        }).collect()
    }
}

fn axis_angle(axis: [f32; 3], a: f32) -> Q4 { let (s, c) = (0.5 * a).sin_cos(); [axis[0] * s, axis[1] * s, axis[2] * s, c] }
const X: [f32; 3] = [1.0, 0.0, 0.0];

// ------------------------------------------------------------------ gait (Effort)
/// Leg swing + root motion from ctx.gait_phase / speed / yaw_rate. `stride_fn(speed) -> (stride m, cadence Hz)`;
/// in the game wire it to `|v| anim_gait::stride_cadence(v, ref_speed, ref_stride)`.
pub struct GaitAbility { pub id: AbilityId, pub stride_fn: fn(f32) -> (f32, f32), pub leg_len: f32, pub frame: Frame }
pub fn default_stride(v: f32) -> (f32, f32) { let s = (0.6 + 0.37 * v).min(3.2); (s, if s > 0.0 { v / s } else { 0.0 }) }
impl GaitAbility {
    fn hips(&self, speed: f32, phase: f32) -> Vec<(BoneIdx, Q4)> {
        let (stride, _) = (self.stride_fn)(speed.max(0.0));
        let amp = ((0.5 * stride * 0.5) / self.leg_len).clamp(0.0, 0.95).asin(); // half-stride per leg
        let s = (std::f32::consts::TAU * phase).sin();
        let knee = |u: f32| 1.2 * amp * (std::f32::consts::TAU * u).sin().max(0.0);
        vec![(anim_acro::bone::L_THIGH, axis_angle(X, amp * s)), (anim_acro::bone::L_CALF, axis_angle(X, -knee(phase))),
             (anim_acro::bone::R_THIGH, axis_angle(X, -amp * s)), (anim_acro::bone::R_CALF, axis_angle(X, -knee(phase + 0.5)))]
    }
}
impl AnimAbility for GaitAbility {
    fn id(&self) -> AbilityId { self.id }
    fn name(&self) -> &'static str { "gait" }
    fn targets(&self, ctx: &AnimCtx, tick: Tick) -> AnimOutput {
        let (_, cad) = (self.stride_fn)(ctx.speed.max(0.0));
        let dphase = cad * ctx.dt;
        let raw = |t: i64| self.hips(ctx.speed, (ctx.gait_phase + dphase * (t - tick as i64) as f32).rem_euclid(1.0));
        let mask = vec![anim_acro::bone::L_THIGH, anim_acro::bone::L_CALF, anim_acro::bone::R_THIGH, anim_acro::bone::R_CALF];
        AnimOutput {
            bones: self.frame.targets(&raw, tick, ctx.dt),
            root: Some(RootMotion { dpos: Vec3([0.0, 0.0, ctx.speed * ctx.dt]), dyaw: ctx.yaw_rate * ctx.dt }),
            mode: LayerMode::Masked(mask), weight: 1.0, priority: Priority::Effort, strike_window: false,
        }
    }
}

// ------------------------------------------------------------------ strike flow (clip-driven)
/// One authored strike clip in a combo. Emits no bones (the clip drives them); flags authored
/// active frames and exposes the cancel window the scheduler cuts in. Tick is local to the strike.
pub struct StrikeFlowAbility { pub id: AbilityId, pub phases: anim_flow::StrikePhases, pub hit_confirmed: bool }
impl AnimAbility for StrikeFlowAbility {
    fn id(&self) -> AbilityId { self.id }
    fn name(&self) -> &'static str { "strike_flow" }
    fn targets(&self, _ctx: &AnimCtx, tick: Tick) -> AnimOutput {
        let p = self.phases;
        AnimOutput { bones: vec![], root: None, mode: LayerMode::Additive, weight: 1.0, priority: Priority::Style,
                     strike_window: tick >= p.strike && tick < p.recovery }
    }
    fn cancel_window(&self, _tick: Tick) -> Option<(Tick, Tick)> {
        let w = anim_flow::cancel_window(self.phases, self.hit_confirmed);
        Some((w.open, w.close))
    }
    fn finished(&self, tick: Tick) -> bool { tick >= self.phases.len }
}

// ------------------------------------------------------------------ flare (Additive, Style)
/// Strike twist + follow-through (from the strike tick) + lean from ctx.accel[2] + run swing from ctx.
pub struct FlareAbility {
    pub id: AbilityId, pub strike: anim_flare::StrikeFlare, pub active_ticks: u32,
    pub follow_swing: f32, pub overshoot: f32, pub settle_s: f32,
    pub lean_gain: f32, pub lean_max: f32, pub run_max_speed: f32, pub frame: Frame,
}
impl FlareAbility {
    pub fn new(id: AbilityId) -> Self {
        Self { id, strike: anim_flare::StrikeFlare::default(), active_ticks: 8, follow_swing: 0.5, overshoot: 0.15, settle_s: 0.3,
               lean_gain: 0.6, lean_max: 0.35, run_max_speed: 8.0, frame: Frame::default() }
    }
    pub fn raw(&self, ctx: &AnimCtx, t: i64) -> Vec<(BoneIdx, Q4)> {
        let tu = t.max(0) as u32;
        let since = (t - self.strike.strike_tick as i64).max(0) as u32;
        // gait phase advanced consistently with speed for neighbouring ticks (derivatives)
        let mut layers = vec![anim_flare::strike_layer(&self.strike, tu),
                              anim_flare::lean_layer(ctx.accel.0[2], self.lean_gain, self.lean_max)];
        if t >= self.strike.strike_tick as i64 {
            layers.push(anim_flare::follow_through_layer(since, self.follow_swing, self.overshoot, self.settle_s, self.strike.side));
        }
        if ctx.speed > 0.0 { layers.push(anim_flare::run_layer(ctx.gait_phase, ctx.speed, self.run_max_speed)); }
        let mut out = anim_flare::compose(&layers);
        // keep a fixed bone set so derivatives line up across ticks
        for b in [anim_flare::bone::SPINE2, anim_flare::bone::R_UPPERARM, anim_flare::bone::R_FOREARM, anim_flare::bone::L_UPPERARM, anim_flare::bone::L_FOREARM] {
            if let Err(i) = out.binary_search_by_key(&b, |e| e.0) { out.insert(i, (b, [0.0, 0.0, 0.0, 1.0])); }
        }
        out
    }
}
impl AnimAbility for FlareAbility {
    fn id(&self) -> AbilityId { self.id }
    fn name(&self) -> &'static str { "flare" }
    fn targets(&self, ctx: &AnimCtx, tick: Tick) -> AnimOutput {
        let s = self.strike.strike_tick as u64;
        AnimOutput { bones: self.frame.targets(&|t| self.raw(ctx, t), tick, ctx.dt), root: None, mode: LayerMode::Additive,
                     weight: 1.0, priority: Priority::Style, strike_window: tick >= s && tick < s + self.active_ticks as u64 }
    }
    fn finished(&self, tick: Tick) -> bool {
        // whole ticks: settle time rounded once to ticks at 120 Hz
        let settle = (self.settle_s * 1.5 * 120.0).round() as u64;
        tick > self.strike.strike_tick as u64 + settle + self.strike.recover_ticks as u64
    }
}

// ------------------------------------------------------------------ acrobatics (Replace, Style)
pub struct AcroAbility { pub id: AbilityId, pub clip: anim_acro::AcroClip, pub frame: Frame }
impl AcroAbility {
    pub fn handspring(id: AbilityId, back_step: bool) -> Self { Self { id, clip: anim_acro::back_handspring(back_step), frame: Frame::default() } }
    pub fn backflip(id: AbilityId, back_step: bool) -> Self { Self { id, clip: anim_acro::backflip(back_step), frame: Frame::default() } }
    fn raw(&self, t: i64) -> Vec<(BoneIdx, Q4)> {
        // Out of range -> empty, so clip edges get one-sided derivatives instead of a held pose.
        if t < 0 || t >= self.clip.poses.len() as i64 { return vec![]; }
        let i = t as usize;
        anim_acro::AcroClip::bone_angles(&self.clip.poses[i]).iter()
            .map(|&(b, a)| (b, axis_angle(X, a as f32))).collect()
    }
}
impl AnimAbility for AcroAbility {
    fn id(&self) -> AbilityId { self.id }
    fn name(&self) -> &'static str { self.clip.name }
    fn targets(&self, ctx: &AnimCtx, tick: Tick) -> AnimOutput {
        let n = self.clip.poses.len();
        if n == 0 { return AnimOutput { bones: vec![], root: None, mode: LayerMode::Replace, weight: 0.0,
                                        priority: Priority::Style, strike_window: false }; }
        let i = (tick as usize).min(n - 1);
        let prev = i.saturating_sub(1);
        let (a, b) = (self.clip.poses[prev].pelvis, self.clip.poses[i].pelvis);
        let root = if i == 0 { RootMotion::default() } else {
            RootMotion { dpos: Vec3([0.0, (b[1] - a[1]) as f32, (b[0] - a[0]) as f32]), dyaw: 0.0 }
        };
        AnimOutput { bones: self.frame.targets(&|t| self.raw(t), tick, ctx.dt), root: Some(root), mode: LayerMode::Replace,
                     weight: 1.0, priority: Priority::Style, strike_window: false }
    }
    fn finished(&self, tick: Tick) -> bool { tick as usize >= self.clip.poses.len() }
}

// ------------------------------------------------------------------ frame convention for the motor system
/// `BoneTarget.omega/alpha` are the joint's relative rates in the PARENT's local frame.
/// The single motor write converts them each fixed step with the parent body's ACTUAL world
/// rotation `rp` and world angular velocity `wp`:
///   omega_rel_world = Rp * omega_l
///   alpha_rel_world = Rp * alpha_l + wp x (Rp * omega_l)
/// and compares against (child_omega - parent_omega) from the bodies. Torque is applied
/// +tau to the child, -tau to the parent.
pub fn rates_to_world_rel(rp: Q4, wp: [f32; 3], omega_l: [f32; 3], alpha_l: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let rot = |v: [f32; 3]| { let p = qmul(qmul(rp, [v[0], v[1], v[2], 0.0]), qconj(rp)); [p[0], p[1], p[2]] };
    let w = rot(omega_l);
    let a = rot(alpha_l);
    let c = [wp[1] * w[2] - wp[2] * w[1], wp[2] * w[0] - wp[0] * w[2], wp[0] * w[1] - wp[1] * w[0]];
    (w, [a[0] + c[0], a[1] + c[1], a[2] + c[2]])
}

// ------------------------------------------------------------------ capsule chains (65 bones -> 13 capsules)
/// Skin joints strictly below `from` down to and including `to`, ordered parent -> child.
/// None if `to` is not a descendant of `from`.
pub fn chain(rig: &anim_rig::Rig, from: usize, to: usize) -> Option<Vec<usize>> {
    let mut node_joint = vec![usize::MAX; rig.nodes.len()];
    for (j, &n) in rig.joints.iter().enumerate() { if n != usize::MAX { node_joint[n] = j; } }
    let stop = *rig.joints.get(from)?;
    let mut n = *rig.joints.get(to)?;
    let mut out = Vec::new();
    while n != stop {
        if node_joint[n] != usize::MAX { out.push(node_joint[n]); }
        let p = rig.nodes[n].parent;
        if p < 0 { return None; }
        n = p as usize;
    }
    out.reverse();
    Some(out)
}

/// Composite target for a capsule joint: ordered product of the chain's FULL local targets
/// (parent -> child, left to right), with omega/alpha taken from that product at t-1, t, t+1
/// in f64 at 1/120 s. Differentiating the product directly gives every cross term exactly.
/// `local(t, joint)` = final local rotation from `apply_edits` at tick t.
pub fn chain_target(chain: &[usize], local: &dyn Fn(i64, usize) -> Q4, tick: Tick) -> BoneTarget {
    let prod = |t: i64| chain.iter().fold([0.0f32, 0.0, 0.0, 1.0], |acc, &j| qmul(acc, local(t, j)));
    let t = tick as i64;
    let (qm, q0, qp) = (prod(t - 1), prod(t), prod(t + 1));
    let (omega, alpha) = derivs(qm, q0, qp);
    BoneTarget { rot: Quat(q0), omega, alpha }
}
