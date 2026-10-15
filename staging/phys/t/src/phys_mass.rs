//! Physics: limb mass, inertia, torque limits and feed-forward motor torque (amendment s8).
//! Pure functions, no HashMap, f32 state. Units: kg, m, s, rad. Bones are plain indices until
//! `event_log::BoneIdx` lands.

pub type V3 = [f32; 3];
fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale(a: V3, s: f32) -> V3 { [a[0] * s, a[1] * s, a[2] * s] }
fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }

/// Symmetric 3x3 inertia tensor about a point, row-major.
pub type I3 = [[f32; 3]; 3];
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

/// Maximum motor torque for a joint: hold the whole hanging chain level against gravity with
/// `margin` headroom, plus enough to reach `alpha_max` on the chain inertia about the axis.
/// `chain_mass`/`chain_com_dist`: mass hanging below the joint and its distance from it.
pub fn torque_limit(chain_mass: f32, chain_com_dist: f32, axis_inertia: f32, alpha_max: f32, margin: f32) -> f32 {
    const G: f32 = 9.81;
    margin * chain_mass * G * chain_com_dist + axis_inertia * alpha_max
}

/// Motor target from the clip: rotation error (axis-angle, world), angular velocity and accel.
#[derive(Clone, Copy, Debug)]
pub struct MotorTarget { pub err: V3, pub omega: V3, pub alpha: V3 }

#[derive(Clone, Copy, Debug)]
pub struct Gains { pub kp: f32, pub kd: f32 }

/// tau = I alpha + omega x (I omega) + gravity_comp + kp err + kd (omega_t - omega), clamped to `limit`
/// by magnitude. All vectors world frame; `inertia` is the world-frame tensor about the joint.
pub fn motor_torque(inertia: &I3, omega: V3, t: MotorTarget, gravity_comp: V3, g: Gains, limit: f32) -> V3 {
    let ff = mul(inertia, t.alpha);
    let gyro = cross(omega, mul(inertia, omega));
    let pd = add(scale(t.err, g.kp), scale(add(t.omega, scale(omega, -1.0)), g.kd));
    let tau = add(add(ff, gyro), add(gravity_comp, pd));
    let n = dot(tau, tau).sqrt();
    if n > limit { scale(tau, limit / n) } else { tau }
}

/// Gravity torque about the joint from mass `m` at world offset `com` (counteract with -this).
pub fn gravity_torque(m: f32, com: V3) -> V3 { cross(com, [0.0, -9.81 * m, 0.0]) }
