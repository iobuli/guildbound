//! Character presentation: pack characters (converted from the Unity packs by Tools/blender/fbx_pack_to_glb.py
//! into assets/packs/<pack>/*.glb, git-ignored, normalized to 1.80 m) driven by sim state.
//! Any converted pack works: clip roles (stance, walk, run, back, reactions) and combat clusters are
//! resolved from clip names, and a dedicated root-motion bone is held in place so the sim stays the
//! only mover (MASTER L1).

use std::time::Duration;

use bevy::animation::AnimationTargetId;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady};

use crate::sim_bridge::{FighterView, Sim};

pub const FIGHTER_COLORS: [Color; 2] = [Color::srgb(0.25, 0.55, 0.95), Color::srgb(0.9, 0.3, 0.25)];

pub struct LibraryEntry {
    pub name: String,
    pub path: String,
}

/// Every converted pack under assets/packs, and which one the fighters wear.
#[derive(Resource)]
pub struct Libraries {
    pub list: Vec<LibraryEntry>,
    pub selected: usize,
    pub requested: Option<usize>,
}

impl Libraries {
    pub fn scan() -> Self {
        let mut list = Vec::new();
        let packs = std::path::Path::new(crate::ROOT).join("assets/packs");
        if let Ok(dirs) = std::fs::read_dir(&packs) {
            for dir in dirs.flatten().filter(|d| d.path().is_dir()) {
                let name = dir.file_name().to_string_lossy().to_string();
                let Ok(files) = std::fs::read_dir(dir.path()) else { continue };
                for file in files.flatten() {
                    let f = file.file_name().to_string_lossy().to_string();
                    if f.ends_with(".glb") {
                        list.push(LibraryEntry { name: name.clone(), path: format!("packs/{name}/{f}") });
                    }
                }
            }
        }
        list.sort_by(|a, b| a.name.cmp(&b.name));
        Self { list, selected: 0, requested: None }
    }

    pub fn current(&self) -> Option<&LibraryEntry> {
        self.list.get(self.selected)
    }
}

/// Clip names chosen for each job in the current library.
#[derive(Default, Clone, Debug)]
pub struct Roles {
    pub idle: Option<String>,
    pub walk: Option<String>,
    pub run: Option<String>,
    pub back: Option<String>,
    pub hit: Option<String>,
    pub knockdown: Option<String>,
    pub getup: Option<String>,
}

#[derive(Resource)]
pub struct AnimLibrary {
    gltf: Handle<Gltf>,
    pub graph: Option<Handle<AnimationGraph>>,
    /// Clip names and graph nodes, sorted by name.
    pub clips: Vec<(String, AnimationNodeIndex)>,
    pub roles: Roles,
    /// (cluster, limb, clips) resolved for this library; index matches CLUSTER_SPECS.
    pub clusters: Vec<(&'static str, &'static str, Vec<String>)>,
}

impl AnimLibrary {
    fn load(assets: &AssetServer, path: &str) -> Self {
        Self { gltf: assets.load(path.to_owned()), graph: None, clips: Vec::new(), roles: Roles::default(), clusters: Vec::new() }
    }

    pub fn node(&self, name: &str) -> Option<AnimationNodeIndex> {
        self.clips.iter().find(|(n, _)| n == name).map(|(_, i)| *i)
    }

    /// First clip (in name order) containing any preference, earlier preferences win; skips excluded words.
    fn find(&self, prefs: &[&str], exclude: &[&str]) -> Option<String> {
        prefs.iter().find_map(|p| {
            self.clips.iter().map(|(n, _)| n).find(|n| {
                let l = n.to_lowercase();
                l.contains(p) && !exclude.iter().any(|x| l.contains(x))
            })
        }).cloned()
    }
}

#[derive(Resource)]
pub struct AnimSettings {
    /// Ground speed (m/s) at which the walk clip plays at 1x, and the run clip.
    pub reference_speed: f32,
    pub run_reference_speed: f32,
    pub blend_seconds: f32,
    /// Idle clip used as the fighting stance (falls back to the library's idle role).
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
            run_reference_speed: 4.0,
            blend_seconds: 0.2,
            stance_idle: "idle_Tiramis".into(),
            inertialize: true,
            lean_per_accel: 0.018,
            lean_max: 0.22,
        }
    }
}

