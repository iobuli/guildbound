//! Real-time rig and pose layer distilled from the two supplied papers.
//!
//! Puppeteer's trained skeleton/weight networks and video optimizer are offline authoring
//! systems. Make-It-Animatable likewise needs learned weights and its training corpus. The
//! reproducible runtime pieces are implemented here: joint+parent tokens in hierarchical order,
//! graph-distance topology attention, canonical hip framing, parent-aware coarse/fine treatment,
//! body priors, and a closed-form per-frame version of the data + temporal regularization loss.
//! Bevy's existing forward kinematics and linear blend skinning remain the final deformation step.

use crate::{
    fingers, ranger,
    tuning::{Knob as K, Tuning},
};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Default, Resource)]
struct PaperRig {
    joints: Vec<Joint>,
    graph_distance: Vec<Vec<u8>>,
    symmetric_pairs: Vec<(usize, usize)>,
    generation: u32,
    initialized: bool,
}

struct Joint {
    entity: Entity,
    name: String,
    parent: Option<usize>,
    depth: u8,
    rest: Quat,
    previous: Vec3,
    previous2: Vec3,
    fine: bool,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<PaperRig>().add_systems(
        PostUpdate,
        solve_pose
            .in_set(Stage::Solve)
            .after(fingers::Stage::Fingers)
            .before(bevy::transform::TransformSystems::Propagate),
    );
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Stage {
    Solve,
}

fn is_rig_joint(name: &str) -> bool {
    name.starts_with("B_")
        || ["thumb_", "index_", "middle_", "ring_", "pinky_"]
            .iter()
            .any(|part| name.starts_with(part))
}

fn is_fine(name: &str) -> bool {
    ["Finger", "thumb_", "index_", "middle_", "ring_", "pinky_"]
        .iter()
        .any(|part| name.contains(part))
}

fn mirror_name(name: &str) -> Option<String> {
    if let Some(rest) = name.strip_prefix("B_L_") {
        Some(format!("B_R_{rest}"))
    } else if let Some(rest) = name.strip_prefix("B_R_") {
        Some(format!("B_L_{rest}"))
    } else if let Some(rest) = name.strip_suffix("_l") {
        Some(format!("{rest}_r"))
    } else {
        name.strip_suffix("_r").map(|rest| format!("{rest}_l"))
    }
}

fn tree_distance(joints: &[Joint], mut a: usize, mut b: usize) -> u8 {
    let mut distance = 0u8;
    while joints[a].depth > joints[b].depth {
        let Some(parent) = joints[a].parent else {
            return u8::MAX;
        };
        a = parent;
        distance = distance.saturating_add(1);
    }
    while joints[b].depth > joints[a].depth {
        let Some(parent) = joints[b].parent else {
            return u8::MAX;
        };
        b = parent;
        distance = distance.saturating_add(1);
    }
    while a != b {
        let (Some(pa), Some(pb)) = (joints[a].parent, joints[b].parent) else {
            return u8::MAX;
        };
        a = pa;
        b = pb;
        distance = distance.saturating_add(2);
    }
    distance
}

/// Build the compact joint/parent representation once the imported Ranger scene is ready.
/// Ordering is parent-first; spatial order is deliberately not used because both papers show
/// that hierarchy is the reliable representation for deep limbs and fingers.
pub(crate) fn calibrate(world: &mut World, scene_nodes: &[Entity]) {
    let mut candidates: Vec<(Entity, String, Quat, u8)> = scene_nodes
        .iter()
        .filter_map(|&entity| {
            let name = world.get::<Name>(entity)?.as_str();
            let transform = world.get::<Transform>(entity)?;
            if !is_rig_joint(name) {
                return None;
            }
            let mut depth = 0u8;
            let mut cursor = entity;
            while let Some(parent) = world.get::<ChildOf>(cursor) {
                depth = depth.saturating_add(1);
                cursor = parent.parent();
                if depth == u8::MAX {
                    break;
                }
            }
            Some((entity, name.to_string(), transform.rotation, depth))
        })
        .collect();
    candidates.sort_by_key(|entry| entry.3);
    let indices: HashMap<Entity, usize> = candidates
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.0, index))
        .collect();
    let mut joints: Vec<Joint> = Vec::with_capacity(candidates.len());
    for (entity, name, rest, _) in candidates {
        let mut cursor = world.get::<ChildOf>(entity).map(ChildOf::parent);
        let mut parent = None;
        while let Some(ancestor) = cursor {
            if let Some(&index) = indices.get(&ancestor) {
                parent = Some(index);
                break;
            }
            cursor = world.get::<ChildOf>(ancestor).map(ChildOf::parent);
        }
        let depth = parent
            .map(|p| joints[p].depth.saturating_add(1))
            .unwrap_or(0);
        joints.push(Joint {
            entity,
            fine: is_fine(&name),
            name,
            parent,
            depth,
            rest,
            previous: Vec3::ZERO,
            previous2: Vec3::ZERO,
        });
    }
    let count = joints.len();
    let graph_distance = (0..count)
        .map(|a| (0..count).map(|b| tree_distance(&joints, a, b)).collect())
        .collect();
    let by_name: HashMap<_, _> = joints
        .iter()
        .enumerate()
        .map(|(index, joint)| (joint.name.clone(), index))
        .collect();
    let symmetric_pairs = joints
        .iter()
        .enumerate()
        .filter_map(|(left, joint)| {
            let mirror = mirror_name(&joint.name)?;
            let &right = by_name.get(&mirror)?;
            (left < right).then_some((left, right))
        })
        .collect::<Vec<_>>();

    // Make-It-Animatable's canonical frame: hip at the origin, thigh line on X, hip-plane
    // normal on Z. It is recorded for future retarget/import tools; runtime poses stay local.
    let canonical = (|| {
        let hip =
            ranger::world_transform(world, joints[*by_name.get("B_Pelvis")?].entity)?.translation();
        let left = ranger::world_transform(world, joints[*by_name.get("B_L_Thigh")?].entity)?
            .translation();
        let right = ranger::world_transform(world, joints[*by_name.get("B_R_Thigh")?].entity)?
            .translation();
        let x = (left - right).normalize_or(Vec3::X);
        let mut z = (left - hip).cross(right - hip).normalize_or(Vec3::Z);
        let mut y = z.cross(x).normalize_or(Vec3::Y);
        if y.dot(Vec3::Y) < 0.0 {
            y = -y;
            z = -z;
        }
        Some(Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize())
    })()
    .unwrap_or(Quat::IDENTITY);

    let report_joints: Vec<_> = joints
        .iter()
        .map(|joint| {
            serde_json::json!({
                "name": joint.name,
                "parent": joint.parent.map(|p| joints[p].name.as_str()),
                "depth": joint.depth,
                "fine": joint.fine,
            })
        })
        .collect();
    let report = serde_json::json!({
        "sources": ["D:/2508.10898v1.pdf", "D:/2411.18197v3.pdf"],
        "representation": "joint position concept + parent index, hierarchical order",
        "runtime": "closed-form source/velocity/acceleration pose fit with graph-distance attention",
        "canonical_rotation_xyzw": canonical.to_array(),
        "joint_count": count,
        "symmetric_pairs": symmetric_pairs.len(),
        "joints": report_joints,
    });
    if std::fs::create_dir_all("profiles").is_ok() {
        let _ = std::fs::write(
            "profiles/paper_rig.json",
            serde_json::to_string_pretty(&report).unwrap(),
        );
    }
    println!(
        "[paper_motion] {count} joints in hierarchical order; {} mirrored pairs; canonical hip frame ready",
        symmetric_pairs.len()
    );
    world.insert_resource(PaperRig {
        joints,
        graph_distance,
        symmetric_pairs,
        generation: 0,
        initialized: false,
    });
}

