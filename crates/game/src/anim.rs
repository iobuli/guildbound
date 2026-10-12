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

/// Player blue, sparring partner red, mobs in earthy tones.
pub fn color_of(i: usize) -> Color {
    const MOBS: [Color; 4] = [Color::srgb(0.45, 0.62, 0.30), Color::srgb(0.70, 0.55, 0.25), Color::srgb(0.50, 0.40, 0.60), Color::srgb(0.35, 0.55, 0.55)];
    if i < 2 { FIGHTER_COLORS[i] } else { MOBS[(i - 2) % MOBS.len()] }
}

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
        // the hero library (RPG rig + retargeted FightingMotions) is the default model when it exists
        let selected = list.iter().position(|l| l.name == "hero").unwrap_or(0);
        Self { list, selected, requested: None }
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
    pub block: Option<String>,
    pub block_body: Option<String>,
    pub block_low: Option<String>,
    pub guard: Option<String>,
    pub spell_charge: Option<String>,
    pub hit_body: Option<String>,
    pub hit_low: Option<String>,
    pub jump: Option<String>,
    pub fall: Option<String>,
    pub roll: Option<String>,
    pub dizzy: Option<String>,
    pub drink: Option<String>,
    pub hit_back: Option<String>,
    pub death: Option<String>,
    pub death_backstab: Option<String>,
    pub guard_break: Option<String>,
    pub staggered: Option<String>,
    pub pickup: Option<String>,
}

#[derive(Resource)]
pub struct AnimLibrary {
    gltf: Handle<Gltf>,
    pub graph: Option<Handle<AnimationGraph>>,
    /// Clip names and graph nodes, sorted by name.
    pub clips: Vec<(String, AnimationNodeIndex)>,
    /// Clip lengths in seconds, same order as `clips`.
    pub durations: Vec<f32>,
    pub roles: Roles,
    /// (cluster, limb, clips) resolved for this library; index matches CLUSTER_SPECS.
    pub clusters: Vec<(&'static str, &'static str, Vec<String>)>,
    strides_m: std::collections::HashMap<String, f32>,
    contact_timing: std::collections::HashMap<String, crate::motion_timing::ContactTiming>,
    foot_contacts: std::collections::HashMap<String, Vec<[f32; 2]>>,
    pub sole_height_m: [f32; 2],
}

impl AnimLibrary {
    fn load(assets: &AssetServer, path: &str) -> Self {
        let motion = std::fs::read_to_string(format!("{}/assets/{path}.motion.json", crate::ROOT)).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
        let strides_m = motion.as_ref().filter(|m| m["verified"] == true).and_then(|m| m["strides_m"].as_object()).map(|m| m.iter().filter_map(|(name, v)| {
            let stride = v.as_f64()? as f32;
            (stride.is_finite() && stride > 0.05).then(|| (name.clone(), stride))
        }).collect()).unwrap_or_default();
        let contact_timing = motion.as_ref().filter(|m| m["verified"] == true).and_then(|m| m["strikes"].as_object()).map(|m| m.iter().filter_map(|(name, v)| {
            let timing = crate::motion_timing::ContactTiming {
                start: v["contact_start"].as_f64()? as f32,
                peak: v["contact_peak"].as_f64()? as f32,
                end: v["contact_end"].as_f64()? as f32,
            };
            timing.valid().then(|| (name.clone(), timing))
        }).collect()).unwrap_or_default();
        let foot_contacts = motion.as_ref().filter(|m| m["verified"] == true).and_then(|m| m["foot_contacts"].as_object()).map(|m| m.iter().filter_map(|(name, value)| {
            let curve: Vec<_> = value.as_array()?.iter().filter_map(|row| Some([row[0].as_f64()? as f32, row[1].as_f64()? as f32])).collect();
            (curve.len() >= 2 && curve.iter().flatten().all(|v| v.is_finite() && (0.0..=1.0).contains(v))).then(|| (name.clone(), curve))
        }).collect()).unwrap_or_default();
        let sole_height_m = std::array::from_fn(|i| motion.as_ref().and_then(|m| m["sole_height_m"][i].as_f64()).map(|v| v as f32).filter(|v| v.is_finite() && (0.03..0.2).contains(v)).unwrap_or(0.1));
        Self { gltf: assets.load(path.to_owned()), graph: None, clips: Vec::new(), durations: Vec::new(), roles: Roles::default(), clusters: Vec::new(), strides_m, contact_timing, foot_contacts, sole_height_m }
    }

    pub fn node(&self, name: &str) -> Option<AnimationNodeIndex> {
        self.clips.iter().find(|(n, _)| n == name).map(|(_, i)| *i)
    }

    pub fn duration(&self, name: &str) -> f32 {
        self.clips.iter().position(|(n, _)| n == name).and_then(|i| self.durations.get(i).copied()).unwrap_or(1.0)
    }

    pub fn stride(&self, name: &str, reference_speed: f32) -> f32 {
        self.strides_m.get(name).copied().unwrap_or_else(|| reference_speed * self.duration(name))
    }

    pub fn foot_contact(&self, name: &str, phase: f32) -> Option<[f32; 2]> {
        let curve = self.foot_contacts.get(name)?;
        let sample = phase.clamp(0.0, 1.0) * (curve.len() - 1) as f32;
        let first = sample.floor() as usize;
        let next = (first + 1).min(curve.len() - 1);
        Some(std::array::from_fn(|i| curve[first][i] + (curve[next][i] - curve[first][i]) * sample.fract()))
    }

    /// Phase (0..1) at which the left foot plants in a cyclic gait clip: the onset of its longest
    /// left-contact run. Locomotion clips are aligned on it so blends never mix opposite footfalls.
    pub fn left_strike(&self, name: &str) -> f32 {
        let Some(curve) = self.foot_contacts.get(name) else { return 0.0 };
        let n = curve.len() - 1; // last sample repeats the first
        if n < 2 {
            return 0.0;
        }
        let down = |k: usize| curve[k % n][0] > 0.5;
        let mut best = (0usize, 0usize);
        for k in 0..n {
            if down(k) && !down(k + n - 1) {
                let run = (0..n).take_while(|&j| down(k + j)).count();
                if run > best.1 {
                    best = (k, run);
                }
            }
        }
        best.0 as f32 / n as f32
    }

