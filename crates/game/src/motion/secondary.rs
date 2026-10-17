//! Secondary motion: one angular spring per bone chasing the composed target pose, giving
//! overlap / follow-through, plus impulse injection for hits and spine squash/stretch.
//! Springs act on LOCAL rotations in parent space; impulses arrive in world space and are
//! converted using the model rotation and FK.
//!
//! Arms also carry inertia in the WORLD: when the torso or the whole body turns, upper arms and
//! forearms keep part of their world orientation (the spring then pulls them back), and the
//! upper arms' goal trails opposite to the body's acceleration like a pendulum. Local springs
//! alone moved arms rigidly with their parents, so they never swung.
//!
//! The chest, neck and head nod against vertical body acceleration (the step bounce), so each
//! footfall carries through to the head.
use super::{Bones, Pose, Skeleton};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct SpringParams {
    /// Natural frequency in Hz for the core (spine/neck/head).
    pub core_hz: f32,
    /// Natural frequency in Hz for limbs (arms; legs use 3× this so feet stay planted).
    pub limb_hz: f32,
    /// Damping ratio: 1 = critically damped, <1 overshoot.
    pub damping: f32,
    /// Scale (0..2) of the arms' world inertia and acceleration trail; 0 = local springs only.
    pub arm_swing: f32,
}

impl Default for SpringParams {
    fn default() -> Self {
        Self {
            core_hz: 7.0,
            limb_hz: 9.0,
            damping: 0.6,
            arm_swing: 1.0,
        }
    }
}

#[derive(Component, Clone, Debug, Default)]
pub struct Springs {
    /// Current local rotation per bone (empty until the first step).
    pub current: Vec<Quat>,
    /// Angular velocity per bone (parent space, rad/s).
    pub velocity: Vec<Vec3>,
    pub squash: f32,
    pub squash_velocity: f32,
    /// World rotation of each swinging arm bone's parent last frame (upper arms, forearms).
    pub parent_world: [Option<Quat>; 4],
}

/// Largest frame step the springs integrate; longer frames are clamped (slow-motion, not a jump).
const MAX_DT: f32 = 1.0 / 20.0;
/// Frames longer than this (loading hitch, window drag) reset the springs onto the target.
const RESET_DT: f32 = 0.25;
/// Upper bound for one integration sub-step.
const MAX_SUBSTEP: f32 = 1.0 / 240.0;
/// Sub-steps also keep `omega * h` below this so semi-implicit Euler stays stable and accurate.
const MAX_OMEGA_H: f32 = 0.4;
/// Highest spring frequency used (Hz), whatever the tuning says.
const MAX_HZ: f32 = 60.0;
/// A bone never lags its target by more than this (rad, just under 120°); the excess snaps.
pub const MAX_ERROR: f32 = 2.07;
/// Angular speed cap (rad/s) so absurd impulses cannot blow up the integration.
const MAX_SPEED: f32 = 60.0;
/// Squash value limits (contact maps roughly −0.3..0.3 to visible deformation).
const MAX_SQUASH: f32 = 1.0;
/// Share of a parent's world rotation an arm bone resists each frame at `arm_swing = 1`.
const ARM_INERTIA: f32 = 0.6;
/// Upper-arm trail (rad) per g of horizontal body acceleration at `arm_swing = 1`, and its cap.
const ARM_TRAIL: f32 = 0.15;
const MAX_TRAIL: f32 = 0.6;
/// Forward nod (rad) per g of upward body acceleration, its share per bone (spine_03, neck,
/// head) and the acceleration cap (g).
const NOD: f32 = 0.08;
const NOD_SHARE: [f32; 3] = [0.3, 0.3, 0.4];
const MAX_NOD_G: f32 = 2.0;
const GRAVITY: f32 = 9.81;

/// Spring group of a canonical bone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// Copied straight from the target (root, fingers, leaf bones).
    Pass,
    /// Spine, neck, head: `core_hz`.
    Core,
    /// Clavicles, arms, hands: `limb_hz`.
    Limb,
    /// Pelvis rotation, thighs, calves, feet, balls: `3 × limb_hz` so contacts stay crisp
    /// (the pelvis carries the legs, so a soft pelvis would slide planted feet).
    Leg,
}

