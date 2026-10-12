//! Townsfolk: presentation-only NPCs (no sim fighters). Quaternius base characters in Modular Character
//! Outfits, built by Tools/blender/build_master.py (--set npc) with UAL idles and walks retargeted onto
//! them. Each NPC loads its own GLB, gets an animation graph from that file's clips, and either holds an
//! idle loop or walks a closed route. Standing NPCs are sim obstacles (props::obstacles) so fighters walk
//! around them. Add a row to NPCS to add a person.
use std::collections::HashMap;

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

const WALK_SPEED: f32 = 1.25;

#[derive(Component)]
pub struct Npc {
    pub row: usize,
    leg: usize,
    /// Distance along the current route leg (m).
    along: f32,
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
            .spawn((Npc { row: i, leg: 0, along: 0.0 }, WorldAssetRoot(scene), Transform::from_xyz(x, 0.0, z).with_rotation(Quat::from_rotation_y(yaw.to_radians()))))
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

fn on_ready(ready: On<WorldInstanceReady>, mut commands: Commands, children: Query<&Children>, players: Query<(), With<AnimationPlayer>>) {
    for e in children.iter_descendants(ready.entity) {
        if players.contains(e) {
            commands.entity(e).insert(NpcPlayer { npc: ready.entity });
        }
    }
}

/// Attach the graph and start the row's loop once both the scene and the graph exist; walk the routes.
pub fn drive(
    mut commands: Commands,
    libs: Res<NpcLibraries>,
    time: Res<Time>,
    mut npcs: Query<(&mut Npc, &mut Transform)>,
    mut players: Query<(Entity, &NpcPlayer, &mut AnimationPlayer, Option<&AnimationGraphHandle>)>,
) {
    for (e, link, mut player, graph) in &mut players {
        let Ok((npc, _)) = npcs.get(link.npc) else { continue };
        let (model, .., clip, _) = NPCS[npc.row];
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
    let dt = time.delta_secs();
    for (mut npc, mut t) in &mut npcs {
        let route = NPCS[npc.row].5;
        if route.len() < 2 {
            continue;
        }
        let a = Vec3::new(route[npc.leg].0, 0.0, route[npc.leg].1);
        let next = (npc.leg + 1) % route.len();
        let b = Vec3::new(route[next].0, 0.0, route[next].1);
        let len = a.distance(b).max(0.01);
        npc.along += WALK_SPEED * dt;
        if npc.along >= len {
            npc.along -= len;
            npc.leg = next;
            continue;
        }
        let dir = (b - a) / len;
        t.translation = a + dir * npc.along;
        let want = Quat::from_rotation_y(dir.x.atan2(dir.z));
        t.rotation = t.rotation.slerp(want, (8.0 * dt).min(1.0));
    }
}
