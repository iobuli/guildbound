//! Donor GLB parsing with the `gltf` crate (no Bevy assets): node hierarchy, rest transforms,
//! skin-0 joint list and every animation's rotation/translation curves.
//! Based on the loader that lived in `motion_library.rs` (`load` + `sample`).
use super::Skeleton;
use bevy::prelude::*;
use gltf::animation::util::ReadOutputs;
use gltf::animation::{Interpolation, Property};
use std::collections::HashMap;
use std::path::Path;

/// A keyframed curve. Cubic-spline glTF samplers are reduced to their value keys.
#[derive(Clone, Debug, Default)]
pub struct Curve<T> {
    pub times: Vec<f32>,
    pub values: Vec<T>,
    pub step: bool,
}

impl<T> Curve<T> {
    /// Surrounding key indices and the blend factor between them for time `t` (clamped).
    /// `None` for an empty curve.
    fn locate(&self, t: f32) -> Option<(usize, usize, f32)> {
        let n = self.times.len().min(self.values.len());
        if n == 0 {
            return None;
        }
        let first = self.times[0];
        let last = self.times[n - 1];
        // NaN goes to the first key; ±inf clamp naturally.
        if n == 1 || t.is_nan() || t <= first {
            return Some((0, 0, 0.0));
        }
        if t >= last {
            return Some((n - 1, n - 1, 0.0));
        }
        // First key strictly after `t`; in 1..n because first <= t < last.
        let i1 = self.times[..n].partition_point(|&x| x <= t).clamp(1, n - 1);
        let i0 = i1 - 1;
        if self.step {
            return Some((i0, i0, 0.0));
        }
        let span = self.times[i1] - self.times[i0];
        let u = if span > 1e-9 {
            ((t - self.times[i0]) / span).clamp(0.0, 1.0)
        } else {
            1.0
        };
        Some((i0, i1, u))
    }
}

impl Curve<Quat> {
    /// Slerp sample, clamped to the curve's range.
    pub fn sample(&self, t: f32) -> Quat {
        match self.locate(t) {
            None => Quat::IDENTITY,
            Some((i0, i1, u)) if i0 == i1 || u <= 0.0 => self.values[i0],
            Some((i0, i1, u)) => safe_normalize(self.values[i0].slerp(self.values[i1], u)),
        }
    }
}

impl Curve<Vec3> {
    pub fn sample(&self, t: f32) -> Vec3 {
        match self.locate(t) {
            None => Vec3::ZERO,
            Some((i0, i1, u)) if i0 == i1 || u <= 0.0 => self.values[i0],
            Some((i0, i1, u)) => self.values[i0].lerp(self.values[i1], u),
        }
    }
}

/// Normalize a quaternion, falling back to identity for zero / non-finite input.
pub(crate) fn safe_normalize(q: Quat) -> Quat {
    let len_sq = q.length_squared();
    if len_sq.is_finite() && len_sq > 1e-12 {
        q * len_sq.sqrt().recip()
    } else {
        Quat::IDENTITY
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
        let Some(n) = self.nodes.get(node) else {
            return Transform::IDENTITY;
        };
        let mut world = n.rest;
        let mut cursor = n.parent;
        // Bounded walk: a malformed (cyclic) hierarchy cannot hang us.
        let mut guard = self.nodes.len();
        while let Some(p) = cursor {
            if guard == 0 {
                break;
            }
            guard -= 1;
            world = self.nodes[p].rest.mul_transform(world);
            cursor = self.nodes[p].parent;
        }
        world
    }
    /// Model-space transforms of every node for `clip` at time `t` (unanimated channels use rest).
    pub fn world_at(&self, clip: &DonorClip, t: f32) -> Vec<Transform> {
        let locals: Vec<Transform> = (0..self.nodes.len())
            .map(|i| {
                let mut local = self.nodes[i].rest;
                if let Some(c) = clip.rotations.get(&i)
                    && !c.times.is_empty()
                {
                    local.rotation = c.sample(t);
                }
                if let Some(c) = clip.translations.get(&i)
                    && !c.times.is_empty()
                {
                    local.translation = c.sample(t);
                }
                local
            })
            .collect();
        self.compose_world(&locals)
    }
    /// Model-space transforms from per-node local transforms (any node order).
    fn compose_world(&self, locals: &[Transform]) -> Vec<Transform> {
        let n = self.nodes.len();
        let mut world: Vec<Option<Transform>> = vec![None; n];
        let mut stack = Vec::new();
        for i in 0..n {
            if world[i].is_some() {
                continue;
            }
            // Push the uncomputed ancestor chain, then resolve it top-down.
            let mut cursor = Some(i);
            while let Some(c) = cursor {
                if world[c].is_some() || stack.len() > n {
                    break;
                }
                stack.push(c);
                cursor = self.nodes[c].parent;
            }
            while let Some(c) = stack.pop() {
                let parent = self.nodes[c].parent.and_then(|p| world[p]);
                world[c] = Some(match parent {
                    Some(pw) => pw.mul_transform(locals[c]),
                    None => locals[c],
                });
            }
        }
        world.into_iter().map(|w| w.unwrap_or_default()).collect()
    }
}