/// Spring group per canonical bone index.
pub fn groups(skeleton: &Skeleton, bones: &Bones) -> Vec<Group> {
    let mut out = vec![Group::Pass; skeleton.len()];
    let mut set = |i: usize, g: Group| {
        if let Some(slot) = out.get_mut(i) {
            *slot = g;
        }
    };
    set(bones.pelvis, Group::Leg);
    for i in bones.spine {
        set(i, Group::Core);
    }
    set(bones.neck, Group::Core);
    set(bones.head, Group::Core);
    for side in 0..2 {
        for i in [
            bones.clavicle[side],
            bones.upperarm[side],
            bones.lowerarm[side],
            bones.hand[side],
        ] {
            set(i, Group::Limb);
        }
        for i in [
            bones.thigh[side],
            bones.calf[side],
            bones.foot[side],
            bones.ball[side],
        ] {
            set(i, Group::Leg);
        }
    }
    out
}

impl SpringParams {
    /// Natural frequency (Hz) of a group, clamped to a stable range; `None` = pass-through.
    fn hz(&self, group: Group) -> Option<f32> {
        let clean = |hz: f32| {
            if hz.is_finite() {
                hz.clamp(0.1, MAX_HZ)
            } else {
                7.0
            }
        };
        match group {
            Group::Pass => None,
            Group::Core => Some(clean(self.core_hz)),
            Group::Limb => Some(clean(self.limb_hz)),
            Group::Leg => Some(clean(self.limb_hz * 3.0)),
        }
    }
    fn swing(&self) -> f32 {
        if self.arm_swing.is_finite() {
            self.arm_swing.clamp(0.0, 2.0)
        } else {
            1.0
        }
    }
    fn zeta(&self) -> f32 {
        if self.damping.is_finite() {
            self.damping.clamp(0.05, 2.0)
        } else {
            0.6
        }
    }
}

/// Finite, normalized quaternion or `fallback`.
pub fn clean_quat(q: Quat, fallback: Quat) -> Quat {
    if q.is_finite() && q.length_squared() > 1e-8 {
        q.normalize()
    } else {
        fallback
    }
}

/// Rotation vector (axis * angle, angle in 0..=π) of `q`, taking the short way round.
pub fn rotation_vector(q: Quat) -> Vec3 {
    let q = if q.w < 0.0 { -q } else { q };
    let v = Vec3::new(q.x, q.y, q.z);
    let s = v.length();
    if s < 1e-7 {
        return v * 2.0;
    }
    v * (2.0 * s.atan2(q.w) / s)
}

impl Springs {
    /// Drop all state; the next `step` starts exactly on its target.
    pub fn reset(&mut self) {
        self.current.clear();
        self.velocity.clear();
        self.squash = 0.0;
        self.squash_velocity = 0.0;
        self.parent_world = [None; 4];
    }

    /// Hold the springs on `pose` at rest (used while something else owns the pose, e.g.
    /// ragdoll), so they resume without a jump.
    pub fn follow(&mut self, pose: &Pose) {
        self.current.clear();
        self.current
            .extend(pose.rot.iter().map(|q| clean_quat(*q, Quat::IDENTITY)));
        self.velocity.clear();
        self.velocity.resize(pose.rot.len(), Vec3::ZERO);
        self.squash_velocity = 0.0;
        self.parent_world = [None; 4];
    }

