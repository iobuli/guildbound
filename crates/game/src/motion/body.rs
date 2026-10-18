//! Body self-collision on the pose. A few capsules sized to the UAL2 mannequin (torso, head,
//! arm and leg segments) stand in for the body. `resolve` runs after the springs (so after
//! exaggeration and overlap) and before contact IK: it swings each upper arm out of the torso,
//! head or opposite thigh, and the thighs apart where the legs cross, each by the smallest
//! rotation that clears the overlap. Poses whose capsules don't touch are left unchanged.
//! Pure pose maths in model space; `play::monitor` measures the same capsules in the world.
use super::secondary::clean_quat;
use super::{Bones, Pose, Skeleton};
use bevy::prelude::*;

pub const TORSO_R: f32 = 0.12;
pub const HEAD_R: f32 = 0.1;
pub const ARM_R: f32 = 0.04;
pub const THIGH_R: f32 = 0.065;
pub const CALF_R: f32 = 0.045;
/// Head sphere centre above the head joint, along neck → head (m).
pub const HEAD_LIFT: f32 = 0.08;
/// Hand capsule length past the wrist (m).
const HAND_LENGTH: f32 = 0.07;
/// Clearance (m) added to every correction so a resolved pose doesn't touch again next frame.
const MARGIN: f32 = 0.01;
const PASSES: usize = 2;
/// Largest swing (rad) one correction may apply.
const MAX_SWING: f32 = 0.6;

/// Extra capsule radii (m at model scale 1) for what a character wears over the body (armour),
/// so limbs are also kept clear of the plates. On the character root; read by `compose`
/// (`resolve`) and the monitor.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyPadding {
    pub torso: f32,
    pub head: f32,
    pub upper_arm: f32,
    pub forearm: f32,
    pub hand: f32,
    pub thigh: f32,
    pub calf: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub a: Vec3,
    pub b: Vec3,
    pub r: f32,
}

/// Overlap of capsule `x` with capsule `y`.
#[derive(Clone, Copy, Debug)]
pub struct Overlap {
    /// Closest points on the two axes.
    pub on_x: Vec3,
    pub on_y: Vec3,
    /// Unit direction that moves `x` out of `y`.
    pub out: Vec3,
    pub depth: f32,
}

/// Closest points between segments `p1–q1` and `p2–q2` (Ericson, Real-Time Collision
/// Detection 5.1.9).
pub fn closest_segments(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    const EPS: f32 = 1e-9;
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    if a <= EPS && e <= EPS {
        return (p1, p2);
    }
    let (s, t) = if a <= EPS {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= EPS {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let s = if denom > EPS { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let t = (b * s + f) / e;
            if t < 0.0 {
                ((-c / a).clamp(0.0, 1.0), 0.0)
            } else if t > 1.0 {
                (((b - c) / a).clamp(0.0, 1.0), 1.0)
            } else {
                (s, t)
            }
        }
    };
    (p1 + d1 * s, p2 + d2 * t)
}

/// How far capsule `x` sinks into capsule `y`; `None` when apart (or not finite).
pub fn overlap(x: Capsule, y: Capsule, fallback_out: Vec3) -> Option<Overlap> {
    let (on_x, on_y) = closest_segments(x.a, x.b, y.a, y.b);
    let gap = on_x - on_y;
    let depth = x.r + y.r - gap.length();
    (depth > 0.0 && depth.is_finite()).then(|| Overlap {
        on_x,
        on_y,
        out: gap.try_normalize().unwrap_or(fallback_out),
        depth,
    })
}

/// The body's capsules from joint positions (model or world space).
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub torso: Capsule,
    pub head: Capsule,
    /// Per side: lower half of the upper arm, forearm, hand.
    pub arm: [[Capsule; 3]; 2],
    pub thigh: [Capsule; 2],
    pub calf: [Capsule; 2],
    /// Body's left (+x of the pelvis frame) for separating coincident axes.
    pub left: Vec3,
}

