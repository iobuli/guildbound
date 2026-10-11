//! Character presentation: the FightingMotions fighter (converted from the Unity pack by
//! Tools/blender/fbx_pack_to_glb.py into assets/packs/, git-ignored) driven by sim state.
//! Locomotion uses the pack's in-place clips; the sim stays the only mover (MASTER L1).

use std::time::Duration;

use bevy::animation::AnimationTargetId;
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
    /// Idle clip used as the fighting stance.
    pub stance_idle: String,
    /// true: inertialize (start the new clip, ease out the pose offset from the last frame). false: crossfade.
    pub inertialize: bool,
    /// Body lean (rad) per m/s^2 of acceleration, and its limit.
    pub lean_per_accel: f32,
    pub lean_max: f32,
}

impl Default for AnimSettings {
    fn default() -> Self {
        Self {
            reference_speed: 1.4,
            blend_seconds: 0.2,
            stance_idle: "idle_Tiramis".into(),
            inertialize: true,
            lean_per_accel: 0.018,
            lean_max: 0.22,
        }
    }
}

/// Combat clusters: same strike type and limb, different clips. Each press takes the next variant,
/// so a repeated string never shows the same clip twice in a row. (cluster, limb, clips)
pub const CLUSTERS: &[(&str, &str, &[&str])] = &[
    ("jab", "left arm", &["hp_straight_left_A", "hp_straight_left_Tiramis"]),
    ("cross", "right arm", &["hp_straight_right_A", "hp_straight_right_A2"]),
    ("hook", "both arms", &["hp_hook_left_A", "bp_hook_right_A", "hp_hook_left_Tiramis", "bp_hook_right_A2"]),
    ("uppercut", "both arms", &["hp_upper_right_A", "bp_upper_left_A", "hp_upper_right_A2", "bp_upper_left_A2"]),
    ("roundhouse", "right leg", &["hk_rh_right_A2", "bk_rh_right_A2", "lk_rh_right_A2", "hk_rh_right_A", "lk_rh_right_Tiramis"]),
    ("side kick", "left leg", &["hk_side_left_A2", "lk_rh_left_A2", "hk_side_left_A", "lk_rh_left_A"]),
];

/// Next variant per cluster, so combos rotate through the cluster deterministically.
#[derive(Resource, Default)]
pub struct ClusterCursor(pub [usize; 6]);

impl ClusterCursor {
    pub fn next(&mut self, cluster: usize) -> &'static str {
        let clips = CLUSTERS[cluster].2;
        let clip = clips[self.0[cluster] % clips.len()];
        self.0[cluster] += 1;
        clip
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
    // inertialization: animated bones, last frame's final local pose, decaying offsets
    bones: Vec<Entity>,
    prev: Vec<Transform>,
    off_t: Vec<Vec3>,
    off_r: Vec<Quat>,
    pending_inertia: bool,
    inertia_left: f32,
    inertia_total: f32,
    // body lean spring (pitch, roll) in radians
    lean: Vec2,
    lean_vel: Vec2,
}

