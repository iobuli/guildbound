//! Contact layer, applied after springs: foot IK onto the ground (optionally in a narrow stance,
//! feet drawn toward the body's mid-plane), pelvis drop (only as far as a planted foot cannot
//! reach lower ground by straightening its leg; feet in the air don't count), step bounce, foot
//! IK onto world targets (climb holds), COP lean (Kwon 2020: torso over COP + h·a/g), squash,
//! look-at, hand IK. Pure pose maths; the ground is queried through a callback so this stays
//! testable.
use super::secondary::clean_quat;
use super::{Bones, ContactRequest, Pose, Skeleton};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Ground {
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct ContactParams {
    /// Lean gain (1 = physical h·a/g).
    pub lean: f32,
    /// Max lean angle (rad).
    pub max_lean: f32,
    /// Foot IK reach above / below the animated foot (m).
    pub reach_up: f32,
    pub reach_down: f32,
    /// Smoothing rate for pelvis drop (1/s).
    pub pelvis_rate: f32,
}

impl Default for ContactParams {
    fn default() -> Self {
        Self {
            lean: 1.0,
            max_lean: 0.35,
            reach_up: 0.45,
            reach_down: 0.35,
            pelvis_rate: 14.0,
        }
    }
}

/// Smoothed per-character contact state.
#[derive(Component, Clone, Debug, Default)]
pub struct ContactState {
    pub pelvis_drop: f32,
    pub lean: Vec3,
    pub look: Quat,
    /// Last `ContactRequest.root_ease` seen (None until the first frame).
    pub root_ease: Option<f32>,
    /// Smoothed aim pitch (rad, + up) and the world axis it bends about.
    pub aim_pitch: f32,
    pub aim_axis: Vec3,
}

const GRAVITY: f32 = 9.81;
/// Smoothing rates (1/s) for the lean vector and the look rotation.
const LEAN_RATE: f32 = 8.0;
const LOOK_RATE: f32 = 10.0;
/// Look-at limits relative to the body (rad): ±70° yaw, ±40° pitch.
pub const MAX_LOOK_YAW: f32 = 70.0 * std::f32::consts::PI / 180.0;
pub const MAX_LOOK_PITCH: f32 = 40.0 * std::f32::consts::PI / 180.0;
/// Share of the look rotation taken by the neck (the head takes the rest).
const NECK_SHARE: f32 = 0.4;
/// Lean share per spine bone (spine_01, spine_02, spine_03).
const LEAN_SHARE: [f32; 3] = [0.6, 0.4, 0.0];
/// Aim pitch: share per spine bone, limit (rad), smoothing rate (1/s).
const AIM_SHARE: [f32; 3] = [0.3, 0.35, 0.35];
pub const MAX_AIM_PITCH: f32 = 0.9;
const AIM_RATE: f32 = 10.0;
/// Squash curl / arch share per spine bone.
const CURL_SHARE: [f32; 3] = [0.3, 0.4, 0.3];
/// Pelvis drop (m) per unit of negative squash.
const SQUASH_DROP: f32 = 0.3;
/// Forward spine curl (rad) per unit of negative squash; backward arch per unit of stretch.
const SQUASH_CURL: f32 = 0.9;
const SQUASH_ARCH: f32 = 0.5;
/// How much of the ground slope a planted foot takes on.
const FOOT_ALIGN: f32 = 0.6;
/// Animated ankle lift (m) over which slope alignment fades out (swing feet stay animated).
const ALIGN_FADE: f32 = 0.15;
/// Band (m) past the reach limits over which foot IK fades out instead of popping.
const REACH_FADE: f32 = 0.12;
/// Ground normals flatter than this (y component) are not footing (walls, overhangs).
const MIN_GROUND_NORMAL_Y: f32 = 0.25;
/// Largest pelvis drop (m at model scale 1).
const MAX_DROP: f32 = 0.6;
/// How far (m) the pelvis may briefly sit above the animation while the root eases down a step.
const MAX_RAISE: f32 = 0.05;
/// Deepest step bounce dip (m at model scale 1), and the difference in foot heights (m) over
/// which the bounce fades out.
const MAX_BOB: f32 = 0.15;
const UNEVEN: f32 = 0.12;
/// Share of its length a bent leg may straighten to before the pelvis has to come down for a
/// lower foot, and the ankle lift (m) over which a foot stops counting as planted.
const LEG_REACH: f32 = 0.97;
const PLANT_LIFT: f32 = 0.1;
/// Limbs never straighten past this share of their length: a locked knee or elbow snaps.
const MAX_EXTENSION: f32 = 0.999;
/// How hard (m) knees lean toward the body's front and elbows toward out-down-back, so a near-straight
/// limb keeps a stable bend side instead of flipping with noise.
const KNEE_BIAS: f32 = 0.04;
const ELBOW_BIAS: f32 = 0.04;
/// Knees of feet on world targets (climb holds) bend toward the body's front turned out by this
/// much (rad), so they stay off the wall the feet stand on.
const KNEE_SPLAY: f32 = 0.7;
/// Natural elbow direction in model space for the left arm (x mirrored for the right).
const ELBOW_OUT: Vec3 = Vec3::new(0.5, -0.6, -0.6);

/// Analytic two-bone IK (from `skeleton_ik::solve`): returns the middle joint position.
pub fn two_bone(root: Vec3, target: Vec3, pole: Vec3, lengths: [f32; 2]) -> Vec3 {
    solve(root, target, pole, pole, lengths).0
}

/// Two-bone solve returning (middle joint, end joint). The end lands on `target` when it is
/// reachable, otherwise as close as the chain allows along root→target. `pole` picks the bend
/// side (projected off the root→target axis); `fallback` biases it when `pole` is ~on-axis.
fn solve(root: Vec3, target: Vec3, pole: Vec3, fallback: Vec3, lengths: [f32; 2]) -> (Vec3, Vec3) {
    let clean = |l: f32| if l.is_finite() { l.max(1e-4) } else { 1e-4 };
    let (a, b) = (clean(lengths[0]), clean(lengths[1]));
    if !root.is_finite() {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    let target = if target.is_finite() { target } else { root };
    let offset = target - root;
    let axis = offset.normalize_or(Vec3::NEG_Y);
    let min = (a - b).abs() + 1e-4;
    let max = ((a + b) * MAX_EXTENSION).max(min);
    let distance = offset.length().clamp(min, max);
    let project = |v: Vec3| {
        let v = if v.is_finite() { v } else { Vec3::ZERO };
        v - axis * v.dot(axis)
    };
    // A small fallback bias keeps the bend plane continuous as the pole collapses onto the axis.
    let mut side = project(pole) + project(fallback).normalize_or_zero() * 0.01;
    if side.length_squared() < 1e-10 {
        side = project(if axis.x.abs() < 0.8 { Vec3::X } else { Vec3::Z });
    }
    let side = side.normalize_or(Vec3::Z);
    let along = (a * a - b * b + distance * distance) / (2.0 * distance);
    let height = (a * a - along * along).max(0.0).sqrt();
    (root + axis * along + side * height, root + axis * distance)
}

/// Bend-side pole for a limb from `root` through `mid` toward `target`: the animated bend,
/// with any part pointing against `natural` removed (knees never fold backwards, elbows never
/// inwards), plus `natural · bias` fading out as the bend grows, so a near-straight limb keeps
/// its side while a clearly bent one follows the animation exactly.
fn bend_pole(root: Vec3, mid: Vec3, target: Vec3, natural: Vec3, bias: f32) -> Vec3 {
    let axis = (target - root).normalize_or(Vec3::NEG_Y);
    let off = |v: Vec3| v - axis * v.dot(axis);
    let natural = off(natural).normalize_or_zero();
    let mut bend = off(mid - root);
    let against = bend.dot(natural);
    if against < 0.0 {
        bend -= natural * against;
    }
    let fade = (1.0 - bend.length() / (3.0 * bias).max(1e-6)).clamp(0.0, 1.0);
    bend + natural * (bias * fade)
}

/// Model-space FK lifted into world space, plus helpers to edit local rotations by world deltas.
struct Frame<'a> {
    skeleton: &'a Skeleton,
    pelvis: usize,
    model: Transform,
}

impl Frame<'_> {
    fn world(&self, pose: &Pose) -> Vec<Transform> {
        self.skeleton
            .fk(pose, self.pelvis)
            .into_iter()
            .map(|t| self.model.mul_transform(t))
            .collect()
    }
    /// World transform of `bone`'s parent (the model × base for the skeleton root).
    fn parent(&self, world: &[Transform], bone: usize) -> Transform {
        match self.skeleton.parent[bone] {
            Some(p) => world[p],
            None => self.model.mul_transform(self.skeleton.base),
        }
    }
    /// Set `bone`'s local rotation so its world rotation becomes `rotation`, given a parent
    /// world rotation.
    fn set_world(pose: &mut Pose, bone: usize, parent: Quat, rotation: Quat) {
        let fallback = pose.rot[bone];
        pose.rot[bone] = clean_quat(parent.inverse() * rotation, fallback);
    }
    /// Pre-multiply `bone`'s world rotation by `delta` (its descendants follow).
    fn rotate(&self, pose: &mut Pose, world: &[Transform], bone: usize, delta: Quat) {
        let parent = self.parent(world, bone).rotation;
        Self::set_world(pose, bone, parent, delta * world[bone].rotation);
    }
    /// Rotate `chain = [upper, lower, end]` so `end` reaches `target` with the middle joint bent
    /// toward `pole`. Returns the world delta applied to `lower` and everything below it.
    fn limb(
        &self,
        pose: &mut Pose,
        world: &[Transform],
        chain: [usize; 3],
        target: Vec3,
        pole: Vec3,
        fallback: Vec3,
    ) -> Option<Quat> {
        let [upper, lower, end] = chain;
        let (p0, p1, p2) = (
            world[upper].translation,
            world[lower].translation,
            world[end].translation,
        );
        let (a, b) = (p0.distance(p1), p1.distance(p2));
        if !(a > 1e-4 && b > 1e-4 && target.is_finite() && pole.is_finite()) {
            return None;
        }
        let (knee, tip) = solve(p0, target, pole, fallback, [a, b]);
        let arc = |from: Vec3, to: Vec3| match (from.try_normalize(), to.try_normalize()) {
            (Some(f), Some(t)) => Quat::from_rotation_arc(f, t),
            _ => Quat::IDENTITY,
        };
        // Swing the upper bone so the middle joint lands on the solved knee.
        let r1 = arc(p1 - p0, knee - p0);
        let upper_parent = self.parent(world, upper).rotation;
        Self::set_world(pose, upper, upper_parent, r1 * world[upper].rotation);
        // Then the lower bone so the end lands on the solved tip.
        let knee_now = p0 + r1 * (p1 - p0);
        let end_now = p0 + r1 * (p2 - p0);
        let r2 = arc(end_now - knee_now, tip - knee_now);
        let lower_parent = r1 * self.parent(world, lower).rotation;
        Self::set_world(pose, lower, lower_parent, r2 * r1 * world[lower].rotation);
        Some((r2 * r1).normalize())
    }
}

