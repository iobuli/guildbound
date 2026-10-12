//! Compare and retarget matching source clips in bind-relative anatomical frames.
use std::collections::{BTreeMap, HashMap};
use bevy::{animation::AnimationTargetId, gltf::Gltf, prelude::*};
use bevy::world_serialization::WorldInstanceReady;
use crate::{motion_math::world_transform, procedural::RestPose};

const SOURCE: &str = "packs/_sources/rpg_hero/fighter.glb";

#[derive(Component, Default)]
struct ReferenceRig { bones: Vec<Entity>, player: Option<Entity> }

#[derive(Resource, Default)]
struct Reference {
    asset: Handle<Gltf>,
    roots: HashMap<Entity, Entity>,
    graph: Option<Handle<AnimationGraph>>,
    clips: BTreeMap<String, (AnimationNodeIndex, f32)>,
    active: HashMap<Entity, String>,
    comparisons: HashMap<Entity, Vec<serde_json::Value>>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Reference>()
        .add_systems(Startup, spawn)
        .add_systems(Update, drive.after(crate::studio::drive).after(crate::anim::drive_animation))
        .add_systems(PostUpdate, apply.after(bevy::app::AnimationSystems).before(crate::anim::inertialize));
}

fn spawn(assets: Res<AssetServer>, mut reference: ResMut<Reference>) {
    if !std::path::Path::new(&format!("{}/assets/{SOURCE}", crate::ROOT)).is_file() { return; }
    reference.asset = assets.load(SOURCE);
}

fn ready(event: On<WorldInstanceReady>, children: Query<&Children>, players: Query<(), With<AnimationPlayer>>,
    targets: Query<&Transform, With<AnimationTargetId>>, mut rigs: Query<&mut ReferenceRig>, mut commands: Commands) {
    let Ok(mut rig) = rigs.get_mut(event.entity) else { return };
    let mut rest = RestPose::default();
    for e in children.iter_descendants(event.entity) {
        if players.contains(e) { rig.player = Some(e); }
        if let Ok(t) = targets.get(e) {
            rig.bones.push(e);
            rest.0.insert(e, t.rotation);
            rest.1.insert(e, t.translation);
        }
    }
    commands.entity(event.entity).insert(rest);
}

