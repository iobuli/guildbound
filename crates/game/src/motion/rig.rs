//! Model spawning and bone binding.
//! When a `ModelRequest` is added to a character root: spawn the glTF scene
//! (`GltfAssetLabel::Scene(0)`, `WorldAssetRoot`) as a child with `request.offset`; on
//! `WorldInstanceReady` map descendant `Name`s to canonical skeleton indices, mark them
//! `RigBone`, REMOVE every `AnimationPlayer`/`AnimationGraphHandle` in the scene (clips are data
//! only), then insert `Rig`, `Springs`, `ContactState` and `MotionLayers` on the root.
//! Pattern reference: the old `ranger.rs` `spawn`/`ready`/`prepare` (see git history).
//!
//! `tint` recolours the model's materials, except `hero_*` ones (the character kit's skin, hair,
//! cloth: `play::appearance` colours those from the look).
//!
//! Lifecycle: changing / re-inserting the `ModelRequest` replaces the model (the old one is
//! despawned, `Rig` removed until the new one binds); removing it despawns the model; if the
//! model entity disappears on its own, `Rig` is removed so `compose` never writes stale bones.
use super::bank::PoseBank;
use super::contact::ContactState;
use super::secondary::Springs;
use super::{ModelRequest, MotionLayers, MotionSet, Rig, RigBone, RigPose};
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use std::collections::{HashMap, HashSet};

/// On a spawned model scene entity: the character root it belongs to.
#[derive(Component, Clone, Copy, Debug)]
pub struct ModelOf(pub Entity);

/// On a character root: the model entity spawned for its current `ModelRequest` (bound or not).
#[derive(Component, Clone, Copy, Debug)]
pub struct ModelSlot(pub Entity);

/// On a model whose scene instance is ready but not yet bound (waits for `PoseBank`).
#[derive(Component)]
struct PendingBind;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (dropped_requests, spawn_models, lost_models).chain(),
    )
    .add_systems(PostUpdate, bind_models.before(MotionSet::Compose));
}

/// Spawn (or replace) the model scene for every new or changed `ModelRequest`.
fn spawn_models(
    mut commands: Commands,
    assets: Res<AssetServer>,
    requests: Query<(Entity, &ModelRequest, Option<&ModelSlot>), Changed<ModelRequest>>,
) {
    for (root, request, slot) in &requests {
        if let Some(slot) = slot {
            commands.entity(slot.0).try_despawn();
        }
        let offset = if request.offset.is_finite() {
            request.offset
        } else {
            Vec3::ZERO
        };
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(request.path.clone()));
        let model = commands
            .spawn((
                Name::new("model"),
                ModelOf(root),
                WorldAssetRoot(scene),
                Transform::from_translation(offset),
            ))
            .observe(on_ready)
            .id();
        commands
            .entity(root)
            .try_remove::<Rig>()
            .try_insert(ModelSlot(model))
            .add_child(model);
        info!("[rig] {root}: loading {}", request.path);
    }
}

fn on_ready(event: On<WorldInstanceReady>, mut commands: Commands) {
    commands.entity(event.entity).try_insert(PendingBind);
}

/// `ModelRequest` removed from a living root: drop its model and rig.
fn dropped_requests(
    mut commands: Commands,
    mut removed: RemovedComponents<ModelRequest>,
    slots: Query<&ModelSlot>,
    requested: Query<(), With<ModelRequest>>,
) {
    for root in removed.read() {
        if requested.contains(root) {
            continue;
        }
        if let Ok(slot) = slots.get(root) {
            commands.entity(slot.0).try_despawn();
        }
        commands.entity(root).try_remove::<(Rig, ModelSlot)>();
    }
}

/// The model entity vanished (despawned by someone else): unbind the root.
fn lost_models(
    mut commands: Commands,
    roots: Query<(Entity, &ModelSlot)>,
    models: Query<(), With<ModelOf>>,
) {
    for (root, slot) in &roots {
        if models.contains(slot.0) {
            continue;
        }
        warn!("[rig] {root}: model despawned; unbound (change the ModelRequest to respawn)");
        commands.entity(root).try_remove::<(Rig, ModelSlot)>();
    }
}