impl Body {
    pub fn new(pos: &dyn Fn(usize) -> Vec3, bones: &Bones, scale: f32, padding: &BodyPadding) -> Self {
        let pad = |r: f32| if r.is_finite() { r.clamp(0.0, 0.2) } else { 0.0 };
        let cap = |a: Vec3, b: Vec3, r: f32| Capsule { a, b, r: r * scale };
        let (neck, head) = (pos(bones.neck), pos(bones.head));
        let crown = head + (head - neck).normalize_or(Vec3::Y) * (HEAD_LIFT * scale);
        let arm = |side: usize| {
            let (shoulder, elbow, hand) = (
                pos(bones.upperarm[side]),
                pos(bones.lowerarm[side]),
                pos(bones.hand[side]),
            );
            let tip = hand + (hand - elbow).normalize_or_zero() * (HAND_LENGTH * scale);
            [
                cap(shoulder.lerp(elbow, 0.5), elbow, ARM_R + pad(padding.upper_arm)),
                cap(elbow, hand, ARM_R + pad(padding.forearm)),
                cap(hand, tip, ARM_R + pad(padding.hand)),
            ]
        };
        let leg = |side: usize| {
            let (hip, knee, ankle) = (pos(bones.thigh[side]), pos(bones.calf[side]), pos(bones.foot[side]));
            (
                cap(hip, knee, THIGH_R + pad(padding.thigh)),
                cap(knee, ankle, CALF_R + pad(padding.calf)),
            )
        };
        let (thigh_l, calf_l) = leg(0);
        let (thigh_r, calf_r) = leg(1);
        Self {
            torso: cap(pos(bones.pelvis), pos(bones.spine[2]), TORSO_R + pad(padding.torso)),
            head: cap(crown, crown, HEAD_R + pad(padding.head)),
            arm: [arm(0), arm(1)],
            thigh: [thigh_l, thigh_r],
            calf: [calf_l, calf_r],
            left: (pos(bones.thigh[0]) - pos(bones.thigh[1])).normalize_or(Vec3::X),
        }
    }

    /// Deepest overlap of an arm segment with the torso, head or opposite thigh.
    pub fn arm_overlap(&self, side: usize) -> Option<Overlap> {
        let away = if side == 0 { self.left } else { -self.left };
        let obstacles = [self.torso, self.head, self.thigh[1 - side]];
        self.arm[side]
            .iter()
            .flat_map(|&seg| obstacles.iter().filter_map(move |&ob| overlap(seg, ob, away)))
            .max_by(|a, b| a.depth.total_cmp(&b.depth))
    }

    /// Deepest overlap of the left leg with the right (thigh–thigh or calf–calf), as seen from
    /// the left leg.
    pub fn leg_overlap(&self) -> Option<Overlap> {
        [(self.thigh[0], self.thigh[1]), (self.calf[0], self.calf[1])]
            .into_iter()
            .filter_map(|(l, r)| overlap(l, r, self.left))
            .max_by(|a, b| a.depth.total_cmp(&b.depth))
    }
}

/// Rotation about `pivot` that carries `point` by `distance` along `out` (only the part of
/// `out` across the lever counts); `None` when the lever is degenerate or `out` runs along it.
fn swing_away(pivot: Vec3, point: Vec3, out: Vec3, distance: f32) -> Option<Quat> {
    let lever = point - pivot;
    let length = lever.length();
    let along = lever.try_normalize()?;
    let across = out - along * out.dot(along);
    let reach = across.length();
    if length < 1e-3 || reach < 0.05 {
        return None;
    }
    let axis = along.cross(across / reach).try_normalize()?;
    let angle = (distance / (length * reach)).min(MAX_SWING);
    angle.is_finite().then(|| Quat::from_axis_angle(axis, angle))
}

/// Pre-multiply `bone`'s model-space rotation by `delta` (its children follow).
fn rotate(pose: &mut Pose, model: &[Transform], skeleton: &Skeleton, bone: usize, delta: Quat) {
    let parent = skeleton.parent[bone].map_or(skeleton.base.rotation, |p| model[p].rotation);
    let fallback = pose.rot[bone];
    pose.rot[bone] = clean_quat(parent.inverse() * delta * model[bone].rotation, fallback);
}

