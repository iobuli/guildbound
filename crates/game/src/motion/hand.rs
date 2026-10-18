//! Hand rig: fingers that close on what the hand holds.
//!
//! A `Grip` (on `ContactRequest.grips`) names a world-space shape: a `Cylinder` (weapon grip,
//! lever, bar, door handle), a `Sphere` (door knob) or a `Flat` surface (pushing a door with an
//! open hand). `compose` calls this module twice around `contact::apply`:
//! 1. `wrist_targets` (before the arm IK): turns the palm toward the shape (fingers wrap around a
//!    cylinder's axis, face a knob's centre, lie flat on a plane, plus the grip's `twist` about the
//!    palm normal) and places the wrist so the palm surface touches the shape. The result replaces
//!    that hand's `ContactRequest.hands` target, so the existing arm IK reaches it.
//! 2. `close` (after the arm IK): sets the hand's world rotation from step 1 and closes every
//!    finger from open: the curl parameter `c` (0 = open, 1 = full fist, per-joint limits) is
//!    bisected until a finger point would come within `FINGER_R` of the shape, then squeezed by
//!    `SQUEEZE`. A flat grip keeps the fingers straight. Everything blends with the grip weight
//!    against the animated pose.
//! All geometry (curl axis of each finger joint, palm centre and normal, fingertips) is measured
//! once from the skeleton's rest pose (`HandRig::new`), so any rig with UAL finger names works.
use super::{Bones, Pose, Skeleton};
use bevy::prelude::*;

/// Finger radius used for contact (m).
pub const FINGER_R: f32 = 0.009;
/// Palm surface offset from the palm centre along the palm normal (m).
pub const PALM: f32 = 0.022;
/// Extra curl after first contact, as a fraction of the full curl.
pub const SQUEEZE: f32 = 0.04;
/// Curl of a hand that found nothing to close on (empty grip).
pub const EMPTY: f32 = 0.85;
const FINGERS: [&str; 5] = ["thumb", "index", "middle", "ring", "pinky"];
/// Full-curl angle per joint (rad): knuckle, middle, tip.
const FINGER_MAX: [f32; 3] = [1.55, 1.75, 1.25];
const THUMB_MAX: [f32; 3] = [0.55, 0.8, 1.0];
const BISECT: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A handle: the segment `a`–`b` with `radius`.
    Cylinder { a: Vec3, b: Vec3, radius: f32 },
    /// A knob or ball.
    Sphere { center: Vec3, radius: f32 },
    /// A surface pressed with an open palm; `normal` points out of the surface toward the hand.
    Flat { point: Vec3, normal: Vec3 },
}

impl Shape {
    /// Signed distance from `p` to the surface (negative inside).
    pub fn distance(&self, p: Vec3) -> f32 {
        match *self {
            Shape::Cylinder { a, b, radius } => {
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
                p.distance(a + ab * t) - radius
            }
            Shape::Sphere { center, radius } => p.distance(center) - radius,
            Shape::Flat { point, normal } => (p - point).dot(normal.normalize_or(Vec3::Y)),
        }
    }
    /// The same shape moved by `t` (hand-local → world; radii scale with `t`'s uniform scale).
    pub fn moved(&self, t: &Transform) -> Shape {
        match *self {
            Shape::Cylinder { a, b, radius } => Shape::Cylinder { a: t.transform_point(a), b: t.transform_point(b), radius: radius * t.scale.x.abs() },
            Shape::Sphere { center, radius } => Shape::Sphere { center: t.transform_point(center), radius: radius * t.scale.x.abs() },
            Shape::Flat { point, normal } => Shape::Flat { point: t.transform_point(point), normal: t.rotation * normal },
        }
    }
    fn finite(&self) -> bool {
        match *self {
            Shape::Cylinder { a, b, radius } => a.is_finite() && b.is_finite() && radius.is_finite(),
            Shape::Sphere { center, radius } => center.is_finite() && radius.is_finite(),
            Shape::Flat { point, normal } => point.is_finite() && normal.is_finite(),
        }
    }
}

/// One hand's grip request, world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grip {
    pub shape: Shape,
    /// 0..1 blend against the animated hand.
    pub weight: f32,
    /// Wrist roll about the palm normal (rad): turning a knob.
    pub twist: f32,
    /// The shape rides in the hand (a held weapon or tool): it is given in the hand bone's frame,
    /// the wrist is left to the animation and only the fingers close.
    pub in_hand: bool,
}

