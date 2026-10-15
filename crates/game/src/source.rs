//! GLB motion converted from source bone frames to the ragdoll's world-aligned rest frames.
use bevy::prelude::*;
use gltf::animation::{Interpolation, util::ReadOutputs};
use guildbound_sim::{anim::target::target_from, api::Target, math::Quat as SQuat};

use crate::rig::{Seg, Shape};

struct Track {
    node: usize,
    times: Vec<f32>,
    values: Values,
    step: bool,
}
enum Values {
    Translation(Vec<Vec3>),
    Rotation(Vec<Quat>),
    Scale(Vec<Vec3>),
}
struct Clip {
    name: String,
    duration: f32,
    tracks: Vec<Track>,
}

pub struct SourceMotion {
    rest: Vec<Transform>,
    parents: Vec<Option<usize>>,
    bones: Vec<usize>,
    ends: Vec<Option<usize>>,
    clips: Vec<Clip>,
    base: Vec<Mat4>,
}

impl SourceMotion {
    pub fn load(model: &str) -> Result<Self, String> {
        let file = match model {
            "master" => "hero/master",
            "hero" => "hero/fighter",
            "peasant" | "dummy" => "npc/male_peasant",
            "mannequin" => "hero/mannequin",
            _ => return Err(format!("unknown source model {model}")),
        };
        let path = format!(
            "{}/../../assets/packs/{file}.glb",
            env!("CARGO_MANIFEST_DIR")
        );
        let glb = gltf::Gltf::open(&path).map_err(|e| format!("{path}: {e}"))?;
        let blob = glb.blob.as_deref().ok_or("source must be a binary GLB")?;
        let nodes: Vec<_> = glb.nodes().collect();
        let rest = nodes
            .iter()
            .map(|n| {
                let (t, r, s) = n.transform().decomposed();
                Transform {
                    translation: Vec3::from(t),
                    rotation: Quat::from_array(r).normalize(),
                    scale: Vec3::from(s),
                }
            })
            .collect();
        let mut parents = vec![None; nodes.len()];
        for n in &nodes {
            for c in n.children() {
                parents[c.index()] = Some(n.index());
            }
        }
        let find = |name: &str| {
            nodes
                .iter()
                .find(|n| n.name() == Some(name))
                .map(|n| n.index())
        };
        let segments = crate::rig::segments();
        let bones: Vec<_> = segments
            .iter()
            .map(|s| find(s.name).ok_or_else(|| format!("{path}: missing {}", s.name)))
            .collect::<Result<_, _>>()?;
        let end_names = [
            "B_Spine2",
            "B_Neck",
            "",
            "B_L_Forearm",
            "B_L_Forearm.001",
            "B_R_Forearm",
            "B_R_Hand",
            "B_L_Calf",
            "B_L_Foot",
            "B_R_Calf",
            "B_R_Foot",
            "B_L_Toe0",
            "B_R_Toe0",
        ];
        let ends = end_names.iter().map(|n| find(n)).collect();
        let mut clips = Vec::new();
        for (kind, names) in [
            ("idle", &["Unarmed-Idle", "nc_idle", "npc_idle"][..]),
            ("walk", &["nc_walk_forward", "npc_walk"][..]),
            ("run", &["nc_run_forward", "npc_run"][..]),
        ] {
            let Some(animation) = names
                .iter()
                .find_map(|name| glb.animations().find(|a| a.name() == Some(name)))
            else {
                continue;
            };
            let name = animation.name().unwrap();
            let mut tracks = Vec::new();
            let mut duration = 0.0f32;
            for channel in animation.channels() {
                let interpolation = channel.sampler().interpolation();
                if interpolation == Interpolation::CubicSpline {
                    return Err(format!("{name}: cubic tracks are not supported"));
                }
                let reader = channel.reader(|b| match b.source() {
                    gltf::buffer::Source::Bin => Some(blob),
                    _ => None,
                });
                let times: Vec<_> = reader
                    .read_inputs()
                    .ok_or("missing animation times")?
                    .collect();
                if times.is_empty() {
                    return Err(format!("{name}: empty animation track"));
                }
                duration = duration.max(*times.last().unwrap());
                let values = match reader.read_outputs().ok_or("missing animation values")? {
                    ReadOutputs::Translations(v) => {
                        Values::Translation(v.map(Vec3::from).collect())
                    }
                    ReadOutputs::Rotations(v) => Values::Rotation(
                        v.into_f32()
                            .map(|q| Quat::from_array(q).normalize())
                            .collect(),
                    ),
                    ReadOutputs::Scales(v) => Values::Scale(v.map(Vec3::from).collect()),
                    ReadOutputs::MorphTargetWeights(_) => continue,
                };
                let count = match &values {
                    Values::Translation(v) | Values::Scale(v) => v.len(),
                    Values::Rotation(v) => v.len(),
                };
                if count != times.len() {
                    return Err(format!("{name}: animation sample count mismatch"));
                }
                tracks.push(Track {
                    node: channel.target().node().index(),
                    times,
                    values,
                    step: interpolation == Interpolation::Step,
                });
            }
            clips.push(Clip {
                name: kind.into(),
                duration,
                tracks,
            });
        }
        println!(
            "[source] {path}: {} mapped segments; motion {:?}",
            bones.len(),
            clips.iter().map(|c| c.name.as_str()).collect::<Vec<_>>()
        );
        let mut source = Self {
            rest,
            parents,
            bones,
            ends,
            clips,
            base: Vec::new(),
        };
        source.base = source.world_pose(0, 0.0);
        Ok(source)
    }

