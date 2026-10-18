//! The single bone writer. For every character with a `Rig`:
//! 1. evaluate `MotionLayers` bottom→top with `PoseBank::sample` + masks,
//! 2. `Springs::step_moving` (secondary motion, arm inertia, impulses, squash),
//!    then `body::resolve` (limbs out of the body, padded by the root's `BodyPadding`),
//! 3. `contact::apply` (feet/hands/lean/look) using Avian ray casts against `Phys::World`,
//! 4. write local rotations (+ pelvis translation) into the bone entities' `Transform`s, and the
//!    same pose in model space into `RigPose` (for armour plates riding on the body).
//! Runs in `PostUpdate` / `MotionSet::Compose` (after Bevy animation, before propagation).
use super::bank::PoseBank;
use super::contact::{self, ContactState, Ground};
use super::secondary::Springs;
use super::body::BodyPadding;
use super::{Mask, MotionLayers, MotionSet, Pose, Rig, RigBone, RigPose, Source};
use crate::play::Phys;
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashSet;

/// Global motion feel; edited by `play::tuning`.
#[derive(Resource, Clone, Debug)]
pub struct MotionFeel {
    pub style: super::timing::Style,
    pub springs: super::secondary::SpringParams,
    pub contact: super::contact::ContactParams,
    /// Character bounce (0..2, 1 = default): step dip, step squash rhythm, landing / jump
    /// squash and idle breathing, all requested by `play::select`.
    pub bounce: f32,
    /// Debug: draw skeleton gizmos.
    pub debug: bool,
}

impl Default for MotionFeel {
    fn default() -> Self {
        Self {
            style: default(),
            springs: default(),
            contact: default(),
            bounce: 1.0,
            debug: false,
        }
    }
}

/// One character's motion style on top of `MotionFeel` (from `play::traits`): multipliers for the
/// pose exaggeration, the holds of the snap curve and the step bounce (read by `play::select`).
/// Layers with their own `style` keep it.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct StyleScale {
    pub exaggeration: f32,
    pub hold: f32,
    pub bounce: f32,
}

impl Default for StyleScale {
    fn default() -> Self {
        Self {
            exaggeration: 1.0,
            hold: 1.0,
            bounce: 1.0,
        }
    }
}

impl StyleScale {
    /// `feel` with this character's style applied.
    pub fn apply(&self, feel: &MotionFeel) -> MotionFeel {
        let mut f = feel.clone();
        let ok = |x: f32| if x.is_finite() { x } else { 1.0 };
        f.style.exaggeration = (feel.style.exaggeration * ok(self.exaggeration)).clamp(0.5, 2.0);
        f.style.hold = (feel.style.hold * ok(self.hold)).clamp(0.0, 1.0);
        f
    }
}

/// Longest downward ground probe (m) from pelvis height.
const GROUND_PROBE: f32 = 3.0;
/// Arms keep this far from world surfaces (m); another body's capsule counts only this far in (m).
const ARM_CLEARANCE: f32 = 0.04;
const BODY_SLACK: f32 = 0.12;

pub fn plugin(app: &mut App) {
    app.init_resource::<MotionFeel>().add_systems(
        PostUpdate,
        (adopt_bank_style, compose)
            .chain()
            .in_set(MotionSet::Compose),
    );
}

/// Once the bank exists, adopt its `poses.json` style as the global default unless tuning
/// already changed `MotionFeel.style` (so `Layer.style = None` means the bank default).
fn adopt_bank_style(
    bank: Option<Res<PoseBank>>,
    mut feel: ResMut<MotionFeel>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(bank) = bank else { return };
    *done = true;
    if feel.style == super::timing::Style::default() && bank.style != feel.style {
        feel.style = bank.style;
    }
}

/// Bottom→top layer evaluation. Layers with a bad pose length or zero weight are skipped.
fn evaluate(
    bank: &PoseBank,
    layers: &MotionLayers,
    feel: &MotionFeel,
    warned: &mut HashSet<String>,
) -> Pose {
    let n = bank.skeleton.len();
    let mut pose = bank.rest.clone();
    if pose.rot.len() != n {
        pose = bank.skeleton.rest_pose();
    }
    let full = vec![1.0; n];
    for layer in &layers.layers {
        let weight = layer.weight;
        if !(weight > 1e-4) {
            continue;
        }
        let source = match &layer.source {
            Source::Track {
                name,
                time,
                looping,
            } => {
                let style = layer.style.unwrap_or(feel.style);
                let time = if time.is_finite() { *time } else { 0.0 };
                bank.sample(name, time, *looping, &style)
            }
            Source::Pose(p) => p.clone(),
        };
        if source.rot.len() != n {
            let name = layer_name(layer);
            if warned.insert(format!("len:{name}")) {
                warn!(
                    "[compose] layer {name} has {} bones, skeleton has {n}; skipped",
                    source.rot.len()
                );
            }
            continue;
        }
        let mask = match bank.masks.get(&layer.mask) {
            Some(m) if m.len() == n => m.as_slice(),
            _ => {
                if warned.insert(format!("mask:{:?}", layer.mask)) {
                    warn!(
                        "[compose] bank has no mask {:?}; using full body",
                        layer.mask
                    );
                }
                full.as_slice()
            }
        };
        let pelvis_weight = match layer.mask {
            Mask::Full | Mask::Lower => 1.0,
            _ => 0.0,
        };
        pose = pose.blend_masked(&source, weight.min(1.0), mask, pelvis_weight);
    }
    pose
}

