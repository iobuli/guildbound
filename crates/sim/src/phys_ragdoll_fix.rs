//! Physics: fixed anatomical hinge axes and ankle motors for the ragdoll (amendment s8, s11).
//! Extends ragdoll.rs `build_with`; pose order slot: Avian and ragdoll. Pure f32 maths.

use crate::phys_mass::Gains;
use crate::phys_math::*;

/// Elbow flexion axis from the REST pose only: perpendicular to the upper arm and the body's
/// forward, oriented so flexion brings the forearm forward. Never from the current bend, so a
/// straight (T-pose) arm still gets the right axis. Measure once at load and keep it in the parent
/// body's local frame (L11: rest data never changes).
pub fn elbow_axis_rest(upper_arm_dir: V3, forward: V3) -> Option<V3> { normalize(cross(upper_arm_dir, forward)) }

/// Knee axis: the body's side axis, flexion swings the shin back (matches knee.rs boss rule).
pub fn knee_axis_rest(forward: V3) -> Option<V3> { normalize(cross([0.0, 1.0, 0.0], forward)) }

/// Signed bend about `axis` from parent direction to child direction: + = flexion,
/// - = hyperextension. Replaces the unsigned acos, so limits can tell the two apart.
pub fn signed_bend(axis: V3, parent_dir: V3, child_dir: V3) -> f32 {
    dot(axis, cross(parent_dir, child_dir)).atan2(dot(parent_dir, child_dir))
}

/// Hinge limits relative to the spawn pose for a joint whose anatomical range is [0, flex].
pub fn hinge_limits(bend_now: f32, flex: f32) -> (f32, f32) { (-bend_now, flex - bend_now) }

/// Ankle motor about one axis (pitch or roll): tau = m_above g x_com + kp (target - angle) - kd rate.
/// `com_offset` is the horizontal COM offset from the ankle (m). Returns RAW torque; the joint clamp
/// (with `ankle_cap` as the sole-edge bound) is applied once by the single motor writer.
/// Replaces the old phys_stance::ankle_pd: use `ankle_gains` for inertia-scaled gains.
pub fn ankle_torque(mass_above: f32, com_offset: f32, angle: f32, rate: f32, target: f32, g: Gains) -> f32 {
    mass_above * G * com_offset + g.kp * (target - angle) - g.kd * rate
}

/// Inertia-scaled PD gains: kp = I w^2, kd = 2 zeta I w (same natural frequency on every rig).
pub fn ankle_gains(inertia: f32, omega: f32, zeta: f32) -> Gains {
    Gains { kp: inertia * omega * omega, kd: inertia * 2.0 * zeta * omega }
}

/// Helper only (the joint clamp bound for ankles). Ankle cap: a foot can only push while the COM pressure point stays on the sole, so the cap is
/// m g times the distance from ankle to the sole edge in that direction (toe or heel).
pub fn ankle_cap(mass_above: f32, sole_edge: f32) -> f32 { mass_above * G * sole_edge.abs() }
