//! Physics: limb mass, inertia, torque limits and feed-forward motor torque (amendment s8).
//! Pure functions, no HashMap, f32 state. Units: kg, m, s, rad. Bones are plain indices until
//! `event_log::BoneIdx` lands.

use crate::phys_math::*;

/// Symmetric 3x3 inertia tensor about a point, row-major.
pub type I3 = [[f32; 3]; 3];
/// I v for a 3x3 tensor.
pub fn mul(i: &I3, v: V3) -> V3 { [dot(i[0], v), dot(i[1], v), dot(i[2], v)] }

/// A segment modelled as a solid cylinder along local +Y from the joint, radius `r`.
#[derive(Clone, Copy, Debug)]
pub struct Segment { pub mass: f32, pub length: f32, pub radius: f32 }

/// A point load (armour or weapon) at local position `at` from the joint.
#[derive(Clone, Copy, Debug)]
pub struct Load { pub mass: f32, pub at: V3 }

#[derive(Clone, Copy, Debug)]
pub struct MassProps { pub mass: f32, pub com: V3, pub inertia_joint: I3 }

/// Parallel-axis contribution of a point mass m at offset d: m (|d|^2 E - d d^T).
fn point_inertia(m: f32, d: V3) -> I3 {
    let dd = dot(d, d);
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 { for j in 0..3 { r[i][j] = m * ((if i == j { dd } else { 0.0 }) - d[i] * d[j]); } }
    r
}
fn add_i(a: &mut I3, b: &I3) { for i in 0..3 { for j in 0..3 { a[i][j] += b[i][j]; } } }

/// Mass properties of a segment plus its loads, inertia taken about the joint.
pub fn segment_props(s: Segment, loads: &[Load]) -> MassProps {
    let (m, l, r) = (s.mass, s.length, s.radius);
    // Cylinder about its own centre: Iyy = m r^2/2, Ixx = Izz = m(3r^2 + l^2)/12.
    let ic = m * (3.0 * r * r + l * l) / 12.0;
    let mut i: I3 = [[ic, 0.0, 0.0], [0.0, m * r * r / 2.0, 0.0], [0.0, 0.0, ic]];
    let c = [0.0, l / 2.0, 0.0];
    add_i(&mut i, &point_inertia(m, c));
    let mut mass = m;
    let mut mom = scale(c, m);
    for ld in loads {
        add_i(&mut i, &point_inertia(ld.mass, ld.at));
        mass += ld.mass;
        mom = add(mom, scale(ld.at, ld.mass));
    }
    MassProps { mass, com: scale(mom, 1.0 / mass.max(1e-6)), inertia_joint: i }
}

/// Default leg clamp gain k.
pub const LEG_K: f32 = 1.2;
/// Max alpha_max multiplier inside a strike's active window.
pub const STRIKE_ALPHA_SCALE: f32 = 1.5;

/// Per-joint clamp for leg joints (hip, knee, ankle), the only clamp in the motor path:
///
///   tau_max = k * g * (c * M_above + (1 - c) * m_below) * l_com
///
/// `c` in [0, 1] is the foot's contact blend from the same `Contact` hold the balance uses
/// (1 = planted: the joint holds up the body above it; 0 = swinging: it carries the chain below).
/// Blended, not switched, so the cap does not jump on plant/lift. `l_com` is the distance from the
/// joint to the COM of the chain being moved (not the bone length). k defaults to `LEG_K` = 1.2.
/// SUPERSEDED (pass 6): motion now uses phys_balance::stable_pd with no shaping clamp; kept only for the safety-cap fault log.
pub fn leg_torque_limit(k: f32, c: f32, m_above: f32, m_below: f32, l_com: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    k * G * (c * m_above + (1.0 - c) * m_below) * l_com
}

/// Per-joint clamp for arm joints (shoulder, elbow), inertia-driven:
///
///   tau_max = margin * (I * alpha_eff + m * g * l_com),  alpha_eff = alpha_max * (1 + (s - 1) * w)
///
/// `w` in [0, 1] is the `inside_strike_window` flag (or ramp); `s` = `STRIKE_ALPHA_SCALE` (1.5),
/// so alpha_max grows by at most 1.5x inside a strike and never on standing joints. The caller
/// logs every tick where w > 0. `m`, `l_com`: chain mass below the joint and its COM distance;
/// `axis_inertia`: chain inertia about the joint axis.
/// SUPERSEDED (pass 6): motion now uses phys_balance::stable_pd with no shaping clamp; kept only for the safety-cap fault log.
pub fn arm_torque_limit(chain_mass: f32, l_com: f32, axis_inertia: f32, alpha_max: f32, margin: f32, strike: f32) -> f32 {
    let alpha = alpha_max * (1.0 + (STRIKE_ALPHA_SCALE - 1.0) * strike.clamp(0.0, 1.0));
    margin * (axis_inertia * alpha + chain_mass * G * l_com)
}

/// Motor target from the clip: rotation error (axis-angle, world), angular velocity and accel.
#[derive(Clone, Copy, Debug)]
pub struct MotorTarget { pub err: V3, pub omega: V3, pub alpha: V3 }

#[derive(Clone, Copy, Debug)]
pub struct Gains { pub kp: f32, pub kd: f32 }

/// tau = I alpha + omega x (I omega) + gravity_comp + kp err + kd (omega_t - omega). Returns RAW
/// torque; the single per-joint clamp (`leg_torque_limit` / `arm_torque_limit`) is applied once by
/// the motor writer. All vectors world frame; `inertia` is the world-frame tensor about the joint.
/// SUPERSEDED (pass 6): motion now uses phys_balance::stable_pd with no shaping clamp; kept only for the safety-cap fault log.
pub fn motor_torque(inertia: &I3, omega: V3, t: MotorTarget, gravity_comp: V3, g: Gains) -> V3 {
    let ff = mul(inertia, t.alpha);
    let gyro = cross(omega, mul(inertia, omega));
    let pd = add(scale(t.err, g.kp), scale(add(t.omega, scale(omega, -1.0)), g.kd));
    add(add(ff, gyro), add(gravity_comp, pd))
}

/// Gravity torque about the joint from mass `m` at world offset `com` (counteract with -this).
pub fn gravity_torque(m: f32, com: V3) -> V3 { cross(com, [0.0, -G * m, 0.0]) }

/// Strike boost (pass 6): multiplier blending Kp toward its strike value, kp = kp_base + boost·(kp_strike − kp_base).
/// Formerly scaled the arm alpha term in the clamp, by whole ticks since the strike event.
/// 1.0 for ticks 0..=5, smooth ease to 0.0 by tick 9, then 0.0.
/// Restart only allowed when `ticks_since_last_start >= STRIKE_RESTART_TICKS` (6 + 12 cooldown).
pub const STRIKE_HOLD_TICKS: u32 = 6;
pub const STRIKE_END_TICKS: u32 = 9;
pub const STRIKE_RESTART_TICKS: u32 = 18;

pub fn strike_boost(ticks_since_start: u32) -> f32 {
    if ticks_since_start < STRIKE_HOLD_TICKS {
        1.0
    } else if ticks_since_start >= STRIKE_END_TICKS {
        0.0
    } else {
        let t = (ticks_since_start - STRIKE_HOLD_TICKS + 1) as f32
            / (STRIKE_END_TICKS - STRIKE_HOLD_TICKS + 1) as f32;
        let s = t * t * (3.0 - 2.0 * t);
        1.0 - s
    }
}

pub fn strike_can_restart(ticks_since_last_start: u32) -> bool {
    ticks_since_last_start >= STRIKE_RESTART_TICKS
}