impl FighterRig {
    pub fn new(fighter: usize, color: Color) -> Self {
        Self {
            fighter,
            color,
            player: None,
            attached: false,
            current: None,
            one_shot: None,
            bones: Vec::new(),
            prev: Vec::new(),
            off_t: Vec::new(),
            off_r: Vec::new(),
            pending_inertia: false,
            inertia_left: 0.0,
            inertia_total: 0.0,
            lean: Vec2::ZERO,
            lean_vel: Vec2::ZERO,
        }
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
    targets: Query<(), With<AnimationTargetId>>,
    mut mats: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut rigs: Query<&mut FighterRig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(mut rig) = rigs.get_mut(ready.entity) else { return };
    let tint = materials.add(StandardMaterial { base_color: rig.color, perceptual_roughness: 0.7, ..default() });
    rig.bones.clear();
    for e in children.iter_descendants(ready.entity) {
        if players.contains(e) {
            rig.player = Some(e);
        }
        if targets.contains(e) {
            rig.bones.push(e);
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
                if settings.inertialize {
                    player.stop_all();
                    player.start(node);
                    rig.pending_inertia = true;
                } else {
                    transitions.play(&mut player, node, blend);
                }
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
        let idle = if lib.node(&settings.stance_idle).is_some() { settings.stance_idle.as_str() } else { IDLE };
        let wanted = if speed < 0.1 {
            idle
        } else if f.velocity.dot(f.facing) >= -0.3 * speed {
            FORWARD
        } else {
            BACKWARD
        };
        let Some(node) = lib.node(wanted) else { continue };
        if rig.current != Some(node) {
            if settings.inertialize {
                player.stop_all();
                player.start(node).repeat();
                rig.pending_inertia = true;
            } else {
                transitions.play(&mut player, node, blend).repeat();
            }
            rig.current = Some(node);
        }
        let rate = if wanted == idle { 1.0 } else { (speed / settings.reference_speed).clamp(0.4, 3.0) };
        if let Some(active) = player.animation_mut(node) {
            active.set_speed(rate);
        }
    }
    requests.0.clear();
}

/// Inertialization (MASTER 4.8, alpha lesson L4): on a clip change, capture the per-bone offset between
/// last frame's final pose and the new clip's first pose, then ease it to zero. The body moves on from
/// where it really was, so weight and limbs shift into the new move instead of crossfading two poses.
/// Offsets live in each bone's parent space (local transforms), which keeps rotations composable.
/// Runs after Bevy evaluates animation and before transforms propagate.
pub fn inertialize(time: Res<Time>, settings: Res<AnimSettings>, mut rigs: Query<&mut FighterRig>, mut bones: Query<&mut Transform, With<AnimationTargetId>>) {
    let dt = time.delta_secs();
    for mut rig in &mut rigs {
        let rig = &mut *rig;
        let n = rig.bones.len();
        if n == 0 {
            continue;
        }
        if rig.pending_inertia {
            rig.pending_inertia = false;
            if rig.prev.len() == n {
                rig.off_t.resize(n, Vec3::ZERO);
                rig.off_r.resize(n, Quat::IDENTITY);
                for (k, &b) in rig.bones.iter().enumerate() {
                    if let Ok(t) = bones.get(b) {
                        let p = rig.prev[k];
                        rig.off_t[k] = p.translation - t.translation;
                        let r = p.rotation * t.rotation.inverse();
                        rig.off_r[k] = if r.w < 0.0 { -r } else { r };
                    }
                }
                rig.inertia_total = settings.blend_seconds.max(0.01);
                rig.inertia_left = rig.inertia_total;
            }
        }
        let w = if rig.inertia_left > 0.0 {
            let s = 1.0 - rig.inertia_left / rig.inertia_total;
            1.0 - s * s * s * (s * (s * 6.0 - 15.0) + 10.0)
        } else {
            0.0
        };
        rig.prev.resize(n, Transform::IDENTITY);
        for (k, &b) in rig.bones.iter().enumerate() {
            let Ok(mut t) = bones.get_mut(b) else { continue };
            if w > 0.0 && k < rig.off_t.len() {
                t.translation += rig.off_t[k] * w;
                t.rotation = (Quat::IDENTITY.slerp(rig.off_r[k], w) * t.rotation).normalize();
            }
            rig.prev[k] = *t;
        }
        rig.inertia_left = (rig.inertia_left - dt).max(0.0);
    }
}

/// Weight shift from momentum: the body leans into acceleration and back against braking, from the feet,
/// on a critically damped spring. Reads the sim's per-tick acceleration; never writes the sim.
pub fn lean(time: Res<Time>, settings: Res<AnimSettings>, sim: Res<Sim>, mut rigs: Query<(&mut FighterRig, &mut Transform)>) {
    let dt = time.delta_secs().min(0.05);
    let omega = 12.0;
    for (mut rig, mut t) in &mut rigs {
        let Some(f) = sim.world.fighters.get(rig.fighter) else { continue };
        let forward = f.accel.dot(f.facing);
        let right = f.accel.dot(guildbound_sim::V2::new(-f.facing.z, f.facing.x));
        let max = settings.lean_max;
        let target = Vec2::new((-forward * settings.lean_per_accel).clamp(-max, max), (-right * settings.lean_per_accel).clamp(-max, max));
        let a = omega * omega * (target - rig.lean) - 2.0 * omega * rig.lean_vel;
        rig.lean_vel += a * dt;
        let v = rig.lean_vel;
        rig.lean += v * dt;
        t.rotation = Quat::from_rotation_x(rig.lean.x) * Quat::from_rotation_z(rig.lean.y) * Quat::from_rotation_y(std::f32::consts::PI);
    }
}