    fn strike_time(&self, clip: &str, m: guildbound_sim::combat::MoveId, clock: f32, fallback_rate: f32) -> f32 {
        let mv = &guildbound_sim::MOVES[m];
        self.contact_timing.get(clip).map_or(clock * fallback_rate, |timing| timing.sample(clock, mv.startup, mv.active, mv.recovery, self.duration(clip)))
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
    /// Locomotion blend space (true) or the nearest single clip (false), for A/B checks.
    pub blend_space: bool,
    /// Body lean (rad) per m/s^2 of acceleration, per m/s of speed (lean into a run), and its limit.
    pub lean_per_accel: f32,
    pub lean_per_speed: f32,
    pub lean_max: f32,
}

impl Default for AnimSettings {
    fn default() -> Self {
        Self {
            reference_speed: 2.1,
            run_reference_speed: 4.0,
            blend_seconds: 0.2,
            stance_idle: "idle_Tiramis".into(),
            inertialize: true,
            blend_space: true,
            lean_per_accel: 0.018,
            lean_per_speed: 0.02,
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
    ("roundhouse", "right leg", "L", &["hk_rh_right_A2", "hk_rh_right_A"], &["kick-r", "kick"]),
    ("side kick", "left leg", "O", &["hk_side_left_A2", "hk_side_left_A"], &["kick-l", "kick"]),
    ("run punch", "either arm", "Y", &[], &["run-forward-attack"]),
    ("front kick", "left leg", "M", &["bk_push_left_A"], &["kick"]),
    ("low kick", "either leg", "N", &["lk_rh_right_A2", "lk_rh_left_A2", "lk_rh_right_A", "lk_rh_left_A", "lk_rh_right_Tiramis"], &["kick"]),
    ("counter", "body strike", "H", &["bp_upper_left_A", "bp_hook_right_A2", "bk_rh_right_A2", "bk_rh_right_A"], &["attack-r3", "attack-l3"]),
];

fn resolve_clusters(lib: &AnimLibrary) -> Vec<(&'static str, &'static str, Vec<String>)> {
    let hero = lib.node("Unarmed-Idle").is_some() && lib.node("hp_straight_left_A").is_some();
    let mapping: serde_json::Value = serde_json::from_str(include_str!("../../../Tools/blender/hero_mapping.json")).expect("valid hero mapping");
    CLUSTER_SPECS
        .iter()
        .map(|(name, limb, _, exact, fragments)| {
            let mut clips: Vec<String> = exact.iter().filter(|c| lib.node(c).is_some()).map(|c| c.to_string()).collect();
            if hero && let Some(mapped) = mapping["combat"][*name].as_array() {
                clips = mapped.iter().filter_map(|c| c.as_str()).filter(|c| lib.node(c).is_some()).map(str::to_owned).collect();
            }
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
    pub(crate) bones: Vec<Entity>,
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
    /// Pelvis/hip bone (rest translation) and chest bone, for the procedural layers.
    pelvis: Option<(Entity, Vec3)>,
    chest: Option<Entity>,
    /// Smoothed chest aim (radians of yaw) and its velocity.
    aim: f32,
    aim_vel: f32,
    /// Last sim action seq presented, playback rate of the current clip, last block contact tick seen.
    action_seq: u32,
    action_kind: guildbound_sim::ActionKind,
    rate: f32,
    block_tick: u64,
    rule_sample: bool,
    clip_time_s: f32,
    gait_phase: f32,
    last_position: Option<guildbound_sim::V2>,
    last_tick: u64,
    prepared_clip: Option<(guildbound_sim::combat::MoveId, String)>,
    transition_s: f32,
    /// Bag size last frame (usize::MAX until first seen): growth plays the pickup clip.
    bag_len: usize,
    /// Nodes in the locomotion blend last frame, and the smoothed travel angle relative to facing.
    loco: Vec<AnimationNodeIndex>,
    loco_angle: f32,
}

impl FighterRig {
    pub fn sample_info(&self, world: &World, lib: &AnimLibrary) -> Option<(String, f32)> {
        let node = self.current?;
        let clip = lib.clips.iter().find(|(_, n)| *n == node)?.0.clone();
        let seek = world.get::<AnimationPlayer>(self.player?)?.animation(node)?.seek_time();
        let duration = lib.duration(&clip).max(0.001);
        let phase = if self.one_shot.is_some() { (seek / duration).clamp(0.0, 1.0) } else { seek.rem_euclid(duration) / duration };
        Some((clip, phase))
    }

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
            pelvis: None,
            chest: None,
            aim: 0.0,
            aim_vel: 0.0,
            action_seq: 0,
            action_kind: guildbound_sim::ActionKind::None,
            rate: 1.0,
            block_tick: 0,
            rule_sample: false,
            clip_time_s: 0.0,
            gait_phase: 0.0,
            last_position: None,
            last_tick: 0,
            prepared_clip: None,
            transition_s: 0.2,
            bag_len: usize::MAX,
            loco: Vec::new(),
            loco_angle: 0.0,
        }
    }
}

/// A compact read-only probe, including pose hashes to detect motion while paused.
pub fn snapshot(world: &mut World) -> serde_json::Value {
    let mut query = world.query::<&FighterRig>();
    let lib = world.resource::<AnimLibrary>();
    let rows: Vec<_> = query.iter(world).map(|rig| {
        let clip = rig.current.and_then(|node| lib.clips.iter().find(|(_, n)| *n == node)).map(|(n, _)| n);
        let seek = rig.player.and_then(|e| world.get::<AnimationPlayer>(e)).and_then(|p| rig.current.and_then(|n| p.animation(n))).map(|a| a.seek_time());
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut finite = true;
        let mut geometry = serde_json::Map::new();
        for &bone in &rig.bones {
            if let Some(name) = world.get::<Name>(bone)
                && matches!(name.as_str(), "Motion" | "B_Pelvis" | "B_Head" | "B_L_Foot" | "B_R_Foot" | "B_L_Forearm.001" | "B_R_Hand"
                    | "B_L_Thigh" | "B_R_Thigh" | "B_L_Calf" | "B_R_Calf" | "B_L_Toe0" | "B_R_Toe0")
                && let Some(global) = world.get::<GlobalTransform>(bone)
            {
                geometry.insert(name.to_string(), serde_json::json!(global.translation().to_array()));
            }
            if let Some(t) = world.get::<Transform>(bone) {
                for value in t.translation.to_array().into_iter().chain(t.rotation.to_array()).chain(t.scale.to_array()) {
                    finite &= value.is_finite();
                    hash = (hash ^ u64::from(value.to_bits())).wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        let contact = clip.and_then(|c| lib.contact_timing.get(c)).map(|t| serde_json::json!({"start": t.start, "peak": t.peak, "end": t.end, "duration_s": lib.duration(clip.unwrap())}));
        serde_json::json!({"fighter": rig.fighter, "clip": clip, "seek_s": seek, "gait_phase": rig.gait_phase, "contact_timing": contact,
                          "bones": rig.bones.len(), "finite": finite, "pose_hash": format!("{hash:016x}"), "rule_sample": rig.rule_sample, "geometry": geometry})
    }).collect();
    let knees = world.get_resource::<crate::knee::KneeStats>().copied().unwrap_or_default();
    let auth = world.get_resource::<crate::joints::JointAuthority>();
    let joint_rule = auth.map(|a| serde_json::json!({"enabled": a.enabled, "evaluated": a.evaluated, "corrected": a.corrected, "worst_excess_deg": a.worst_excess_deg,
        "player": a.readings.iter().map(|(n, bend)| serde_json::json!([n, bend.round()])).collect::<Vec<_>>()}));
    serde_json::json!({"fighters": rows, "joint_rule": joint_rule, "knee_rule": {"enabled": knees.enabled, "checked": knees.checked, "backward_fixed": knees.backward, "overflex_fixed": knees.overflex}})
}

/// The pack model under a fighter view. Pack models face +Z; the view faces -Z (Bevy forward).
pub fn spawn_rig(commands: &mut Commands, view: Entity, fighter: usize, scene: Handle<WorldAsset>) {
    let color = color_of(fighter);
    let rig = commands
        .spawn((FighterRig::new(fighter, color), WorldAssetRoot(scene), Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))))
        .observe(on_rig_ready)
        .id();
    commands.entity(view).add_child(rig);
}

pub fn load_library(mut commands: Commands, assets: Res<AssetServer>, libs: Res<Libraries>) {
    let path = libs.current().map_or("packs/hero/fighter.glb".to_owned(), |l| l.path.clone());
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

pub fn build_library(mut lib: ResMut<AnimLibrary>, gltfs: Res<Assets<Gltf>>, clips: Res<Assets<AnimationClip>>, mut graphs: ResMut<Assets<AnimationGraph>>) {
    if lib.graph.is_some() {
        return;
    }
    let Some(gltf) = gltfs.get(&lib.gltf) else { return };
    let mut named: Vec<_> = gltf.named_animations.iter().map(|(n, h)| (n.to_string(), h.clone())).collect();
    named.sort_by(|a, b| a.0.cmp(&b.0));
    if named.iter().any(|(_, h)| clips.get(h).is_none()) {
        return; // wait until every clip has loaded so durations are known
    }
    lib.durations = named.iter().map(|(_, h)| clips.get(h).map_or(1.0, |c| c.duration())).collect();
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
        block: lib.find(&["hb_front_a", "bb_front_a", "block"], &[]),
        block_body: lib.find(&["bb_front_a", "block"], &[]),
        block_low: lib.find(&["lb_front_a", "block"], &[]),
        guard: lib.find(&["idle_a", "unarmed-idle"], &["static"]),
        spell_charge: lib.find(&["unarmed-idle", "idle"], &["static"]),
        hit_body: lib.find(&["bd_front_a", "gethit-f2", "hit"], &[]),
        hit_low: lib.find(&["ld_front_a", "gethit-f2", "hit"], &[]),
        jump: lib.find(&["jump_start", "unarmed-jump", "jump"], &["flip", "land", "loop"]),
        fall: lib.find(&["jump_loop", "unarmed-fall", "fall"], &[]),
        roll: lib.find(&["diveroll", "roll"], &["_rm"]),
        dizzy: lib.find(&["mx_dizzy", "dizzy", "stunned"], &[]),
        drink: lib.find(&["mx_drink", "drink"], &[]),
        hit_back: lib.find(&["mx_hit_back", "gethit-b"], &[]),
        death: lib.find(&["mx_death", "death"], &["backstab", "kneel"]),
        death_backstab: lib.find(&["mx_death_backstab"], &[]),
        guard_break: lib.find(&["ual_guard_break", "shield_break"], &[]),
        staggered: lib.find(&["ual_hit_knockback", "knockback"], &["knockdown"]),
        pickup: lib.find(&["ual_pickup", "pickup"], &[]),
    };
    // The merged hero has a verified semantic map shared with the offline
    // converter. Keep fuzzy name discovery only for other inspectable packs.
    if lib.node("Unarmed-Idle").is_some() && lib.node("hp_straight_left_A").is_some() {
        let mapping: serde_json::Value = serde_json::from_str(include_str!("../../../Tools/blender/hero_mapping.json")).expect("valid hero mapping");
        let role = |key: &str| {
            let name = mapping["roles"][key].as_str()?;
            if lib.node(name).is_none() {
                warn!("Hero role {key}: UNKNOWN clip {name}");
                return None;
            }
            Some(name.to_owned())
        };
        lib.roles = Roles {
            idle: role("idle"), walk: role("walk"), run: role("run"), back: role("back"),
            hit: role("hit"), hit_body: role("hit_body"), hit_low: role("hit_low"), block: role("block"),
            block_body: role("block_body"), block_low: role("block_low"), guard: role("guard"), spell_charge: role("spell_charge"),
            knockdown: role("knockdown"), getup: role("getup"), jump: role("jump"), fall: role("fall"), roll: role("roll"),
            dizzy: role("dizzy"), drink: role("drink"), hit_back: role("hit_back"), death: role("death"), death_backstab: role("death_backstab"),
            guard_break: role("guard_break"), staggered: role("staggered"), pickup: role("pickup"),
        };
    }
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
    rig.pelvis = None;
    rig.chest = None;
    for e in children.iter_descendants(ready.entity) {
        if players.contains(e) {
            rig.player = Some(e);
        }
        if let Ok((name, t)) = targets.get(e) {
            rig.bones.push(e);
            if rig.root_lock.is_none() && matches!(name.as_str(), "Motion" | "motion" | "root" | "Root_M") {
                rig.root_lock = Some((e, t.translation));
            }
            let lower = name.as_str().to_lowercase();
            if rig.pelvis.is_none() && (lower == "hip" || lower == "hips" || lower.ends_with("pelvis")) {
                rig.pelvis = Some((e, t.translation));
            }
            // upper chest: FightingMotions "Chest", RPG "B_Spine2", UAL "spine_03"
            if matches!(lower.as_str(), "chest" | "b_spine2" | "spine_03" | "spine2") {
                rig.chest = Some(e);
            }
        }
        if let Ok(mut m) = mats.get_mut(e) {
            // textured models (the Quaternius master character) keep their textures: the player as authored,
            // others with the fighter colour multiplied in; untextured mannequins take the flat tint
            match materials.get(&m.0).cloned() {
                Some(mut mat) if mat.base_color_texture.is_some() => {
                    if rig.fighter != 0 {
                        mat.base_color = rig.color.mix(&Color::WHITE, 0.35);
                        m.0 = materials.add(mat);
                    }
                }
                _ => m.0 = tint.clone(),
            }
        }
    }
}

/// In-place normalization at runtime: a dedicated root-motion bone keeps its vertical motion (hops, crouches)
/// but loses its horizontal travel, measured against world up in its parent's space.
pub fn lock_root_motion(world: &mut World) {
    use crate::motion_math::world_transform;
    let mut query = world.query::<(Entity, &FighterRig)>();
    let rigs: Vec<_> = query.iter(world).map(|(e, r)| (e, r.fighter, r.root_lock, r.pelvis, r.one_shot.is_some(), r.bones.clone())).collect();
    for (entity, fighter, root, pelvis, committed, bones) in rigs {
        let Some(position) = world.resource::<Sim>().world.fighters.get(fighter).map(|f| f.position) else { continue };
        if let Some((bone, rest)) = root {
            // The sim owns all translation. Motion carries no vertical motion in any hero clip (measured: 0.000 m
            // range across all 130), so pin it to rest. Keeping its "vertical" part through a parent-derived up
            // axis leaked root travel into height whenever the body leaned (0.43 m pelvis sag at a 5 deg run lean,
            // released at the cycle wrap as an 83 deg knee pop).
            if let Some(mut pose) = world.get_mut::<Transform>(bone) {
                pose.translation = rest;
            }
        }
        if committed && let Some((bone, _)) = pelvis
            && let Some(parent) = world.get::<ChildOf>(bone).map(|p| p.parent())
            && let Some(parent_world) = world_transform(world, parent)
            && let Some(pelvis_world) = world_transform(world, bone)
        {
            // A rotating root can move an unchanged local hip by a metre.
            // Anchor XZ in world space, preserving the authored height/rotation.
            let target = Vec3::new(position.x, pelvis_world.translation().y, position.z);
            if let Some(mut pose) = world.get_mut::<Transform>(bone) {
                pose.translation = parent_world.affine().inverse().transform_point3(target);
            }
        }
        // Inertialization must remember the corrected outgoing base pose.
        let previous: Vec<_> = bones.iter().map(|&b| world.get::<Transform>(b).copied().unwrap_or(Transform::IDENTITY)).collect();
        if let Some(mut rig) = world.get_mut::<FighterRig>(entity) { rig.prev = previous; }
    }
}

/// Procedural layers on top of the clips, computed from the sim (code only, no new clips):
/// - chest aim: during a strike or cast the upper body turns toward the target (bounded +-35 deg, sprung),
///   so punches track the head or body the rules aimed at, without stretching any bone;
/// - weight shift: a small lift through the wind-up into contact, the "hop onto the lead leg" into a strike.
/// Runs after inertialization so it sits on the final clip pose.
pub fn procedural_layers(
    sim: Res<Sim>,
    mut rigs: Query<&mut FighterRig>,
    parents: Query<&ChildOf>,
    globals: Query<&GlobalTransform>,
    mut bones: Query<&mut Transform, With<AnimationTargetId>>,
) {
    use guildbound_sim::ActionKind as K;
    for mut rig in &mut rigs {
        let Some(f) = sim.world.fighters.get(rig.fighter) else { continue };
        let dt = if f.hit_stop > 0.0 { 0.0 } else { sim.frame_step_s };
        // target yaw relative to the body's facing
        let mut want = 0.0;
        let mut lift = 0.0;
        if let Some(t) = f.strike_target.and_then(|t| sim.world.fighters.get(t)) {
            let aiming = match f.action.kind {
                K::Strike(m) => {
                    let d = &guildbound_sim::MOVES[m];
                    let e = f.action.elapsed;
                    let into = (e / (d.startup + d.active)).clamp(0.0, 1.0);
                    // lift rises through the wind-up, peaks at contact, settles in recovery
                    lift = 0.045 * (std::f32::consts::PI * into).sin() * if e > d.startup + d.active { 0.0 } else { 1.0 };
                    e < d.total() - d.recovery * 0.4
                }
                K::Cast(_) | K::Charge(_) => true,
                _ => false,
            };
            if aiming {
                let to = t.position - f.position;
                if to.length_squared() > 1e-4 {
                    want = f.facing.signed_angle_to(to.normalized()).clamp(-0.6, 0.6);
                }
            }
        }
        // critically damped spring toward the wanted aim
        (rig.aim, rig.aim_vel) = crate::motion_math::spring(rig.aim, rig.aim_vel, want, 18.0, dt);

        if let Some(chest) = rig.chest
            && rig.aim.abs() > 1e-3
            && let Ok(parent) = parents.get(chest)
            && let Ok(pg) = globals.get(parent.parent())
            && let Ok(mut t) = bones.get_mut(chest)
        {
            // yaw about world up, expressed in the chest's parent space (parent-space composition, no stretch);
            // the sim's yaw is measured +X toward +Z, the same sense as a negative rotation about +Y
            let (_, prot, _) = pg.to_scale_rotation_translation();
            let world_yaw = Quat::from_rotation_y(-rig.aim);
            let local = prot.inverse() * world_yaw * prot;
            t.rotation = (local * t.rotation).normalize();
        }
        if lift > 0.0
            && let Some((pelvis, _)) = rig.pelvis
            && let Ok(parent) = parents.get(pelvis)
            && let Ok(pg) = globals.get(parent.parent())
            && let Ok(mut t) = bones.get_mut(pelvis)
        {
            let up = pg.affine().inverse().transform_vector3(Vec3::Y);
            t.translation += up * lift;
        }
    }
}

/// The clip for a sim action, and how long it must last: strikes play their cluster's next variant,
/// reactions play the library's role clips.
/// What the last contact on this fighter tells the reaction picker.
#[derive(Clone, Copy, Default)]
struct LastHit {
    aim: Option<guildbound_sim::combat::Aim>,
    from_behind: bool,
    deathblow: bool,
}

fn action_clip(lib: &AnimLibrary, cursor: &mut ClusterCursor, f: &guildbound_sim::Fighter, hit: LastHit) -> Option<(String, f32)> {
    let aim = hit.aim;
    // looping or long-form clips play at their own speed; the sim's action ends them
    let natural = |clip: &Option<String>| clip.clone().map(|c| { let d = lib.duration(&c); (c, d) });
    use guildbound_sim::ActionKind as K;
    let r = &lib.roles;
    let (clip, length) = match f.action.kind {
        K::Strike(m) | K::HeavyCharge(m) => {
            let mv = &guildbound_sim::MOVES[m];
            let pick = |name: &str| CLUSTER_SPECS.iter().position(|c| c.0 == name);
            let fallback = if mv.heavy { "roundhouse" } else { "cross" };
            let clip = [mv.cluster, fallback, "jab"].iter().filter_map(|n| pick(n)).find_map(|i| cursor.next(lib, i));
            (clip, mv.total())
        }
        K::HitStun if hit.from_behind && r.hit_back.is_some() => (r.hit_back.clone(), f.action.length),
        K::Dizzy => return natural(&r.dizzy).or_else(|| r.hit_body.clone().map(|c| (c, f.action.length))),
        K::Drink(_) => (r.drink.clone(), f.action.length),
        K::Knockdown if f.hp <= 0.0 => {
            let clip = if hit.deathblow && hit.from_behind { r.death_backstab.clone().or(r.death.clone()) } else { r.death.clone() };
            return natural(&clip).or_else(|| r.knockdown.clone().map(|c| (c, f.action.length)));
        }
        K::HitStun => {
            // head, body or low reaction from where the hit landed
            use guildbound_sim::combat::Aim;
            let clip = match aim {
                Some(Aim::Body) => r.hit_body.clone(),
                Some(Aim::Low) => r.hit_low.clone(),
                _ => None,
            };
            (clip.or(r.hit.clone()), f.action.length)
        }
        K::GuardBreak => (r.guard_break.clone().or(r.hit_body.clone()).or(r.hit.clone()), f.action.length),
        K::Staggered => (r.staggered.clone().or(r.hit_body.clone()).or(r.hit.clone()), f.action.length),
        K::Knockdown => (r.knockdown.clone(), f.action.length),
        K::GetUp => (r.getup.clone(), f.action.length),
        // Fire/lightning use mage clips; ki retains its quick punching motion.
        K::Cast(s) => {
            let spell = &guildbound_sim::SPELLS[s];
            let map: serde_json::Value = serde_json::from_str(include_str!("../../../Tools/blender/hero_mapping.json")).expect("valid hero mapping");
            let key = if s == guildbound_sim::magic::FIREBALL && f.charge > 0.05 { "fireball_charged" } else { spell.name };
            if let Some(names) = map["spells"][key].as_array() {
                let available: Vec<_> = names.iter().filter_map(|n| n.as_str()).filter(|n| lib.node(n).is_some()).collect();
                if !available.is_empty() {
                    let index = CLUSTER_SPECS.len() + s;
                    if cursor.0.len() <= index { cursor.0.resize(index + 1, 0); }
                    let clip = available[cursor.0[index] % available.len()].to_owned();
                    cursor.0[index] += 1;
                    return Some((clip, spell.prepare + spell.recovery));
                }
            }
            let pick = |name: &str| CLUSTER_SPECS.iter().position(|c| c.0 == name);
            let cluster = if matches!(spell.kind, guildbound_sim::SpellKind::Nova) { "uppercut" } else { "cross" };
            let clip = [cluster, "jab"].iter().filter_map(|n| pick(n)).find_map(|i| cursor.next(lib, i));
            (clip, spell.prepare + spell.recovery)
        }
        K::Charge(_) => {
            let clip = r.spell_charge.clone();
            let duration = clip.as_ref().map_or(1.0, |c| lib.duration(c));
            (clip, duration)
        }
        K::Roll => (r.roll.clone().or(r.walk.clone()), f.action.length),
        K::Step => {
            let right = guildbound_sim::V2::new(f.facing.z, -f.facing.x);
            let sideways = f.velocity.dot(right);
            let forward = f.velocity.dot(f.facing);
            let name = if sideways.abs() > forward.abs() {
                if sideways > 0.0 { "Unarmed-Strafe-Right" } else { "Unarmed-Strafe-Left" }
            } else if forward > 0.0 { "Unarmed-Strafe-Forward" } else { "Unarmed-Strafe-Backward" };
            (lib.node(name).map(|_| name.to_owned()).or(r.walk.clone()), f.action.length)
        }
        _ => (None, 0.0),
    };
    clip.map(|c| (c, length))
}

pub fn drive_animation(
    mut commands: Commands,
    mut cursor: ResMut<ClusterCursor>,
    lib: Res<AnimLibrary>,
    settings: Res<AnimSettings>,
    sim: Res<Sim>,
    locomotion: Res<crate::controller::LocomotionMap>,
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
        // A reset can reuse an action seq; invalidate the old view explicitly.
        if sim.world.tick < rig.last_tick {
            player.stop_all();
            rig.current = None;
            rig.one_shot = None;
            rig.rule_sample = false;
            rig.clip_time_s = 0.0;
            rig.action_seq = u32::MAX;
            rig.block_tick = 0;
            rig.gait_phase = 0.0;
            rig.last_position = None;
            rig.pending_inertia = false;
            rig.inertia_left = 0.0;
            rig.prev.clear();
            rig.lean = Vec2::ZERO;
            rig.lean_vel = Vec2::ZERO;
            rig.aim = 0.0;
            rig.aim_vel = 0.0;
            rig.prepared_clip = None;
        }
        rig.last_tick = sim.world.tick;
        let travel = rig.last_position.map_or(0.0, |p| (f.position - p).length());
        rig.last_position = Some(f.position);

        let me = rig.fighter;
        // picking something up plays the pickup motion as a free (cancellable) one-shot
        let bag = f.kit.bag.len();
        let picked = bag > rig.bag_len && f.action.free() && rig.bag_len != usize::MAX;
        rig.bag_len = bag;
        let pickup = lib.roles.pickup.clone().filter(|_| picked).map(|c| (me, c));
        for (_, clip) in requests.0.iter().filter(|(i, _)| *i == me).chain(pickup.iter()) {
            if let Some(node) = lib.node(clip) {
                if settings.inertialize {
                    player.stop_all();
                    player.start(node);
                    rig.pending_inertia = true;
                } else {
                    transitions.play(&mut player, node, blend);
                }
                rig.current = Some(node);
                rig.rate = 1.0;
                rig.one_shot = Some(clip.clone());
                rig.rule_sample = false;
                rig.clip_time_s = 0.0;
                rig.transition_s = settings.blend_seconds;
            }
        }
        // sim actions drive the clip: a new action seq starts its clip, retimed to the rule length
        if f.action.seq != rig.action_seq || f.action.kind != rig.action_kind {
            rig.action_seq = f.action.seq;
            rig.action_kind = f.action.kind;
            let hit = sim.recent.iter().rev().find(|(c, _)| c.defender == me).map(|(c, _)| {
                let from = sim.world.fighters.get(c.attacker).map_or(f.position, |a| a.position) - f.position;
                LastHit { aim: Some(c.aim), from_behind: from.dot(f.facing) < 0.0, deathblow: c.outcome == guildbound_sim::Outcome::Deathblow }
            }).unwrap_or_default();
            let chosen = match f.action.kind {
                guildbound_sim::ActionKind::Strike(m) if rig.prepared_clip.as_ref().is_some_and(|(id, _)| *id == m) => {
                    rig.prepared_clip.take().map(|(_, clip)| (clip, guildbound_sim::MOVES[m].total()))
                }
                _ => action_clip(&lib, &mut cursor, f, hit),
            };
            if let Some((clip, length)) = chosen {
                rig.prepared_clip = match f.action.kind {
                    guildbound_sim::ActionKind::HeavyCharge(m) => Some((m, clip.clone())),
                    _ => None,
                };
                if let Some(node) = lib.node(&clip) {
                    rig.transition_s = match f.action.kind {
                        guildbound_sim::ActionKind::Strike(m) => (guildbound_sim::MOVES[m].startup * 0.65).clamp(0.035, settings.blend_seconds.max(0.035)),
                        _ => settings.blend_seconds.min(0.12),
                    };
                    if settings.inertialize {
                        player.stop_all();
                        player.start(node);
                        rig.pending_inertia = true;
                    } else {
                        transitions.play(&mut player, node, Duration::from_secs_f32(rig.transition_s.max(0.01)));
                    }
                    rig.rate = lib.duration(&clip) / length.max(0.05);
                    rig.current = Some(node);
                    rig.one_shot = Some(clip);
                    rig.rule_sample = true;
                }
            } else if rig.rule_sample {
                // A cancelled strike must not keep sampling its old clip while
                // a new guard/charge action owns the actor's clock.
                rig.one_shot = None;
                rig.rule_sample = false;
                rig.prepared_clip = None;
            }
        }
        // a blocked hit plays a short block flinch on the defender
        let blocked = sim.recent.iter().rev().find(|(c, _)| c.defender == me && c.outcome == guildbound_sim::Outcome::Blocked).map(|(c, _)| (c.tick, c.aim));
        if let Some((tick, aim)) = blocked.filter(|(t, _)| *t != rig.block_tick) {
            rig.block_tick = tick;
            let block_clip = match aim {
                guildbound_sim::combat::Aim::Head => lib.roles.block.as_ref(),
                guildbound_sim::combat::Aim::Body => lib.roles.block_body.as_ref(),
                guildbound_sim::combat::Aim::Low => lib.roles.block_low.as_ref(),
            }.or(lib.roles.block.as_ref());
            if let Some(node) = block_clip.and_then(|b| lib.node(b)) {
                player.stop_all();
                player.start(node);
                rig.pending_inertia = settings.inertialize;
                rig.rate = 1.6;
                rig.current = Some(node);
                rig.one_shot = block_clip.cloned();
                rig.rule_sample = false;
                rig.clip_time_s = 0.0;
                rig.transition_s = 0.045;
            }
        }
        // hit-stop freezes the pose; otherwise one-shots run at their retimed rate
        if let Some(node) = rig.current.filter(|_| rig.one_shot.is_some()) {
            if !rig.rule_sample && f.hit_stop <= 0.0 {
                rig.clip_time_s += sim.frame_step_s * rig.rate;
            }
            if let Some(a) = player.animation_mut(node) {
                if rig.rule_sample {
                    let clock = match f.action.kind {
                        guildbound_sim::ActionKind::HeavyCharge(m) => guildbound_sim::MOVES[m].startup * 0.75 * f.heavy_charge,
                        _ => f.action.elapsed,
                    };
                    let seek = match f.action.kind {
                        guildbound_sim::ActionKind::Strike(m) | guildbound_sim::ActionKind::HeavyCharge(m) => {
                            lib.strike_time(rig.one_shot.as_deref().unwrap(), m, clock, rig.rate)
                        }
                        guildbound_sim::ActionKind::Cast(s) => {
                            let spell = &guildbound_sim::SPELLS[s];
                            let clip = rig.one_shot.as_deref().unwrap();
                            lib.contact_timing.get(clip).map_or(clock * rig.rate, |timing| timing.release_sample(clock, spell.prepare, spell.recovery, lib.duration(clip)))
                        }
                        guildbound_sim::ActionKind::Charge(_) => clock.min(lib.duration(rig.one_shot.as_deref().unwrap()) - 0.001),
                        _ => clock * rig.rate,
                    };
                    a.set_speed(0.0).set_seek_time(seek);
                } else {
                    a.set_speed(0.0).set_seek_time(rig.clip_time_s);
                }
            }
            let busy = !f.action.free();
            let preview_running = !rig.rule_sample && rig.one_shot.as_ref().is_some_and(|name| rig.clip_time_s < lib.duration(name));
            if busy || preview_running {
                continue;
            }
            rig.one_shot = None;
            rig.rule_sample = false;
        }

        let speed = f.velocity.length();
        let r = &lib.roles;
        // airborne: jump clip on the way up, fall clip on the way down
        if f.height > 0.0 || f.crouch_left > 0.0 {
            let air = if f.vertical > 0.0 || f.crouch_left > 0.0 { r.jump.as_deref() } else { r.fall.as_deref().or(r.jump.as_deref()) };
            if let Some(node) = air.and_then(|a| lib.node(a)) {
                if rig.current != Some(node) {
                    player.stop_all();
                    player.start(node);
                    rig.pending_inertia = settings.inertialize;
                    rig.current = Some(node);
                    rig.clip_time_s = 0.0;
                }
                if f.hit_stop <= 0.0 {
                    rig.clip_time_s += sim.frame_step_s;
                }
                if let Some(active) = player.animation_mut(node) {
                    active.set_speed(0.0).set_seek_time(rig.clip_time_s);
                }
                continue;
            }
        }
        let stance = if lib.node(&settings.stance_idle).is_some() { Some(settings.stance_idle.as_str()) } else { r.idle.as_deref() };
        let pose = match f.action.kind {
            guildbound_sim::ActionKind::Guard => r.guard.as_deref().or(stance),
            guildbound_sim::ActionKind::Charge(_) => r.spell_charge.as_deref().or(stance),
            _ => stance,
        };
        // Blend space: the two nearest of eight directions, each at walk and at run, weighted by the
        // smoothed travel angle and by speed between walk and run; the stance fades in below 0.6 m/s.
        // Every gait clip is sampled at one shared phase aligned on its left-foot plant, so footfalls
        // agree across the blend and the stride follows the blended clips (no direction or gait pops).
        let dt_anim = if f.hit_stop > 0.0 { 0.0 } else { sim.frame_step_s };
        if speed > 0.05 {
            // the character's right: forward x up = (fz, 0, -fx) x ... -> (-fz, fx) on the ground plane.
            // (facing +Z with +Y up has its right at -X; the old (fz, -fx) was the left, measured as slip)
            let right = guildbound_sim::V2::new(-f.facing.z, f.facing.x);
            let want = f.velocity.dot(right).atan2(f.velocity.dot(f.facing));
            let d = (want - rig.loco_angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            rig.loco_angle += d * (1.0 - (-14.0 * dt_anim).exp());
        }
        let (walk_speed, run_speed, sprint_speed) = (sim.world.tuning.walk_speed, sim.world.tuning.run_speed, sim.world.tuning.sprint_speed);
        // out of combat the relaxed set plays (walk -> jog -> sprint); in combat the RPG strafe set
        let relaxed = sim.world.tuning.relaxed && !sim.world.in_combat(rig.fighter);
        let gait = if speed <= run_speed || !relaxed {
            ((speed - walk_speed) / (run_speed - walk_speed).max(0.1)).clamp(0.0, 1.0)
        } else {
            1.0 + ((speed - run_speed) / (sprint_speed - run_speed).max(0.1)).clamp(0.0, 1.0)
        };
        let idle_k = (speed / 0.6).clamp(0.0, 1.0);
        let idle_w = 1.0 - idle_k * idle_k * (3.0 - 2.0 * idle_k);
        let mut set: Vec<(String, f32, bool)> = Vec::new(); // clip, weight, gait-synced
        if idle_w < 0.999 {
            let (angle, gait) = if settings.blend_space {
                (rig.loco_angle, gait)
            } else {
                // A/B reference: snap to the nearest direction and to walk or run
                let q = std::f32::consts::FRAC_PI_4;
                ((rig.loco_angle / q).round() * q, gait.round())
            };
            for (clip, w) in locomotion.blend(&lib, angle, gait, relaxed) {
                set.push((clip.to_owned(), w * (1.0 - idle_w), true));
            }
        }
        if let Some(p) = pose.filter(|_| idle_w > 0.001 || set.is_empty()) {
            let w = if set.is_empty() { 1.0 } else { idle_w };
            set.push((p.to_owned(), w, false));
        }
        let nodes: Vec<(AnimationNodeIndex, String, f32, bool)> = set.into_iter().filter_map(|(c, w, g)| lib.node(&c).map(|n| (n, c, w, g))).collect();
        let Some(dominant) = nodes.iter().max_by(|a, b| a.2.total_cmp(&b.2)).map(|x| x.0) else { continue };
        let entering = rig.loco.is_empty() || rig.current.is_none_or(|c| !rig.loco.contains(&c)) || !rig.loco.iter().all(|n| player.is_playing_animation(*n));
        if entering {
            let first_pose = rig.current.is_none();
            rig.transition_s = settings.blend_seconds;
            player.stop_all();
            rig.loco.clear();
            // An imported bind pose is not an outgoing gameplay pose: never inertialize from it.
            rig.pending_inertia = settings.inertialize && !first_pose;
        }
        for &old in &rig.loco {
            if !nodes.iter().any(|x| x.0 == old) {
                player.stop(old);
            }
        }
        // shared gait phase advances by distance over the blended stride
        let synced: f32 = nodes.iter().filter(|x| x.3).map(|x| x.2).sum();
        if synced > 1e-4 {
            let stride: f32 = nodes.iter().filter(|x| x.3).map(|x| {
                let reference = if x.1.contains("sprint") { sim.world.tuning.sprint_speed } else if x.1.contains("Run") || x.1.contains("run") { settings.run_reference_speed } else { settings.reference_speed };
                lib.stride(&x.1, reference) * x.2
            }).sum::<f32>() / synced;
            rig.gait_phase = crate::controller::advance_phase(rig.gait_phase, travel, stride);
        }
        let rate = if sim.paused || f.hit_stop > 0.0 { 0.0 } else { sim.speed };
        for (node, clip, w, synced_clip) in &nodes {
            let active = player.play(*node).repeat();
            active.set_weight(*w);
            if *synced_clip {
                let phase = (rig.gait_phase + lib.left_strike(clip)).rem_euclid(1.0);
                active.set_speed(0.0).set_seek_time(phase * lib.duration(clip));
            } else {
                active.set_speed(rate);
            }
        }
        rig.loco = nodes.iter().map(|x| x.0).collect();
        rig.current = Some(dominant);
    }
    requests.0.clear();
}

/// Inertialization (MASTER 4.8, alpha lesson L4): on a clip change, capture the per-bone offset between
/// last frame's final pose and the new clip's first pose, then ease it to zero. The body moves on from
/// where it really was, so weight and limbs shift into the new move instead of crossfading two poses.
/// Offsets live in each bone's parent space (local transforms), which keeps rotations composable.
/// Runs after Bevy evaluates animation and before transforms propagate.
pub fn inertialize(sim: Res<Sim>, mut rigs: Query<&mut FighterRig>, mut bones: Query<&mut Transform, With<AnimationTargetId>>) {
    for mut rig in &mut rigs {
        let rig = &mut *rig;
        let dt = if sim.world.fighters.get(rig.fighter).is_some_and(|f| f.hit_stop > 0.0) { 0.0 } else { sim.frame_step_s };
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
                rig.inertia_total = rig.transition_s.max(0.01);
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
pub fn lean(settings: Res<AnimSettings>, sim: Res<Sim>, mut rigs: Query<(&mut FighterRig, &mut Transform)>) {
    let omega = 12.0;
    for (mut rig, mut t) in &mut rigs {
        let Some(f) = sim.world.fighters.get(rig.fighter) else { continue };
        let dt = if f.hit_stop > 0.0 { 0.0 } else { sim.frame_step_s };
        // lean = speed term (into the run) + acceleration term (weight shift on starts, stops and turns)
        let right_axis = guildbound_sim::V2::new(-f.facing.z, f.facing.x);
        let forward = f.accel.dot(f.facing) * settings.lean_per_accel + f.velocity.dot(f.facing) * settings.lean_per_speed;
        let right = f.accel.dot(right_axis) * settings.lean_per_accel + f.velocity.dot(right_axis) * settings.lean_per_speed;
        let max = settings.lean_max;
        let target = Vec2::new((-forward).clamp(-max, max), (-right).clamp(-max, max));
        let x = crate::motion_math::spring(rig.lean.x, rig.lean_vel.x, target.x, omega, dt);
        let y = crate::motion_math::spring(rig.lean.y, rig.lean_vel.y, target.y, omega, dt);
        rig.lean = Vec2::new(x.0, y.0);
        rig.lean_vel = Vec2::new(x.1, y.1);
        t.rotation = Quat::from_rotation_x(rig.lean.x) * Quat::from_rotation_z(rig.lean.y) * Quat::from_rotation_y(std::f32::consts::PI);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_centimetre_root_cannot_move_the_hip_ahead_of_the_motor() {
        let mut world = World::new();
        world.insert_resource(Sim::new());
        let actor = world.spawn((Transform::from_xyz(0.0, 0.0, -1.5).with_scale(Vec3::splat(0.01)), FighterRig::new(0, Color::WHITE))).id();
        let motion = world.spawn(Transform::from_rotation(Quat::from_rotation_x(1.1))).id();
        let hip = world.spawn(Transform::from_xyz(0.0, 90.0, 0.0)).id();
        world.entity_mut(actor).add_child(motion);
        world.entity_mut(motion).add_child(hip);
        {
            let mut rig = world.get_mut::<FighterRig>(actor).unwrap();
            rig.root_lock = Some((motion, Vec3::ZERO));
            rig.pelvis = Some((hip, Vec3::Y * 90.0));
            rig.one_shot = Some("roll".to_owned());
            rig.bones = vec![motion, hip];
        }
        let before = crate::motion_math::world_transform(&world, hip).unwrap().translation();
        assert!((before.z + 1.5).abs() > 0.7, "fixture must reproduce the rotating-root offset");
        lock_root_motion(&mut world);
        let after = crate::motion_math::world_transform(&world, hip).unwrap().translation();
        assert!(after.x.abs() < 1e-5 && (after.z + 1.5).abs() < 1e-5);
        assert!((after.y - before.y).abs() < 1e-5);
        assert_eq!(world.get::<Transform>(hip).unwrap().scale, Vec3::ONE);
        assert_eq!(world.get::<FighterRig>(actor).unwrap().prev[1], *world.get::<Transform>(hip).unwrap());
    }
}