/// Push limbs out of the body (see the module docs), with `padding` for what is worn over it.
/// `pose` must match `skeleton`.
pub fn resolve(pose: &mut Pose, skeleton: &Skeleton, bones: &Bones, padding: &BodyPadding) {
    if pose.rot.len() != skeleton.len() {
        return;
    }
    for _ in 0..PASSES {
        let model = skeleton.fk(pose, bones.pelvis);
        let pos = |i: usize| model[i].translation;
        let body = Body::new(&pos, bones, 1.0, padding);
        let mut moved = false;
        for side in 0..2 {
            let Some(hit) = body.arm_overlap(side) else { continue };
            let shoulder = pos(bones.upperarm[side]);
            if let Some(delta) = swing_away(shoulder, hit.on_x, hit.out, hit.depth + MARGIN) {
                rotate(pose, &model, skeleton, bones.upperarm[side], delta);
                moved = true;
            }
        }
        if let Some(hit) = body.leg_overlap() {
            let half = 0.5 * hit.depth + MARGIN;
            let sides = [(hit.on_x, hit.out), (hit.on_y, -hit.out)];
            for (side, (point, out)) in sides.into_iter().enumerate() {
                let hip = pos(bones.thigh[side]);
                if let Some(delta) = swing_away(hip, point, out, half) {
                    rotate(pose, &model, skeleton, bones.thigh[side], delta);
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
}

/// Pull `point` (on a limb from `pivot`) back by `depth` along `out`: a swing where `out` crosses
/// the limb, else (pushing straight back along it) a fold toward the limb's down side by the angle
/// that shortens its reach enough.
fn retract(pivot: Vec3, point: Vec3, out: Vec3, depth: f32) -> Option<Quat> {
    if let Some(q) = swing_away(pivot, point, out, depth) {
        return Some(q);
    }
    let lever = point - pivot;
    let length = lever.length();
    let along = lever.try_normalize()?;
    if along.dot(out) > -0.5 || length < 1e-3 {
        return None;
    }
    let side = Vec3::NEG_Y - along * along.dot(Vec3::NEG_Y);
    let side = side.try_normalize().or_else(|| Vec3::Z.cross(along).try_normalize())?;
    let angle = (1.0 - (depth / length).min(1.0)).clamp(-1.0, 1.0).acos().min(2.0 * MAX_SWING);
    let axis = along.cross(side).try_normalize()?;
    angle.is_finite().then(|| Quat::from_axis_angle(axis, angle))
}

/// Body spheres kept out of walls while climbing (`body_out`): bone and radius (m).
pub fn wall_spheres(bones: &Bones) -> Vec<(usize, f32)> {
    let mut v = vec![(bones.pelvis, 0.13), (bones.spine[0], 0.13), (bones.spine[1], 0.14), (bones.spine[2], 0.14), (bones.neck, 0.07), (bones.head, 0.11)];
    for side in 0..2 {
        v.extend([(bones.calf[side], 0.065), (bones.foot[side], 0.05), (bones.upperarm[side], 0.06)]);
    }
    v
}

/// Climbing: push the whole body (the pelvis, so everything rides along) out of the wall
/// before the hands and feet reach for their holds. `probe(p)` is as for `clear_world`
/// (the way out of what `p` is inside, and how deep); each sphere's radius is added. Returns the
/// world push applied (m).
pub fn body_out(
    pose: &mut Pose,
    skeleton: &Skeleton,
    bones: &Bones,
    model_world: &Transform,
    mut probe: impl FnMut(Vec3, f32) -> Option<(Vec3, f32)>,
) -> Vec3 {
    if pose.rot.len() != skeleton.len() {
        return Vec3::ZERO;
    }
    let scale = model_world.scale.max_element().max(1e-3);
    let spheres = wall_spheres(bones);
    let mut total = Vec3::ZERO;
    for _ in 0..PASSES {
        let model = skeleton.fk(pose, bones.pelvis);
        let deepest = spheres
            .iter()
            .filter_map(|&(i, r)| probe(model_world.transform_point(model[i].translation), r * scale))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let Some((out, depth)) = deepest.filter(|d| d.1 > 1e-3) else { break };
        let push = out * (depth + MARGIN * scale);
        total += push;
        pose.pelvis += model_world.rotation.inverse() * push / scale;
    }
    total
}

/// Keep the arms out of the world and other bodies, after contact IK. `probe(p)` (world space)
/// returns the way out of whatever `p` is inside, and how deep (m). For each arm not held by an
/// IK target (`held`), a hand that is inside swings the forearm out about the elbow; an elbow that
/// is inside swings the upper arm out about the shoulder (smallest rotation, `MAX_SWING` at most).
pub fn clear_world(
    pose: &mut Pose,
    skeleton: &Skeleton,
    bones: &Bones,
    model_world: &Transform,
    held: [bool; 2],
    mut probe: impl FnMut(Vec3) -> Option<(Vec3, f32)>,
) {
    if pose.rot.len() != skeleton.len() {
        return;
    }
    let scale = model_world.scale.max_element().max(1e-3);
    let to_model = |out: Vec3| (model_world.rotation.inverse() * out).normalize_or_zero();
    for _ in 0..PASSES {
        let model = skeleton.fk(pose, bones.pelvis);
        let pos = |i: usize| model[i].translation;
        let mut moved = false;
        for side in 0..2 {
            if held[side] {
                continue;
            }
            let (shoulder, elbow, wrist) = (pos(bones.upperarm[side]), pos(bones.lowerarm[side]), pos(bones.hand[side]));
            let hand = wrist + (wrist - elbow).normalize_or_zero() * HAND_LENGTH;
            if let Some((out, depth)) = probe(model_world.transform_point(hand))
                && let Some(delta) = retract(elbow, hand, to_model(out), depth / scale + MARGIN)
            {
                rotate(pose, &model, skeleton, bones.lowerarm[side], delta);
                moved = true;
                continue;
            }
            if let Some((out, depth)) = probe(model_world.transform_point(elbow))
                && let Some(delta) = retract(shoulder, elbow, to_model(out), depth / scale + MARGIN)
            {
                rotate(pose, &model, skeleton, bones.upperarm[side], delta);
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::contact::tests::skeleton;

    #[test]
    fn closest_points_of_crossing_and_parallel_segments() {
        let (a, b) = closest_segments(Vec3::new(-1.0, 0.0, 0.0), Vec3::X, Vec3::new(0.0, 1.0, -1.0), Vec3::new(0.0, 1.0, 1.0));
        assert!(a.distance(Vec3::ZERO) < 1e-5 && b.distance(Vec3::Y) < 1e-5);
        let (a, b) = closest_segments(Vec3::ZERO, Vec3::X, Vec3::new(3.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0));
        assert!(a.distance(Vec3::X) < 1e-5 && b.distance(Vec3::new(3.0, 0.0, 0.0)) < 1e-5);
        let (a, b) = closest_segments(Vec3::ONE, Vec3::ONE, Vec3::ZERO, Vec3::Y * 2.0);
        assert!(a == Vec3::ONE && b.distance(Vec3::Y) < 1e-5);
    }

    #[test]
    fn arm_folded_into_the_torso_is_pushed_out_and_clear_poses_are_untouched() {
        let (skeleton, bones) = skeleton();
        let rest = skeleton.rest_pose();
        let mut untouched = rest.clone();
        resolve(&mut untouched, &skeleton, &bones, &BodyPadding::default());
        assert_eq!(untouched, rest, "the T-pose touches nothing");

        // Swing the left upper arm across the chest: the forearm ends up inside the torso.
        let model = skeleton.fk(&rest, bones.pelvis);
        let mut pose = rest.clone();
        let shoulder = model[bones.upperarm[0]].translation;
        let hand = model[bones.hand[0]].translation;
        let across = Quat::from_rotation_arc((hand - shoulder).normalize(), Vec3::new(-1.0, -0.2, 0.15).normalize());
        rotate(&mut pose, &model, &skeleton, bones.upperarm[0], across);
        let depth = |p: &Pose| {
            let m = skeleton.fk(p, bones.pelvis);
            Body::new(&|i| m[i].translation, &bones, 1.0, &BodyPadding::default()).arm_overlap(0).map_or(0.0, |o| o.depth)
        };
        assert!(depth(&pose) > 0.02);
        for _ in 0..4 {
            resolve(&mut pose, &skeleton, &bones, &BodyPadding::default());
        }
        assert!(depth(&pose) < 0.01, "left {}", depth(&pose));
        assert!(pose.rot.iter().all(|q| q.is_finite()));
    }

    #[test]
    fn an_arm_reaching_into_a_wall_is_pulled_out_unless_held() {
        let (skeleton, bones) = skeleton();
        let rest = skeleton.rest_pose();
        let hand_x = |p: &Pose| skeleton.fk(p, bones.pelvis)[bones.hand[0]].translation.x;
        // A wall just short of the left hand (the T-pose reaches along +x).
        let wall = hand_x(&rest) - 0.12;
        let probe = |p: Vec3| (p.x > wall).then(|| (Vec3::NEG_X, p.x - wall));
        let mut pose = rest.clone();
        for _ in 0..4 {
            clear_world(&mut pose, &skeleton, &bones, &Transform::IDENTITY, [false, false], probe);
        }
        assert!(hand_x(&pose) < wall + 0.03, "hand {} wall {wall}", hand_x(&pose));
        assert!(pose.rot.iter().all(|q| q.is_finite()));
        // A hand an IK target holds (a climb hold, a lever) is left where it is.
        let mut held = rest.clone();
        clear_world(&mut held, &skeleton, &bones, &Transform::IDENTITY, [true, true], probe);
        assert_eq!(held, rest);
    }
}