/// Parse `path` (absolute) as donor library `lib`.
pub fn load(lib: &str, path: &Path) -> Result<DonorLib, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let gltf = gltf::Gltf::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let blob = gltf.blob.as_deref();
    let buffers = |buffer: gltf::Buffer| match buffer.source() {
        gltf::buffer::Source::Bin => blob,
        gltf::buffer::Source::Uri(_) => None,
    };

    let count = gltf.nodes().len();
    let mut parent = vec![None; count];
    for node in gltf.nodes() {
        for child in node.children() {
            parent[child.index()] = Some(node.index());
        }
    }
    let nodes: Vec<DonorNode> = gltf
        .nodes()
        .map(|node| {
            let (t, r, s) = node.transform().decomposed();
            let rest = Transform {
                translation: finite_vec(Vec3::from_array(t), Vec3::ZERO),
                rotation: safe_normalize(Quat::from_array(r)),
                scale: finite_vec(Vec3::from_array(s), Vec3::ONE),
            };
            DonorNode {
                name: node
                    .name()
                    .map_or_else(|| format!("node{}", node.index()), str::to_string),
                parent: parent[node.index()],
                rest,
            }
        })
        .collect();

    let mut joints: Vec<usize> = gltf
        .skins()
        .next()
        .map(|skin| skin.joints().map(|j| j.index()).collect())
        .unwrap_or_default();
    // Local Sparking cache clips are animation-only GLBs with the named canonical hierarchy.
    if joints.is_empty() && nodes.iter().any(|n| n.name == "pelvis")
        && nodes.iter().any(|n| n.name == "hand_l") {
        joints = (0..nodes.len()).collect();
    }
    if joints.is_empty() {
        return Err(format!("{}: no skin 0 joints", path.display()));
    }

    let mut clips = Vec::new();
    for animation in gltf.animations() {
        let name = animation
            .name()
            .map_or_else(|| format!("anim{}", animation.index()), str::to_string);
        let mut duration = 0.0f32;
        let mut rotations = HashMap::new();
        let mut translations = HashMap::new();
        for channel in animation.channels() {
            let node = channel.target().node().index();
            let interpolation = channel.sampler().interpolation();
            let reader = channel.reader(buffers);
            let Some(inputs) = reader.read_inputs() else {
                continue;
            };
            let times: Vec<f32> = inputs.collect();
            if let Some(&last) = times.last()
                && last.is_finite()
            {
                duration = duration.max(last);
            }
            let property = channel.target().property();
            if !matches!(property, Property::Rotation | Property::Translation) {
                continue;
            }
            match reader.read_outputs() {
                Some(ReadOutputs::Rotations(values)) => {
                    let values: Vec<Quat> = values
                        .into_f32()
                        .map(|q| safe_normalize(Quat::from_array(q)))
                        .collect();
                    if let Some(curve) = make_curve(times, values, interpolation) {
                        rotations.insert(node, curve);
                    }
                }
                Some(ReadOutputs::Translations(values)) => {
                    let values: Vec<Vec3> = values.map(Vec3::from_array).collect();
                    if let Some(curve) = make_curve(times, values, interpolation) {
                        translations.insert(node, curve);
                    }
                }
                _ => {}
            }
        }
        clips.push(DonorClip {
            name,
            duration,
            rotations,
            translations,
        });
    }

    Ok(DonorLib {
        lib: lib.to_string(),
        nodes,
        joints,
        clips,
    })
}

fn finite_vec(v: Vec3, fallback: Vec3) -> Vec3 {
    if v.is_finite() { v } else { fallback }
}

trait Finite {
    fn finite(&self) -> bool;
}
impl Finite for Quat {
    fn finite(&self) -> bool {
        self.is_finite()
    }
}
impl Finite for Vec3 {
    fn finite(&self) -> bool {
        self.is_finite()
    }
}

/// Build a curve from raw sampler data: cubic-spline triplets reduce to their value element,
/// non-finite or out-of-order keys are dropped. `None` when nothing usable remains.
fn make_curve<T: Copy + Finite>(
    times: Vec<f32>,
    values: Vec<T>,
    interpolation: Interpolation,
) -> Option<Curve<T>> {
    let values: Vec<T> = if interpolation == Interpolation::CubicSpline {
        if values.len() != times.len() * 3 {
            return None;
        }
        values.chunks_exact(3).map(|c| c[1]).collect()
    } else {
        values
    };
    if values.len() != times.len() || times.is_empty() {
        return None;
    }
    let mut curve = Curve {
        times: Vec::with_capacity(times.len()),
        values: Vec::with_capacity(values.len()),
        step: interpolation == Interpolation::Step,
    };
    for (t, v) in times.into_iter().zip(values) {
        if !t.is_finite() || !v.finite() {
            continue;
        }
        if curve.times.last().is_some_and(|&prev| t <= prev) {
            continue;
        }
        curve.times.push(t);
        curve.values.push(v);
    }
    (!curve.times.is_empty()).then_some(curve)
}