/// Combat clusters: same strike type and limb, different clips. Each press takes the next variant, so a
/// repeated string never shows the same clip twice in a row. (cluster, limb, key, exact names, name fragments)
/// Exact names are the curated FightingMotions set; fragments resolve the cluster in any other pack.
pub const CLUSTER_SPECS: &[(&str, &str, &str, &[&str], &[&str])] = &[
    ("jab", "left arm", "J", &["hp_straight_left_A", "hp_straight_left_Tiramis"], &["attack-l1", "attack-l2", "attack-l"]),
    ("cross", "right arm", "K", &["hp_straight_right_A", "hp_straight_right_A2"], &["attack-r1", "attack-r2", "attack-r"]),
    ("hook", "both arms", "U", &["hp_hook_left_A", "bp_hook_right_A", "hp_hook_left_Tiramis", "bp_hook_right_A2"], &["attack-l2", "attack-r2", "hook"]),
    ("uppercut", "both arms", "I", &["hp_upper_right_A", "bp_upper_left_A", "hp_upper_right_A2", "bp_upper_left_A2"], &["attack-l3", "attack-r3", "upper"]),
    ("roundhouse", "right leg", "L", &["hk_rh_right_A2", "bk_rh_right_A2", "lk_rh_right_A2", "hk_rh_right_A", "lk_rh_right_Tiramis"], &["kick-r", "kick"]),
    ("side kick", "left leg", "O", &["hk_side_left_A2", "lk_rh_left_A2", "hk_side_left_A", "lk_rh_left_A"], &["kick-l", "kick"]),
    ("run punch", "either arm", "Y", &[], &["run-forward-attack"]),
];

fn resolve_clusters(lib: &AnimLibrary) -> Vec<(&'static str, &'static str, Vec<String>)> {
    CLUSTER_SPECS
        .iter()
        .map(|(name, limb, _, exact, fragments)| {
            let mut clips: Vec<String> = exact.iter().filter(|c| lib.node(c).is_some()).map(|c| c.to_string()).collect();
            if clips.is_empty() {
                for frag in *fragments {
                    for (n, _) in &lib.clips {
                        if n.to_lowercase().contains(frag) && !clips.contains(n) {
                            clips.push(n.clone());
                        }
                    }
                }
            }
            (*name, *limb, clips)
        })
        .collect()
}

/// Next variant per cluster, so combos rotate through the cluster deterministically.
#[derive(Resource, Default)]
pub struct ClusterCursor(pub Vec<usize>);

impl ClusterCursor {
    pub fn next(&mut self, lib: &AnimLibrary, cluster: usize) -> Option<String> {
        let clips = &lib.clusters.get(cluster)?.2;
        if clips.is_empty() {
            return None;
        }
        if self.0.len() <= cluster {
            self.0.resize(cluster + 1, 0);
        }
        let clip = clips[self.0[cluster] % clips.len()].clone();
        self.0[cluster] += 1;
        Some(clip)
    }
}

/// One-shot clip requests from the panel, keys or the remote API: (fighter index, clip name).
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
    /// A dedicated root-motion bone ("Motion"/"root") and its rest translation, held in place.
    root_lock: Option<(Entity, Vec3)>,
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
            root_lock: None,
        }
    }
}

/// The pack model under a fighter view. Pack models face +Z; the view faces -Z (Bevy forward).
pub fn spawn_rig(commands: &mut Commands, view: Entity, fighter: usize, scene: Handle<WorldAsset>) {
    let color = FIGHTER_COLORS[fighter % FIGHTER_COLORS.len()];
    let rig = commands
        .spawn((FighterRig::new(fighter, color), WorldAssetRoot(scene), Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))))
        .observe(on_rig_ready)
        .id();
    commands.entity(view).add_child(rig);
}

pub fn load_library(mut commands: Commands, assets: Res<AssetServer>, libs: Res<Libraries>) {
    let path = libs.current().map_or("packs/fighting_motions/fighter.glb".to_owned(), |l| l.path.clone());
    commands.insert_resource(AnimLibrary::load(&assets, &path));
}

