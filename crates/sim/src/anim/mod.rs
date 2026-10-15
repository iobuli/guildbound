//! Animation system (overhaul `sim::anim`). Pure sim code, no Bevy. Produces per-joint targets
//! (`target::BoneTarget`, f64) that the one motor writer receives through `api::Sim::set_target`.
//! Maths: hive/ANIMATION_MATH.md. Quaternions: `crate::math::Quat` (w,x,y,z, f64).
//! - `rig`: rest rig, FK, report/self-check, skin-joint index (position in skin.joints, else -1).
//! - `target`: BoneTarget, f64 derivatives at DT = 1/120, chains, rates_to_world_rel (B_p = R_p·O_p).
//! - `ability`: ability contract + gait, strike flow, flare, acrobatics, acro_active, strike boost.
//! - `trail`: ghost trail target positions.
//! - `proc`: procedural run, jump, air recovery, spin footwork (`PROC_BONES` table).
//! - `acro`, `flare`, `flow`: procedural sources the abilities sample (f32 xyzw hinge keys).

pub mod ability;
pub mod acro;
pub mod flare;
pub mod flow;
pub mod gait;
pub mod proc;
pub mod rig;
pub mod target;
pub mod trail;

use crate::math::Quat;

/// Fixed animation step (s). Derivatives never use a variable dt.
pub const DT: f64 = 1.0 / 120.0;

/// `[x, y, z, w]` (glTF / source-module order) -> `math::Quat`.
pub fn quat_xyzw(q: [f32; 4]) -> Quat {
    Quat {
        w: q[3] as f64,
        x: q[0] as f64,
        y: q[1] as f64,
        z: q[2] as f64,
    }
}
/// `math::Quat` -> `[x, y, z, w]` f32 (for engine hand-off).
pub fn to_xyzw(q: Quat) -> [f32; 4] {
    [q.x as f32, q.y as f32, q.z as f32, q.w as f32]
}
/// Rotation angle (rad, 0..π) between two orientations.
pub fn angle_between(a: Quat, b: Quat) -> f64 {
    (a.conj() * b).to_rotvec().length()
}
