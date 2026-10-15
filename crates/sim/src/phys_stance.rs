//! Physics: stance weight shares (amendment s7). Contact moved to phys_balance::Contact and the
//! gravity hold to phys_balance::gravity_ff in pass 6.
//! Extends phys_balance; pose order slot: Avian and ragdoll. Pure f32, bones in BoneIdx order.

use crate::phys_math::*;

/// Weight share per planted foot (sums to 1 across planted feet): inverse horizontal distance
/// from each foot to the COM, so the foot under the body carries more.
pub fn weight_shares(com: V3, feet: &[(V3, bool)]) -> Vec<f32> {
    let w: Vec<f32> = feet.iter().map(|&(p, on)| if on {
        let (dx, dz) = (p[0] - com[0], p[2] - com[2]);
        1.0 / ((dx * dx + dz * dz).sqrt() + 0.02)
    } else { 0.0 }).collect();
    let s: f32 = w.iter().sum();
    if s > 0.0 { w.iter().map(|x| x / s).collect() } else { w }
}