    /// `step_moving` for a body that is not accelerating.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        target: &Pose,
        skeleton: &Skeleton,
        bones: &Bones,
        model_rotation: Quat,
        impulses: &[(usize, Vec3)],
        squash_target: f32,
        params: &SpringParams,
        dt: f32,
    ) -> Pose {
        self.step_moving(target, skeleton, bones, model_rotation, Vec3::ZERO, impulses, squash_target, params, dt)
    }

    /// Advance toward `target` by `dt` (sub-stepped for stability, dt clamped to 1/20 s),
    /// apply `impulses` (bone, world axis*angle rad/s), return the sprung pose. `accel` is the
    /// body's world acceleration (m/s²), which the upper arms trail.
    /// Bones without a spring (pelvis translation, root, fingers) pass through.
    #[allow(clippy::too_many_arguments)]
    pub fn step_moving(
        &mut self,
        target: &Pose,
        skeleton: &Skeleton,
        bones: &Bones,
        model_rotation: Quat,
        accel: Vec3,
        impulses: &[(usize, Vec3)],
        squash_target: f32,
        params: &SpringParams,
        dt: f32,
    ) -> Pose {
        let n = skeleton.len();
        // Sanitized target: finite, normalized, right length (missing bones = rest).
        let mut goal: Vec<Quat> = (0..n)
            .map(|i| {
                let rest = clean_quat(skeleton.rest[i].rotation, Quat::IDENTITY);
                target.rot.get(i).map_or(rest, |q| clean_quat(*q, rest))
            })
            .collect();
        let pelvis = if target.pelvis.is_finite() {
            target.pelvis
        } else {
            Vec3::ZERO
        };
        let squash_goal = if squash_target.is_finite() {
            squash_target.clamp(-MAX_SQUASH, MAX_SQUASH)
        } else {
            0.0
        };

        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let fresh = self.current.len() != n || self.velocity.len() != n;
        if fresh || dt > RESET_DT || !self.squash.is_finite() || !self.squash_velocity.is_finite() {
            self.current = goal.clone();
            self.velocity = vec![Vec3::ZERO; n];
            self.squash = squash_goal;
            self.squash_velocity = 0.0;
            self.parent_world = [None; 4];
            if fresh || dt > RESET_DT {
                return Pose { rot: goal, pelvis };
            }
        }
        let model_rotation = clean_quat(model_rotation, Quat::IDENTITY);

        let groups = groups(skeleton, bones);
        let zeta = params.zeta();

        // Impulses: world axis-angle rate → the bone's parent frame. A bone without a spring
        // hands its impulse to the nearest sprung ancestor (finger hit → hand).
        if !impulses.is_empty() {
            let fk_pose = Pose {
                rot: self.current.clone(),
                pelvis,
            };
            let model_space = skeleton.fk(&fk_pose, bones.pelvis);
            for &(bone, impulse) in impulses {
                if !impulse.is_finite() || bone >= n {
                    continue;
                }
                let mut b = bone;
                while groups[b] == Group::Pass {
                    match skeleton.parent[b] {
                        Some(p) => b = p,
                        None => break,
                    }
                }
                if groups[b] == Group::Pass {
                    continue;
                }
                let parent_rotation = match skeleton.parent[b] {
                    Some(p) => model_space[p].rotation,
                    None => skeleton.base.rotation,
                };
                let to_parent = clean_quat(model_rotation * parent_rotation, Quat::IDENTITY);
                self.velocity[b] =
                    (self.velocity[b] + to_parent.inverse() * impulse).clamp_length_max(MAX_SPEED);
            }
        }

        let dt = dt.min(MAX_DT);
        if dt > 0.0 {
            self.swing_arms(&mut goal, skeleton, bones, model_rotation, accel, pelvis, params.swing());
            self.nod(&mut goal, skeleton, bones, model_rotation, accel, pelvis);
            let max_hz = [Group::Core, Group::Limb, Group::Leg]
                .into_iter()
                .filter_map(|g| params.hz(g))
                .fold(0.1_f32, f32::max);
            let max_omega = std::f32::consts::TAU * max_hz;
            let h_limit = MAX_SUBSTEP.min(MAX_OMEGA_H / max_omega);
            let steps = (dt / h_limit).ceil().clamp(1.0, 512.0) as usize;
            let h = dt / steps as f32;

            for i in 0..n {
                let Some(hz) = params.hz(groups[i]) else {
                    self.current[i] = goal[i];
                    self.velocity[i] = Vec3::ZERO;
                    continue;
                };
                let omega = std::f32::consts::TAU * hz;
                let k = omega * omega;
                let c = 2.0 * zeta * omega;
                let mut q = clean_quat(self.current[i], goal[i]);
                let mut v = if self.velocity[i].is_finite() {
                    self.velocity[i]
                } else {
                    Vec3::ZERO
                };
                for _ in 0..steps {
                    // Parent-space error: the rotation that carries `q` onto the goal.
                    let err = rotation_vector(goal[i] * q.inverse());
                    v = (v + (err * k - v * c) * h).clamp_length_max(MAX_SPEED);
                    q = (Quat::from_scaled_axis(v * h) * q).normalize();
                }
                self.current[i] = q;
                self.velocity[i] = v;
            }

            // Squash: 1D spring at the core frequency.
            let omega = std::f32::consts::TAU * params.hz(Group::Core).unwrap_or(7.0);
            let (k, c) = (omega * omega, 2.0 * zeta * omega);
            for _ in 0..steps {
                self.squash_velocity +=
                    (k * (squash_goal - self.squash) - c * self.squash_velocity) * h;
                self.squash_velocity = self.squash_velocity.clamp(-50.0, 50.0);
                self.squash += self.squash_velocity * h;
            }
            self.squash = self.squash.clamp(-MAX_SQUASH, MAX_SQUASH);
        }

        // Final guarantees: finite, normalized, and never more than MAX_ERROR behind.
        for i in 0..n {
            let mut q = self.current[i];
            if !q.is_finite() || q.length_squared() < 1e-8 || !self.velocity[i].is_finite() {
                q = goal[i];
                self.velocity[i] = Vec3::ZERO;
            }
            let q = q.normalize();
            let err = rotation_vector(goal[i] * q.inverse());
            let angle = err.length();
            self.current[i] = if angle > MAX_ERROR {
                // Snap the excess: sit exactly MAX_ERROR behind along the same arc.
                (Quat::from_scaled_axis(-err * (MAX_ERROR / angle)) * goal[i]).normalize()
            } else {
                q
            };
        }
        if !self.squash.is_finite() || !self.squash_velocity.is_finite() {
            self.squash = squash_goal;
            self.squash_velocity = 0.0;
        }

        Pose {
            rot: self.current.clone(),
            pelvis,
        }
    }

    /// Head and chest inertia against vertical body acceleration (the step bounce): when the
    /// body is pushed up they lag down, tipping forward about the character's left, and the
    /// springs carry the wobble.
    #[allow(clippy::too_many_arguments)]
    fn nod(
        &self,
        goal: &mut [Quat],
        skeleton: &Skeleton,
        bones: &Bones,
        model_rotation: Quat,
        accel: Vec3,
        pelvis: Vec3,
    ) {
        let g = if accel.y.is_finite() { (accel.y / GRAVITY).clamp(-MAX_NOD_G, MAX_NOD_G) } else { 0.0 };
        if g.abs() < 1e-4 {
            return;
        }
        let fk_pose = Pose {
            rot: self.current.clone(),
            pelvis,
        };
        let model_space = skeleton.fk(&fk_pose, bones.pelvis);
        let left = model_rotation * Vec3::X;
        for (&bone, share) in [bones.spine[2], bones.neck, bones.head].iter().zip(NOD_SHARE) {
            let Some(p) = skeleton.parent.get(bone).copied().flatten() else {
                continue;
            };
            let parent_world = clean_quat(model_rotation * model_space[p].rotation, Quat::IDENTITY);
            let tip = Quat::from_axis_angle(left, NOD * share * g);
            goal[bone] = clean_quat(parent_world.inverse() * tip * parent_world * goal[bone], goal[bone]);
        }
    }

    /// World inertia of the arms: upper arms and forearms keep a share of their world
    /// orientation against their parent's world rotation since last frame, and the upper arms'
    /// goals trail opposite to `accel`.
    #[allow(clippy::too_many_arguments)]
    fn swing_arms(
        &mut self,
        goal: &mut [Quat],
        skeleton: &Skeleton,
        bones: &Bones,
        model_rotation: Quat,
        accel: Vec3,
        pelvis: Vec3,
        swing: f32,
    ) {
        let arm_bones = [bones.upperarm[0], bones.upperarm[1], bones.lowerarm[0], bones.lowerarm[1]];
        let fk_pose = Pose {
            rot: self.current.clone(),
            pelvis,
        };
        let model_space = skeleton.fk(&fk_pose, bones.pelvis);
        let keep = (ARM_INERTIA * swing).clamp(0.0, 0.95);
        let accel = if accel.is_finite() { accel.with_y(0.0) / GRAVITY } else { Vec3::ZERO };
        for (slot, &i) in arm_bones.iter().enumerate() {
            let Some(p) = skeleton.parent.get(i).copied().flatten() else {
                continue;
            };
            let parent_world = clean_quat(model_rotation * model_space[p].rotation, Quat::IDENTITY);
            if let Some(prev) = self.parent_world[slot]
                && keep > 0.0
            {
                // Holding the world orientation fully: local' = P^-1 * prev * local.
                let undo = Quat::IDENTITY.slerp((prev * parent_world.inverse()).normalize(), keep);
                let held = parent_world.inverse() * undo * parent_world * self.current[i];
                self.current[i] = clean_quat(held, goal[i]);
            }
            self.parent_world[slot] = Some(parent_world);

            // Upper arms: the goal swings so the arm trails the acceleration, like a pendulum.
            if slot < 2 && swing > 0.0 {
                let reach = model_space[bones.hand[slot]].translation - model_space[i].translation;
                let Some(down) = (model_rotation * reach).try_normalize() else {
                    continue;
                };
                let trail = (down.cross(-accel) * (ARM_TRAIL * swing)).clamp_length_max(MAX_TRAIL);
                if trail.is_finite() && trail.length_squared() > 1e-10 {
                    let swung = parent_world.inverse() * Quat::from_scaled_axis(trail) * parent_world * goal[i];
                    goal[i] = clean_quat(swung, goal[i]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::contact::tests::skeleton;

    const DT: f32 = 1.0 / 60.0;

    fn angle(a: Quat, b: Quat) -> f32 {
        rotation_vector(a * b.inverse()).length()
    }

    /// Rest with a bent spine, raised arm, swung leg, moved pelvis and curled finger.
    fn target_pose(skeleton: &Skeleton, bones: &Bones) -> Pose {
        let mut pose = skeleton.rest_pose();
        let bend = |pose: &mut Pose, bone: usize, axis: Vec3, deg: f32| {
            pose.rot[bone] = (Quat::from_axis_angle(axis.normalize(), deg.to_radians())
                * pose.rot[bone])
                .normalize();
        };
        bend(&mut pose, bones.spine[1], Vec3::new(1.0, 0.3, 0.0), 60.0);
        bend(&mut pose, bones.upperarm[0], Vec3::Z, 80.0);
        bend(&mut pose, bones.thigh[1], Vec3::X, -40.0);
        bend(
            &mut pose,
            skeleton.find("index_01_l").unwrap(),
            Vec3::Y,
            50.0,
        );
        pose.pelvis = Vec3::new(0.0, -0.1, 0.02);
        pose
    }

    fn assert_same(a: &[Quat], b: &[Quat]) {
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b) {
            assert!(angle(*x, *y) < 1e-5, "{x} != {y}");
        }
    }

    fn assert_healthy(pose: &Pose, goal: &Pose) {
        assert!(pose.pelvis.is_finite());
        for (q, g) in pose.rot.iter().zip(&goal.rot) {
            assert!(
                q.is_finite() && (q.length() - 1.0).abs() < 1e-4,
                "bad quat {q}"
            );
            assert!(angle(*q, *g) <= MAX_ERROR + 1e-3, "lag beyond the clamp");
        }
    }

    #[test]
    fn springs_lag_then_converge_within_a_second() {
        let (skeleton, bones) = skeleton();
        let params = SpringParams::default();
        let rest = skeleton.rest_pose();
        let mut springs = Springs::default();
        let first = springs.step(
            &rest,
            &skeleton,
            &bones,
            Quat::IDENTITY,
            &[],
            0.0,
            &params,
            DT,
        );
        assert_same(&first.rot, &rest.rot); // the first step starts on target

        let target = target_pose(&skeleton, &bones);
        let lagged = springs.step(
            &target,
            &skeleton,
            &bones,
            Quat::IDENTITY,
            &[],
            -0.3,
            &params,
            DT,
        );
        assert!(angle(lagged.rot[bones.spine[1]], target.rot[bones.spine[1]]) > 0.2);
        assert!(angle(lagged.rot[bones.upperarm[0]], target.rot[bones.upperarm[0]]) > 0.2);
        let mut out = lagged;
        for _ in 1..60 {
            out = springs.step(
                &target,
                &skeleton,
                &bones,
                Quat::IDENTITY,
                &[],
                -0.3,
                &params,
                DT,
            );
        }
        for i in 0..skeleton.len() {
            let err = angle(out.rot[i], target.rot[i]);
            assert!(
                err < 1f32.to_radians(),
                "{} still {err} rad off",
                skeleton.names[i]
            );
        }
        assert_eq!(out.pelvis, target.pelvis);
        assert!(
            (springs.squash + 0.3).abs() < 0.01,
            "squash {}",
            springs.squash
        );
    }

    #[test]
    fn pass_through_bones_copy_the_target() {
        let (skeleton, bones) = skeleton();
        let params = SpringParams::default();
        let mut springs = Springs::default();
        springs.step(
            &skeleton.rest_pose(),
            &skeleton,
            &bones,
            Quat::IDENTITY,
            &[],
            0.0,
            &params,
            DT,
        );
        let target = target_pose(&skeleton, &bones);
        let out = springs.step(
            &target,
            &skeleton,
            &bones,
            Quat::IDENTITY,
            &[],
            0.0,
            &params,
            DT,
        );
        for name in ["root", "index_01_l", "index_01_r"] {
            let i = skeleton.find(name).unwrap();
            assert!(
                angle(out.rot[i], target.rot[i]) < 1e-5,
                "{name} passes through"
            );
        }
    }

    #[test]
    fn impulses_turn_bones_about_their_world_axis() {
        let (skeleton, bones) = skeleton();
        let params = SpringParams::default();
        let model = Quat::from_rotation_y(0.9);
        let rest = skeleton.rest_pose();
        let world =
            |pose: &Pose, bone: usize| model * skeleton.fk(pose, bones.pelvis)[bone].rotation;
        let mut springs = Springs::default();
        springs.step(&rest, &skeleton, &bones, model, &[], 0.0, &params, DT);
        let impulse = [(bones.spine[1], Vec3::X * 3.0)];
        let out = springs.step(
            &rest,
            &skeleton,
            &bones,
            model,
            &impulse,
            0.0,
            &params,
            1.0 / 240.0,
        );
        let spine = bones.spine[1];
        let delta = rotation_vector(world(&out, spine) * world(&rest, spine).inverse());
        assert!(delta.length() > 1e-3);
        assert!(delta.normalize().dot(Vec3::X) > 0.95, "world delta {delta}");

        // A finger has no spring: its impulse goes to the hand.
        let mut springs = Springs::default();
        springs.step(&rest, &skeleton, &bones, model, &[], 0.0, &params, DT);
        let finger = skeleton.find("index_01_l").unwrap();
        let out = springs.step(
            &rest,
            &skeleton,
            &bones,
            model,
            &[(finger, Vec3::Y * 3.0)],
            0.0,
            &params,
            DT,
        );
        assert!(angle(out.rot[bones.hand[0]], rest.rot[bones.hand[0]]) > 1e-3);
        assert!(angle(out.rot[finger], rest.rot[finger]) < 1e-6);
    }

    #[test]
    fn upward_acceleration_nods_the_head_forward() {
        let (skeleton, bones) = skeleton();
        let params = SpringParams::default();
        let rest = skeleton.rest_pose();
        let model = Quat::from_rotation_y(0.7);
        let world = |pose: &Pose| model * skeleton.fk(pose, bones.pelvis)[bones.head].rotation;
        let mut springs = Springs::default();
        let mut out = rest.clone();
        for _ in 0..60 {
            out = springs.step_moving(&rest, &skeleton, &bones, model, Vec3::Y * GRAVITY, &[], 0.0, &params, DT);
        }
        let tip = rotation_vector(world(&out) * world(&rest).inverse());
        let left = model * Vec3::X;
        assert!(tip.dot(left) > 0.5 * NOD, "head tips forward about the left axis: {tip}");
        // Falling (downward acceleration) tips it back; no vertical acceleration, no nod.
        let mut springs = Springs::default();
        for _ in 0..60 {
            out = springs.step_moving(&rest, &skeleton, &bones, model, -Vec3::Y * GRAVITY, &[], 0.0, &params, DT);
        }
        assert!(rotation_vector(world(&out) * world(&rest).inverse()).dot(left) < -0.5 * NOD);
        let mut springs = Springs::default();
        for _ in 0..10 {
            out = springs.step_moving(&rest, &skeleton, &bones, model, Vec3::ZERO, &[], 0.0, &params, DT);
        }
        assert!(angle(world(&out), world(&rest)) < 1e-5);
    }

    #[test]
    fn huge_dt_impulses_and_garbage_stay_finite() {
        let (skeleton, bones) = skeleton();
        let target = target_pose(&skeleton, &bones);
        let mut bad = target.clone();
        bad.rot[bones.head] = Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0);
        bad.rot[bones.neck] = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
        bad.pelvis = Vec3::NAN;
        let mut clean_bad = bad.clone();
        clean_bad.rot[bones.head] = skeleton.rest[bones.head].rotation;
        clean_bad.rot[bones.neck] = skeleton.rest[bones.neck].rotation;
        let mut impulses: Vec<(usize, Vec3)> = (0..skeleton.len() + 3)
            .map(|i| (i, Vec3::new(1e9, -1e9, 1e9)))
            .collect();
        impulses.push((bones.pelvis, Vec3::NAN));
        let wild = SpringParams {
            core_hz: 1e9,
            limb_hz: f32::NAN,
            damping: -5.0,
            arm_swing: f32::INFINITY,
        };
        for params in [SpringParams::default(), wild] {
            let mut springs = Springs::default();
            let dts = [DT, 1e6, 0.0, -1.0, f32::NAN, 0.2, DT, f32::INFINITY, DT, DT];
            for (k, dt) in dts.into_iter().enumerate() {
                let (tgt, goal) = if k % 2 == 0 {
                    (&target, &target)
                } else {
                    (&bad, &clean_bad)
                };
                let out = springs.step(
                    tgt,
                    &skeleton,
                    &bones,
                    Quat::IDENTITY,
                    &impulses,
                    1e9,
                    &params,
                    dt,
                );
                assert_healthy(&out, goal);
                assert!(springs.squash.is_finite() && springs.squash.abs() <= MAX_SQUASH);
                assert!(
                    springs
                        .velocity
                        .iter()
                        .all(|v| v.is_finite() && v.length() <= MAX_SPEED + 1e-3)
                );
            }
        }
    }

    #[test]
    fn resets_on_length_mismatch_and_long_frames() {
        let (skeleton, bones) = skeleton();
        let params = SpringParams::default();
        let target = target_pose(&skeleton, &bones);
        let mut springs = Springs {
            current: vec![Quat::IDENTITY; 3],
            velocity: vec![Vec3::ONE; 3],
            squash: 0.5,
            squash_velocity: 3.0,
            parent_world: [Some(Quat::NAN); 4],
        };
        let out = springs.step(
            &target,
            &skeleton,
            &bones,
            Quat::IDENTITY,
            &[],
            0.1,
            &params,
            DT,
        );
        assert_same(&out.rot, &target.rot);
        assert_eq!(springs.squash, 0.1);
        let rest = skeleton.rest_pose();
        let out = springs.step(
            &rest,
            &skeleton,
            &bones,
            Quat::IDENTITY,
            &[],
            0.0,
            &params,
            0.5,
        );
        assert_same(&out.rot, &rest.rot); // a long hitch snaps to the target
        assert!(springs.velocity.iter().all(|v| *v == Vec3::ZERO));
    }
}