impl Grip {
    pub fn new(shape: Shape, weight: f32) -> Self {
        Self { shape, weight, twist: 0.0, in_hand: false }
    }
    /// A grip on something carried in the hand; `shape` in the hand bone's local frame.
    pub fn held(shape: Shape, weight: f32) -> Self {
        Self { shape, weight, twist: 0.0, in_hand: true }
    }
}

#[derive(Clone, Debug)]
struct Finger {
    joints: [usize; 3],
    /// Local curl axis of each joint (unit, in that joint's own frame).
    axes: [Vec3; 3],
    /// Fingertip in the last joint's local frame.
    tip: Vec3,
    max: [f32; 3],
}

#[derive(Clone, Debug)]
struct Side {
    hand: usize,
    fingers: Vec<Finger>,
    /// Palm centre in the hand's local frame.
    palm: Vec3,
    /// Palm normal (out of the palm) and finger direction, hand-local, unit.
    normal: Vec3,
    along: Vec3,
}

/// Hand geometry for both sides, measured from a skeleton's rest pose.
#[derive(Clone, Debug, Default)]
pub struct HandRig {
    sides: [Option<Side>; 2],
}

/// Step-1 output kept for step 2.
#[derive(Clone, Copy, Debug, Default)]
pub struct Plan {
    sides: [Option<(Quat, f32)>; 2],
}

impl HandRig {
    pub fn new(skeleton: &Skeleton, bones: &Bones) -> Self {
        let rest = skeleton.fk(&skeleton.rest_pose(), bones.pelvis);
        let mut sides: [Option<Side>; 2] = [None, None];
        for (side, s) in ["l", "r"].into_iter().enumerate() {
            sides[side] = measure(skeleton, &rest, bones.hand[side], s);
        }
        Self { sides }
    }

    pub fn ready(&self, side: usize) -> bool {
        self.sides[side].is_some()
    }