/// Bind ready models to the canonical skeleton. Runs right before `compose` so a model is
/// never rendered for a frame in its bind pose.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn bind_models(
    mut commands: Commands,
    bank: Option<Res<PoseBank>>,
    pending: Query<(Entity, &ModelOf), With<PendingBind>>,
    slots: Query<&ModelSlot>,
    requests: Query<&ModelRequest>,
    children: Query<&Children>,
    names: Query<&Name>,
    animated: Query<
        (),
        Or<(
            With<AnimationPlayer>,
            With<AnimationGraphHandle>,
            With<AnimationTransitions>,
        )>,
    >,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    material_names: Query<&bevy::gltf::GltfMaterialName>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut warned: Local<HashSet<String>>,
) {
    if pending.is_empty() {
        return;
    }
    let Some(bank) = bank else {
        if warned.insert(String::new()) {
            info!("[rig] model ready; waiting for the pose bank to bind it");
        }
        return;
    };
    let skeleton = &bank.skeleton;
    for (model, &ModelOf(root)) in &pending {
        commands.entity(model).try_remove::<PendingBind>();
        if slots.get(root).map(|s| s.0) != Ok(model) {
            // Superseded by a newer request (or the root is gone).
            commands.entity(model).try_despawn();
            continue;
        }

        let nodes: Vec<Entity> = std::iter::once(model)
            .chain(children.iter_descendants(model))
            .collect();
        let mut bones = vec![Entity::PLACEHOLDER; skeleton.len()];
        for &node in &nodes {
            if let Ok(name) = names.get(node)
                && let Some(i) = skeleton.find(name.as_str())
                && bones[i] == Entity::PLACEHOLDER
            {
                bones[i] = node;
                commands.entity(node).try_insert(RigBone);
            }
            if animated.contains(node) {
                commands
                    .entity(node)
                    .remove::<(AnimationPlayer, AnimationGraphHandle, AnimationTransitions)>();
            }
        }

        let request = requests.get(root).ok();
        let path = request.map_or("<model>", |r| r.path.as_str());
        let found = bones.iter().filter(|e| **e != Entity::PLACEHOLDER).count();
        if found == 0 {
            error!("[rig] {path}: no skeleton bones found in the scene; not rigged");
            continue;
        }
        if found < bones.len() && warned.insert(path.to_string()) {
            let missing: Vec<&str> = skeleton
                .names
                .iter()
                .zip(&bones)
                .filter(|(_, e)| **e == Entity::PLACEHOLDER)
                .map(|(n, _)| n.as_str())
                .collect();
            warn!(
                "[rig] {path}: {} of {} skeleton bones missing: {}",
                missing.len(),
                bones.len(),
                missing.join(", ")
            );
        }

        if let Some(tint) = request.and_then(|r| r.tint) {
            let tintable: Vec<Entity> = nodes
                .iter()
                .copied()
                .filter(|&e| !material_names.get(e).is_ok_and(|n| n.0.starts_with("hero_")))
                .collect();
            tint_materials(&mut commands, &tintable, &mesh_materials, &mut materials, tint);
        }

        commands
            .entity(root)
            .try_insert((
                Rig { model, bones },
                Springs::default(),
                ContactState::default(),
                RigPose::default(),
            ))
            .try_insert_if_new(MotionLayers::default());
        info!("[rig] {root}: bound {found}/{} bones", skeleton.len());
    }
}

fn luminance(c: LinearRgba) -> f32 {
    0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue
}

