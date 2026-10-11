//! Character presentation: the FightingMotions fighter (converted from the Unity pack by
//! Tools/blender/fbx_pack_to_glb.py into assets/packs/, git-ignored) driven by sim state.
//! Locomotion uses the pack's in-place clips; the sim stays the only mover (MASTER L1).

use std::time::Duration;

use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use crate::sim_bridge::Sim;

pub const FIGHTER_GLB: &str = "packs/fighting_motions/fighter.glb";
const IDLE: &str = "idle_A";
const FORWARD: &str = "move_forward_B2";
const BACKWARD: &str = "move_backward_B2";

#[derive(Resource)]
pub struct AnimLibrary {
    gltf: Handle<Gltf>,
    pub graph: Option<Handle<AnimationGraph>>,
    /// Clip names and graph nodes, sorted by name.
    pub clips: Vec<(String, AnimationNodeIndex)>,
}

impl AnimLibrary {
    pub fn node(&self, name: &str) -> Option<AnimationNodeIndex> {
        self.clips.iter().find(|(n, _)| n == name).map(|(_, i)| *i)
    }
}

#[derive(Resource)]
pub struct AnimSettings {
    /// Ground speed (m/s) at which the walk clips play at 1x.
    pub reference_speed: f32,
    pub blend_seconds: f32,
}

impl Default for AnimSettings {
    fn default() -> Self {
        Self { reference_speed: 1.4, blend_seconds: 0.15 }
    }
}

/// One-shot clip requests from the panel or the remote API: (fighter index, clip name).
#[derive(Resource, Default)]
pub struct AnimRequests(pub Vec<(usize, String)>);

#[derive(Component)]
pub struct FighterRig {
    pub fighter: usize,
    pub color: Color,
    player: Option<Entity>,
    attached: bool,
    current: Option<AnimationNodeIndex>,
    pub one_shot: Option<String>,
}

impl FighterRig {
    pub fn new(fighter: usize, color: Color) -> Self {
        Self { fighter, color, player: None, attached: false, current: None, one_shot: None }
    }
}

pub fn load_library(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(AnimLibrary { gltf: assets.load(FIGHTER_GLB), graph: None, clips: Vec::new() });
}

pub fn build_library(mut lib: ResMut<AnimLibrary>, gltfs: Res<Assets<Gltf>>, mut graphs: ResMut<Assets<AnimationGraph>>) {
    if lib.graph.is_some() {
        return;
    }
    let Some(gltf) = gltfs.get(&lib.gltf) else { return };
    let mut named: Vec<_> = gltf.named_animations.iter().map(|(n, h)| (n.to_string(), h.clone())).collect();
    named.sort_by(|a, b| a.0.cmp(&b.0));
    let (graph, nodes) = AnimationGraph::from_clips(named.iter().map(|(_, h)| h.clone()));
    lib.clips = named.into_iter().map(|(n, _)| n).zip(nodes).collect();
    lib.graph = Some(graphs.add(graph));
}

/// When a fighter's model has spawned: remember its AnimationPlayer and give it the fighter's colour.
pub fn on_rig_ready(
    ready: On<WorldInstanceReady>,
    children: Query<&Children>,
    players: Query<(), With<AnimationPlayer>>,
    mut mats: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut rigs: Query<&mut FighterRig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(mut rig) = rigs.get_mut(ready.entity) else { return };
    let tint = materials.add(StandardMaterial { base_color: rig.color, perceptual_roughness: 0.7, ..default() });
    for e in children.iter_descendants(ready.entity) {
        if players.contains(e) {
            rig.player = Some(e);
        }
        if let Ok(mut m) = mats.get_mut(e) {
            m.0 = tint.clone();
        }
    }
}

pub fn drive_animation(
    mut commands: Commands,
    lib: Res<AnimLibrary>,
    settings: Res<AnimSettings>,
    sim: Res<Sim>,
    mut requests: ResMut<AnimRequests>,
    mut rigs: Query<&mut FighterRig>,
    mut players: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
) {
    let Some(graph) = &lib.graph else { return };
    let blend = Duration::from_secs_f32(settings.blend_seconds.max(0.01));
    for mut rig in &mut rigs {
        let Some(player_entity) = rig.player else { continue };
        if !rig.attached {
            commands.entity(player_entity).insert((AnimationGraphHandle(graph.clone()), AnimationTransitions::new()));
            rig.attached = true;
            continue;
        }
        let Ok((mut player, Some(mut transitions))) = players.get_mut(player_entity) else { continue };
        let Some(f) = sim.world.fighters.get(rig.fighter) else { continue };

        let me = rig.fighter;
        for (_, clip) in requests.0.iter().filter(|(i, _)| *i == me) {
            if let Some(node) = lib.node(clip) {
                transitions.play(&mut player, node, blend);
                rig.current = Some(node);
                rig.one_shot = Some(clip.clone());
            }
        }
        if let Some(node) = rig.current.filter(|_| rig.one_shot.is_some()) {
            if player.animation(node).is_some_and(|a| !a.is_finished()) {
                continue;
            }
            rig.one_shot = None;
        }

        let speed = f.velocity.length();
        let wanted = if speed < 0.1 {
            IDLE
        } else if f.velocity.dot(f.facing) >= -0.3 * speed {
            FORWARD
        } else {
            BACKWARD
        };
        let Some(node) = lib.node(wanted) else { continue };
        if rig.current != Some(node) {
            transitions.play(&mut player, node, blend).repeat();
            rig.current = Some(node);
        }
        let rate = if wanted == IDLE { 1.0 } else { (speed / settings.reference_speed).clamp(0.4, 3.0) };
        if let Some(active) = player.animation_mut(node) {
            active.set_speed(rate);
        }
    }
    requests.0.clear();
}
