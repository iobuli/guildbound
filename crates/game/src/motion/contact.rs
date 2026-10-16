//! Contact layer, applied after springs: foot IK onto the ground, pelvis drop to the lower
//! foot, COP lean (Kwon 2020: torso over COP + h·a/g), squash, look-at, hand IK.
//! Pure pose maths; the ground is queried through a callback so this stays testable.
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
    let max = (a + b - 1e-4).max(min);
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
}

impl Default for FootPlan {
    fn default() -> Self {
        Self {
            correction: 0.0,
            normal: Vec3::Y,
            align: 0.0,
        }
    }
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

    // 1. Ground probes under each animated foot.
    let foot_w = unit_weight(request.foot_ik);
    let mut feet = [FootPlan::default(); 2];
    if foot_w > 1e-3 {
        let floor = model.translation.y;
        let probe_y = world0[bones.pelvis].translation.y;
        for (side, plan) in feet.iter_mut().enumerate() {
            let ankle = world0[bones.foot[side]].translation;
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
            let ball = world0[bones.ball[side]].translation;
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
            };
        }
    }

    // 2. Pelvis: smoothed drop to the lowest foot (never above animated) + squash drop.
    let need = foot_w * feet.iter().map(|f| f.correction).fold(0.0_f32, f32::min);
    state.pelvis_drop += (need - state.pelvis_drop) * smoothing(params.pelvis_rate, dt);
    state.pelvis_drop = state.pelvis_drop.clamp(-MAX_DROP * scale, 0.0);
    let squash = if request.squash.is_finite() {
        request.squash.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let drop = state.pelvis_drop + squash.min(0.0) * SQUASH_DROP * scale;
    if drop.abs() > 1e-6 {
        let parent = frame.parent(&world0, bones.pelvis);
        let local = parent.rotation.inverse() * (Vec3::Y * drop);
        let s = parent.scale;
        if s.x.abs() > 1e-6 && s.y.abs() > 1e-6 && s.z.abs() > 1e-6 {
            pose.pelvis += local / s;
        }
    }

    // 3. Legs: ankles to the planned heights, knees toward the animated knees, feet keep their
    //    animated world orientation, partially aligned to the ground.
    if foot_w > 1e-3 {
        let world1 = frame.world(pose);
        for (side, plan) in feet.iter().enumerate() {
            let (thigh, calf, foot) = (bones.thigh[side], bones.calf[side], bones.foot[side]);
            let planted = world0[foot].translation + Vec3::Y * plan.correction;
            let target = world1[foot].translation.lerp(planted, foot_w);
            let pole = world1[calf].translation - world1[thigh].translation;
            let Some(delta) = frame.limb(pose, &world1, [thigh, calf, foot], target, pole, forward)
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

    // 6. Hands: two-bone IK on the arms toward weighted world targets, elbows as animated.
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
        let pole = world[chain[1]].translation - world[chain[0]].translation;
        let fallback = model.rotation * Vec3::new(0.0, -0.5, -1.0);
        frame.limb(pose, &world, chain, goal, pole, fallback);
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
        mut each: impl FnMut(&ContactState),
    ) -> Pose {
        let (skeleton, bones) = skeleton();
        let mut pose = skeleton.rest_pose();
        for _ in 0..frames {
            pose = skeleton.rest_pose();
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
        // Ground a metre up: beyond reach_up plus the fade band, ignored.
        let mut state = ContactState::default();
        let pose = run(
            10,
            &model,
            &ContactRequest::default(),
            &mut flat(1.0),
            &mut state,
            |_| {},
        );
        for (a, b) in pose.rot.iter().zip(&rest.rot) {
            let angle = crate::motion::secondary::rotation_vector(*a * b.inverse()).length();
            assert!(angle < 1e-4, "moved by {angle}");
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
            squash: f32::INFINITY,
            look_at: Some(Vec3::NAN),
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