    fn world_pose(&self, clip: usize, seconds: f32) -> Vec<Mat4> {
        let mut local = self.rest.clone();
        if let Some(clip) = self.clips.get(clip) {
            let time = if clip.duration > 0.0 {
                seconds.rem_euclid(clip.duration)
            } else {
                0.0
            };
            for track in &clip.tracks {
                let lo = track
                    .times
                    .partition_point(|&t| t <= time)
                    .saturating_sub(1);
                let hi = (lo + 1).min(track.times.len() - 1);
                let span = track.times[hi] - track.times[lo];
                let u = if track.step || span <= 0.0 {
                    0.0
                } else {
                    ((time - track.times[lo]) / span).clamp(0.0, 1.0)
                };
                match &track.values {
                    Values::Translation(v) => local[track.node].translation = v[lo].lerp(v[hi], u),
                    Values::Rotation(v) => {
                        local[track.node].rotation = v[lo].slerp(v[hi], u).normalize()
                    }
                    Values::Scale(v) => local[track.node].scale = v[lo].lerp(v[hi], u),
                }
            }
        }
        fn world(
            i: usize,
            local: &[Transform],
            parents: &[Option<usize>],
            cache: &mut [Option<Mat4>],
        ) -> Mat4 {
            if let Some(m) = cache[i] {
                return m;
            }
            let m = parents[i]
                .map(|p| world(p, local, parents, cache))
                .unwrap_or(Mat4::IDENTITY)
                * local[i].to_matrix();
            cache[i] = Some(m);
            m
        }
        let mut cache = vec![None; local.len()];
        (0..local.len())
            .map(|i| world(i, &local, &self.parents, &mut cache))
            .collect()
    }

    pub fn fit(&self, segments: &mut [Seg]) {
        let p = |i: usize| self.base[i].transform_point3(Vec3::ZERO);
        let root = p(self.bones[0]);
        let ground = p(self.bones[11]).y.min(p(self.bones[12]).y) - 0.08;
        let offset = Vec3::new(root.x, ground, root.z);
        for (i, s) in segments.iter_mut().enumerate() {
            s.anchor = p(self.bones[i]) - offset;
            s.end = self.ends[i]
                .map(|n| p(n) - offset)
                .unwrap_or(s.anchor + Vec3::Y * 0.2);
            if let Shape::Capsule(r) = &mut s.shape {
                *r = (*r).min((s.end - s.anchor).length() * 0.45);
            }
        }
    }

