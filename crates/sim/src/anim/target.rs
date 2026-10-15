//! Bone targets and their rates. All f64 at the fixed DT = 1/120 s.
//! With Δ₊ = Log(q₊ q₀⁻¹), Δ₋ = Log(q₀ q₋⁻¹) as FULL-angle rotation vectors (`Quat::to_rotvec`):
//!   ω = (Δ₊ + Δ₋) / (2·DT),   α = (Δ₊ − Δ₋) / DT².
//! Rates are in the PARENT BONE frame; the motor writer maps them to world with `rates_to_world_rel`.

use super::{DT, quat_xyzw, rig::Rig};
use crate::api::{JointId, Target};
use crate::math::{Quat, Vec3};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct BoneTarget {
    pub rot: Quat,
    pub omega: Vec3,
    pub alpha: Vec3,
}

impl BoneTarget {
    /// The api motor target for `joint` (rig.json index; the caller maps skin joint -> capsule).
    pub fn to_api(&self, joint: u16) -> Target {
        Target {
            joint: JointId(joint),
            q: self.rot,
            w: self.omega,
            alpha_ff: self.alpha,
        }
    }
}

/// ω and α at q₀ from neighbours one tick either side.
pub fn derivs(qm: Quat, q0: Quat, qp: Quat) -> (Vec3, Vec3) {
    let dp = (qp * q0.conj()).to_rotvec();
    let dm = (q0 * qm.conj()).to_rotvec();
    ((dp + dm) * (0.5 / DT), (dp - dm) * (1.0 / (DT * DT)))
}

/// Build a target from optional neighbours. A missing side is mirrored (α = 0, one-sided ω),
/// never a silent zero ω.
pub fn target_from(qm: Option<Quat>, q0: Quat, qp: Option<Quat>) -> BoneTarget {
    let mirror = |o: Quat| q0 * o.conj() * q0;
    let (qm, qp) = match (qm, qp) {
        (Some(a), Some(z)) => (a, z),
        (Some(a), None) => (a, mirror(a)),
        (None, Some(z)) => (mirror(z), z),
        (None, None) => (q0, q0),
    };
    let (omega, alpha) = derivs(qm, q0, qp);
    BoneTarget {
        rot: q0,
        omega,
        alpha,
    }
}

/// Optional rest-rig conversion shared by all abilities: with a rig, model-space hinge deltas become
/// local post-multiply rotations (`Rig::to_local_post`).
#[derive(Clone, Default)]
pub struct Frame {
    pub rig: Option<Arc<Rig>>,
}

impl Frame {
    pub fn with_rig(rig: Arc<Rig>) -> Self {
        Self { rig: Some(rig) }
    }
    pub fn conv(&self, bone: u16, q: [f32; 4]) -> Quat {
        let q = quat_xyzw(q);
        match &self.rig {
            Some(r) if r.node_of(bone as usize).is_some() => r.to_local_post(bone as usize, q),
            _ => q,
        }
    }
    /// Sample `raw` at t−1, t, t+1 and build targets; neighbours are matched by bone id.
    pub fn targets(
        &self,
        raw: &dyn Fn(i64) -> Vec<(u16, [f32; 4])>,
        tick: u64,
    ) -> Vec<(u16, BoneTarget)> {
        let t = tick as i64;
        let (m, c, p) = (raw(t - 1), raw(t), raw(t + 1));
        let find =
            |v: &[(u16, [f32; 4])], b: u16| v.iter().find(|e| e.0 == b).map(|e| self.conv(b, e.1));
        c.iter()
            .map(|&(b, q)| (b, target_from(find(&m, b), self.conv(b, q), find(&p, b))))
            .collect()
    }
}

/// Map parent-bone-frame rates to world relative rates. `bp` MUST be B_p = R_p·O_p (parent capsule
/// world rotation times its bone offset), not R_p. `wp` = parent world angular velocity.
///   ω* = B_p·ω_L,   α* = B_p·α_L + ω_p × (B_p·ω_L)
pub fn rates_to_world_rel(bp: Quat, wp: Vec3, omega_l: Vec3, alpha_l: Vec3) -> (Vec3, Vec3) {
    let w = bp.rotate(omega_l);
    (w, bp.rotate(alpha_l) + wp.cross(w))
}

/// Skin joints strictly below `from` down to and including `to`, parent -> child.
/// None if `to` is not a descendant of `from`.
pub fn chain(rig: &Rig, from: usize, to: usize) -> Option<Vec<usize>> {
    let node_joint = rig.node_joint_table();
    let stop = *rig.joints.get(from)?;
    let mut n = *rig.joints.get(to)?;
    let mut out = Vec::new();
    while n != stop {
        if node_joint[n] != usize::MAX {
            out.push(node_joint[n]);
        }
        let p = rig.nodes[n].parent;
        if p < 0 {
            return None;
        }
        n = p as usize;
    }
    out.reverse();
    Some(out)
}

/// Capsule target: ordered product of the chain's full local targets (parent -> child), with ω/α taken
/// from that product at t−1, t, t+1 (exact cross terms). `local(t, joint)` = final local rotation at tick t.
pub fn chain_target(chain: &[usize], local: &dyn Fn(i64, usize) -> Quat, tick: u64) -> BoneTarget {
    let prod = |t: i64| {
        chain
            .iter()
            .fold(Quat::IDENTITY, |acc, &j| acc * local(t, j))
            .normalize()
    };
    let t = tick as i64;
    target_from(Some(prod(t - 1)), prod(t), Some(prod(t + 1)))
}