fn layer_name(layer: &super::Layer) -> &str {
    match &layer.source {
        Source::Track { name, .. } => name,
        Source::Pose(_) => "<pose>",
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn compose(
    bank: Option<Res<PoseBank>>,
    feel: Res<MotionFeel>,
    time: Res<Time>,
    spatial: SpatialQuery,
    mut roots: Query<
        (
            Entity,
            &Rig,
            &mut MotionLayers,
            &mut Springs,
            &mut ContactState,
            &Transform,
            Option<&BodyPadding>,
            Option<&mut RigPose>,
            Option<&StyleScale>,
        ),
        Without<RigBone>,
    >,
    models: Query<&Transform, Without<RigBone>>,
    mut bone_transforms: Query<&mut Transform, With<RigBone>>,
    mut gizmos: Gizmos,
    mut warned: Local<HashSet<String>>,
) {
    let Some(bank) = bank else { return };
    let skeleton = &bank.skeleton;
    let bones = &bank.bones;
    let n = skeleton.len();
    let dt = time.delta_secs();
    let filter = SpatialQueryFilter::from_mask(Phys::World);

    for (root, rig, mut layers, mut springs, mut state, root_transform, padding, rig_pose, scale) in &mut roots {
        if rig.bones.len() != n {
            if warned.insert(format!("rig:{root}")) {
                warn!(
                    "[compose] {root}: rig has {} bones, skeleton {n}; not animated",
                    rig.bones.len()
                );
            }
            continue;
        }
        let own = scale.map(|s| s.apply(&feel));
        let mut pose = evaluate(&bank, &layers, own.as_ref().unwrap_or(&feel), &mut warned);

        let model_local = models.get(rig.model).copied().unwrap_or_default();
        let model_world = root_transform.mul_transform(model_local);

        if layers.raw {
            springs.follow(&pose);
        } else {
            let mut request = layers.contact.clone();
            pose = springs.step_moving(
                &pose,
                skeleton,
                bones,
                model_world.rotation,
                request.accel,
                &layers.impulses,
                request.squash,
                &feel.springs,
                dt,
            );
            request.squash = springs.squash;
            super::body::resolve(&mut pose, skeleton, bones, &padding.copied().unwrap_or_default());
            let mut ground = |origin: Vec3| {
                spatial
                    .cast_ray(origin, Dir3::NEG_Y, GROUND_PROBE, true, &filter)
                    .map(|hit| Ground {
                        point: origin - Vec3::Y * hit.distance,
                        normal: hit.normal,
                    })
            };
            contact::apply(
                &mut pose,
                skeleton,
                bones,
                &model_world,
                &request,
                &mut ground,
                &feel.contact,
                &mut state,
                dt,
            );
            // Arms out of walls, props and other bodies (not hands an IK target holds).
            let held = [0, 1].map(|i| request.hands[i].is_some_and(|(_, w)| w > 0.3));
            let others = SpatialQueryFilter::from_mask(Phys::Character).with_excluded_entities([root]);
            super::body::clear_world(&mut pose, skeleton, bones, &model_world, held, |p| {
                // Nearest surface (solid = false, so a point inside still gets its way out).
                let wall = spatial.project_point(p, false, &filter).and_then(|hit| {
                    let to_surface = hit.point - p;
                    let d = to_surface.length();
                    if hit.is_inside {
                        Some((to_surface.normalize_or(Vec3::Y), d + ARM_CLEARANCE))
                    } else {
                        (d < ARM_CLEARANCE).then(|| ((-to_surface).normalize_or(Vec3::Y), ARM_CLEARANCE - d))
                    }
                });
                // Capsules are fatter than the bodies inside them: only the inner part counts.
                wall.or_else(|| {
                    spatial.project_point(p, false, &others).filter(|h| h.is_inside).and_then(|hit| {
                        let to_surface = hit.point - p;
                        let depth = to_surface.length() - BODY_SLACK;
                        (depth > 0.0).then(|| (to_surface.normalize_or(Vec3::Y), depth))
                    })
                })
            });
        }
        layers.impulses.clear();
        if let Some(mut out) = rig_pose {
            out.0 = skeleton.fk(&pose, bones.pelvis);
        }

        for (i, &entity) in rig.bones.iter().enumerate() {
            if entity == Entity::PLACEHOLDER {
                continue;
            }
            let Ok(mut transform) = bone_transforms.get_mut(entity) else {
                continue;
            };
            let q = pose.rot[i];
            if q.is_finite() && q.length_squared() > 1e-8 {
                transform.rotation = q.normalize();
            }
            if i == bones.pelvis {
                let p = skeleton.rest[i].translation + pose.pelvis;
                if p.is_finite() {
                    transform.translation = p;
                }
            }
        }

        if feel.debug {
            draw_skeleton(&mut gizmos, skeleton, bones.pelvis, &pose, &model_world);
        }
    }
}

/// Bone lines parent→child in world space: left cyan, right magenta, centre yellow.
fn draw_skeleton(
    gizmos: &mut Gizmos,
    skeleton: &super::Skeleton,
    pelvis: usize,
    pose: &Pose,
    model: &Transform,
) {
    let world: Vec<Vec3> = skeleton
        .fk(pose, pelvis)
        .iter()
        .map(|t| model.transform_point(t.translation))
        .collect();
    for (i, parent) in skeleton.parent.iter().enumerate() {
        let Some(p) = *parent else { continue };
        let name = skeleton.names[i].as_str();
        let color = if name.ends_with("_l") {
            Color::srgb(0.2, 0.9, 1.0)
        } else if name.ends_with("_r") {
            Color::srgb(1.0, 0.3, 0.8)
        } else {
            Color::srgb(1.0, 0.9, 0.2)
        };
        gizmos.line(world[p], world[i], color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::Layer;
    use crate::motion::contact::tests::skeleton;
    use crate::motion::secondary::rotation_vector;
    use std::collections::HashMap;

    fn angle(a: Quat, b: Quat) -> f32 {
        rotation_vector(a * b.inverse()).length()
    }

    /// A hand-built bank with only an `Upper` mask (no sampling is ever called).
    fn bank() -> PoseBank {
        let (skeleton, bones) = skeleton();
        let rest = skeleton.rest_pose();
        let upper: Vec<f32> = (0..skeleton.len())
            .map(|i| {
                let mut b = Some(i);
                while let Some(j) = b {
                    if j == bones.spine[0] {
                        return 1.0;
                    }
                    b = skeleton.parent[j];
                }
                0.0
            })
            .collect();
        let masks = HashMap::from([(Mask::Upper, upper)]);
        PoseBank {
            skeleton,
            bones,
            rest,
            tracks: default(),
            style: default(),
            masks,
        }
    }

    fn turned(bank: &PoseBank, angle: f32) -> Pose {
        let mut pose = bank.rest.clone();
        for q in &mut pose.rot {
            *q = (Quat::from_rotation_y(angle) * *q).normalize();
        }
        pose.pelvis = Vec3::new(0.0, -0.2, 0.1);
        pose
    }

    fn layer(pose: Pose, weight: f32, mask: Mask) -> Layer {
        Layer {
            source: Source::Pose(pose),
            weight,
            mask,
            style: None,
        }
    }

    fn close(a: &Pose, b: &Pose) -> bool {
        a.pelvis.distance(b.pelvis) < 1e-5
            && a.rot.iter().zip(&b.rot).all(|(x, y)| angle(*x, *y) < 1e-4)
    }

    #[test]
    fn evaluates_layers_bottom_to_top_with_masks() {
        let bank = bank();
        let feel = MotionFeel::default();
        let mut warned = HashSet::new();
        let mut layers = MotionLayers::default();
        assert_eq!(evaluate(&bank, &layers, &feel, &mut warned), bank.rest);

        let base = turned(&bank, 0.5);
        let top = turned(&bank, -0.7);
        layers.layers = vec![
            layer(base.clone(), 1.0, Mask::Full),
            layer(top.clone(), 1.0, Mask::Upper),
        ];
        let out = evaluate(&bank, &layers, &feel, &mut warned);
        let upper = &bank.masks[&Mask::Upper];
        for i in 0..bank.skeleton.len() {
            let want = if upper[i] > 0.5 {
                top.rot[i]
            } else {
                base.rot[i]
            };
            assert!(angle(out.rot[i], want) < 1e-4, "bone {i}");
        }
        assert_eq!(
            out.pelvis, base.pelvis,
            "upper layers never move the pelvis"
        );

        // Half weight blends halfway; zero / NaN weights and wrong-length poses are skipped;
        // a mask the bank lacks falls back to full body.
        let mut short = top.clone();
        short.rot.pop();
        layers.layers = vec![
            layer(base.clone(), 1.0, Mask::Full),
            layer(top.clone(), 0.0, Mask::Full),
            layer(top.clone(), f32::NAN, Mask::Full),
            layer(short, 1.0, Mask::Full),
            layer(top.clone(), 0.5, Mask::Lower),
        ];
        let out = evaluate(&bank, &layers, &feel, &mut warned);
        assert!(close(&out, &base.lerp(&top, 0.5)));
    }
}