    fn rotations(&self, clip: usize, seconds: f32, parents: &[Option<usize>]) -> Vec<SQuat> {
        let pose = self.world_pose(clip, seconds);
        let world: Vec<_> = self
            .bones
            .iter()
            .map(|&n| {
                let (_, q, _) = pose[n].to_scale_rotation_translation();
                let (_, base, _) = self.base[n].to_scale_rotation_translation();
                (q * base.inverse()).normalize()
            })
            .collect();
        parents
            .iter()
            .enumerate()
            .map(|(i, parent)| {
                let q = parent
                    .map(|p| world[p].inverse() * world[i])
                    .unwrap_or(Quat::IDENTITY)
                    .normalize();
                SQuat {
                    x: q.x as f64,
                    y: q.y as f64,
                    z: q.z as f64,
                    w: q.w as f64,
                }
            })
            .collect()
    }

    pub fn targets(&self, tick: u64, speed: f32, parents: &[Option<usize>]) -> Vec<Target> {
        self.targets_at(tick as f32 / 120.0, speed, parents, 1.0)
    }

    pub fn targets_at(
        &self,
        t: f32,
        speed: f32,
        parents: &[Option<usize>],
        rate: f32,
    ) -> Vec<Target> {
        let name = if speed > 2.0 {
            "run"
        } else if speed > 0.1 {
            "walk"
        } else {
            "idle"
        };
        let clip = self
            .clips
            .iter()
            .position(|c| c.name == name)
            .or_else(|| {
                self.clips
                    .iter()
                    .position(|c| c.name == "walk" && speed > 0.1)
            })
            .unwrap_or(0);
        let c = self.rotations(clip, t, parents);
        let m = self.rotations(clip, (t - rate / 120.0).max(0.0), parents);
        let p = self.rotations(clip, t + rate / 120.0, parents);
        c.iter()
            .enumerate()
            .map(|(i, &q)| {
                target_from((t >= rate / 120.0).then_some(m[i]), q, Some(p[i])).to_api(i as u16)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_rigs_fit_and_reference_frame_zero_has_no_bind_twist() {
        for model in ["master", "hero", "mannequin", "peasant"] {
            let source = SourceMotion::load(model).unwrap();
            let mut segments = crate::rig::segments();
            source.fit(&mut segments);
            let parents: Vec<_> = segments.iter().map(|s| s.parent).collect();
            for (i, q) in source.rotations(0, 0.0, &parents).iter().enumerate() {
                assert!(
                    q.to_rotvec().length() < 1e-5,
                    "{model} joint {i} has a bind-frame error"
                );
            }
            assert!(
                segments[0].anchor.y > 0.5 && segments[0].anchor.y < 1.5,
                "{model} source units"
            );
            for s in &segments {
                assert!(s.anchor.is_finite() && s.end.is_finite());
                assert!(
                    s.length() > 0.02 && s.length() < 0.8,
                    "{model} {} length {}",
                    s.name,
                    s.length()
                );
            }
            for tick in [0, 1, 60, 120, 4800] {
                for target in source.targets(tick, 0.0, &parents) {
                    assert!(target.w.is_finite() && target.alpha_ff.is_finite());
                    assert!(target.q.to_rotvec().is_finite());
                }
            }
        }
    }

    #[test]
    fn collapsed_joint_chain_reconstructs_source_world_rotation() {
        let source = SourceMotion::load("master").unwrap();
        let segments = crate::rig::segments();
        let parents: Vec<_> = segments.iter().map(|s| s.parent).collect();
        let pose = source.world_pose(0, 0.37);
        let local = source.rotations(0, 0.37, &parents);
        let mut world = vec![SQuat::IDENTITY; local.len()];
        let (_, root_q, _) = pose[source.bones[0]].to_scale_rotation_translation();
        let (_, root_base, _) = source.base[source.bones[0]].to_scale_rotation_translation();
        let root_delta = root_q * root_base.inverse();
        for i in 0..local.len() {
            world[i] = parents[i]
                .map(|p| world[p] * local[i])
                .unwrap_or(SQuat::IDENTITY);
            let (_, q, _) = pose[source.bones[i]].to_scale_rotation_translation();
            let (_, base, _) = source.base[source.bones[i]].to_scale_rotation_translation();
            let expected = (root_delta.inverse() * q * base.inverse()).normalize();
            let expected = SQuat {
                w: expected.w as f64,
                x: expected.x as f64,
                y: expected.y as f64,
                z: expected.z as f64,
            };
            assert!(
                (expected * world[i].conj()).to_rotvec().length() < 1e-4,
                "joint {i}"
            );
        }
    }
}