/// Give this model its own copies of its materials, recoloured to `tint`. The brightest
/// material becomes the tint; darker ones (joints, accents) keep their relative brightness.
fn tint_materials(
    commands: &mut Commands,
    nodes: &[Entity],
    mesh_materials: &Query<&MeshMaterial3d<StandardMaterial>>,
    materials: &mut Assets<StandardMaterial>,
    tint: Color,
) {
    let users: Vec<(Entity, AssetId<StandardMaterial>)> = nodes
        .iter()
        .filter_map(|&e| mesh_materials.get(e).ok().map(|m| (e, m.0.id())))
        .collect();
    let mut originals: HashMap<AssetId<StandardMaterial>, StandardMaterial> = HashMap::new();
    for &(_, id) in &users {
        if let Some(material) = materials.get(id) {
            originals.entry(id).or_insert_with(|| material.clone());
        }
    }
    if originals.is_empty() {
        warn!("[rig] tint requested but no StandardMaterials are loaded on the model");
        return;
    }
    let brightest = originals
        .values()
        .map(|m| luminance(m.base_color.to_linear()))
        .fold(1e-4_f32, f32::max);
    let tint = tint.to_linear();
    let mut copies: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>> = HashMap::new();
    for (id, original) in originals {
        let base = original.base_color.to_linear();
        let ratio = (luminance(base) / brightest).clamp(0.15, 1.0);
        let mut material = original;
        material.base_color = Color::LinearRgba(LinearRgba::new(
            tint.red * ratio,
            tint.green * ratio,
            tint.blue * ratio,
            base.alpha * tint.alpha,
        ));
        copies.insert(id, materials.add(material));
    }
    for (entity, id) in users {
        if let Some(handle) = copies.get(&id) {
            commands
                .entity(entity)
                .insert(MeshMaterial3d(handle.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::contact::tests::skeleton;
    use bevy::ecs::system::RunSystemOnce;

    /// A bank built by hand (no `PoseBank::build`, no sampling).
    fn bank() -> PoseBank {
        let (skeleton, bones) = skeleton();
        let rest = skeleton.rest_pose();
        PoseBank {
            skeleton,
            bones,
            rest,
            tracks: default(),
            style: default(),
            masks: default(),
        }
    }

    /// Root + model + fake scene: an animated "Armature" holding every bone but `missing`, and
    /// a mesh node. Returns (world, root, model, armature, mesh, original material).
    fn scene(
        missing: &str,
        tint: Option<Color>,
    ) -> (
        World,
        Entity,
        Entity,
        Entity,
        Entity,
        Handle<StandardMaterial>,
    ) {
        let mut world = World::new();
        let bank = bank();
        let names: Vec<String> = bank.skeleton.names.clone();
        world.insert_resource(bank);
        world.init_resource::<Assets<StandardMaterial>>();
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.4, 0.04),
                ..default()
            });
        let root = world
            .spawn(ModelRequest {
                path: "test.glb".into(),
                offset: Vec3::ZERO,
                tint,
            })
            .id();
        let model = world
            .spawn((Name::new("model"), ModelOf(root), PendingBind))
            .id();
        world
            .entity_mut(root)
            .insert(ModelSlot(model))
            .add_child(model);
        let armature = world
            .spawn((Name::new("Armature"), AnimationPlayer::default()))
            .id();
        world.entity_mut(model).add_child(armature);
        for name in names.iter().filter(|n| n.as_str() != missing) {
            let bone = world.spawn(Name::new(name.clone())).id();
            world.entity_mut(armature).add_child(bone);
        }
        let mesh = world
            .spawn((Name::new("Mannequin"), MeshMaterial3d(material.clone())))
            .id();
        world.entity_mut(model).add_child(mesh);
        (world, root, model, armature, mesh, material)
    }

    #[test]
    fn binds_named_descendants_strips_animation_and_tints() {
        let tint = Color::srgb(0.1, 0.5, 0.9);
        let (mut world, root, model, armature, mesh, original) = scene("index_01_r", Some(tint));
        world.run_system_once(bind_models).unwrap();

        let names = world.resource::<PoseBank>().skeleton.names.clone();
        let rig = world.get::<Rig>(root).expect("rig inserted").clone();
        assert_eq!(rig.model, model);
        for (i, &entity) in rig.bones.iter().enumerate() {
            if names[i] == "index_01_r" {
                assert_eq!(entity, Entity::PLACEHOLDER);
            } else {
                assert_eq!(world.get::<Name>(entity).unwrap().as_str(), names[i]);
                assert!(world.entity(entity).contains::<RigBone>());
            }
        }
        assert!(!world.entity(armature).contains::<AnimationPlayer>());
        assert!(!world.entity(model).contains::<PendingBind>());
        for present in [
            world.entity(root).contains::<Springs>(),
            world.entity(root).contains::<ContactState>(),
            world.entity(root).contains::<MotionLayers>(),
        ] {
            assert!(present);
        }
        let handle = world
            .get::<MeshMaterial3d<StandardMaterial>>(mesh)
            .unwrap()
            .0
            .clone();
        assert_ne!(
            handle.id(),
            original.id(),
            "each character gets its own material"
        );
        let materials = world.resource::<Assets<StandardMaterial>>();
        let tinted = materials.get(&handle).unwrap().base_color.to_linear();
        let want = tint.to_linear();
        assert!((tinted.red - want.red).abs() < 1e-4 && (tinted.blue - want.blue).abs() < 1e-4);
        let untouched = materials.get(&original).unwrap().base_color;
        assert_eq!(untouched, Color::srgb(0.8, 0.4, 0.04));
    }

    #[test]
    fn tint_leaves_hero_materials_alone() {
        let (mut world, _, _, _, mesh, original) = scene("", Some(Color::srgb(0.1, 0.5, 0.9)));
        world
            .entity_mut(mesh)
            .insert(bevy::gltf::GltfMaterialName("hero_skin".into()));
        world.run_system_once(bind_models).unwrap();
        let handle = world.get::<MeshMaterial3d<StandardMaterial>>(mesh).unwrap();
        assert_eq!(handle.0.id(), original.id(), "hero_* keeps its material");
    }

    #[test]
    fn existing_motion_layers_are_kept() {
        let (mut world, root, ..) = scene("", None);
        let mut layers = MotionLayers::default();
        layers.raw = true;
        world.entity_mut(root).insert(layers);
        world.run_system_once(bind_models).unwrap();
        assert!(world.get::<MotionLayers>(root).unwrap().raw);
        assert!(world.entity(root).contains::<Rig>());
    }

    #[test]
    fn waits_for_the_bank_and_drops_superseded_models() {
        let (mut world, root, model, ..) = scene("", None);
        let bank = world.remove_resource::<PoseBank>().unwrap();
        world.run_system_once(bind_models).unwrap();
        assert!(
            world.entity(model).contains::<PendingBind>(),
            "still waiting"
        );
        assert!(!world.entity(root).contains::<Rig>());

        world.insert_resource(bank);
        let newer = world.spawn(ModelOf(root)).id();
        world.entity_mut(root).insert(ModelSlot(newer));
        world.run_system_once(bind_models).unwrap();
        assert!(
            world.get_entity(model).is_err(),
            "superseded model despawned"
        );
        assert!(!world.entity(root).contains::<Rig>());
    }

    #[test]
    fn lost_model_unbinds_the_root() {
        let (mut world, root, model, ..) = scene("", None);
        world.run_system_once(bind_models).unwrap();
        assert!(world.entity(root).contains::<Rig>());
        world.entity_mut(model).despawn();
        world.run_system_once(lost_models).unwrap();
        assert!(!world.entity(root).contains::<Rig>());
        assert!(!world.entity(root).contains::<ModelSlot>());
    }
}