fn drive(world: &mut World) {
    let mut reference = world.remove_resource::<Reference>().unwrap_or_default();
    reference.active.clear();
    if reference.graph.is_none() {
        let named = world.resource::<Assets<Gltf>>().get(&reference.asset).map(|g| g.named_animations.iter()
            .map(|(n, h)| (n.to_string(), h.clone())).collect::<Vec<_>>());
        if let Some(mut named) = named {
            named.sort_by(|a, b| a.0.cmp(&b.0));
            let clips = world.resource::<Assets<AnimationClip>>();
            if named.iter().all(|(_, h)| clips.get(h).is_some()) {
                let durations: Vec<_> = named.iter().map(|(_, h)| clips.get(h).unwrap().duration()).collect();
                let (graph, nodes) = AnimationGraph::from_clips(named.iter().map(|(_, h)| h.clone()));
                reference.clips = named.iter().zip(nodes).zip(durations).map(|(((n, _), node), duration)| (n.clone(), (node, duration))).collect();
                reference.graph = Some(world.resource_mut::<Assets<AnimationGraph>>().add(graph));
            }
        }
    }
    let s = world.resource::<crate::studio::Studio>();
    let p = world.resource::<crate::possession::Possession>();
    let enabled = p.reference_corrector;
    let mut samples = Vec::new();
    if s.open && s.model.as_deref() != Some(SOURCE) && enabled && p.motion.is_none()
        && let (Some(e), Some(c)) = (s.entity, s.clip.clone()) {
        samples.push((e, c, s.time / s.duration.max(0.01)));
    }
    let mut q = world.query::<(Entity, &crate::anim::FighterRig)>();
    if enabled && let Some(lib) = world.get_resource::<crate::anim::AnimLibrary>() {
        for (e, rig) in q.iter(world) {
            if let Some((clip, phase)) = rig.sample_info(world, lib) { samples.push((e, clip, phase)); }
        }
    }
    samples.retain(|(_, clip, _)| reference.clips.contains_key(clip));
    let stale: Vec<_> = reference.roots.keys().copied().filter(|e| !samples.iter().any(|(target, _, _)| target == e)).collect();
    for target in stale {
        if let Some(root) = reference.roots.remove(&target) { world.despawn(root); }
    }
    for (target, clip, phase) in samples {
        let root = if let Some(&root) = reference.roots.get(&target) { root } else {
            let scene = world.resource::<AssetServer>().load(GltfAssetLabel::Scene(0).from_asset(SOURCE));
            let root = world.spawn((Name::new("pose reference"), Visibility::Hidden, Transform::IDENTITY,
                WorldAssetRoot(scene), ReferenceRig::default())).observe(ready).id();
            reference.roots.insert(target, root);
            root
        };
        let player = world.get::<ReferenceRig>(root).and_then(|r| r.player);
        if let (Some(player), Some(graph)) = (player, reference.graph.clone()) {
            let (node, duration) = reference.clips[&clip];
            world.entity_mut(player).insert(AnimationGraphHandle(graph));
            if let Some(mut player) = world.get_mut::<AnimationPlayer>(player) {
                player.stop_all();
                player.play(node).set_speed(0.0).set_seek_time(phase.clamp(0.0, 1.0) * duration);
                reference.active.insert(target, clip);
            }
        }
    }
    world.insert_resource(reference);
}

fn bind_world(world: &World, e: Entity, rest: &RestPose, depth: u8) -> Option<GlobalTransform> {
    if depth > 64 { return None; }
    let mut local = *world.get::<Transform>(e)?;
    if let Some(q) = rest.0.get(&e) { local.rotation = *q; }
    if let Some(p) = rest.1.get(&e) { local.translation = *p; }
    match world.get::<ChildOf>(e) {
        Some(parent) => Some(bind_world(world, parent.parent(), rest, depth + 1)?.mul_transform(local)),
        None => Some(GlobalTransform::from(local)),
    }
}

fn frame(rest: &HashMap<String, GlobalTransform>) -> Option<Quat> {
    let left = rest.get("B_L_Thigh")?.translation() - rest.get("B_R_Thigh")?.translation();
    let up = (rest.get("B_Head")?.translation() - rest.get("B_Pelvis")?.translation()).normalize_or(Vec3::Y);
    let left = (left - up * left.dot(up)).normalize_or(Vec3::X);
    let forward = left.cross(up).normalize_or(Vec3::Z);
    Some(Quat::from_mat3(&Mat3::from_cols(left, forward.cross(left), forward)))
}

fn child(name: &str, source: &HashMap<String, GlobalTransform>) -> Option<String> {
    let exact = match name {
        "B_Pelvis" => "B_Spine", "B_Spine" => if source.contains_key("B_Spine1") { "B_Spine1" } else { "B_Spine2" },
        "B_Spine1" => "B_Spine2", "B_Spine2" => "B_Neck", "B_Neck" => "B_Head",
        "B_L_Forearm" => "B_L_Forearm.001", "B_R_Forearm" => "B_R_Hand",
        "B_L_Forearm.001" => "B_L_Finger2", "B_R_Hand" => "B_R_Finger2", _ => "",
    };
    if !exact.is_empty() { return Some(exact.into()); }
    for s in ["L", "R"] {
        for (a, b) in [("Clavicle", "UpperArm"), ("UpperArm", "Forearm"), ("Thigh", "Calf"), ("Calf", "Foot"), ("Foot", "Toe0")] {
            if name == format!("B_{s}_{a}") { return Some(format!("B_{s}_{b}")); }
        }
    }
    None
}