/// Canonical skeleton from a library's skin-0 joints (used with the UAL2 model library).
/// `base` = model-space transform of the first joint's parent chain.
pub fn skeleton(lib: &DonorLib) -> Skeleton {
    let joint_of: HashMap<usize, usize> = lib
        .joints
        .iter()
        .enumerate()
        .map(|(i, &node)| (node, i))
        .collect();
    let names: Vec<String> = lib.joints.iter().map(|&n| lib.nodes[n].name.clone()).collect();
    let parent: Vec<Option<usize>> = lib
        .joints
        .iter()
        .map(|&n| lib.nodes[n].parent.and_then(|p| joint_of.get(&p).copied()))
        .collect();
    let rest: Vec<Transform> = lib.joints.iter().map(|&n| lib.nodes[n].rest).collect();
    let base = lib
        .joints
        .first()
        .and_then(|&n| lib.nodes[n].parent)
        .map_or(Transform::IDENTITY, |p| lib.rest_world(p));
    let index = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.clone(), i))
        .collect();
    Skeleton {
        names,
        parent,
        rest,
        base,
        index,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::OnceLock;

    /// Every donor library, loaded once for all tests in the crate.
    pub(crate) fn libs() -> &'static [DonorLib] {
        static LIBS: OnceLock<Vec<DonorLib>> = OnceLock::new();
        LIBS.get_or_init(|| {
            super::super::LIBRARIES
                .iter()
                .map(|(lib, rel)| {
                    load(lib, &Path::new(crate::ASSETS).join(rel))
                        .unwrap_or_else(|e| panic!("load {lib}: {e}"))
                })
                .collect()
        })
    }

    pub(crate) fn lib(name: &str) -> &'static DonorLib {
        libs().iter().find(|l| l.lib == name).expect("library")
    }

    #[test]
    fn loads_all_libraries() {
        let expected = [("ual1", 45), ("ual2", 43), ("sekiro", 57)];
        for (name, clips) in expected {
            let l = lib(name);
            assert_eq!(l.clips.len(), clips, "{name} clip count");
            assert!(l.joints.len() >= 65, "{name} joints {}", l.joints.len());
            for c in &l.clips {
                assert!(c.duration > 0.0, "{name}/{} duration", c.name);
                assert!(!c.rotations.is_empty(), "{name}/{} rotations", c.name);
            }
        }
    }

    #[test]
    fn canonical_skeleton() {
        let s = skeleton(lib("ual2"));
        assert_eq!(s.len(), 65);
        assert!(super::super::Bones::new(&s).is_some());
        assert_eq!(s.names[0], "root");
        assert!(s.parent[0].is_none());
        for i in 1..s.len() {
            let p = s.parent[i].expect("non-root bone has a parent joint");
            assert!(p < i, "parents precede children ({} -> {})", s.names[i], s.names[p]);
        }
        // Rest FK: pelvis ≈ 0.92 m, head ≈ 1.57 m, feet near the ground.
        let b = super::super::Bones::new(&s).unwrap();
        let w = s.fk(&s.rest_pose(), b.pelvis);
        assert!((w[b.pelvis].translation.y - 0.92).abs() < 0.05);
        assert!((w[b.head].translation.y - 1.57).abs() < 0.08);
        assert!(w[b.foot[0]].translation.y < 0.15);
        // Left = +X.
        assert!(w[b.hand[0]].translation.x > 0.3 && w[b.hand[1]].translation.x < -0.3);
    }

    #[test]
    fn curve_sampling() {
        let c = Curve {
            times: vec![0.0, 1.0, 2.0],
            values: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            step: false,
        };
        assert_eq!(c.sample(-1.0), Vec3::ZERO);
        assert!((c.sample(0.5) - Vec3::X * 0.5).length() < 1e-6);
        assert!((c.sample(1.5) - Vec3::new(0.5, 0.5, 0.0)).length() < 1e-6);
        assert_eq!(c.sample(9.0), Vec3::Y);
        assert_eq!(c.sample(f32::NAN), Vec3::ZERO);
        let s = Curve { step: true, ..c };
        assert_eq!(s.sample(1.9), Vec3::X);
        let q = Curve {
            times: vec![0.0, 1.0],
            values: vec![Quat::IDENTITY, Quat::from_rotation_y(1.0)],
            step: false,
        };
        assert!(q.sample(0.5).angle_between(Quat::from_rotation_y(0.5)) < 2e-3);
        assert!(Curve::<Quat>::default().sample(0.3).is_finite());
    }
}
