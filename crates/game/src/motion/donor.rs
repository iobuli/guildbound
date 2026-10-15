//! Donor GLB parsing with the `gltf` crate (no Bevy assets): node hierarchy, rest transforms,
//! skin-0 joint list and every animation's rotation/translation curves.
//! Based on the loader that lived in `motion_library.rs` (`load` + `sample`).
use super::Skeleton;
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// A keyframed curve. Cubic-spline glTF samplers are reduced to their value keys.
#[derive(Clone, Debug, Default)]
pub struct Curve<T> {
    pub times: Vec<f32>,
    pub values: Vec<T>,
    pub step: bool,
}

impl Curve<Quat> {
    /// Slerp sample, clamped to the curve's range.
    pub fn sample(&self, t: f32) -> Quat {
        todo!("donor::Curve<Quat>::sample {t}")
    }
}

impl Curve<Vec3> {
    pub fn sample(&self, t: f32) -> Vec3 {
        todo!("donor::Curve<Vec3>::sample {t}")
    }
}

#[derive(Clone, Debug)]
pub struct DonorNode {
    pub name: String,
    pub parent: Option<usize>,
    /// Rest LOCAL transform (from the node's TRS).
    pub rest: Transform,
}

#[derive(Clone, Debug)]
pub struct DonorClip {
    /// glTF animation name verbatim.
    pub name: String,
    pub duration: f32,
    /// Keyed by node index.
    pub rotations: HashMap<usize, Curve<Quat>>,
    pub translations: HashMap<usize, Curve<Vec3>>,
}

#[derive(Clone, Debug)]
pub struct DonorLib {
    /// `ual1` / `ual2` / `sekiro`.
    pub lib: String,
    pub nodes: Vec<DonorNode>,
    /// Node indices of skin 0's joints, in skin order.
    pub joints: Vec<usize>,
    pub clips: Vec<DonorClip>,
}

impl DonorLib {
    pub fn node(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }
    /// Model-space rest transform of a node (full parent chain incl. armature nodes).
    pub fn rest_world(&self, node: usize) -> Transform {
        todo!("donor::DonorLib::rest_world {node}")
    }
    /// Model-space transforms of every node for `clip` at time `t` (unanimated channels use rest).
    pub fn world_at(&self, clip: &DonorClip, t: f32) -> Vec<Transform> {
        todo!("donor::DonorLib::world_at {} {t}", clip.name)
    }
}

/// Parse `path` (absolute) as donor library `lib`.
pub fn load(lib: &str, path: &Path) -> Result<DonorLib, String> {
    todo!("donor::load {lib} {}", path.display())
}

/// Canonical skeleton from a library's skin-0 joints (used with the UAL2 model library).
/// `base` = model-space transform of the first joint's parent chain.
pub fn skeleton(lib: &DonorLib) -> Skeleton {
    todo!("donor::skeleton {}", lib.lib)
}