fn unit_weight(w: f32) -> f32 {
    if w.is_finite() {
        w.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Exponential smoothing factor for `rate` (1/s) over `dt`.
fn smoothing(rate: f32, dt: f32) -> f32 {
    let rate = if rate.is_finite() { rate.max(0.0) } else { 0.0 };
    1.0 - (-rate * dt).exp()
}

/// Per-foot ground plan from the probes.
#[derive(Clone, Copy, Debug)]
struct FootPlan {
    /// Vertical ankle correction (m, world), already faded by reach.
    correction: f32,
    /// Ground normal and how much of it the foot takes (0..FOOT_ALIGN).
    normal: Vec3,
    align: f32,
    /// The animation's lift of this ankle above its planted height (m); 0 = planted.
    lift: f32,
    /// Ground was found under this foot.
    probed: bool,
    /// Horizontal shift (world) of the foot toward the body's mid-plane (narrow stance).
    shift: Vec3,
}

impl Default for FootPlan {
    fn default() -> Self {
        Self {
            correction: 0.0,
            normal: Vec3::Y,
            align: 0.0,
            lift: 0.0,
            probed: false,
            shift: Vec3::ZERO,
        }
    }
}

/// Horizontal shift (world) that brings `foot` within `stance` (m) of the body's mid-plane
/// through `mid` facing `left`; zero when `stance` is not a finite width (the animated stance).
pub fn stance_shift(foot: Vec3, mid: Vec3, left: Vec3, stance: f32) -> Vec3 {
    if !(stance.is_finite() && stance >= 0.0 && foot.is_finite() && mid.is_finite()) {
        return Vec3::ZERO;
    }
    let side = left.with_y(0.0).normalize_or_zero();
    let lateral = (foot - mid).dot(side);
    side * (lateral.clamp(-stance, stance) - lateral)
}

fn smoothstep01(x: f32) -> f32 {
    let x = if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 };
    x * x * (3.0 - 2.0 * x)
}

/// How far (m) the hip must come down so a leg of `cap` reach gets from `hip` to `target`; 0
/// when the leg reaches as it is.
fn reach_drop(hip: Vec3, target: Vec3, cap: f32) -> f32 {
    let h = Vec2::new(target.x - hip.x, target.z - hip.z).length();
    let room = (cap * cap - h * h).max(0.0).sqrt();
    let d = (hip.y - target.y) - room;
    if d.is_finite() { d.max(0.0) } else { 0.0 }
}

/// Rotate `pose` in place so its feet sit on the ground, hands reach their targets, the
/// torso leans with acceleration and the head looks toward `look_at`.
/// `model` = world transform of the model (skeleton base is applied on top of it).
pub fn apply(
    pose: &mut Pose,
    skeleton: &Skeleton,
    bones: &Bones,
    model: &Transform,
    request: &ContactRequest,
    ground: &mut dyn FnMut(Vec3) -> Option<Ground>,
    params: &ContactParams,
    state: &mut ContactState,
    dt: f32,
) {
    let n = skeleton.len();
    if n == 0 || pose.rot.len() != n || skeleton.parent.len() != n || skeleton.rest.len() != n {
        return;
    }
    if !(model.translation.is_finite() && model.rotation.is_finite() && model.scale.is_finite()) {
        return;
    }
    let dt = if dt.is_finite() {
        dt.clamp(0.0, 0.1)
    } else {
        0.0
    };
    if !state.pelvis_drop.is_finite() {
        state.pelvis_drop = 0.0;
    }
    if !state.lean.is_finite() {
        state.lean = Vec3::ZERO;
    }
    state.look = clean_quat(state.look, Quat::IDENTITY);
    for q in &mut pose.rot {
        *q = clean_quat(*q, Quat::IDENTITY);
    }
    if !pose.pelvis.is_finite() {
        pose.pelvis = Vec3::ZERO;
    }
    let original = pose.clone();
    let model = Transform {
        rotation: clean_quat(model.rotation, Quat::IDENTITY),
        ..*model
    };
    let frame = Frame {
        skeleton,
        pelvis: bones.pelvis,
        model,
    };
    let scale = model.scale.y.abs().max(1e-3);
    let forward = model.rotation * Vec3::Z;
    let left = model.rotation * Vec3::X;
    let rest = skeleton.fk(&skeleton.rest_pose(), bones.pelvis);
    let world0 = frame.world(pose);

    // 1. Ground probes under each animated foot (drawn toward the mid-plane in a narrow stance).
    let foot_w = unit_weight(request.foot_ik);
    let mut feet = [FootPlan::default(); 2];
    if foot_w > 1e-3 {
        let floor = model.translation.y;
        let mid = world0[bones.pelvis].translation;
        let probe_y = mid.y;
        for (side, plan) in feet.iter_mut().enumerate() {
            let shift = stance_shift(world0[bones.foot[side]].translation, mid, left, request.stance);
            plan.shift = shift;
            let ankle = world0[bones.foot[side]].translation + shift;
            let ankle_height = rest[bones.foot[side]].translation.y * scale;
            // The animation's own lift above its planted height is kept (swing feet stay up).
            let lift = (ankle.y - floor - ankle_height).max(0.0);
            let origin = Vec3::new(ankle.x, probe_y.max(ankle.y), ankle.z);
            let Some(hit) = ground(origin) else { continue };
            if !(hit.point.is_finite() && hit.normal.is_finite()) {
                continue;
            }
            let normal = hit.normal.normalize_or(Vec3::Y);
            if normal.y < MIN_GROUND_NORMAL_Y {
                continue;
            }
            let mut correction = hit.point.y + ankle_height + lift - ankle.y;
            // The toes must not sink either (stair nosings, rock lips): the end over higher ground
            // decides, so the foot rests on the ball with the heel hanging, as on real stairs.
            let ball = world0[bones.ball[side]].translation + shift;
            let ball_height = rest[bones.ball[side]].translation.y * scale;
            let ball_lift = (ball.y - floor - ball_height).max(0.0);
            if let Some(toe) = ground(Vec3::new(ball.x, probe_y.max(ball.y), ball.z))
                && toe.point.is_finite()
                && toe.normal.is_finite()
                && toe.normal.normalize_or(Vec3::Y).y >= MIN_GROUND_NORMAL_Y
            {
                correction = correction.max(toe.point.y + ball_height + ball_lift - ball.y);
            }
            let reach = if correction >= 0.0 {
                params.reach_up
            } else {
                params.reach_down
            };
            let reach = if reach.is_finite() {
                reach.max(0.0)
            } else {
                0.0
            };
            let excess = correction.abs() - reach;
            let fade = if excess <= 0.0 {
                1.0
            } else {
                (1.0 - excess / REACH_FADE).clamp(0.0, 1.0)
            };
            *plan = FootPlan {
                correction: correction * fade,
                normal,
                align: FOOT_ALIGN * fade * (1.0 - lift / ALIGN_FADE).clamp(0.0, 1.0),
                lift,
                probed: true,
                shift,
            };
        }
    }

    // 2. Pelvis: smoothed drop, only as far as a planted foot cannot reach its ground by
    //    straightening its leg, + squash drop + step bounce.
    //    The controller eases the drawn root over steps; undoing that motion here keeps the
    //    pelvis still in the world, so this is the only smoother of the visible body height
    //    (two in series made the body dip and rebound on stairs).
    let eased = state.root_ease.map_or(0.0, |prev| request.root_ease - prev);
    state.root_ease = request.root_ease.is_finite().then_some(request.root_ease);
    if eased.is_finite() && eased.abs() < MAX_DROP * scale {
        state.pelvis_drop -= eased;
    }
    let mut drop_needed = 0.0_f32;
    for (side, plan) in feet.iter().enumerate() {
        if !plan.probed || plan.correction >= 0.0 {
            continue;
        }
        let (thigh, calf, foot) = (bones.thigh[side], bones.calf[side], bones.foot[side]);
        let leg = (rest[calf].translation.distance(rest[thigh].translation)
            + rest[foot].translation.distance(rest[calf].translation))
            * scale;
        let hip = world0[thigh].translation;
        let ankle = world0[foot].translation + plan.shift;
        // A bent leg may straighten (to `LEG_REACH` of its length) before the body comes down;
        // a leg the animation already holds straighter keeps its own length.
        let cap = hip.distance(ankle).max(LEG_REACH * leg);
        let planted = 1.0 - smoothstep01(plan.lift / PLANT_LIFT);
        let d = reach_drop(hip, ankle + Vec3::Y * plan.correction, cap) * planted;
        drop_needed = drop_needed.max(d);
    }
    let need = -foot_w * drop_needed;
    state.pelvis_drop += (need - state.pelvis_drop) * smoothing(params.pelvis_rate, dt);
    state.pelvis_drop = state.pelvis_drop.clamp(-MAX_DROP * scale, MAX_RAISE * scale);
    let squash = if request.squash.is_finite() {
        request.squash.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    // Step bounce: only ever down, only while the feet are planted (their IK holds them), and
    // fading out where the feet stand at different heights (stairs get careful footing).
    let uneven = if feet[0].probed && feet[1].probed {
        (feet[0].correction - feet[1].correction).abs()
    } else {
        0.0
    };
    let bob = if request.bob.is_finite() {
        request.bob.clamp(-MAX_BOB, 0.0) * foot_w * scale * (1.0 - smoothstep01(uneven / (UNEVEN * scale)))
    } else {
        0.0
    };
    // A brief raise only stands while the feet are planted (their IK reaches down for it).
    let drop = state.pelvis_drop.min(0.0) * (1.0 - foot_w) + state.pelvis_drop * foot_w
        + squash.min(0.0) * SQUASH_DROP * scale
        + bob;
    if drop.abs() > 1e-6 {
        let parent = frame.parent(&world0, bones.pelvis);
        let local = parent.rotation.inverse() * (Vec3::Y * drop);
        let s = parent.scale;
        if s.x.abs() > 1e-6 && s.y.abs() > 1e-6 && s.z.abs() > 1e-6 {
            pose.pelvis += local / s;
        }
    }

    // 3. Legs: ankles to the planned heights, knees toward the animated knees (never toward the
    //    back of the body), feet keep their animated world orientation, partially aligned to the ground.
    if foot_w > 1e-3 {
        let world1 = frame.world(pose);
        for (side, plan) in feet.iter().enumerate() {
            let (thigh, calf, foot) = (bones.thigh[side], bones.calf[side], bones.foot[side]);
            let planted = world0[foot].translation + plan.shift + Vec3::Y * plan.correction;
            let target = world1[foot].translation.lerp(planted, foot_w);
            // Knees bend toward the body's front. Not the toes: in a running back-kick the foot
            // points down and back, which flipped the knee mid-swing.
            let natural = forward;
            let (hip, knee) = (world1[thigh].translation, world1[calf].translation);
            let pole = bend_pole(hip, knee, target, natural, KNEE_BIAS * scale);
            let Some(delta) = frame.limb(pose, &world1, [thigh, calf, foot], target, pole, natural)
            else {
                continue;
            };
            let chained = delta * world1[foot].rotation;
            let tilt = Quat::from_rotation_arc(Vec3::Y, plan.normal);
            let tilt = Quat::IDENTITY.slerp(tilt, plan.align);
            let wanted = chained.slerp(tilt * world0[foot].rotation, foot_w);
            let parent = delta * frame.parent(&world1, foot).rotation;
            Frame::set_world(pose, foot, parent, wanted);
        }
    }

    // 3b. Foot targets (climb holds): ankles to weighted world targets, knees toward the body's
    //     front turned out by `KNEE_SPLAY`, feet keep their animated world orientation.
    for side in 0..2 {
        let Some((target, weight)) = request.feet[side] else {
            continue;
        };
        let weight = unit_weight(weight);
        if weight < 1e-3 || !target.is_finite() {
            continue;
        }
        let world = frame.world(pose);
        let chain = [bones.thigh[side], bones.calf[side], bones.foot[side]];
        let goal = world[chain[2]].translation.lerp(target, weight);
        let out = if side == 0 { left } else { -left };
        let natural = forward * KNEE_SPLAY.cos() + out * KNEE_SPLAY.sin();
        let (hip, knee) = (world[chain[0]].translation, world[chain[1]].translation);
        let pole = bend_pole(hip, knee, goal, natural, KNEE_BIAS * scale);
        let Some(delta) = frame.limb(pose, &world, chain, goal, pole, natural) else {
            continue;
        };
        let parent = delta * frame.parent(&world, chain[2]).rotation;
        Frame::set_world(pose, chain[2], parent, world[chain[2]].rotation);
    }

    // 4. Torso: COP lean into the horizontal acceleration + squash curl / stretch arch.
    let accel = if request.accel.is_finite() {
        Vec3::new(request.accel.x, 0.0, request.accel.z)
    } else {
        Vec3::ZERO
    };
    let gain = if params.lean.is_finite() {
        params.lean
    } else {
        0.0
    };
    let max_lean = if params.max_lean.is_finite() {
        params.max_lean.clamp(0.0, 1.2)
    } else {
        0.0
    };
    let lean_target = (accel * (gain / GRAVITY)).clamp_length_max(max_lean);
    state.lean += (lean_target - state.lean) * smoothing(LEAN_RATE, dt);
    state.lean = state.lean.clamp_length_max(max_lean.max(1e-3));
    // Rotating about (Y × lean) tips the torso toward the lean vector by |lean| rad.
    let lean_axis = Vec3::Y.cross(state.lean);
    let curl = if squash < 0.0 {
        -squash * SQUASH_CURL
    } else {
        -squash * SQUASH_ARCH
    };
    for (k, &spine) in bones.spine.iter().enumerate() {
        // Rotating about the character's left (+X) pitches the chest forward.
        let rv = lean_axis * LEAN_SHARE[k] + left * (curl * CURL_SHARE[k]);
        if rv.length_squared() < 1e-12 {
            continue;
        }
        let world = frame.world(pose);
        frame.rotate(pose, &world, spine, Quat::from_scaled_axis(rv));
    }

    // 4b. Aim: bend the spine about the horizontal axis across the aim so the chest (and the
    //     arms holding a bow or crossbow) follow its pitch.
    let (aim_target, axis) = match request.aim.filter(|d| d.is_finite()) {
        Some(d) if Vec2::new(d.x, d.z).length_squared() > 1e-6 => {
            let flat = Vec3::new(d.x, 0.0, d.z);
            let pitch = d.y.atan2(flat.length()).clamp(-MAX_AIM_PITCH, MAX_AIM_PITCH);
            (pitch, flat.normalize().cross(Vec3::Y))
        }
        _ => (0.0, state.aim_axis),
    };
    if !state.aim_pitch.is_finite() {
        state.aim_pitch = 0.0;
    }
    if !state.aim_axis.is_finite() {
        state.aim_axis = Vec3::ZERO;
    }
    if axis.is_finite() && axis.length_squared() > 0.5 {
        state.aim_axis = axis;
    }
    state.aim_pitch += (aim_target - state.aim_pitch) * smoothing(AIM_RATE, dt);
    if state.aim_pitch.abs() > 1e-4 && state.aim_axis.length_squared() > 0.5 {
        for (k, &spine) in bones.spine.iter().enumerate() {
            let world = frame.world(pose);
            let bend = Quat::from_axis_angle(state.aim_axis.normalize(), state.aim_pitch * AIM_SHARE[k]);
            frame.rotate(pose, &world, spine, bend);
        }
    }

    // 5. Look-at: neck + head turn toward the target, clamped relative to the body.
    let world = frame.world(pose);
    let mut look_target = Quat::IDENTITY;
    if let Some(point) = request.look_at.filter(|p| p.is_finite()) {
        let to = model.rotation.inverse() * (point - world[bones.head].translation);
        if to.length_squared() > 0.05 * 0.05 {
            let yaw = to.x.atan2(to.z).clamp(-MAX_LOOK_YAW, MAX_LOOK_YAW);
            let pitch =
                to.y.atan2(Vec2::new(to.x, to.z).length())
                    .clamp(-MAX_LOOK_PITCH, MAX_LOOK_PITCH);
            let desired = Vec3::new(
                yaw.sin() * pitch.cos(),
                pitch.sin(),
                yaw.cos() * pitch.cos(),
            );
            if let Some(current) = head_forward(&rest, &world, bones, &model).try_normalize() {
                look_target = Quat::from_rotation_arc(current, desired);
            }
        }
    }
    state.look = state
        .look
        .slerp(look_target, smoothing(LOOK_RATE, dt))
        .normalize();
    if state.look.w.abs() < 0.99999 {
        let delta = (model.rotation * state.look * model.rotation.inverse()).normalize();
        let neck = Quat::IDENTITY.slerp(delta, NECK_SHARE);
        frame.rotate(pose, &world, bones.neck, neck);
        let world = frame.world(pose);
        frame.rotate(pose, &world, bones.head, delta * neck.inverse());
    }

    // 6. Hands: two-bone IK on the arms toward weighted world targets, elbows as animated but
    //    never folded inwards.
    for side in 0..2 {
        let Some((target, weight)) = request.hands[side] else {
            continue;
        };
        let weight = unit_weight(weight);
        if weight < 1e-3 || !target.is_finite() {
            continue;
        }
        let world = frame.world(pose);
        let chain = [bones.upperarm[side], bones.lowerarm[side], bones.hand[side]];
        let goal = world[chain[2]].translation.lerp(target, weight);
        let out = if side == 0 { ELBOW_OUT } else { ELBOW_OUT * Vec3::new(-1.0, 1.0, 1.0) };
        let natural = model.rotation * out;
        let (shoulder, elbow) = (world[chain[0]].translation, world[chain[1]].translation);
        let pole = bend_pole(shoulder, elbow, goal, natural, ELBOW_BIAS * scale);
        frame.limb(pose, &world, chain, goal, pole, natural);
    }

    // Never hand back a broken pose.
    let healthy = pose.pelvis.is_finite() && pose.rot.iter().all(|q| q.is_finite());
    if !healthy {
        *pose = original;
        *state = ContactState::default();
        return;
    }
    for q in &mut pose.rot {
        *q = q.normalize();
    }
}

/// The head's current facing direction in MODEL space (rest head forward = model +Z).
fn head_forward(rest: &[Transform], world: &[Transform], bones: &Bones, model: &Transform) -> Vec3 {
    let local_forward = rest[bones.head].rotation.inverse() * Vec3::Z;
    model.rotation.inverse() * (world[bones.head].rotation * local_forward)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::f32::consts::{FRAC_PI_2, PI};

    /// (name, parent, model-space joint position). Proportions roughly match the UAL mannequin.
    const JOINTS: &[(&str, Option<&str>, [f32; 3])] = &[
        ("root", None, [0.0, 0.0, 0.0]),
        ("pelvis", Some("root"), [0.0, 0.95, 0.0]),
        ("spine_01", Some("pelvis"), [0.0, 1.05, 0.0]),
        ("spine_02", Some("spine_01"), [0.0, 1.2, 0.0]),
        ("spine_03", Some("spine_02"), [0.0, 1.35, 0.0]),
        ("neck_01", Some("spine_03"), [0.0, 1.48, 0.0]),
        ("Head", Some("neck_01"), [0.0, 1.57, 0.0]),
        ("clavicle_l", Some("spine_03"), [0.05, 1.45, 0.0]),
        ("upperarm_l", Some("clavicle_l"), [0.18, 1.45, 0.0]),
        ("lowerarm_l", Some("upperarm_l"), [0.46, 1.45, -0.01]),
        ("hand_l", Some("lowerarm_l"), [0.72, 1.45, 0.0]),
        ("index_01_l", Some("hand_l"), [0.8, 1.45, 0.03]),
        ("clavicle_r", Some("spine_03"), [-0.05, 1.45, 0.0]),
        ("upperarm_r", Some("clavicle_r"), [-0.18, 1.45, 0.0]),
        ("lowerarm_r", Some("upperarm_r"), [-0.46, 1.45, -0.01]),
        ("hand_r", Some("lowerarm_r"), [-0.72, 1.45, 0.0]),
        ("index_01_r", Some("hand_r"), [-0.8, 1.45, 0.03]),
        ("thigh_l", Some("pelvis"), [0.1, 0.9, 0.0]),
        ("calf_l", Some("thigh_l"), [0.1, 0.48, 0.015]),
        ("foot_l", Some("calf_l"), [0.1, 0.08, 0.0]),
        ("ball_l", Some("foot_l"), [0.1, 0.02, 0.12]),
        ("thigh_r", Some("pelvis"), [-0.1, 0.9, 0.0]),
        ("calf_r", Some("thigh_r"), [-0.1, 0.48, 0.015]),
        ("foot_r", Some("calf_r"), [-0.1, 0.08, 0.0]),
        ("ball_r", Some("foot_r"), [-0.1, 0.02, 0.12]),
    ];
    const ANKLE_HEIGHT: f32 = 0.08;

    /// Synthetic skeleton with deliberately odd bone orientations and a rotated armature base,
    /// so nothing silently relies on identity rest rotations.
    pub(crate) fn skeleton() -> (Skeleton, Bones) {
        let base = Transform::from_rotation(Quat::from_rotation_x(-FRAC_PI_2))
            .with_translation(Vec3::new(0.0, 0.0, 0.01));
        let index: HashMap<String, usize> = JOINTS
            .iter()
            .enumerate()
            .map(|(i, j)| (j.0.to_string(), i))
            .collect();
        let orient = |i: usize| {
            let f = i as f32;
            Quat::from_euler(EulerRot::XYZ, f * 0.7, f * 1.3 + 0.2, f * 0.4)
        };
        let model_space = |i: usize| {
            Transform::from_translation(Vec3::from(JOINTS[i].2)).with_rotation(orient(i))
        };
        let mut parent = Vec::new();
        let mut rest = Vec::new();
        for (i, (_, p, _)) in JOINTS.iter().enumerate() {
            let p = p.map(|name| index[name]);
            let parent_model = p.map_or(base, model_space);
            let child = model_space(i);
            let inv = parent_model.rotation.inverse();
            rest.push(Transform {
                translation: inv * (child.translation - parent_model.translation),
                rotation: (inv * child.rotation).normalize(),
                scale: Vec3::ONE,
            });
            parent.push(p);
        }
        let skeleton = Skeleton {
            names: JOINTS.iter().map(|j| j.0.to_string()).collect(),
            parent,
            rest,
            base,
            index,
        };
        let bones = Bones::new(&skeleton).expect("synthetic skeleton has every runtime bone");
        (skeleton, bones)
    }

    fn world(skeleton: &Skeleton, bones: &Bones, model: &Transform, pose: &Pose) -> Vec<Transform> {
        skeleton
            .fk(pose, bones.pelvis)
            .into_iter()
            .map(|t| model.mul_transform(t))
            .collect()
    }

    fn flat(height: f32) -> impl FnMut(Vec3) -> Option<Ground> {
        move |o: Vec3| {
            Some(Ground {
                point: Vec3::new(o.x, height, o.z),
                normal: Vec3::Y,
            })
        }
    }

    fn nothing(_: Vec3) -> Option<Ground> {
        None
    }

    /// Run `frames` frames of contact on the rest pose (state persists); returns the last pose.
    fn run(
        frames: usize,
        model: &Transform,
        request: &ContactRequest,
        ground: &mut dyn FnMut(Vec3) -> Option<Ground>,
        state: &mut ContactState,
        each: impl FnMut(&ContactState),
    ) -> Pose {
        let (skeleton, _) = skeleton();
        run_from(&skeleton.rest_pose(), frames, model, request, ground, state, each)
    }

    /// `run` on an animated `base` pose instead of rest.
    fn run_from(
        base: &Pose,
        frames: usize,
        model: &Transform,
        request: &ContactRequest,
        ground: &mut dyn FnMut(Vec3) -> Option<Ground>,
        state: &mut ContactState,
        mut each: impl FnMut(&ContactState),
    ) -> Pose {
        let (skeleton, bones) = skeleton();
        let mut pose = base.clone();
        for _ in 0..frames {
            pose = base.clone();
            apply(
                &mut pose,
                &skeleton,
                &bones,
                model,
                request,
                ground,
                &ContactParams::default(),
                state,
                1.0 / 60.0,
            );
            each(state);
        }
        pose
    }

    fn assert_finite(pose: &Pose) {
        assert!(pose.pelvis.is_finite());
        for q in &pose.rot {
            assert!(
                q.is_finite() && (q.length() - 1.0).abs() < 1e-3,
                "bad quat {q}"
            );
        }
    }

    #[test]
    fn two_bone_hits_reachable_targets_and_clamps_unreachable_ones() {
        let root = Vec3::Y;
        let lengths = [0.45, 0.42];
        for target in [Vec3::new(0.1, 0.3, 0.2), Vec3::new(-0.2, 0.5, 0.0)] {
            let (knee, tip) = solve(root, target, Vec3::Z, Vec3::Z, lengths);
            assert!(tip.distance(target) < 1e-4);
            assert!((knee.distance(root) - 0.45).abs() < 1e-4);
            assert!((tip.distance(knee) - 0.42).abs() < 1e-4);
            assert_eq!(two_bone(root, target, Vec3::Z, lengths), knee);
        }
        for target in [
            Vec3::ZERO,
            Vec3::new(10.0, -10.0, 2.0),
            root,
            Vec3::new(0.00001, 0.1, 0.0),
            Vec3::NAN,
        ] {
            let (knee, tip) = solve(root, target, Vec3::Z, Vec3::Z, lengths);
            assert!(knee.is_finite() && tip.is_finite());
            assert!((knee.distance(root) - 0.45).abs() < 1e-3);
            assert!((tip.distance(knee) - 0.42).abs() < 1e-3);
            assert!(
                (knee - root).dot(Vec3::Z) >= -1e-4,
                "knee stays on the pole side"
            );
        }
        // A degenerate pole still yields a finite bend.
        assert!(two_bone(root, Vec3::new(0.0, 0.3, 0.0), Vec3::ZERO, lengths).is_finite());
    }

    #[test]
    fn foot_ik_keeps_ankles_on_flat_ground() {
        let (skeleton, bones) = skeleton();
        let model = Transform::from_xyz(3.0, 0.5, -2.0).with_rotation(Quat::from_rotation_y(0.7));
        let mut state = ContactState::default();
        let pose = run(
            30,
            &model,
            &ContactRequest::default(),
            &mut flat(0.5),
            &mut state,
            |s| {
                assert!(s.pelvis_drop <= 0.0 && s.pelvis_drop > -1e-4);
            },
        );
        assert_finite(&pose);
        let w = world(&skeleton, &bones, &model, &pose);
        for side in 0..2 {
            let y = w[bones.foot[side]].translation.y;
            assert!(
                (y - (0.5 + ANKLE_HEIGHT)).abs() < 2e-3,
                "ankle {side} at {y}"
            );
        }
    }

    #[test]
    fn foot_ik_lifts_a_foot_onto_raised_ground() {
        let (skeleton, bones) = skeleton();
        let model = Transform::IDENTITY;
        // A 12 cm step under the left foot (+X).
        let mut ground = |o: Vec3| {
            Some(Ground {
                point: Vec3::new(o.x, if o.x > 0.0 { 0.12 } else { 0.0 }, o.z),
                normal: Vec3::Y,
            })
        };
        let mut state = ContactState::default();
        let pose = run(
            60,
            &model,
            &ContactRequest::default(),
            &mut ground,
            &mut state,
            |s| {
                assert!(
                    s.pelvis_drop <= 0.0 && s.pelvis_drop > -1e-4,
                    "never raised, no drop"
                );
            },
        );
        assert_finite(&pose);
        let w = world(&skeleton, &bones, &model, &pose);
        let left = w[bones.foot[0]].translation;
        let right = w[bones.foot[1]].translation;
        assert!(
            (left.y - (0.12 + ANKLE_HEIGHT)).abs() < 2e-3,
            "left ankle {left}"
        );
        assert!((right.y - ANKLE_HEIGHT).abs() < 2e-3, "right ankle {right}");
        // The knee bends forward (+Z), toward the animated knee.
        assert!(w[bones.calf[0]].translation.z > 0.05);
        // Bone lengths are untouched.
        let rest = world(&skeleton, &bones, &model, &skeleton.rest_pose());
        let len = |w: &[Transform], a: usize, b: usize| w[a].translation.distance(w[b].translation);
        for (a, b) in [
            (bones.thigh[0], bones.calf[0]),
            (bones.calf[0], bones.foot[0]),
        ] {
            assert!((len(&w, a, b) - len(&rest, a, b)).abs() < 1e-4);
        }
    }

    #[test]
    fn pelvis_drops_smoothly_to_the_lower_foot() {
        let (skeleton, bones) = skeleton();
        let model = Transform::IDENTITY;
        // Ground 15 cm lower under the right foot (−X).
        let mut ground = |o: Vec3| {
            Some(Ground {
                point: Vec3::new(o.x, if o.x < 0.0 { -0.15 } else { 0.0 }, o.z),
                normal: Vec3::Y,
            })
        };
        let mut state = ContactState::default();
        let mut drops = Vec::new();
        let pose = run(
            120,
            &model,
            &ContactRequest::default(),
            &mut ground,
            &mut state,
            |s| drops.push(s.pelvis_drop),
        );
        assert_finite(&pose);
        assert!(drops.iter().all(|d| *d <= 0.0), "never above animated");
        assert!(drops[0] > -0.1, "first frame is smoothed, got {}", drops[0]);
        assert!(drops.windows(2).all(|w| w[1] <= w[0] + 1e-6), "monotonic");
        let last = *drops.last().unwrap();
        assert!(
            (last + 0.15).abs() < 0.01,
            "settles at the lower foot, got {last}"
        );
        let w = world(&skeleton, &bones, &model, &pose);
        assert!((w[bones.foot[1]].translation.y - (ANKLE_HEIGHT - 0.15)).abs() < 3e-3);
        assert!((w[bones.foot[0]].translation.y - ANKLE_HEIGHT).abs() < 3e-3);
    }

    /// Rest pose crouched by `depth` with both feet held on flat ground at y = 0 by the IK:
    /// an animated pose with bent knees.
    fn crouched(depth: f32) -> Pose {
        let (skeleton, bones) = skeleton();
        let mut pose = skeleton.rest_pose();
        let world = skeleton.fk(&pose, bones.pelvis);
        let parent = match skeleton.parent[bones.pelvis] {
            Some(p) => world[p],
            None => skeleton.base,
        };
        pose.pelvis += parent.rotation.inverse() * (Vec3::NEG_Y * depth);
        let mut state = ContactState::default();
        apply(
            &mut pose,
            &skeleton,
            &bones,
            &Transform::IDENTITY,
            &ContactRequest::default(),
            &mut flat(0.0),
            &ContactParams::default(),
            &mut state,
            1.0 / 60.0,
        );
        pose
    }

    /// Lift the right foot by swinging the thigh forward (`angle` rad about the body's left).
    fn right_leg_raised(angle: f32) -> Pose {
        let (skeleton, bones) = skeleton();
        let mut pose = skeleton.rest_pose();
        let world = skeleton.fk(&pose, bones.pelvis);
        let thigh = bones.thigh[1];
        let parent = world[skeleton.parent[thigh].unwrap()].rotation;
        let swing = Quat::from_rotation_x(-angle);
        pose.rot[thigh] = (parent.inverse() * swing * parent * pose.rot[thigh]).normalize();
        pose
    }

    #[test]
    fn bent_legs_straighten_before_the_pelvis_drops() {
        let (skeleton, bones) = skeleton();
        let base = crouched(0.1);
        let w0 = world(&skeleton, &bones, &Transform::IDENTITY, &base);
        assert!((w0[bones.foot[1]].translation.y - ANKLE_HEIGHT).abs() < 2e-3, "base feet on the ground");
        // Ground 15 cm lower under the right foot (−X).
        let mut ground = |o: Vec3| {
            Some(Ground {
                point: Vec3::new(o.x, if o.x < 0.0 { -0.15 } else { 0.0 }, o.z),
                normal: Vec3::Y,
            })
        };
        let mut state = ContactState::default();
        let pose = run_from(&base, 120, &Transform::IDENTITY, &ContactRequest::default(), &mut ground, &mut state, |_| {});
        assert_finite(&pose);
        let drop = state.pelvis_drop;
        assert!(drop < -0.02 && drop > -0.12, "the leg takes most of the step, the pelvis the rest: {drop}");
        let w = world(&skeleton, &bones, &Transform::IDENTITY, &pose);
        assert!((w[bones.foot[1]].translation.y - (ANKLE_HEIGHT - 0.15)).abs() < 3e-3, "lower ankle lands");
        assert!((w[bones.foot[0]].translation.y - ANKLE_HEIGHT).abs() < 3e-3, "upper ankle stays");
    }

    #[test]
    fn a_foot_in_the_air_does_not_pull_the_pelvis_down() {
        let (skeleton, bones) = skeleton();
        let base = right_leg_raised(0.7);
        let w0 = world(&skeleton, &bones, &Transform::IDENTITY, &base);
        assert!(w0[bones.foot[1]].translation.y > ANKLE_HEIGHT + PLANT_LIFT + 0.03, "foot swung up");
        // The swing foot passes over ground 20 cm lower.
        let mut ground = |o: Vec3| {
            Some(Ground {
                point: Vec3::new(o.x, if o.x < 0.0 { -0.2 } else { 0.0 }, o.z),
                normal: Vec3::Y,
            })
        };
        let mut state = ContactState::default();
        let pose = run_from(&base, 60, &Transform::IDENTITY, &ContactRequest::default(), &mut ground, &mut state, |s| {
            assert!(s.pelvis_drop > -1e-3, "no drop for a swing foot, got {}", s.pelvis_drop);
        });
        assert_finite(&pose);
    }

    #[test]
    fn bounce_is_full_on_flat_ground_and_fades_on_a_step() {
        let (skeleton, bones) = skeleton();
        let pelvis_y = |ground: &mut dyn FnMut(Vec3) -> Option<Ground>, bob: f32| {
            let request = ContactRequest {
                bob,
                ..default()
            };
            let mut state = ContactState::default();
            let pose = run(30, &Transform::IDENTITY, &request, ground, &mut state, |_| {});
            world(&skeleton, &bones, &Transform::IDENTITY, &pose)[bones.pelvis].translation.y
        };
        let flat_dip = pelvis_y(&mut flat(0.0), -0.04) - pelvis_y(&mut flat(0.0), 0.0);
        assert!((flat_dip + 0.04).abs() < 2e-3, "full dip on flat ground: {flat_dip}");
        let mut step = |o: Vec3| {
            Some(Ground {
                point: Vec3::new(o.x, if o.x > 0.0 { 0.25 } else { 0.0 }, o.z),
                normal: Vec3::Y,
            })
        };
        let with = pelvis_y(&mut step, -0.04);
        let without = pelvis_y(&mut step, 0.0);
        assert!((with - without).abs() < 2e-3, "no dip with the feet a step apart: {}", with - without);
    }

    #[test]
    fn foot_ik_off_or_out_of_reach_leaves_the_pose() {
        let (skeleton, bones) = skeleton();
        let model = Transform::IDENTITY;
        let rest = skeleton.rest_pose();
        let off = ContactRequest {
            foot_ik: 0.0,
            ..default()
        };
        let mut state = ContactState::default();
        let pose = run(10, &model, &off, &mut flat(0.3), &mut state, |_| {});
        let w = world(&skeleton, &bones, &model, &pose);
        assert!((w[bones.foot[0]].translation.y - ANKLE_HEIGHT).abs() < 1e-4);
        // Ground a metre up: beyond reach_up plus the fade band, ignored. The legs only take the
        // `MAX_EXTENSION` cap (the rest legs stand at 99.93 % of their length).
        let mut state = ContactState::default();
        let pose = run(
            10,
            &model,
            &ContactRequest::default(),
            &mut flat(1.0),
            &mut state,
            |_| {},
        );
        let legs = [bones.thigh, bones.calf, bones.foot].concat();
        for (i, (a, b)) in pose.rot.iter().zip(&rest.rot).enumerate() {
            let angle = crate::motion::secondary::rotation_vector(*a * b.inverse()).length();
            let limit = if legs.contains(&i) { 0.02 } else { 1e-4 };
            assert!(angle < limit, "bone {i} moved by {angle}");
        }
        let w = world(&skeleton, &bones, &model, &pose);
        for foot in bones.foot {
            assert!((w[foot].translation.y - ANKLE_HEIGHT).abs() < 1e-3, "ankle pulled toward the ground");
        }
        assert_eq!(state.pelvis_drop, 0.0);
    }

    #[test]
    fn look_at_is_clamped_relative_to_the_body() {
        let (skeleton, bones) = skeleton();
        let model = Transform::from_rotation(Quat::from_rotation_y(1.1));
        let rest_model = skeleton.fk(&skeleton.rest_pose(), bones.pelvis);
        let head_dir = |pose: &Pose| {
            let w = world(&skeleton, &bones, &model, pose);
            head_forward(&rest_model, &w, &bones, &model).normalize()
        };
        let at = |local: Vec3| model.transform_point(local);
        let cases = [
            // Behind: yaw clamps to 70°.
            (at(Vec3::new(0.3, 1.57, -5.0)), MAX_LOOK_YAW, None),
            // High above: pitch clamps to 40°.
            (at(Vec3::new(0.0, 30.0, 1.0)), 0.0, Some(MAX_LOOK_PITCH)),
            // Inside the limits: reached exactly.
            (
                at(Vec3::new(-2.0, 1.57, 12f32.sqrt())),
                -30f32.to_radians(),
                Some(0.0),
            ),
        ];
        for (point, yaw, pitch) in cases {
            let request = ContactRequest {
                foot_ik: 0.0,
                look_at: Some(point),
                ..default()
            };
            let mut state = ContactState::default();
            let pose = run(90, &model, &request, &mut nothing, &mut state, |_| {});
            assert_finite(&pose);
            let d = head_dir(&pose);
            let got_yaw = d.x.atan2(d.z);
            assert!((got_yaw - yaw).abs() < 0.03, "yaw {got_yaw} want {yaw}");
            if let Some(pitch) = pitch {
                let got = d.y.asin();
                assert!((got - pitch).abs() < 0.03, "pitch {got} want {pitch}");
            }
        }
        // Released: eases back to the animated head.
        let mut request = ContactRequest {
            look_at: Some(at(Vec3::new(5.0, 1.57, 0.0))),
            ..default()
        };
        let mut state = ContactState::default();
        run(60, &model, &request, &mut nothing, &mut state, |_| {});
        request.look_at = None;
        let pose = run(120, &model, &request, &mut nothing, &mut state, |_| {});
        assert!(head_dir(&pose).angle_between(Vec3::Z) < 0.01);
    }

    #[test]
    fn aim_bends_the_spine_to_the_aims_pitch_and_eases_back() {
        let (skeleton, bones) = skeleton();
        let model = Transform::IDENTITY;
        let up = 30f32.to_radians();
        let mut request = ContactRequest {
            foot_ik: 0.0,
            aim: Some(Vec3::new(up.cos(), up.sin(), 0.0)),
            ..default()
        };
        let mut state = ContactState::default();
        let pose = run(120, &model, &request, &mut nothing, &mut state, |_| {});
        assert!((state.aim_pitch - up).abs() < 0.01, "{}", state.aim_pitch);
        let w = world(&skeleton, &bones, &model, &pose);
        // Aiming up along +X the torso leans back, away from the target.
        assert!(w[bones.head].translation.x < -0.05, "{}", w[bones.head].translation);
        request.aim = Some(Vec3::new(0.0, -1.0, 0.0).lerp(Vec3::X, 0.01));
        run(120, &model, &request, &mut nothing, &mut state, |_| {});
        assert!(state.aim_pitch >= -MAX_AIM_PITCH - 1e-4, "clamped");
        request.aim = None;
        run(200, &model, &request, &mut nothing, &mut state, |_| {});
        assert!(state.aim_pitch.abs() < 0.01, "eases back");
    }

    #[test]
    fn lean_tips_the_chest_into_acceleration_and_clamps() {
        let (skeleton, bones) = skeleton();
        let model = Transform::IDENTITY;
        let mut request = ContactRequest {
            foot_ik: 0.0,
            accel: Vec3::new(0.0, 0.0, 9.81 * 0.2),
            ..default()
        };
        let mut state = ContactState::default();
        let pose = run(120, &model, &request, &mut nothing, &mut state, |_| {});
        let w = world(&skeleton, &bones, &model, &pose);
        assert!(w[bones.head].translation.z > 0.05, "chest leans forward");
        assert!((state.lean.length() - 0.2).abs() < 0.01);
        request.accel = Vec3::new(1000.0, 50.0, 0.0);
        let mut state = ContactState::default();
        run(120, &model, &request, &mut nothing, &mut state, |_| {});
        assert!(state.lean.length() <= ContactParams::default().max_lean + 1e-4);
        assert!(state.lean.x > 0.0 && state.lean.y == 0.0);
    }

    #[test]
    fn squash_drops_and_curls_with_feet_planted() {
        let (skeleton, bones) = skeleton();
        let model = Transform::IDENTITY;
        let request = ContactRequest {
            squash: -0.3,
            ..default()
        };
        let mut state = ContactState::default();
        let pose = run(1, &model, &request, &mut flat(0.0), &mut state, |_| {});
        let w = world(&skeleton, &bones, &model, &pose);
        assert!(w[bones.pelvis].translation.y < 0.9, "pelvis drops");
        assert!(w[bones.head].translation.z > 0.05, "spine curls forward");
        assert!(
            (w[bones.foot[0]].translation.y - ANKLE_HEIGHT).abs() < 2e-3,
            "feet stay"
        );
    }

    #[test]
    fn hand_ik_reaches_weighted_targets() {
        let (skeleton, bones) = skeleton();
        let model = Transform::from_xyz(1.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(-0.4));
        let target = model.transform_point(Vec3::new(0.3, 1.2, 0.35));
        let rest = world(&skeleton, &bones, &model, &skeleton.rest_pose());
        for weight in [1.0, 0.5] {
            let request = ContactRequest {
                foot_ik: 0.0,
                hands: [Some((target, weight)), None],
                ..default()
            };
            let mut state = ContactState::default();
            let pose = run(1, &model, &request, &mut nothing, &mut state, |_| {});
            let w = world(&skeleton, &bones, &model, &pose);
            let want = rest[bones.hand[0]].translation.lerp(target, weight);
            assert!(w[bones.hand[0]].translation.distance(want) < 1e-3);
            let right = w[bones.hand[1]].translation;
            assert!(rest[bones.hand[1]].translation.distance(right) < 1e-5);
        }
    }

    #[test]
    fn garbage_inputs_stay_finite() {
        let (skeleton, bones) = skeleton();
        let request = ContactRequest {
            foot_ik: f32::NAN,
            hands: [Some((Vec3::NAN, 1.0)), Some((Vec3::ONE, f32::NAN))],
            accel: Vec3::NAN,
            bob: f32::NEG_INFINITY,
            squash: f32::INFINITY,
            look_at: Some(Vec3::NAN),
            root_ease: f32::NAN,
            feet: [Some((Vec3::NAN, 1.0)), Some((Vec3::ONE, f32::NAN))],
            stance: f32::NAN,
            aim: Some(Vec3::NAN),
            grips: [None, None],
        };
        let mut garbage = |_: Vec3| {
            Some(Ground {
                point: Vec3::NAN,
                normal: Vec3::ZERO,
            })
        };
        let mut state = ContactState {
            pelvis_drop: f32::NAN,
            lean: Vec3::INFINITY,
            look: Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
            root_ease: Some(f32::NAN),
            aim_pitch: f32::NAN,
            aim_axis: Vec3::NAN,
        };
        for dt in [f32::NAN, -1.0, 0.0, 1e9, 1.0 / 60.0] {
            let mut pose = skeleton.rest_pose();
            pose.rot[bones.spine[1]] = Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0);
            apply(
                &mut pose,
                &skeleton,
                &bones,
                &Transform::from_rotation(Quat::from_rotation_y(PI)),
                &request,
                &mut garbage,
                &ContactParams::default(),
                &mut state,
                dt,
            );
            assert_finite(&pose);
            assert!(state.pelvis_drop.is_finite() && state.lean.is_finite());
            assert!(state.look.is_finite());
        }
    }
}