    /// Step 1: per gripping hand, the wrist target (world) and the weight. Also returns the plan
    /// for `close`.
    pub fn wrist_targets(
        &self,
        pose: &Pose,
        skeleton: &Skeleton,
        bones: &Bones,
        model: &Transform,
        grips: &[Option<Grip>; 2],
    ) -> ([Option<(Vec3, f32)>; 2], Plan) {
        let mut targets = [None, None];
        let mut plan = Plan::default();
        if grips.iter().all(Option::is_none) {
            return (targets, plan);
        }
        let world = world_pose(skeleton, pose, bones, model);
        for side in 0..2 {
            let (Some(grip), Some(geo)) = (grips[side], &self.sides[side]) else { continue };
            let weight = grip.weight.clamp(0.0, 1.0);
            if weight < 1e-3 || !grip.shape.finite() || !grip.twist.is_finite() {
                continue;
            }
            if grip.in_hand {
                plan.sides[side] = Some((Quat::NAN, weight));
                continue;
            }
            let hand = world[geo.hand];
            let current = hand.rotation;
            let palm = hand.transform_point(geo.palm);
            let (n_cur, f_cur) = (current * geo.normal, current * geo.along);
            let (n, f, contact) = match grip.shape {
                Shape::Cylinder { a, b, radius } => {
                    let d = (b - a).normalize_or(Vec3::Y);
                    let t = ((palm - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
                    let c = a + (b - a) * t;
                    let to = c - palm;
                    let n = (to - d * to.dot(d)).normalize_or(n_cur);
                    let mut f = d.cross(n).normalize_or(f_cur);
                    if f.dot(f_cur) < 0.0 {
                        f = -f;
                    }
                    (n, f, c - n * (radius + PALM))
                }
                Shape::Sphere { center, radius } => {
                    let n = (center - palm).normalize_or(n_cur);
                    let f = (f_cur - n * f_cur.dot(n)).normalize_or(n.any_orthonormal_vector());
                    (n, f, center - n * (radius + PALM))
                }
                Shape::Flat { point, normal } => {
                    let normal = normal.normalize_or(Vec3::Y);
                    let n = -normal;
                    let f = (f_cur - n * f_cur.dot(n)).normalize_or(n.any_orthonormal_vector());
                    (n, f, point + normal * PALM)
                }
            };
            let f = Quat::from_axis_angle(n, grip.twist) * f;
            let goal = basis(n, f) * basis(geo.normal, geo.along).inverse();
            let wrist = contact - goal * (geo.palm * hand.scale);
            if wrist.is_finite() && goal.is_finite() {
                targets[side] = Some((wrist, weight));
                plan.sides[side] = Some((goal.normalize(), weight));
            }
        }
        (targets, plan)
    }

    /// Step 2 (after the arm IK): wrist rotation and finger curl.
    pub fn close(
        &self,
        pose: &mut Pose,
        skeleton: &Skeleton,
        bones: &Bones,
        model: &Transform,
        grips: &[Option<Grip>; 2],
        plan: &Plan,
    ) {
        for side in 0..2 {
            let (Some(grip), Some(geo), Some((goal, weight))) = (grips[side], &self.sides[side], plan.sides[side]) else {
                continue;
            };
            let hand = geo.hand;
            if !grip.in_hand {
                let world = world_pose(skeleton, pose, bones, model);
                let parent = skeleton.parent[hand].map(|p| world[p].rotation).unwrap_or(model.rotation);
                let target = world[hand].rotation.slerp(goal, weight);
                pose.rot[hand] = (parent.inverse() * target).normalize();
            }

            let world = world_pose(skeleton, pose, bones, model);
            let shape = if grip.in_hand { grip.shape.moved(&world[hand]) } else { grip.shape };
            let flat = matches!(shape, Shape::Flat { .. });
            for finger in &geo.fingers {
                let base = world[skeleton.parent[finger.joints[0]].unwrap_or(hand)];
                let open: [Quat; 3] = finger.joints.map(|j| skeleton.rest[j].rotation);
                let curl = if flat { 0.0 } else { reach(skeleton, &base, finger, &open, &shape) };
                for (k, &j) in finger.joints.iter().enumerate() {
                    let q = open[k] * Quat::from_axis_angle(finger.axes[k], curl * finger.max[k]);
                    pose.rot[j] = pose.rot[j].slerp(q, weight).normalize();
                }
            }
        }
    }
}

/// The largest curl at which the finger stays outside the shape, plus `SQUEEZE`.
fn reach(skeleton: &Skeleton, base: &Transform, finger: &Finger, open: &[Quat; 3], shape: &Shape) -> f32 {
    let touches = |c: f32| {
        let mut parent = *base;
        for k in 0..3 {
            let j = finger.joints[k];
            let local = Transform {
                rotation: open[k] * Quat::from_axis_angle(finger.axes[k], c * finger.max[k]),
                ..skeleton.rest[j]
            };
            let joint = parent.mul_transform(local);
            if k > 0 && shape.distance(joint.translation) < FINGER_R {
                return true;
            }
            let next = if k == 2 { joint.transform_point(finger.tip) } else { joint.transform_point(skeleton.rest[finger.joints[k + 1]].translation) };
            let mid = (joint.translation + next) * 0.5;
            if shape.distance(mid) < FINGER_R || shape.distance(next) < FINGER_R {
                return true;
            }
            parent = joint;
        }
        false
    };
    if touches(0.0) {
        return 0.0;
    }
    if !touches(1.0) {
        return EMPTY;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..BISECT {
        let mid = 0.5 * (lo + hi);
        if touches(mid) { hi = mid } else { lo = mid }
    }
    (lo + SQUEEZE).min(1.0)
}

fn world_pose(skeleton: &Skeleton, pose: &Pose, bones: &Bones, model: &Transform) -> Vec<Transform> {
    skeleton.fk(pose, bones.pelvis).into_iter().map(|t| model.mul_transform(t)).collect()
}

/// Rotation whose X axis is `n` and Y axis is `f` (made orthogonal).
fn basis(n: Vec3, f: Vec3) -> Quat {
    let x = n.normalize();
    let z = x.cross(f).normalize();
    let y = z.cross(x);
    Quat::from_mat3(&Mat3::from_cols(x, y, z))
}

fn measure(skeleton: &Skeleton, rest: &[Transform], hand: usize, s: &str) -> Option<Side> {
    let p = |name: &str| skeleton.find(name).map(|i| rest[i].translation);
    let hand_t = rest[hand];
    let knuckles: Vec<Vec3> = ["index", "middle", "ring", "pinky"].iter().filter_map(|f| p(&format!("{f}_01_{s}"))).collect();
    if knuckles.len() < 4 {
        return None;
    }
    let mean = knuckles.iter().sum::<Vec3>() / knuckles.len() as f32;
    let along_w = (mean - hand_t.translation).normalize();
    let across = (knuckles[0] - knuckles[3]).normalize();
    let mut normal_w = across.cross(along_w).normalize();
    // The thumb hangs to the palm side.
    let thumb_tip = p(&format!("thumb_04_leaf_{s}")).or_else(|| p(&format!("thumb_03_{s}")))?;
    if normal_w.dot(thumb_tip - hand_t.translation) < 0.0 {
        normal_w = -normal_w;
    }
    let palm_w = hand_t.translation.lerp(mean, 0.6);
    let inv = hand_t.compute_affine().inverse();
    let palm = inv.transform_point3(palm_w);
    let normal = (hand_t.rotation.inverse() * normal_w).normalize();
    let along = (hand_t.rotation.inverse() * along_w).normalize();
    // Point the thumb curls toward: across the palm, just off its surface.
    let thumb_goal = palm_w + normal_w * 0.03 + across * 0.01;

    let mut fingers = Vec::new();
    for name in FINGERS {
        let thumb = name == "thumb";
        let joints = [1, 2, 3].map(|k| skeleton.find(&format!("{name}_0{k}_{s}")));
        let [Some(a), Some(b), Some(c)] = joints else { continue };
        let joints = [a, b, c];
        let tip_w = p(&format!("{name}_04_leaf_{s}")).unwrap_or_else(|| rest[c].translation + (rest[c].translation - rest[b].translation));
        let tip = rest[c].compute_affine().inverse().transform_point3(tip_w);
        let mut axes = [Vec3::X; 3];
        for (k, &j) in joints.iter().enumerate() {
            let pivot = rest[j].translation;
            let lever = tip_w - pivot;
            let want = if thumb { (thumb_goal - tip_w).normalize_or(normal_w) } else { normal_w };
            let mut best = (f32::MIN, Vec3::X);
            for axis in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z] {
                let score = (rest[j].rotation * axis).cross(lever).dot(want);
                if score > best.0 {
                    best = (score, axis);
                }
            }
            axes[k] = best.1;
        }
        fingers.push(Finger { joints, axes, tip, max: if thumb { THUMB_MAX } else { FINGER_MAX } });
    }
    Some(Side { hand, fingers, palm, normal, along })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A palm-down T-pose left arm with five fingers, positions as measured from `ual2.glb`.
    fn skeleton() -> (Skeleton, Bones) {
        let mut joints: Vec<(String, Option<String>, Vec3)> = vec![
            ("root".into(), None, Vec3::ZERO),
            ("pelvis".into(), Some("root".into()), Vec3::new(0.0, 0.95, 0.0)),
            ("spine_01".into(), Some("pelvis".into()), Vec3::new(0.0, 1.05, 0.0)),
            ("spine_02".into(), Some("spine_01".into()), Vec3::new(0.0, 1.2, 0.0)),
            ("spine_03".into(), Some("spine_02".into()), Vec3::new(0.0, 1.35, 0.0)),
            ("neck_01".into(), Some("spine_03".into()), Vec3::new(0.0, 1.48, 0.0)),
            ("Head".into(), Some("neck_01".into()), Vec3::new(0.0, 1.57, 0.0)),
        ];
        for (s, x) in [("l", 1.0f32), ("r", -1.0)] {
            let v = |a: f32, y: f32, z: f32| Vec3::new(a * x, y, z);
            joints.push((format!("clavicle_{s}"), Some("spine_03".into()), v(0.05, 1.44, -0.07)));
            joints.push((format!("upperarm_{s}"), Some(format!("clavicle_{s}")), v(0.17, 1.44, -0.07)));
            joints.push((format!("lowerarm_{s}"), Some(format!("upperarm_{s}")), v(0.466, 1.441, -0.07)));
            joints.push((format!("hand_{s}"), Some(format!("lowerarm_{s}")), v(0.739, 1.441, -0.065)));
            let fingers: [(&str, [[f32; 3]; 4]); 5] = [
                ("thumb", [[0.766, 1.418, -0.032], [0.8, 1.399, -0.014], [0.839, 1.377, 0.006], [0.872, 1.359, 0.023]]),
                ("index", [[0.859, 1.439, -0.034], [0.899, 1.439, -0.034], [0.934, 1.439, -0.034], [0.964, 1.439, -0.034]]),
                ("middle", [[0.86, 1.441, -0.06], [0.903, 1.441, -0.062], [0.937, 1.441, -0.064], [0.971, 1.441, -0.065]]),
                ("ring", [[0.858, 1.441, -0.082], [0.897, 1.441, -0.084], [0.928, 1.441, -0.085], [0.96, 1.441, -0.086]]),
                ("pinky", [[0.847, 1.439, -0.106], [0.887, 1.439, -0.109], [0.914, 1.439, -0.111], [0.943, 1.439, -0.113]]),
            ];
            for (f, pts) in fingers {
                let mut parent = format!("hand_{s}");
                for (k, p) in pts.iter().enumerate() {
                    let name = if k == 3 { format!("{f}_04_leaf_{s}") } else { format!("{f}_0{}_{s}", k + 1) };
                    joints.push((name.clone(), Some(parent.clone()), v(p[0], p[1], p[2])));
                    parent = name;
                }
            }
            for (name, parent, y) in [("thigh", "pelvis", 0.9), ("calf", "thigh", 0.5), ("foot", "calf", 0.08), ("ball", "foot", 0.02)] {
                let parent = if parent == "pelvis" { parent.to_string() } else { format!("{parent}_{s}") };
                joints.push((format!("{name}_{s}"), Some(parent), v(0.1, y, 0.0)));
            }
        }
        let index: HashMap<String, usize> = joints.iter().enumerate().map(|(i, j)| (j.0.clone(), i)).collect();
        let parent: Vec<Option<usize>> = joints.iter().map(|j| j.1.as_ref().map(|p| index[p])).collect();
        let rest = joints
            .iter()
            .enumerate()
            .map(|(i, j)| Transform::from_translation(j.2 - parent[i].map(|p| joints[p].2).unwrap_or(Vec3::ZERO)))
            .collect();
        let skeleton = Skeleton { names: joints.iter().map(|j| j.0.clone()).collect(), parent, rest, base: Transform::IDENTITY, index };
        let bones = Bones::new(&skeleton).unwrap();
        (skeleton, bones)
    }

    fn solve(grip: Grip) -> (Skeleton, Bones, HandRig, Pose, Vec<Transform>) {
        let (skeleton, bones) = skeleton();
        let rig = HandRig::new(&skeleton, &bones);
        let mut pose = skeleton.rest_pose();
        let grips = [Some(grip), None];
        let (_, plan) = rig.wrist_targets(&pose, &skeleton, &bones, &Transform::IDENTITY, &grips);
        rig.close(&mut pose, &skeleton, &bones, &Transform::IDENTITY, &grips, &plan);
        let world = skeleton.fk(&pose, bones.pelvis);
        (skeleton, bones, rig, pose, world)
    }

    #[test]
    fn measures_both_hands_with_five_fingers() {
        let (skeleton, bones) = skeleton();
        let rig = HandRig::new(&skeleton, &bones);
        for side in 0..2 {
            let geo = rig.sides[side].as_ref().unwrap();
            assert_eq!(geo.fingers.len(), 5);
            // Palm faces down in the T-pose.
            let n = skeleton.fk(&skeleton.rest_pose(), bones.pelvis)[geo.hand].rotation * geo.normal;
            assert!(n.y < -0.9, "palm normal {n}");
        }
    }

    #[test]
    fn fingers_wrap_a_bar_under_the_palm_without_entering_it() {
        // A bar along Z just under the left palm.
        let shape = Shape::Cylinder { a: Vec3::new(0.82, 1.40, -0.2), b: Vec3::new(0.82, 1.40, 0.1), radius: 0.016 };
        let (skeleton, _bones, _rig, pose, world) = solve(Grip::new(shape, 1.0));
        let mut closest = f32::MAX;
        for f in ["index", "middle", "ring", "pinky"] {
            for k in 2..=3 {
                let p = world[skeleton.find(&format!("{f}_0{k}_l")).unwrap()].translation;
                closest = closest.min(shape.distance(p));
                assert!(shape.distance(p) > FINGER_R * 0.5, "{f}_0{k} inside: {}", shape.distance(p));
            }
            let j = skeleton.find(&format!("{f}_01_l")).unwrap();
            assert!(pose.rot[j].angle_between(skeleton.rest[j].rotation) > 0.3, "{f} did not curl");
        }
        assert!(closest < 0.03, "fingers stopped {closest} m from the bar");
    }

    #[test]
    fn flat_push_keeps_fingers_straight_and_palm_on_the_plane() {
        // A door face in front of the left hand (normal toward the hand, −Z side... hand is at z −0.065).
        let shape = Shape::Flat { point: Vec3::new(0.85, 1.44, 0.05), normal: Vec3::NEG_Z };
        let (skeleton, bones, rig, pose, world) = solve(Grip::new(shape, 1.0));
        for f in ["index", "middle", "ring", "pinky"] {
            let j = skeleton.find(&format!("{f}_02_l")).unwrap();
            assert!(pose.rot[j].angle_between(skeleton.rest[j].rotation) < 1e-3);
        }
        let geo = rig.sides[0].as_ref().unwrap();
        let n = world[bones.hand[0]].rotation * geo.normal;
        assert!(n.dot(Vec3::Z) > 0.99, "palm faces {n}");
    }

    #[test]
    fn wrist_target_puts_the_palm_on_a_knob() {
        let (skeleton, bones) = skeleton();
        let rig = HandRig::new(&skeleton, &bones);
        let pose = skeleton.rest_pose();
        let shape = Shape::Sphere { center: Vec3::new(0.85, 1.30, 0.0), radius: 0.03 };
        let grips = [Some(Grip::new(shape, 1.0)), None];
        let (targets, plan) = rig.wrist_targets(&pose, &skeleton, &bones, &Transform::IDENTITY, &grips);
        let (wrist, w) = targets[0].unwrap();
        assert_eq!(w, 1.0);
        let goal = plan.sides[0].unwrap().0;
        let geo = rig.sides[0].as_ref().unwrap();
        let palm = wrist + goal * geo.palm;
        assert!((shape.distance(palm) - PALM).abs() < 1e-3, "palm {} from the knob", shape.distance(palm));
        assert!(targets[1].is_none());
    }

    #[test]
    fn held_grip_closes_fingers_and_leaves_the_wrist() {
        let (skeleton, bones) = skeleton();
        let rig = HandRig::new(&skeleton, &bones);
        let mut pose = skeleton.rest_pose();
        // A sword grip in the left hand's frame: under the palm, across the fingers.
        let rest = skeleton.fk(&pose, bones.pelvis);
        let hand = rest[bones.hand[0]];
        let world = Shape::Cylinder { a: Vec3::new(0.82, 1.40, -0.2), b: Vec3::new(0.82, 1.40, 0.1), radius: 0.016 };
        let inv = hand.compute_affine().inverse();
        let Shape::Cylinder { a, b, radius } = world else { unreachable!() };
        let local = Shape::Cylinder { a: inv.transform_point3(a), b: inv.transform_point3(b), radius };
        let grips = [Some(Grip::held(local, 1.0)), None];
        let (targets, plan) = rig.wrist_targets(&pose, &skeleton, &bones, &Transform::IDENTITY, &grips);
        assert!(targets[0].is_none(), "a held grip must not pull the wrist");
        rig.close(&mut pose, &skeleton, &bones, &Transform::IDENTITY, &grips, &plan);
        assert_eq!(pose.rot[bones.hand[0]], skeleton.rest[bones.hand[0]].rotation);
        let j = skeleton.find("middle_01_l").unwrap();
        assert!(pose.rot[j].angle_between(skeleton.rest[j].rotation) > 0.3);
    }

    #[test]
    fn no_grip_changes_nothing() {
        let (skeleton, bones) = skeleton();
        let rig = HandRig::new(&skeleton, &bones);
        let mut pose = skeleton.rest_pose();
        let before = pose.clone();
        let (targets, plan) = rig.wrist_targets(&pose, &skeleton, &bones, &Transform::IDENTITY, &[None, None]);
        rig.close(&mut pose, &skeleton, &bones, &Transform::IDENTITY, &[None, None], &plan);
        assert_eq!(targets, [None, None]);
        assert_eq!(pose, before);
    }
}
