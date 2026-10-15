//! Physics: per-rig segment masses from `rig.json`. Extends ragdoll.rs SEGMENTS (pose order slot:
//! Avian and ragdoll). Pure, no deps: the game crate parses rig.json with serde and passes plain
//! numbers here. Order = file order.

#[derive(Clone, Debug, PartialEq)]
pub struct RigMasses { pub total: f32, pub segments: Vec<(String, f32)> }

/// Scale the segment weights so they sum exactly to `total` kg for the chosen rig.
pub fn rig_masses(weights: &[(&str, f32)], total: f32) -> RigMasses {
    let sum: f32 = weights.iter().map(|w| w.1).sum();
    let k = total / sum.max(1e-6);
    RigMasses { total, segments: weights.iter().map(|&(n, w)| (n.to_string(), w * k)).collect() }
}