pub fn apply(world: &mut World) {
    let reference = world.resource::<Reference>();
    let jobs: Vec<_> = reference.active.keys().filter_map(|&target| reference.roots.get(&target).map(|&source| (source, target))).collect();
    let mut comparisons = HashMap::new();
    for (source, target) in jobs { comparisons.insert(target, correct(world, source, target)); }
    world.resource_mut::<Reference>().comparisons = comparisons;
}

fn correct(world: &mut World, sr: Entity, tr: Entity) -> Vec<serde_json::Value> {
    let target = world.get::<crate::studio::StudioRig>(tr).map(|r| r.bones.clone())
        .or_else(|| world.get::<crate::anim::FighterRig>(tr).map(|r| r.bones.clone()));
    let (Some(source), Some(target), Some(srest), Some(trest)) = (world.get::<ReferenceRig>(sr), target, world.get::<RestPose>(sr), world.get::<RestPose>(tr)) else { return Vec::new() };
    let bind = |bones: &[Entity], rest: &RestPose| bones.iter().filter_map(|&e| Some((world.get::<Name>(e)?.as_str().to_owned(), bind_world(world, e, rest, 0)?))).collect::<HashMap<_, _>>();
    let sb = bind(&source.bones, srest);
    let tb = bind(&target, trest);
    let (Some(sf), Some(tf)) = (frame(&sb), frame(&tb)) else { return Vec::new() };
    let alignment = tf * sf.inverse();
    let pose: HashMap<_, _> = source.bones.iter().filter_map(|&e| Some((world.get::<Name>(e)?.as_str().to_owned(), world_transform(world, e)?))).collect();
    let mut bones = target;
    bones.sort_by_key(|&e| {
        let (mut node, mut depth) = (e, 0);
        while let Some(p) = world.get::<ChildOf>(node) { node = p.parent(); depth += 1; }
        depth
    });
    let mut comparisons = Vec::new();
    for e in bones {
        let Some(name) = world.get::<Name>(e).map(|n| n.as_str().to_owned()).filter(|n| n.starts_with("B_")) else { continue };
        let (Some(sbind), Some(tbind), Some(spose)) = (sb.get(&name), tb.get(&name), pose.get(&name)) else { continue };
        let mut rest_alignment = Quat::IDENTITY;
        let mut ratio = None;
        if let Some(child) = child(&name, &sb) && let (Some(sc), Some(tc)) = (sb.get(&child), tb.get(&child)) {
            let sd = sc.translation() - sbind.translation();
            let td = tc.translation() - tbind.translation();
            if sd.length() > 1e-5 && td.length() > 1e-5 {
                rest_alignment = Quat::from_rotation_arc(td.normalize(), (alignment * sd).normalize());
                ratio = Some(td.length() / sd.length());
            }
        }
        let delta = spose.to_scale_rotation_translation().1 * sbind.to_scale_rotation_translation().1.inverse();
        let desired = (alignment * delta * alignment.inverse() * rest_alignment * tbind.to_scale_rotation_translation().1).normalize();
        let Some(current) = world_transform(world, e) else { continue };
        let degrees = current.to_scale_rotation_translation().1.angle_between(desired).abs().to_degrees();
        crate::ik::rotate_world(world, e, desired);
        comparisons.push(serde_json::json!({"joint":name, "rotation_correction_deg":degrees, "length_ratio":ratio}));
    }
    comparisons
}

pub fn snapshot(world: &World) -> serde_json::Value {
    let r = world.resource::<Reference>();
    let target = world.resource::<crate::studio::Studio>().entity;
    serde_json::json!({"source":SOURCE, "clip":target.and_then(|e| r.active.get(&e)), "joints":target.and_then(|e| r.comparisons.get(&e)),
        "actors":r.active.len(), "scope":"matching clips on fighters and Studio; before foot support and anatomical constraints"})
}
