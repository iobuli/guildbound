//! Townsfolk: presentation-only NPCs (no sim fighters). Quaternius base characters in Modular Character
//! Outfits, built by Tools/blender/build_master.py (--set npc) with UAL idles and walks retargeted onto
//! them. Each NPC loads its own GLB, gets an animation graph from that file's clips, and either holds an
//! idle loop or walks a closed route. Standing NPCs are sim obstacles (props::obstacles) so fighters walk
//! around them. Add a row to NPCS to add a person.
//!
//! Walkers move with the shared locomotion (locomotion.rs: the hero's tuned drag motor, turning and relaxed
//! blend space on the hero library, which plays on these rigs because they share the hero's bone names); they
//! ease in and out of their pace and pause at each corner. Everyone gets the life layers: breathing, relaxed
//! hands on walkers, and a look at the player when near.
use std::collections::HashMap;

use bevy::animation::AnimationTargetId;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady};

/// (model, x, z, yaw deg, clip, walk route or empty)
type Row = (&'static str, f32, f32, f32, &'static str, &'static [(f32, f32)]);

const MARKET_LOOP: &[(f32, f32)] = &[(24.0, 24.0), (36.0, 24.0), (36.0, 36.0), (24.0, 36.0)];
const ROAD_NORTH: &[(f32, f32)] = &[(0.0, 14.0), (0.0, 50.0)];
const ROAD_EAST: &[(f32, f32)] = &[(14.0, 1.0), (50.0, 1.0)];

pub const NPCS: &[Row] = &[
    // market clearing (centre 30, 30): stall keepers, a farmer, idlers
    ("male_peasant", 26.0, 34.0, 180.0, "npc_talk", &[]),
    ("female_peasant", 30.5, 34.6, 180.0, "npc_fold_arms", &[]),
    ("male_peasant", 35.0, 34.0, 200.0, "npc_yes", &[]),
    ("female_peasant", 33.0, 32.5, 120.0, "npc_farm", &[]),
    ("female_ranger", 27.0, 28.0, 60.0, "npc_talk", &[]),
    ("male_ranger", 28.2, 28.6, 240.0, "npc_idle", &[]),
    // guards at the courtyard's entrances
    ("male_ranger", 2.6, 13.0, 180.0, "npc_lantern", &[]),
    ("female_ranger", -13.0, -2.6, 90.0, "npc_fold_arms", &[]),
    // walkers
    ("male_peasant", 24.0, 24.0, 0.0, "npc_carry", MARKET_LOOP),
    ("female_peasant", 0.0, 14.0, 0.0, "npc_walk", ROAD_NORTH),
    ("male_peasant", 14.0, 1.0, 0.0, "npc_walk", ROAD_EAST),
];

/// A walker's pace (m/s): the carrier dawdles, the east road walker jogs, the rest stroll.
fn pace(row: usize) -> f32 {
    match NPCS[row].4 {
        "npc_carry" => 1.1,
        _ if NPCS[row].5 == ROAD_EAST => 3.4,
        _ => 1.35 + 0.2 * (row % 2) as f32,
    }
}

#[derive(Component)]
pub struct Npc {
    pub row: usize,
    leg: usize,
    mover: crate::locomotion::Mover,
    /// Seconds left standing at a corner.
    pause: f32,
}

/// The NPC's animated bones, for the life layers.
#[derive(Component)]
pub struct NpcRig {
    bones: Vec<Entity>,
}

#[derive(Resource, Default)]
pub struct NpcLibraries {
    gltf: HashMap<&'static str, Handle<Gltf>>,
    graphs: HashMap<&'static str, (Handle<AnimationGraph>, HashMap<String, AnimationNodeIndex>)>,
}

/// Standing NPCs block movement like a tree trunk.
pub fn obstacles() -> Vec<(guildbound_sim::V2, f32)> {
    NPCS.iter().filter(|r| r.5.is_empty()).map(|r| (guildbound_sim::V2::new(r.1, r.2), 0.35)).collect()
}

pub fn spawn(mut commands: Commands, assets: Res<AssetServer>, mut libs: ResMut<NpcLibraries>) {
    for (i, &(model, x, z, yaw, _, _)) in NPCS.iter().enumerate() {
        let path = format!("packs/npc/{model}.glb");
        if !std::path::Path::new(&format!("{}/assets/{path}", crate::ROOT)).exists() {
            continue;
        }
        libs.gltf.entry(model).or_insert_with(|| assets.load(path.clone()));
        let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
        // pack models face +Z; turn them to the row's heading
        commands
            .spawn((Npc { row: i, leg: 0, mover: crate::locomotion::Mover::new(yaw.to_radians()), pause: 0.0 }, WorldAssetRoot(scene), Transform::from_xyz(x, 0.0, z).with_rotation(Quat::from_rotation_y(yaw.to_radians()))))
            .observe(on_ready);
    }
}

/// Build one graph per NPC model once its GLB has loaded.
pub fn build_graphs(mut libs: ResMut<NpcLibraries>, gltfs: Res<Assets<Gltf>>, mut graphs: ResMut<Assets<AnimationGraph>>) {
    let pending: Vec<_> = libs.gltf.iter().filter(|(m, _)| !libs.graphs.contains_key(*m)).map(|(m, h)| (*m, h.clone())).collect();
    for (model, handle) in pending {
        let Some(gltf) = gltfs.get(&handle) else { continue };
        let mut named: Vec<_> = gltf.named_animations.iter().map(|(n, h)| (n.to_string(), h.clone())).collect();
        named.sort_by(|a, b| a.0.cmp(&b.0));
        let (graph, nodes) = AnimationGraph::from_clips(named.iter().map(|(_, h)| h.clone()));
        let index = named.into_iter().map(|(n, _)| n).zip(nodes).collect();
        libs.graphs.insert(model, (graphs.add(graph), index));
    }
}

#[derive(Component)]
pub struct NpcPlayer {
    npc: Entity,
}

fn on_ready(ready: On<WorldInstanceReady>, mut commands: Commands, children: Query<&Children>, players: Query<(), With<AnimationPlayer>>, targets: Query<&Transform, With<AnimationTargetId>>) {
    let mut bones = Vec::new();
    let mut rest = crate::procedural::RestPose::default();
    for e in children.iter_descendants(ready.entity) {
        if players.contains(e) {
            commands.entity(e).insert(NpcPlayer { npc: ready.entity });
        }
        if let Ok(t) = targets.get(e) {
            bones.push(e);
            rest.0.insert(e, t.rotation);
            rest.1.insert(e, t.translation);
        }
    }
    commands.entity(ready.entity).insert((NpcRig { bones }, rest, crate::procedural::Life::default()));
}

/// Attach the graph and start the row's loop once both the scene and the graph exist; walk the routes.
#[allow(clippy::too_many_arguments)]
pub fn drive(
    mut commands: Commands,
    libs: Res<NpcLibraries>,
    hero: Res<crate::anim::AnimLibrary>,
    settings: Res<crate::anim::AnimSettings>,
    map: Res<crate::controller::LocomotionMap>,
    sim: Res<crate::sim_bridge::Sim>,
    time: Res<Time>,
    mut npcs: Query<(&mut Npc, &mut Transform)>,
    mut players: Query<(Entity, &NpcPlayer, &mut AnimationPlayer, Option<&AnimationGraphHandle>)>,
) {
    let tuning = &sim.world.tuning;
    let dt = time.delta_secs();
    for (e, link, mut player, graph) in &mut players {
        let Ok((mut npc, mut t)) = npcs.get_mut(link.npc) else { continue };
        let (model, .., clip, route) = NPCS[npc.row];
        if route.len() >= 2 {
            // walker: the shared motor always moves it; it animates with the hero's relaxed locomotion once that
            // library is loaded, with its own walk loop until then
            let mut pos = t.translation;
            let next = (npc.leg + 1) % route.len();
            let goal = Vec3::new(route[next].0, 0.0, route[next].1);
            let want = if npc.pause > 0.0 {
                npc.pause -= dt;
                None
            } else if pos.distance(goal) < 0.5 {
                // arrived: stand a moment at the corner (deterministic per row and leg), then the next leg
                npc.leg = next;
                npc.pause = 1.2 + ((npc.row * 7 + npc.leg * 3) % 5) as f32 * 0.45;
                None
            } else {
                Some(((goal - pos).normalize_or_zero(), pace(npc.row)))
            };
            let travelled = crate::locomotion::step(&mut npc.mover, &mut pos, want, tuning, dt);
            t.translation = Vec3::new(pos.x, 0.0, pos.z);
            t.rotation = Quat::from_rotation_y(npc.mover.yaw);
            if let Some(hero_graph) = hero.graph.clone() {
                if graph.is_none_or(|g| g.0 != hero_graph) {
                    // anything started on the old graph would replay as an unrelated hero clip: start clean
                    player.stop_all();
                    npc.mover.clear_blend();
                    commands.entity(e).insert(AnimationGraphHandle(hero_graph));
                    continue;
                }
                let idle = if hero.node("nc_idle").is_some() { "nc_idle" } else { "Unarmed-Idle" };
                crate::locomotion::animate(&mut player, &hero, &map, &settings, tuning, &mut npc.mover, travelled, idle);
                continue;
            }
        }
        let Some((handle, index)) = libs.graphs.get(model) else { continue };
        if graph.is_none() {
            commands.entity(e).insert(AnimationGraphHandle(handle.clone()));
            continue;
        }
        if let Some(&node) = index.get(clip).or_else(|| index.get("npc_idle")) && !player.is_playing_animation(node) {
            // de-synchronise identical loops between people
            let offset = (npc.row as f32 * 0.37).fract();
            player.start(node).repeat().seek_to(offset * 2.0);
        }
    }
}

/// Life layers on every NPC: breathing, relaxed hands on walkers, and a look at the player when near.
pub fn life(world: &mut World) {
    let player_head = world.resource::<crate::sim_bridge::Sim>().world.fighters.first().map(|f| Vec3::new(f.position.x, f.height + 1.55, f.position.z));
    let dt = world.resource::<Time>().delta_secs();
    let mut q = world.query::<(Entity, &Npc, &NpcRig, &Transform)>();
    let jobs: Vec<_> = q.iter(world).map(|(e, n, r, t)| (e, NPCS[n.row].5.len() >= 2, n.mover.velocity.length(), r.bones.clone(), t.translation, t.rotation * Vec3::Z)).collect();
    for (root, walker, speed, bones, at, forward) in jobs {
        let look = player_head.filter(|p| {
            let to = *p - (at + Vec3::Y * 1.55);
            to.length() < 7.0 && to.normalize_or_zero().dot(forward) > -0.2
        }).map(|p| (p, 0.9));
        let drive = crate::procedural::Drive {
            grip: walker.then_some([0.3, 0.3]),
            look,
            forward,
            chest_yaw: 0.0,
            breath: 1.0,
            exertion: (speed / 6.0).clamp(0.0, 1.0),
            lean: None,
            dt,
        };
        crate::procedural::apply(world, root, &bones, &drive);
    }
}