fn shortest_scaled_axis(rotation: Quat) -> Vec3 {
    let q = if rotation.w < 0.0 {
        Quat::from_xyzw(-rotation.x, -rotation.y, -rotation.z, -rotation.w)
    } else {
        rotation
    };
    q.normalize().to_scaled_axis()
}

fn is_leg(name: &str) -> bool {
    name.contains("Thigh") || name.contains("Calf") || name.contains("Foot")
}

fn is_upper_limb(name: &str) -> bool {
    name.contains("UpperArm")
        || name.contains("Forearm")
        || name.contains("Hand")
        || name.contains("Finger")
        || name.starts_with("thumb_")
        || name.starts_with("index_")
        || name.starts_with("middle_")
        || name.starts_with("ring_")
        || name.starts_with("pinky_")
}

fn is_contact_landmark(name: &str) -> bool {
    name.contains("Foot") || name.contains("Toe") || name.contains("Hand") || name.contains("Head")
}

/// The optimized tangent vector is the exact minimizer of
///   wd||x-d||² + wv||x-x_prev||² + wa||x-(2x_prev-x_prev2)||².
/// `d` is the complete pose produced by native animation + procedural systems. The other terms
/// are Puppeteer's temporal regularizer in a one-frame online form, so no iterative GPU solve is
/// placed in the game loop.
fn solve_pose(world: &mut World) {
    let Some(actor) = ranger::combat_actor(world) else {
        return;
    };
    let dt = world
        .resource::<Time>()
        .delta_secs()
        .clamp(1.0 / 1000.0, 0.05);
    let feel = world.resource::<Tuning>().clone();
    let Some(mut rig) = world.remove_resource::<PaperRig>() else {
        return;
    };
    if rig.joints.is_empty() {
        world.insert_resource(rig);
        return;
    }

    let pose_limit = feel.get(K::PoseLimit).to_radians();
    let source_rotations: Vec<Quat> = rig
        .joints
        .iter()
        .map(|joint| {
            world
                .get::<Transform>(joint.entity)
                .map(|transform| transform.rotation)
                .unwrap_or(joint.rest)
        })
        .collect();
    let mut desired: Vec<Vec3> = rig
        .joints
        .iter()
        .zip(&source_rotations)
        .map(|(joint, &rotation)| {
            shortest_scaled_axis(joint.rest.inverse() * rotation).clamp_length_max(pose_limit)
        })
        .collect();

    if !rig.initialized || rig.generation != actor.generation {
        for (joint, &target) in rig.joints.iter_mut().zip(&desired) {
            joint.previous = target;
            joint.previous2 = target;
        }
        rig.generation = actor.generation;
        rig.initialized = true;
    }

    // Make-It-Animatable's body priors are used where symmetry is structurally meaningful.
    // Magnitude is paired while each leg retains its own axis and opposite gait phase.
    let symmetry = feel.get(K::SymmetryPrior);
    for &(a, b) in &rig.symmetric_pairs {
        if !is_leg(&rig.joints[a].name) || !is_leg(&rig.joints[b].name) {
            continue;
        }
        let ma = desired[a].length();
        let mb = desired[b].length();
        let average = (ma + mb) * 0.5;
        if ma > 1e-5 {
            desired[a] *= ma.lerp(average, symmetry) / ma;
        }
        if mb > 1e-5 {
            desired[b] *= mb.lerp(average, symmetry) / mb;
        }
    }

    // TAJA's learned QK term is unavailable without the paper's network weights. Its exact graph
    // distance positional term is retained: a normalized exp(-lambda * distance) neighborhood.
    // It measures whether a change is supported by nearby bones without mixing incompatible local axes.
    let falloff = feel.get(K::TopologyFalloff);
    let radius = feel.get(K::GraphRadius).round() as u8;
    let energy: Vec<f32> = desired.iter().map(|target| target.length()).collect();
    let neighborhood_energy: Vec<f32> = rig
        .graph_distance
        .iter()
        .map(|row| {
            let mut weighted = 0.0;
            let mut total = 0.0;
            for (index, &distance) in row.iter().enumerate() {
                if distance <= radius {
                    let weight = (-falloff * distance as f32).exp();
                    weighted += energy[index] * weight;
                    total += weight;
                }
            }
            weighted / total.max(1e-6)
        })
        .collect();

    // Residual freedom governs how much the final stabilizer is allowed to pull the authored,
    // terrain-fitted pose back toward its temporal prior.
    let influence = feel.get(K::PaperBlend) * (1.0 - 0.75 * feel.get(K::ResidualFreedom));
    let frame_scale = (1.0 / (dt * 60.0)).clamp(0.5, 2.0);
    let temporal = feel.get(K::TemporalFit) * frame_scale;
    let acceleration = feel.get(K::AccelerationReg) * frame_scale * frame_scale;
    let coherence = feel.get(K::ChainCoherence);
    let fine_detail = feel.get(K::FineDetail);
    let recovery = feel.get(K::SourceRecovery);
    let upper_active = world
        .resource::<crate::combat::Combat>()
        .upper_body_active();
    let active_freedom = feel.get(K::ActiveLimbFreedom);
    let contact_preserve = feel.get(K::ContactPreserve);
    let settled = (1.0 - actor.velocity.length() / 0.45).clamp(0.0, 1.0);
    let mut final_vectors = Vec::with_capacity(rig.joints.len());
    for (index, joint) in rig.joints.iter().enumerate() {
        let target = desired[index];
        let isolated = ((energy[index] - neighborhood_energy[index]).abs()
            / (0.2 + neighborhood_energy[index]))
            .clamp(0.0, 1.0);
        let depth = joint.depth as f32;
        let fine_scale = if joint.fine {
            1.0 - 0.82 * fine_detail
        } else {
            1.0
        };
        let responsive = if upper_active && is_upper_limb(&joint.name) {
            1.0 - active_freedom
        } else {
            1.0
        };
        let contact_response = if is_contact_landmark(&joint.name) {
            1.0 - contact_preserve * 0.88
        } else {
            1.0
        };
        let velocity_weight = temporal
            * 5.0
            * (1.0 + coherence * (isolated * 2.0 + depth * 0.035))
            * fine_scale
            * responsive
            * contact_response;
        let acceleration_weight = acceleration * 3.0 * fine_scale * responsive * contact_response;
        let near_source = (1.0 - target.length() / 0.35).clamp(0.0, 1.0);
        let data_weight = 1.0
            + recovery * settled * near_source * 5.0
            + if is_contact_landmark(&joint.name) {
                contact_preserve * 6.0
            } else {
                0.0
            };
        let predicted = joint.previous * 2.0 - joint.previous2;
        let solved = ((target * data_weight
            + joint.previous * velocity_weight
            + predicted * acceleration_weight)
            / (data_weight + velocity_weight + acceleration_weight).max(1e-6))
        .clamp_length_max(pose_limit);
        let preserve_authored =
            (actor.grounded && is_leg(&joint.name)) || (upper_active && is_upper_limb(&joint.name));
        final_vectors.push(if preserve_authored {
            // Terrain IK and active combat IK are the authoritative residual constraints.
            target
        } else {
            target.lerp(solved, influence)
        });
    }

    for (index, joint) in rig.joints.iter_mut().enumerate() {
        let vector = final_vectors[index];
        let solved = (joint.rest * Quat::from_scaled_axis(vector)).normalize();
        if let Some(mut transform) = world.get_mut::<Transform>(joint.entity) {
            transform.rotation = solved;
        }
        joint.previous2 = joint.previous;
        joint.previous = vector;
    }

    world.insert_resource(rig);
}