/// Model picker: swap every fighter's model and the clip library together.
pub fn switch_library(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut libs: ResMut<Libraries>,
    mut lib: ResMut<AnimLibrary>,
    mut cursor: ResMut<ClusterCursor>,
    rigs: Query<Entity, With<FighterRig>>,
    views: Query<(Entity, &FighterView)>,
) {
    let Some(i) = libs.requested.take() else { return };
    if i == libs.selected || i >= libs.list.len() {
        return;
    }
    libs.selected = i;
    let path = libs.list[i].path.clone();
    *lib = AnimLibrary::load(&assets, &path);
    cursor.0.clear();
    for e in &rigs {
        commands.entity(e).despawn();
    }
    let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
    for (view, v) in &views {
        spawn_rig(&mut commands, view, v.0, scene.clone());
    }
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
    let side = ["-left", "-right", "attack"];
    lib.roles = Roles {
        idle: lib.find(&["idle_tiramis", "idle_a", "unarmed-idle", "idle"], &["static", "talk", "loop_a"]),
        walk: lib.find(&["move_forward_b2", "strafe-forward", "walk"], &side),
        run: lib.find(&["run-forward", "jog", "sprint"], &side),
        back: lib.find(&["move_backward_b2", "strafe-backward", "run-backward", "backward"], &side),
        hit: lib.find(&["hd_front_a", "gethit-f1", "hit"], &[]),
        knockdown: lib.find(&["knockdown_a2", "knockdown"], &["loop"]),
        getup: lib.find(&["getup"], &[]),
    };
    lib.clusters = resolve_clusters(&lib);
    lib.graph = Some(graphs.add(graph));
}

/// When a fighter's model has spawned: remember its AnimationPlayer, bones and root-motion bone, and tint it.
pub fn on_rig_ready(
    ready: On<WorldInstanceReady>,
    children: Query<&Children>,
    players: Query<(), With<AnimationPlayer>>,
    targets: Query<(&Name, &Transform), With<AnimationTargetId>>,
    mut mats: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut rigs: Query<&mut FighterRig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(mut rig) = rigs.get_mut(ready.entity) else { return };
    let tint = materials.add(StandardMaterial { base_color: rig.color, perceptual_roughness: 0.7, ..default() });
    rig.bones.clear();
    rig.root_lock = None;
    for e in children.iter_descendants(ready.entity) {
        if players.contains(e) {
            rig.player = Some(e);
        }
        if let Ok((name, t)) = targets.get(e) {
            rig.bones.push(e);
            if rig.root_lock.is_none() && matches!(name.as_str(), "Motion" | "motion" | "root" | "Root_M") {
                rig.root_lock = Some((e, t.translation));
            }
        }
        if let Ok(mut m) = mats.get_mut(e) {
            m.0 = tint.clone();
        }
    }
}

/// In-place normalization at runtime: a dedicated root-motion bone keeps its vertical motion (hops, crouches)
/// but loses its horizontal travel, measured against world up in its parent's space.
pub fn lock_root_motion(rigs: Query<&FighterRig>, parents: Query<&ChildOf>, globals: Query<&GlobalTransform>, mut bones: Query<&mut Transform, With<AnimationTargetId>>) {
    for rig in &rigs {
        let Some((bone, rest)) = rig.root_lock else { continue };
        let Ok(parent) = parents.get(bone) else { continue };
        let Ok(pg) = globals.get(parent.parent()) else { continue };
        let up = pg.affine().inverse().transform_vector3(Vec3::Y).normalize_or_zero();
        let Ok(mut t) = bones.get_mut(bone) else { continue };
        let delta = t.translation - rest;
        t.translation = rest + up * delta.dot(up);
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
    let walk_run_split = 0.5 * (sim.world.tuning.walk_speed + sim.world.tuning.run_speed);
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
        let r = &lib.roles;
        let stance = if lib.node(&settings.stance_idle).is_some() { Some(settings.stance_idle.as_str()) } else { r.idle.as_deref() };
        let (wanted, reference) = if speed < 0.1 {
            (stance, 0.0)
        } else if f.velocity.dot(f.facing) < -0.3 * speed && r.back.is_some() {
            (r.back.as_deref(), settings.reference_speed)
        } else if speed > walk_run_split && r.run.is_some() {
            (r.run.as_deref(), settings.run_reference_speed)
        } else {
            (r.walk.as_deref().or(r.run.as_deref()), settings.reference_speed)
        };
        let Some(node) = wanted.and_then(|w| lib.node(w)) else { continue };
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
        let rate = if reference == 0.0 { 1.0 } else { (speed / reference).clamp(0.4, 3.0) };
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
