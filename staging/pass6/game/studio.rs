//! Studio (F2): HUD Layer Lab restyle via crate::ui_theme + crate::studio_kit (tabs Pose/Physics/Animation/
//! Events/Controller, bone tree, view toolbar, status strip, edit log). Original header follows.
//! Studio (F2): the in-game debug editor for animations, models and assets - and the loop between the boss
//! and Claude. Every model and kit in assets/packs can be put on the stage; rigged ones play their own clips
//! or the hero library on a timeline. The inspector shows the numbers Claude reasons with (segment bends
//! against the joint ranges, knee flexion and direction, foot heights). Click a bone, write what is wrong, and
//! the note goes to collab/studio_notes.jsonl with every number of that pose. Corrections are per-clip,
//! per-bone rotation offsets keyed over the clip (assets/corrections.json), applied live under the joint and
//! knee authority; the Patterns tab groups corrections and notes across clips and models and can promote a
//! repeated fix to a library-wide rule. The HUD tab moves HUD elements and saves the layout. Engine tab:
//! frame time, entities, assets, sim and physics. All of it is readable through gb.studio.
//!
//! Claude's side of the loop is Tools/blender/pose_audit.py: every clip of every model, every frame, against
//! the same rules, with each finding marked as inherited from the source clip or introduced by our pipeline.
//! Its digest (collab/pose_audit.json) shows in the Patterns tab; clicking a pattern stages its worst case.
use std::collections::BTreeMap;
use std::io::Write as _;

use bevy::animation::AnimationTargetId;
use bevy::diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady};
use bevy_egui::{EguiContexts, egui};
use serde_json::{Value, json};

use crate::anim::AnimLibrary;
use crate::motion_math::world_transform;
use crate::sim_bridge::Sim;
use crate::ui_theme as theme;

/// Where models are staged: its own place 1.5 km from the world, past the camera's draw distance, so the Studio
/// is a scene of its own (only the stage model, its floor and light), with or without the game behind it.
pub const STAGE: Vec3 = Vec3::new(0.0, 0.0, 1500.0);

/// Models the game itself uses (the hero, townsfolk, your Creator characters): they play the whole library.
fn is_game_model(path: &str) -> bool {
    ["packs/hero/", "packs/npc/", "packs/custom/"].iter().any(|p| path.starts_with(p))
}

const CATEGORIES: [&str; 8] = ["foot sliding", "knee / leg bend", "arm / shoulder", "spine / head", "clipping", "timing", "scale / proportions", "other"];
pub const CREATOR_TAB: usize = 1;
pub const POSE_TAB: usize = 9;
const TAB_HELP: [&str; 10] = [
    "Inspect: see how each joint is bent and send Claude a note about anything that looks wrong.",
    "Creator: build your own character from outfit parts, colour it, and save it as a playable model.",
    "Tuning: change how characters move, animate and come alive. Changes apply straight away.",
    "Moveset: map movement, combat, reactions, magic and weapons. Preview and save your choices.",
    "Corrections: the bone fixes you have saved, per animation.",
    "Patterns: problems that repeat across animations, from your notes and Claude's audit.",
    "Notes: the notes you have sent to Claude.",
    "HUD Layout: move and resize the health, skill and target panels on screen.",
    "Engine: performance numbers - frame rate, object counts and the game simulation.",
    "Pose: per-bone controller, contacts and pins, ragdoll and agent control. Click a bone on the model to select it.",
];
const TABS: [&str; 10] = ["Inspect", "Creator", "Tuning", "Moveset", "Corrections", "Patterns", "Notes", "HUD Layout", "Engine", "Pose"];

#[derive(Clone, Debug, PartialEq)]
pub struct Correction {
    /// Clip name, or "*" for every clip of the library.
    pub clip: String,
    pub bone: String,
    /// (normalized time, x, y, z) rotation offset in degrees, sorted by time.
    pub keys: Vec<[f32; 4]>,
}

impl Correction {
    pub fn offset_at(&self, t: f32) -> Vec3 {
        let k = &self.keys;
        match k.len() {
            0 => Vec3::ZERO,
            1 => Vec3::new(k[0][1], k[0][2], k[0][3]),
            _ => {
                let i = k.iter().position(|q| q[0] > t).unwrap_or(k.len());
                let (a, b) = if i == 0 { (k[0], k[0]) } else if i == k.len() { (k[i - 1], k[i - 1]) } else { (k[i - 1], k[i]) };
                let w = if b[0] > a[0] { ((t - a[0]) / (b[0] - a[0])).clamp(0.0, 1.0) } else { 0.0 };
                Vec3::new(a[1], a[2], a[3]).lerp(Vec3::new(b[1], b[2], b[3]), w)
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct Corrections {
    pub list: Vec<Correction>,
}

impl Corrections {
    fn path() -> String {
        format!("{}/assets/corrections.json", crate::ROOT)
    }

    pub fn load() -> Self {
        let v = crate::read_json("assets/corrections.json");
        let list = v["corrections"].as_array().map(|a| a.iter().filter_map(|c| Some(Correction {
            clip: c["clip"].as_str()?.to_owned(),
            bone: c["bone"].as_str()?.to_owned(),
            keys: c["keys"].as_array()?.iter().filter_map(|k| Some([k[0].as_f64()? as f32, k[1].as_f64()? as f32, k[2].as_f64()? as f32, k[3].as_f64()? as f32])).collect(),
        })).collect()).unwrap_or_default();
        Self { list }
    }

    pub fn to_json(&self) -> Value {
        json!({"corrections": self.list.iter().map(|c| json!({"clip": c.clip, "bone": c.bone, "keys": c.keys})).collect::<Vec<_>>()})
    }

    pub(crate) fn save(&self) {
        let _ = std::fs::write(Self::path(), serde_json::to_string_pretty(&self.to_json()).unwrap_or_default());
    }

    /// Total offset (deg) for a bone in a clip at normalized time; clip-specific plus library-wide rules.
    pub fn offset(&self, clip: &str, bone: &str, t: f32) -> Vec3 {
        self.list.iter().filter(|c| c.bone == bone && (c.clip == clip || c.clip == "*")).map(|c| c.offset_at(t)).sum()
    }
}

/// HUD element offsets (px) and scale, editable in the Studio and saved to assets/ui/hud_layout.json.
#[derive(Resource, Clone, Copy, PartialEq)]
pub struct HudLayout {
    pub vitals: [f32; 2],
    pub skills: [f32; 2],
    pub items: [f32; 2],
    pub target: [f32; 2],
    pub scale: f32,
}

impl Default for HudLayout {
    fn default() -> Self {
        Self { vitals: [0.0, 0.0], skills: [0.0, 0.0], items: [0.0, 0.0], target: [0.0, 0.0], scale: 1.0 }
    }
}

impl HudLayout {
    fn path() -> String {
        format!("{}/assets/ui/hud_layout.json", crate::ROOT)
    }

    pub fn load() -> Self {
        let v = crate::read_json("assets/ui/hud_layout.json");
        let pair = |k: &str| [v[k][0].as_f64().unwrap_or(0.0) as f32, v[k][1].as_f64().unwrap_or(0.0) as f32];
        Self { vitals: pair("vitals"), skills: pair("skills"), items: pair("items"), target: pair("target"), scale: v["scale"].as_f64().unwrap_or(1.0) as f32 }
    }

    fn save(&self) {
        let _ = std::fs::create_dir_all(format!("{}/assets/ui", crate::ROOT));
        let _ = std::fs::write(Self::path(), serde_json::to_string_pretty(&json!({"vitals": self.vitals, "skills": self.skills, "items": self.items, "target": self.target, "scale": self.scale})).unwrap_or_default());
    }
}

/// The stage model: its bones, player and whether the authority rules run on it.
#[derive(Component)]
pub struct StudioRig {
    pub bones: Vec<Entity>,
    pub player: Option<Entity>,
    pub rules: bool,
}

#[derive(Resource)]
pub struct Studio {
    pub open: bool,
    pub viewport: Option<egui::Rect>,
    pub show_skeleton: bool,
    pub reset_view: bool,
    pub preview_weapon: Option<guildbound_sim::rpg::Weapon>,
    preview_request: bool,
    was_paused: bool,
    assets: Vec<crate::catalog::AssetInfo>,
    /// Model browser tab: 0 game models (play the whole library), 1 animated assets (their own clips), 2 props.
    browse: usize,
    /// The file whose clips play when not using the hero library (UAL1, UAL2, a creature pack ...).
    pub pack: Option<String>,
    filter: String,
    clip_filter: String,
    pub model: Option<String>,
    pub(crate) entity: Option<Entity>,
    pub clip: Option<String>,
    pub time: f32,
    pub duration: f32,
    pub playing: bool,
    pub speed: f32,
    /// Play the hero library on the stage model instead of its own clips (needs the hero bone names).
    pub hero_clips: bool,
    pub rules: bool,
    pub yaw: f32,
    pub scale: f32,
    pub selected: Option<String>,
    pub note: String,
    pub category: usize,
    pub offset: Vec3,
    pub tab: usize,
    /// The clip pack in use: its file, glTF, graph and clip index.
    own: Option<(String, Handle<Gltf>, Option<Handle<AnimationGraph>>, BTreeMap<String, (AnimationNodeIndex, f32)>)>,
    graph_attached: Option<AssetId<AnimationGraph>>,
    pub numbers: Value,
    pub status: String,
    spawn_request: Option<String>,
    /// Remote open/close (gb.studio), handled like F2.
    pub toggle_request: bool,
    /// Opened from the main menu as its own scene: closing returns to the menu.
    pub scene: bool,
    /// Last pose audit (collab/pose_audit.json), read when the Studio opens.
    audit: Value,
    /// Moveset picks: move -> (clip, enabled), and the move a clip is being added to.
    /// Name typed for a new clip variant in the clip editor.
    variant_name: String,
}

impl Studio {
    pub fn preview_clip(&mut self, clip: String) {
        if clip.contains("sword") { self.preview_weapon = Some(guildbound_sim::rpg::Weapon::Sword); }
        self.clip = Some(clip);
        self.hero_clips = true;
        self.time = 0.0;
        self.playing = true;
        self.preview_request = true;
    }
    /// Remote tab switch (gb.studio `tab`): any index of TABS, including the Pose tab (POSE_TAB = 9).
    /// Returns false for an out-of-range index so the handler can report it instead of ignoring it.
    pub fn set_tab(&mut self, i: usize) -> bool { let ok = i < TABS.len(); if ok { self.tab = i; } ok }
    pub fn request_spawn(&mut self, path: String) {
        self.spawn_request = Some(path);
    }

    pub fn play_test(&mut self) {
        self.was_paused = false;
        self.scene = false;
        self.toggle_request = true;
    }

    pub fn add_asset(&mut self, path: String) {
        let info = crate::catalog::read(std::path::Path::new(&format!("{}/assets/{path}", crate::ROOT)), path);
        self.assets.retain(|a| a.path != info.path);
        self.assets.push(info);
        self.assets.sort_by(|a, b| a.path.cmp(&b.path));
    }

    /// Every model without a skeleton (furniture, trees, buildings).
    pub fn props(&self) -> Vec<String> {
        self.assets.iter().filter(|a| !a.rigged).map(|a| a.path.clone()).collect()
    }

    pub fn assets_in(&self, prefix: &str) -> Vec<String> {
        self.assets.iter().filter(|a| a.path.starts_with(prefix)).map(|a| a.path.clone()).collect()
    }


}

impl Default for Studio {
    fn default() -> Self {
        Self {
            open: false, viewport: None, show_skeleton: true, reset_view: false, preview_weapon: None, preview_request: false, was_paused: false, assets: Vec::new(), filter: String::new(), clip_filter: String::new(), model: None, entity: None,
            clip: None, time: 0.0, duration: 1.0, playing: true, speed: 1.0, hero_clips: true, rules: true, yaw: 0.0, scale: 1.0,
            selected: None, note: String::new(), category: 0, offset: Vec3::ZERO, tab: 0, own: None, graph_attached: None,
            numbers: json!({}), status: "F2 closes the Studio. Pick a model on the left.".into(), spawn_request: None, toggle_request: false, scene: false, browse: 0, pack: None,
            audit: Value::Null,
            variant_name: String::new(),
        }
    }
}

/// Studio overlay gizmos, drawn on top of the model so the skeleton is always visible.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct StudioGizmos;

fn gizmo_on_top(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<StudioGizmos>();
    config.depth_bias = -1.0;
    config.line.width = 2.5;
}

pub fn plugin(app: &mut App) {
    app.add_plugins(crate::sandbox::plugin);
    app.init_gizmo_group::<StudioGizmos>()
        .add_systems(Startup, gizmo_on_top)
        .add_plugins((FrameTimeDiagnosticsPlugin::default(), EntityCountDiagnosticsPlugin::default()))
        .insert_resource(Corrections::load())
        .insert_resource(HudLayout::load())
        .init_resource::<Studio>()
        .add_systems(Startup, (scan, stage_set, open_at_startup))
        .add_systems(Update, (toggle, spawn_model, drive, pick, drag_bone, draw_skeleton).chain())
        .add_systems(bevy_egui::EguiPrimaryContextPass, ui);
}

fn scan(mut studio: ResMut<Studio>) {
    studio.assets = crate::catalog::scan();
}

fn open_at_startup(mut studio: ResMut<Studio>, mut sim: ResMut<Sim>) {
    studio.scene = true;
    studio.toggle_request = true;
    studio.request_spawn("packs/hero/mannequin.glb".into());
    sim.paused = true;
    sim.clear_player_input();
    sim.pending_steps = 0;
}

/// The stage's own floor (it carries the physical body) and light.
fn stage_set(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    use avian3d::prelude::*;
    commands.spawn((
        Name::new("studio floor"),
        Mesh3d(meshes.add(Circle::new(9.0))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.11, 0.14, 0.15), perceptual_roughness: 0.9, ..default() })),
        Transform::from_translation(STAGE).with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    commands.spawn((
        Name::new("studio floor collider"),
        RigidBody::Static,
        // 1 m thick box (top face at the stage): a fast foot that gets past the surface in one step is still
        // inside the box and pushed back up, instead of leaving through a thin slab
        Collider::cuboid(18.0, 1.0, 18.0),
        CollisionLayers::new(1u32, LayerMask::ALL),
        Transform::from_translation(STAGE - Vec3::Y * 0.5),
    ));
    commands.spawn((
        Name::new("studio key light"),
        PointLight { intensity: 900_000.0, range: 30.0, shadow_maps_enabled: true, ..default() },
        Transform::from_translation(STAGE + Vec3::new(2.5, 4.5, 3.0)),
    ));
    commands.spawn((
        Name::new("studio fill light"),
        PointLight { intensity: 250_000.0, range: 30.0, color: Color::srgb(0.7, 0.85, 1.0), ..default() },
        Transform::from_translation(STAGE + Vec3::new(-3.5, 2.5, -2.0)),
    ));
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut studio: ResMut<Studio>,
    mut sim: ResMut<Sim>,
    mut cam: Single<&mut crate::camera::OrbitCamera>,
    state: Res<State<crate::menu::AppState>>,
    mut next: ResMut<NextState<crate::menu::AppState>>,
) {
    // a request from the menu waits for the game state to reach Playing (it is sent in the same frame)
    if *state.get() != crate::menu::AppState::Playing {
        return;
    }
    let requested = std::mem::take(&mut studio.toggle_request);
    if !(keys.just_pressed(KeyCode::F2) || requested) {
        return;
    }
    studio.open = !studio.open;
    if !studio.open && studio.scene {
        // the Studio scene from the menu closes back to the menu
        studio.scene = false;
        next.set(crate::menu::AppState::MainMenu);
    }
    if studio.open {
        if studio.model.is_none() {
            studio.request_spawn("packs/hero/master.glb".into());
        }
        if studio.clip.is_none() {
            studio.clip = Some("nc_idle".into());
        }
        studio.audit = read_audit();
        studio.was_paused = sim.paused;
        sim.paused = true;
        sim.clear_player_input();
        cam.studio_focus = Some(STAGE + Vec3::Y * 1.0);
        cam.focus = STAGE + Vec3::Y * 1.0;
        cam.distance = 3.6;
    } else {
        sim.paused = studio.was_paused;
        cam.studio_focus = None;
        let player = &sim.world.fighters[0];
        cam.focus = Vec3::new(player.position.x, 1.45 + player.height + guildbound_sim::terrain::ground(player.position.x, player.position.z), player.position.z);
        cam.distance = 4.4;
    }
}

fn spawn_model(mut commands: Commands, mut studio: ResMut<Studio>, assets: Res<AssetServer>, mut possession: ResMut<crate::possession::Possession>) {
    let Some(path) = studio.spawn_request.take() else { return };
    possession.motion = None;
    possession.sequence.clear();
    possession.plant_anchors = [None; 2];
    possession.overrides.clear();
    possession.aims.clear();
    if let Some(e) = studio.entity.take() {
        commands.entity(e).despawn();
    }
    let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(path.clone()));
    let e = commands
        .spawn((WorldAssetRoot(scene), Transform::from_translation(STAGE).with_rotation(Quat::from_rotation_y(studio.yaw.to_radians())).with_scale(Vec3::splat(studio.scale)), StudioRig { bones: Vec::new(), player: None, rules: studio.rules }))
        .observe(on_ready)
        .id();
    studio.entity = Some(e);
    studio.graph_attached = None;
    studio.time = 0.0;
    // a game model plays the whole library (and keeps the chosen clip); any other model plays its own clips
    if is_game_model(&path) {
        if !studio.hero_clips {
            studio.clip = None;
        }
        studio.hero_clips = true;
    } else {
        studio.hero_clips = false;
        studio.pack = Some(path.clone());
        studio.clip = None;
    }
    studio.selected = None;
    studio.status = format!("Loaded {path}");
    studio.model = Some(path);
}

fn on_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    players: Query<(), With<AnimationPlayer>>,
    targets: Query<&Transform, With<AnimationTargetId>>,
    nodes: Query<(&Name, &Transform)>,
    mut rigs: Query<&mut StudioRig>,
    studio: Res<Studio>,
    mut possession: ResMut<crate::possession::Possession>,
) {
    let Ok(mut rig) = rigs.get_mut(ready.entity) else { return };
    rig.bones.clear();
    let mut rest = crate::procedural::RestPose::default();
    if !children.iter_descendants(ready.entity).any(|e| players.contains(e)) {
        let added = crate::anim::add_targets(&mut commands, ready.entity, &children, &nodes);
        rig.player = added.first().copied();
        for e in added {
            if let Ok((_, t)) = nodes.get(e) {
                rig.bones.push(e);
                rest.0.insert(e, t.rotation);
                rest.1.insert(e, t.translation);
            }
        }
    }
    for e in children.iter_descendants(ready.entity) {
        if players.contains(e) {
            rig.player = Some(e);
        }
        if let Ok(t) = targets.get(e) {
            rig.bones.push(e);
            rest.0.insert(e, t.rotation);
            rest.1.insert(e, t.translation);
        }
    }
    let profile = studio.model.as_deref().map(crate::rig_profile::RigProfile::load).unwrap_or_default();
    possession.mass_kg = profile.mass_kg;
    commands.entity(ready.entity).insert((rest, crate::procedural::Life::default(), profile));
}

/// Build the model's own clip graph once loaded; attach the chosen graph; advance and seek the timeline.
pub fn drive(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    assets: Res<AssetServer>,
    time: Res<Time>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    lib: Option<Res<AnimLibrary>>,
    mut rigs: Query<&mut StudioRig>,
    mut players: Query<&mut AnimationPlayer>,
    mut possession: ResMut<crate::possession::Possession>,
) {
    let studio = &mut *studio;
    if std::mem::take(&mut studio.preview_request) {
        possession.sequence.clear();
        possession.motion = None;
    }
    if !studio.hero_clips && let Some(p) = studio.pack.clone() && studio.own.as_ref().is_none_or(|o| o.0 != p) {
        studio.own = Some((p.clone(), assets.load(p), None, BTreeMap::new()));
        studio.graph_attached = None;
    }
    if let Some((_, h, graph, index)) = &mut studio.own
        && graph.is_none()
        && let Some(g) = gltfs.get(&*h)
    {
        let mut named: Vec<_> = g.named_animations.iter().map(|(n, c)| (n.to_string(), c.clone())).collect();
        named.sort_by(|a, b| a.0.cmp(&b.0));
        let (gr, nodes) = AnimationGraph::from_clips(named.iter().map(|(_, c)| c.clone()));
        for ((n, c), node) in named.iter().zip(nodes) {
            index.insert(n.clone(), (node, clips.get(c).map_or(1.0, |c| c.duration())));
        }
        *graph = Some(graphs.add(gr));
    }
    let Some(e) = studio.entity else { return };
    let Ok(mut rig) = rigs.get_mut(e) else { return };
    rig.rules = studio.rules;
    let Some(pe) = rig.player else { return };
    let Ok(mut player) = players.get_mut(pe) else { return };
    if let Some(motion) = possession.motion.as_ref().filter(|_| possession.enabled) {
        studio.duration = motion.duration;
        let (playing, speed, duration) = (studio.playing && studio.open, studio.speed, studio.duration);
        possession.advance(&mut studio.time, playing, time.delta_secs(), speed, duration);
        player.stop_all();
        return;
    }
    let hero = studio.hero_clips;
    let own = if hero { None } else { studio.own.as_ref().and_then(|(_, _, g, idx)| g.clone().map(|g| (g, idx.clone()))) };
    let graph = if hero { lib.as_ref().and_then(|l| l.graph.clone()) } else { own.as_ref().map(|o| o.0.clone()) };
    let Some(graph) = graph else { return };
    let find = |c: &str| if hero { lib.as_ref().and_then(|l| l.node(c).map(|n| (n, l.duration(c)))) } else { own.as_ref().and_then(|o| o.1.get(c).copied()) };
    if studio.graph_attached != Some(graph.id()) {
        commands.entity(pe).insert(AnimationGraphHandle(graph.clone()));
        studio.graph_attached = Some(graph.id());
        player.stop_all();
        return;
    }
    if !possession.sequence.is_empty() {
        // blend sequence: each clip plays through, crossfading into the next over the fade time; loops
        let clips: Vec<(String, AnimationNodeIndex, f32)> = possession.sequence.iter().filter_map(|c| find(c).map(|(n, d)| (c.clone(), n, d.max(0.05)))).collect();
        if clips.is_empty() {
            return;
        }
        // a crossfade longer than half a clip would overlap its windows (weights jump, or a clip drops out)
        let shortest = clips.iter().map(|c| c.2).fold(f32::INFINITY, f32::min);
        let fade = possession.seq_fade.min(shortest * 0.45);
        let total: f32 = clips.iter().map(|c| c.2 - fade).sum::<f32>().max(0.1);
        let advance = if studio.playing { time.delta_secs() * studio.speed } else { 0.0 };
        possession.seq_time = (possession.seq_time + advance).rem_euclid(total);
        let mut start = 0.0;
        let mut entries = Vec::new();
        for (i, (_, _, d)) in clips.iter().enumerate() {
            let local = possession.seq_time - start;
            if (0.0..d - fade).contains(&local) {
                // this clip, plus the next fading in over its last `fade` seconds; a clip has already played
                // `fade` seconds while fading in, so its own window continues from there (no pop at the loop)
                let into = ((local - (d - 2.0 * fade)) / fade.max(1e-3)).clamp(0.0, 1.0);
                entries.push((i, local + fade, 1.0 - into));
                if into > 0.0 {
                    entries.push(((i + 1) % clips.len(), local - (d - 2.0 * fade), into));
                }
            }
            start += d - fade;
        }
        // one blend per animation node: a clip named twice in the sequence is one node
        let mut active: Vec<(AnimationNodeIndex, usize, f32, f32)> = Vec::new();
        for (i, t, w) in entries {
            match active.iter_mut().find(|a| a.0 == clips[i].1) {
                Some(a) => {
                    if w > a.3 {
                        (a.1, a.2) = (i, t);
                    }
                    a.3 += w;
                }
                None => active.push((clips[i].1, i, t, w)),
            }
        }
        let stale: Vec<AnimationNodeIndex> = player.playing_animations().map(|(n, _)| *n).filter(|n| !active.iter().any(|a| a.0 == *n)).collect();
        for n in stale {
            player.stop(n);
        }
        for &(node, i, t, w) in &active {
            if !player.is_playing_animation(node) {
                player.play(node);
            }
            if let Some(a) = player.animation_mut(node) {
                a.set_speed(0.0).set_seek_time(t.clamp(0.0, clips[i].2)).set_weight(w.min(1.0));
            }
        }
        possession.seq_now = active.iter().map(|&(_, i, _, w)| (clips[i].0.clone(), w.min(1.0))).collect();
        // studio.clip always names a real clip (the strongest one now), so ending the sequence any way leaves a
        // playable selection; corrections and stage support stay off while a sequence plays
        if let Some(&(_, i, _, _)) = active.iter().max_by(|a, b| a.3.total_cmp(&b.3)) {
            studio.clip = Some(clips[i].0.clone());
        }
        // the physics readout follows sequence time (velocity across the blends, not per clip)
        studio.time = possession.seq_time;
        studio.duration = total;
        return;
    }
    let Some((node, dur)) = studio.clip.as_deref().and_then(|c| find(c)) else {
        player.stop_all();
        return;
    };
    studio.duration = dur.max(0.01);
    let (playing, speed, duration) = (studio.playing && studio.open, studio.speed, studio.duration);
    possession.advance(&mut studio.time, playing, time.delta_secs(), speed, duration);
    if !player.is_playing_animation(node) {
        player.stop_all();
        player.play(node);
    }
    if let Some(a) = player.animation_mut(node) {
        let t = studio.time.min(studio.duration);
        // hero clips play through the library's clip edits (trim and speed), variants included
        let seek = match (studio.hero_clips, lib.as_ref(), studio.clip.as_deref()) {
            (true, Some(l), Some(c)) => l.seek(c, t),
            _ => t,
        };
        a.set_speed(0.0).set_seek_time(seek);
    }
}

/// Click a bone on the stage model: the nearest bone to the cursor ray within 25 cm.
fn pick(
    mut studio: ResMut<Studio>,
    mouse: Res<ButtonInput<MouseButton>>,
    wants: Res<bevy_egui::input::EguiWantsInput>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    cam: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    rigs: Query<&StudioRig>,
    bones: Query<(&Name, &GlobalTransform)>,
) {
    if !studio.open || !mouse.just_pressed(MouseButton::Left) || wants.wants_any_pointer_input() {
        return;
    }
    let (Some(cursor), Some(e)) = (window.cursor_position(), studio.entity) else { return };
    let Ok(ray) = cam.0.viewport_to_world(cam.1, cursor) else { return };
    let Ok(rig) = rigs.get(e) else { return };
    let best = rig.bones.iter().filter_map(|&b| bones.get(b).ok()).map(|(n, g)| {
        let p = g.translation();
        let t = (p - ray.origin).dot(*ray.direction).max(0.0);
        (n.as_str().to_owned(), (ray.origin + *ray.direction * t).distance(p))
    }).min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((name, d)) = best.filter(|b| b.1 < 0.25) {
        studio.status = format!("Selected {name} ({:.0} mm from the click ray)", d * 1000.0);
        studio.selected = Some(name);
        studio.offset = Vec3::ZERO;
    }
}

/// Pose by hand: hold 1, 2 or 3 and drag with the middle mouse button to turn the selected bone about its X, Y
/// or Z axis (on top of the clip). With the anatomical lock on, a step that takes the joint out of range is
/// undone, so the bone stops at its limit.
fn drag_bone(
    studio: Res<Studio>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    wants: Res<bevy_egui::input::EguiWantsInput>,
    mut possession: ResMut<crate::possession::Possession>,
) {
    let Some(bone) = studio.selected.clone().filter(|_| studio.open) else { return };
    let axis = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3].iter().position(|k| keys.pressed(*k));
    let (Some(axis), true) = (axis, mouse.pressed(MouseButton::Middle) && !wants.wants_any_pointer_input()) else { return };
    let step = (motion.delta.x - motion.delta.y) * 0.4;
    if step == 0.0 {
        return;
    }
    // the lock reads the final pose (after support and the authority rules): when this bone's joint is pinned
    // at its anatomical limit, a step the same way as the one that pinned it is refused; back is always free
    let n = &studio.numbers;
    let joint_pinned = n["joints"].as_array().into_iter().flatten().filter(|j| j["joint"].as_str() == Some(bone.as_str())).any(|j| {
        let (b, lo, hi) = (j["bend_deg"].as_f64().unwrap_or(0.0), j["range"][0].as_f64().unwrap_or(0.0), j["range"][1].as_f64().unwrap_or(180.0));
        b >= hi - 1.0 || (lo > 0.0 && b <= lo + 1.0)
    });
    let side = bone.strip_prefix("B_").and_then(|s| s.strip_suffix("_Calf").or_else(|| s.strip_suffix("_Thigh")));
    let knee_pinned = side.is_some_and(|sd| n["legs"].as_array().into_iter().flatten().any(|l| {
        l["side"].as_str() == Some(sd) && (l["knee_flex_deg"].as_f64().unwrap_or(0.0) >= crate::knee::MAX_FLEX_DEG as f64 - 1.0 || l["knee_backward"] == true)
    }));
    if possession.anatomy_lock && (joint_pinned || knee_pinned)
        && possession.last_good.as_ref().is_some_and(|(b, e)| *b == bone && e[axis].signum() == step.signum() && e[axis] != 0.0)
    {
        return;
    }
    // remember the direction of travel on this axis (the step that may pin the joint)
    possession.last_good = Some((bone.clone(), { let mut d = Vec3::ZERO; d[axis] = step.signum(); d }));
    let o = possession.overrides.entry(bone).or_insert(crate::possession::BoneOverride { euler: Vec3::ZERO, absolute: false });
    o.euler[axis] = (o.euler[axis] + step).clamp(-180.0, 180.0);
}

/// Skeleton overlay: teal bones, red where a joint is outside its range, gold for the selection.
fn draw_skeleton(studio: Res<Studio>, rigs: Query<&StudioRig>, bones: Query<(&Name, &GlobalTransform, Option<&ChildOf>)>, mut gizmos: Gizmos<StudioGizmos>) {
    if !studio.show_skeleton { return; }
    if !studio.open {
        return;
    }
    let Some(rig) = studio.entity.and_then(|e| rigs.get(e).ok()) else { return };
    let bad: Vec<&str> = studio.numbers["violations"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    // body bones only: the root-motion bone rides on the floor (a line from it to the hips matches no body part)
    // and leaf bones are export helpers; fingers are drawn dim so they can still be picked for Possession
    let root = |n: &str| matches!(n, "Motion" | "motion" | "root" | "Root_M");
    for &b in &rig.bones {
        let Ok((name, g, parent)) = bones.get(b) else { continue };
        if name.as_str().contains("_leaf") {
            continue;
        }
        if root(name.as_str()) {
            gizmos.circle(Isometry3d::new(g.translation() + Vec3::Y * 0.01, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 0.08, Color::srgba(0.95, 0.75, 0.3, 0.7));
            continue;
        }
        if let Some(p) = parent.map(|c| c.parent()).filter(|pe| rig.bones.contains(pe)).and_then(|pe| bones.get(pe).ok()).filter(|p| !root(p.0.as_str())) {
            let finger = ["index", "middle", "ring", "pinky", "thumb"].iter().any(|f| name.as_str().starts_with(f)) || name.as_str().contains("Finger");
            let col = if bad.contains(&name.as_str()) {
                Color::srgb(1.0, 0.3, 0.25)
            } else if finger {
                Color::srgba(0.35, 0.86, 0.78, 0.35)
            } else {
                Color::srgb(0.35, 0.86, 0.78)
            };
            gizmos.line(p.1.translation(), g.translation(), col);
        }
        if studio.selected.as_deref() == Some(name.as_str()) {
            gizmos.sphere(Isometry3d::from_translation(g.translation()), 0.05, Color::srgb(1.0, 0.8, 0.3));
        }
    }
    gizmos.circle(Isometry3d::new(STAGE + Vec3::Y * 0.01, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 1.2, Color::srgba(0.35, 0.86, 0.78, 0.6));
}

/// Live numbers for the stage model (runs after the authority rules, so it reports the final pose).
pub fn measure(world: &mut World) {
    let Some(e) = world.resource::<Studio>().entity else { return };
    let Some(bones) = world.get::<StudioRig>(e).map(|r| r.bones.clone()) else { return };
    let find = |n: &str| bones.iter().copied().find(|&b| world.get::<Name>(b).is_some_and(|x| x.as_str() == n));
    let pos = |n: &str| find(n).and_then(|b| world_transform(world, b)).map(|t| t.translation());
    let mut joints = Vec::new();
    let mut violations = Vec::new();
    let profile = world.get::<crate::rig_profile::RigProfile>(e).cloned().unwrap_or_default();
    for rule in &profile.rules {
        let (j, lo, hi) = (rule.joint.as_str(), rule.min, rule.max);
        let (Some(pa), Some(pj), Some(pb)) = (pos(&rule.from), pos(j), pos(&rule.to)) else { continue };
        let bend = (pj - pa).normalize_or_zero().angle_between((pb - pj).normalize_or_zero()).to_degrees();
        let ok = bend >= lo - 0.5 && bend <= hi + 0.5;
        if !ok {
            violations.push(j.to_owned());
        }
        joints.push(json!({"joint": j, "bend_deg": (bend * 10.0).round() / 10.0, "range": [lo, hi], "ok": ok}));
    }
    let mut legs = Vec::new();
    for side in ["L", "R"] {
        if let (Some(h), Some(k), Some(a)) = (pos(&format!("B_{side}_Thigh")), pos(&format!("B_{side}_Calf")), pos(&format!("B_{side}_Foot"))) {
            let flex = 180.0 - (h - k).angle_between(a - k).to_degrees();
            // the same test the knee rule uses: the knee faces the pelvis's forward (hip line x up), not the
            // stage's facing (a strafing or turning pelvis read as a backward knee)
            let facing = world.get::<Transform>(e).map_or(Vec3::Z, |t| t.rotation * Vec3::Z);
            let heading = match (pos("B_L_Thigh"), pos("B_R_Thigh")) {
                (Some(l), Some(r)) => Vec3::new(l.x - r.x, 0.0, l.z - r.z).normalize_or_zero().cross(Vec3::Y).normalize_or(facing),
                _ => facing,
            };
            let backward = crate::knee::fix_with_limit(h, k, a, [h.distance(k), k.distance(a)], heading, profile.knee_max).is_some_and(|f| f.2);
            if backward || flex > profile.knee_max + 0.5 {
                violations.push(format!("B_{side}_Calf"));
            }
            legs.push(json!({"side": side, "knee_flex_deg": (flex * 10.0).round() / 10.0, "knee_backward": backward, "ankle_height_m": (a.y * 1000.0).round() / 1000.0}));
        }
    }
    let selected = world.resource::<Studio>().selected.clone();
    let sel = selected.as_deref().and_then(find).and_then(|b| {
        let local = world.get::<Transform>(b)?.rotation.to_euler(EulerRot::XYZ);
        let p = world_transform(world, b)?.translation();
        Some(json!({"bone": selected, "world": [p.x, p.y, p.z], "local_euler_deg": [local.0.to_degrees(), local.1.to_degrees(), local.2.to_degrees()]}))
    });
    let pelvis = pos("B_Pelvis").map(|p| p.y);
    let mut studio = world.resource_mut::<Studio>();
    studio.numbers = json!({"joints": joints, "legs": legs, "violations": violations, "pelvis_height_m": pelvis, "selected": sel, "bone_count": bones.len()});
}

/// Apply the corrections ledger: fighters (by their current clip and phase) and the stage model.
pub fn apply_corrections(world: &mut World) {
    if world.resource::<Corrections>().list.is_empty() {
        return;
    }
    let mut jobs: Vec<(String, f32, Vec<Entity>)> = Vec::new();
    let mut q = world.query::<&crate::anim::FighterRig>();
    if let Some(lib) = world.get_resource::<AnimLibrary>() {
        for rig in q.iter(world) {
            if let Some((clip, phase)) = rig.sample_info(world, lib) {
                jobs.push((clip, phase, rig.bones.clone()));
            }
        }
    }
    let s = world.resource::<Studio>();
    let p = world.resource::<crate::possession::Possession>();
    if p.motion.is_none() && p.sequence.is_empty()
        && let (Some(e), Some(clip)) = (s.entity, s.clip.clone()) && let Some(r) = world.get::<StudioRig>(e) {
        jobs.push((clip, s.time / s.duration.max(0.01), r.bones.clone()));
    }
    world.resource_scope(|world, ledger: Mut<Corrections>| {
        for (clip, t, bones) in jobs {
            for b in bones {
                let Some(name) = world.get::<Name>(b).map(|n| n.as_str().to_owned()) else { continue };
                let o = ledger.offset(&clip, &name, t);
                if o != Vec3::ZERO && let Some(mut tr) = world.get_mut::<Transform>(b) {
                    tr.rotation = (tr.rotation * Quat::from_euler(EulerRot::XYZ, o.x.to_radians(), o.y.to_radians(), o.z.to_radians())).normalize();
                }
            }
        }
    });
}

/// Group corrections by bone across clips, and notes by (bone, category) across models.
pub fn patterns(corr: &Corrections, notes: &[Value]) -> Vec<Value> {
    let mut by_bone: BTreeMap<&str, Vec<&Correction>> = BTreeMap::new();
    for c in corr.list.iter().filter(|c| c.clip != "*") {
        by_bone.entry(&c.bone).or_default().push(c);
    }
    let mut out = Vec::new();
    for (bone, cs) in by_bone {
        let mean: Vec3 = cs.iter().map(|c| c.offset_at(0.5)).sum::<Vec3>() / cs.len() as f32;
        out.push(json!({"kind": "correction", "bone": bone, "clips": cs.iter().map(|c| c.clip.clone()).collect::<Vec<_>>(), "mean_offset_deg": [mean.x, mean.y, mean.z], "promotable": cs.len() >= 2}));
    }
    let mut by_issue: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for n in notes {
        let key = (n["bone"].as_str().unwrap_or("-").to_owned(), n["category"].as_str().unwrap_or("other").to_owned());
        by_issue.entry(key).or_default().push(format!("{} / {}", n["model"].as_str().unwrap_or("?"), n["clip"].as_str().unwrap_or("?")));
    }
    for ((bone, cat), seen) in by_issue {
        out.push(json!({"kind": "note", "bone": bone, "category": cat, "count": seen.len(), "where": seen}));
    }
    out
}

fn read_audit() -> Value {
    crate::read_json("collab/pose_audit.json")
}

/// The audit's pipeline-introduced patterns, compact (gb.studio and the Patterns tab).
fn audit_patterns(audit: &Value) -> Vec<Value> {
    audit["patterns"].as_array().map(|a| a.iter().take(40).cloned().collect()).unwrap_or_default()
}

fn read_notes() -> Vec<Value> {
    std::fs::read_to_string(format!("{}/collab/studio_notes.jsonl", crate::ROOT)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Remote/Claude view of the Studio (gb.studio).
pub fn snapshot(world: &World) -> Value {
    let s = world.resource::<Studio>();
    let corr = world.resource::<Corrections>();
    let notes = read_notes();
    let clip_list: Vec<String> = if s.hero_clips {
        world.get_resource::<AnimLibrary>().map(|l| l.clips.iter().map(|(n, _)| n.clone()).collect()).unwrap_or_default()
    } else {
        s.own.as_ref().map(|o| o.3.keys().cloned().collect()).unwrap_or_default()
    };
    json!({"open": s.open, "model": s.model, "clip": s.clip, "time": s.time, "duration": s.duration, "hero_clips": s.hero_clips, "rules": s.rules, "clip_list": clip_list,
           "selected": s.selected, "numbers": s.numbers, "corrections": corr.to_json()["corrections"], "patterns": patterns(corr, &notes),
           "notes": notes.iter().rev().take(20).collect::<Vec<_>>(), "assets": s.assets.len(), "audit_patterns": audit_patterns(&read_audit())})
}

/// The Creator tab's state, the player's worn model and the pose network.
#[derive(bevy::ecs::system::SystemParam)]
pub struct CreatorParams<'w> {
    creator: ResMut<'w, crate::creator::Creator>,
    wear: ResMut<'w, crate::creator::Wear>,
    brain: ResMut<'w, crate::brain::Brain>,
}

/// Everything the Tuning tab edits live: sim motion tuning, animation tuning, life layers, the physical body.
#[derive(bevy::ecs::system::SystemParam)]
pub struct TuningParams<'w> {
    sim: ResMut<'w, Sim>,
    anim: ResMut<'w, crate::anim::AnimSettings>,
    life: ResMut<'w, crate::procedural::LifeTuning>,
    physical: ResMut<'w, crate::possession::PhysicalBody>,
    grip: ResMut<'w, crate::weapons::WeaponGrip>,
}

/// What `ui` itself still draws from the kit: the tab bar, the Events page, trail options and the
/// Balance section. Strip, view bar and bone tree are their own systems in studio_kit.rs.
#[derive(bevy::ecs::system::SystemParam)]
pub struct KitParams<'w> {
    kit: ResMut<'w, crate::studio_kit::Kit>,
    trails: ResMut<'w, crate::studio_trails::Trails>,
    shown: Res<'w, crate::sandbox::SandboxShown>,
    /// Engine tab readouts (moved here from `ui`'s own params to free the slot for this struct).
    diag: Res<'w, DiagnosticsStore>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn ui(
    mut contexts: EguiContexts,
    mut studio: ResMut<Studio>,
    mut corr: ResMut<Corrections>,
    mut layout: ResMut<HudLayout>,
    mut kp: KitParams,
    mut lib: Option<ResMut<AnimLibrary>>,
    mut feed: ResMut<crate::feed::DevFeed>,
    assets: Res<AssetServer>,
    bodies: Query<(), With<avian3d::prelude::RigidBody>>,
    mut possession: ResMut<crate::possession::Possession>,
    names: Query<&Name>,
    rigs: Query<&StudioRig>,
    mut tune: TuningParams,
    mut make: CreatorParams,
    mut mapping: ResMut<crate::moveset::Moveset>,
    mut pose: crate::body_controller::PoseParams,
) -> Result {
    if !studio.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    theme::style(&ctx);
    let studio = &mut *studio;
    // studio_kit's status strip, view bar and bone rail ran first (ctx-level panels), so lay out in what is left.
    // TODO(verify): egui 0.40 name for the space left by ctx panels (`available_rect`).
    let mut root = egui::Ui::new(ctx.clone(), "studio_root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.available_rect()));

    // toolbar
    egui::Panel::top("studio_bar").frame(theme::dock()).show_inside(&mut root, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Guildbound").size(15.0).strong().color(theme::PAPER));
            ui.label(egui::RichText::new("Studio").size(15.0).color(theme::MUTED));
            ui.add_space(10.0);
            crate::studio_kit::tab_bar(ui, &mut kp.kit, studio, &TABS);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("F2 {}", if studio.scene { "back to menu" } else { "close" })).color(theme::MUTED));
                if studio.scene && theme::button(ui, "Back to menu", false).clicked() {
                    studio.toggle_request = true;
                }
            });
        });
    });

    // what the open tab is for, in plain words
    egui::Panel::top("studio_tab_help").frame(theme::dock().inner_margin(egui::Margin::symmetric(12, 4))).show_inside(&mut root, |ui| {
        theme::hint(ui, TAB_HELP[studio.tab.min(TAB_HELP.len() - 1)]);
    });

    // bottom status strip and the slim view bar (pause/step, ragdoll, balance, undo/redo, trails) are added
    // before this system by studio_kit::status_strip_ui and studio_kit::toolbar_ui, so both sit outside
    // the central content.

    // asset browser: models (characters, props, everything) and every animation by where it came from
    let mut picked_clip = false;
    egui::Panel::left("studio_assets").default_size(320.0).resizable(true).frame(theme::dock()).show_inside(&mut root, |ui| {
        // rig + bone tree: own rail, studio_kit::bone_tree_ui
        theme::section(ui, "Models");
        theme::tabs(ui, &["Game models", "Assets", "Props"], &mut studio.browse);
        theme::hint(ui, [
            "Models the game uses. Every animation in the library plays on them.",
            "Animated models from the packs. Each plays the animations it came with.",
            "Objects that do not move: furniture, trees, buildings.",
        ][studio.browse.min(2)]);
        let f = studio.filter.to_lowercase();
        let shown: Vec<String> = studio.assets.iter()
            .filter(|a| match studio.browse {
                0 => is_game_model(&a.path),
                1 => a.rigged && !a.clips.is_empty() && !is_game_model(&a.path),
                _ => !a.rigged,
            } && (f.is_empty() || a.path.to_lowercase().contains(&f)))
            .map(|a| a.path.clone()).collect();
        ui.horizontal(|ui| {
            let at = studio.model.as_ref().and_then(|m| shown.iter().position(|a| a == m));
            let step = |k: isize| (!shown.is_empty()).then(|| shown[at.map_or(0, |i| (i as isize + k).rem_euclid(shown.len() as isize)) as usize].clone());
            if theme::button(ui, "< Prev", false).on_hover_text("Show the previous model in this list").clicked() && let Some(p) = step(-1) {
                studio.spawn_request = Some(p);
            }
            if theme::button(ui, "Next >", false).on_hover_text("Show the next model in this list").clicked() && let Some(p) = step(1) {
                studio.spawn_request = Some(p);
            }
            ui.label(egui::RichText::new(format!("{} models", shown.len())).size(11.0).color(theme::MUTED));
        });
        ui.add(egui::TextEdit::singleline(&mut studio.filter).hint_text("search by name"));
        egui::ScrollArea::vertical().id_salt("assets").max_height(ui.available_height() * 0.4).show(ui, |ui| {
            let mut groups: BTreeMap<(u8, String), Vec<&String>> = BTreeMap::new();
            for a in &shown {
                groups.entry(crate::catalog::pack_of(a)).or_default().push(a);
            }
            let mut pick = None;
            for ((_, title), list) in &groups {
                let here = studio.model.as_ref().is_some_and(|m| list.iter().any(|a| *a == m));
                egui::CollapsingHeader::new(egui::RichText::new(format!("{title}  ({})", list.len())).size(12.5).color(theme::PAPER))
                    .id_salt(title)
                    .default_open(here || groups.len() <= 4)
                    .show(ui, |ui| {
                        for a in list.iter().take(300) {
                            let file = a.rsplit('/').next().unwrap_or(a);
                            let r = ui.selectable_label(studio.model.as_deref() == Some(a.as_str()), egui::RichText::new(file.trim_end_matches(".glb").trim_end_matches(".gltf")).size(12.0));
                            if r.on_hover_text(a.as_str()).clicked() {
                                pick = Some((*a).clone());
                            }
                        }
                    });
            }
            if let Some(p) = pick {
                studio.spawn_request = Some(p);
            }
        });

        // the animations this model plays: the library for a game model, its own for an asset
        let model = studio.model.clone().unwrap_or_default();
        theme::section(ui, "Animations");
        ui.add(egui::TextEdit::singleline(&mut studio.clip_filter).hint_text("search: walk, sword, spell ..."));
        let f = studio.clip_filter.to_lowercase();
        let wanted = |n: &str| f.is_empty() || n.to_lowercase().contains(&f);
        let mut chosen: Option<(Option<String>, String)> = None;
        egui::ScrollArea::vertical().id_salt("clips").show(ui, |ui| {
            if model.is_empty() {
                theme::hint(ui, "Pick a model above.");
            } else if is_game_model(&model) {
                let mut hero: BTreeMap<&str, Vec<(String, f32)>> = BTreeMap::new();
                if let Some(l) = lib.as_ref() {
                    for n in l.names().into_iter().filter(|n| wanted(n)) {
                        let group = if l.base(&n) != n { "Your variants" } else { crate::catalog::hero_group(&n) };
                        let d = l.duration(&n);
                        hero.entry(group).or_default().push((n, d));
                    }
                }
                for (group, clips) in &hero {
                    egui::CollapsingHeader::new(egui::RichText::new(format!("{group}  ({})", clips.len())).size(12.5)).id_salt(("hero", group, f.clone())).default_open(!f.is_empty()).show(ui, |ui| {
                        for (n, d) in clips {
                            let on = studio.hero_clips && studio.clip.as_deref() == Some(n.as_str());
                            if ui.selectable_label(on, egui::RichText::new(format!("{n}   {d:.1}s")).size(11.5)).clicked() {
                                chosen = Some((None, n.clone()));
                            }
                        }
                    });
                }
            } else {
                let own: Vec<String> = studio.assets.iter().find(|a| a.path == model).map(|a| a.clips.clone()).unwrap_or_default();
                if own.is_empty() {
                    theme::hint(ui, "This model has no animations.");
                }
                for n in own.iter().filter(|n| wanted(n)) {
                    let on = studio.clip.as_deref() == Some(n.as_str());
                    if ui.selectable_label(on, egui::RichText::new(n.as_str()).size(11.5)).clicked() {
                        chosen = Some((Some(model.clone()), n.clone()));
                    }
                }
            }
        });
        if let Some((pack, clip)) = chosen {
            if clip.contains("sword") { studio.preview_weapon = Some(guildbound_sim::rpg::Weapon::Sword); }
            studio.hero_clips = pack.is_none();
            if pack.is_some() {
                studio.pack = pack;
            }
            studio.clip = Some(clip);
            studio.time = 0.0;
            studio.playing = true;
            picked_clip = true;
        }
    });

    // picking a clip ends a blend sequence
    if picked_clip {
        possession.sequence.clear();
    }

    // inspector / tabs
    egui::Panel::right("studio_inspect").default_size(360.0).resizable(true).frame(theme::dock()).show_inside(&mut root, |ui| {
        if kp.kit.tab == crate::studio_kit::Tab::Events && crate::studio_kit::legacy_index(kp.kit.tab).is_none() {
            // TODO(event_log): pass Some(&mut || sandbox log self_check()) once exposed by sandbox.rs.
            egui::ScrollArea::vertical().id_salt("kit_events_scroll").show(ui, |ui| crate::studio_kit::events_tab(ui, &mut kp.kit, None));
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| match TABS[studio.tab.min(TABS.len() - 1)] {
            "Creator" => crate::creator::tab(ui, &mut make.creator, studio, &mut make.wear, &mut feed),
            "Pose" => {
                crate::body_controller::pose_tab(ui, &mut pose, studio, &tune.sim, &tune.physical);
                crate::studio_trails::options(ui, &mut kp.trails);
            }
            "Inspect" => {
                let playable = studio.entity.and_then(|e| rigs.get(e).ok()).is_some_and(|r| r.bones.iter().any(|&e| names.get(e).is_ok_and(|n| n.as_str() == "B_Pelvis")));
                crate::weapons::ui(ui, studio, &mut tune.grip);
                inspect(ui, studio, &mut corr, &mut feed, tune.sim.world.tick, &mut make.wear, playable);
            }
            "Tuning" => {
                let bone_names: Vec<String> = studio.entity.and_then(|e| rigs.get(e).ok()).map(|r| r.bones.iter().filter_map(|&b| names.get(b).ok().map(|n| n.as_str().to_owned())).collect()).unwrap_or_default();
                egui::CollapsingHeader::new(egui::RichText::new("Possession: bones, fixed fps, life layers, physics readout").color(theme::TEAL)).default_open(true).show(ui, |ui| {
                    crate::possession::tab(ui, &mut possession, studio, &mut corr, &bone_names);
                    crate::brain::ui(ui, &mut make.brain);
                    if let Some(sel) = studio.numbers["selected"].as_object() {
                        theme::section(ui, "Selected bone (live)");
                        let v = |k: &str| sel.get(k).and_then(|a| a.as_array()).map(|a| a.iter().map(|x| format!("{:.2}", x.as_f64().unwrap_or(0.0))).collect::<Vec<_>>().join("  ")).unwrap_or_default();
                        theme::stat(ui, "world", v("world"), theme::PAPER);
                        theme::stat(ui, "local euler deg", v("local_euler_deg"), theme::PAPER);
                    }
                });
                egui::CollapsingHeader::new(egui::RichText::new("Physics: balance (sandbox)").color(theme::TEAL)).default_open(true).show(ui, |ui| {
                    crate::sandbox::physics_section(ui, &kp.shown);
                });
                crate::studio_trails::options(ui, &mut kp.trails);
                egui::CollapsingHeader::new(egui::RichText::new("Physical body: motors track the clip, it must stand on its own").color(theme::TEAL)).show(ui, |ui| {
                ui.checkbox(&mut tune.physical.on, "Physics body").on_hover_text("Turn on the physical body: a ragdoll with muscles that follow the animation.");
                ui.add(egui::Slider::new(&mut tune.physical.assist, 0.0..=1.0).text("helping hand")).on_hover_text("How much invisible help keeps the body upright. 0 = it has to balance by itself.");
                ui.add(egui::Slider::new(&mut tune.physical.motor, 0.1..=3.0).text("muscle strength")).on_hover_text("How strongly the muscles pull the limbs toward the animation.");
                ui.horizontal(|ui| {
                    ui.label("physical:");
                    ui.checkbox(&mut tune.physical.physical_regions[0], "legs");
                    ui.checkbox(&mut tune.physical.physical_regions[1], "body");
                    ui.checkbox(&mut tune.physical.physical_regions[2], "arms");
                });
                for (n, e) in tune.physical.track_err.iter().filter(|(_, e)| *e > 0.5) {
                    theme::stat(ui, &format!("{n} lags its key"), format!("{e:.1} deg"), if *e > 15.0 { theme::BAD } else if *e > 6.0 { theme::WARN } else { theme::GOOD });
                }
                ui.horizontal(|ui| {
                    if theme::button(ui, "Play run > jump > punch", !possession.sequence.is_empty()).on_hover_text("Play three animations in a row with fades, to check the transitions.").clicked() {
                        possession.sequence = vec!["nc_run_forward".into(), "nc_jump_begin".into(), "ual_punch_cross".into()];
                        possession.seq_time = 0.0;
                        studio.playing = true;
                    }
                    if theme::button(ui, "Stop sequence", false).on_hover_text("Stop the sequence and stay on the current animation.").clicked() {
                        // studio.clip already names the clip that was playing; stay on it
                        possession.sequence.clear();
                        studio.time = 0.0;
                    }
                });
                if !possession.sequence.is_empty() {
                    let now: Vec<String> = possession.seq_now.iter().map(|(c, w)| format!("{c} {:.0}%", w * 100.0)).collect();
                    ui.label(egui::RichText::new(format!("playing: {}", now.join(" + "))).size(11.5).color(theme::TEAL));
                    ui.add(egui::Slider::new(&mut possession.seq_fade, 0.05..=0.6).text("fade between clips (s)")).on_hover_text("Seconds each clip in the sequence takes to fade into the next.");
                }
                if tune.physical.on {
                    theme::stat(ui, "State", if tune.physical.standing { format!("standing, pelvis {:.2} m", tune.physical.pelvis_h) } else { format!("fallen, pelvis {:.2} m", tune.physical.pelvis_h) }, if tune.physical.standing { theme::GOOD } else { theme::BAD });
                }
                    ui.add(egui::Slider::new(&mut tune.physical.weight, 0.0..=1.0).text("physics amount")).on_hover_text("0 = pure animation. 1 = the body is fully driven by physics and can stumble or fall.");
                    ui.add(egui::Slider::new(&mut tune.physical.damping, 0.2..=2.0).text("muscle smoothness")).on_hover_text("How smoothly the muscles move the limbs (higher = less wobble).");
                    ui.add(egui::Slider::new(&mut tune.physical.hip_gain, 0.0..=2.0).text("hip strength")).on_hover_text("How hard the hips push to stay upright.");
                    ui.add(egui::Slider::new(&mut tune.physical.balance_pos, 0.0..=2.0).text("balance: weight over feet")).on_hover_text("How strongly the body shifts to keep its weight above its feet.");
                    ui.add(egui::Slider::new(&mut tune.physical.balance_vel, 0.0..=1.0).text("balance: catch momentum")).on_hover_text("How much the body reacts to its weight moving, to stop a fall early.");
                    ui.add(egui::Slider::new(&mut tune.physical.assist_kp, 50.0..=800.0).text("helping hand strength")).on_hover_text("How hard the helping hand pulls the body back to the pose.");
                    ui.add(egui::Slider::new(&mut tune.physical.follow, 0.0..=1.0).text("follow the animation")).on_hover_text("Pull every body part toward where the animation puts it. 0 = only the joints follow it.");
                    ui.checkbox(&mut tune.physical.contact_loss, "Go limp on impact").on_hover_text("Limbs lose strength while they touch something and get it back after, like being knocked.");
                    if tune.physical.on {
                        theme::stat(ui, "Strength", format!("{:.0}%", tune.physical.strength * 100.0), if tune.physical.strength > 0.8 { theme::GOOD } else { theme::WARN });
                    }
                    ui.add(egui::Slider::new(&mut tune.physical.assist_kd, 5.0..=80.0).text("helping hand softness")).on_hover_text("How gently the helping hand corrects (higher = less bounce).");
                });
                egui::CollapsingHeader::new(egui::RichText::new("Motion tuning (hero and NPC walkers)").color(theme::TEAL)).show(ui, |ui| {
                    ui.label(egui::RichText::new("The drag motor, speeds and turn rate shape how every character moves; the clip speeds keep feet on the ground.").size(11.0).color(theme::MUTED));
                    crate::panel::tuning_section(ui, &mut tune.sim);
                });
                egui::CollapsingHeader::new(egui::RichText::new("Animation tuning").color(theme::TEAL)).show(ui, |ui| {
                    crate::panel::anim_tuning(ui, &mut tune.anim);
                });
                egui::CollapsingHeader::new(egui::RichText::new("Life layers (look, hands, breath, lean)").color(theme::TEAL)).show(ui, |ui| {
                    let l = &mut *tune.life;
                    ui.add(egui::Slider::new(&mut l.look_speed, 2.0..=30.0).text("head turn speed")).on_hover_text("How fast the head turns to look at things.");
                    ui.add(egui::Slider::new(&mut l.look_scale, 0.0..=1.5).text("how far the head turns")).on_hover_text("How far the head may turn to look at things (1 = natural range).");
                    ui.add(egui::Slider::new(&mut l.grip_ease, 1.0..=40.0).text("hand open/close speed")).on_hover_text("How quickly hands open and close.");
                    ui.add(egui::Slider::new(&mut l.curl_scale, 0.0..=1.3).text("fist tightness")).on_hover_text("How tightly the fingers close when making a fist.");
                    ui.add(egui::Slider::new(&mut l.thumb_scale, 0.0..=1.5).text("thumb tightness")).on_hover_text("How far the thumb folds in a fist.");
                    ui.add(egui::Slider::new(&mut l.breath_chest_deg, 0.0..=6.0).text("breathing: chest")).on_hover_text("How far the chest rises with each breath (degrees).");
                    ui.add(egui::Slider::new(&mut l.breath_shoulder_deg, 0.0..=8.0).text("breathing: shoulders")).on_hover_text("How far the shoulders lift with each breath (degrees).");
                    ui.add(egui::Slider::new(&mut l.breath_rest, 0.2..=3.0).text("breathing speed at rest")).on_hover_text("How fast the character breathes standing still.");
                    ui.add(egui::Slider::new(&mut l.breath_max, 1.0..=8.0).text("breathing speed when tired")).on_hover_text("How fast the character breathes after running or fighting.");
                    ui.add(egui::Slider::new(&mut l.lean_speed_deg, 0.0..=3.0).text("lean with speed")).on_hover_text("How far the body leans forward for each metre per second of speed.");
                    ui.add(egui::Slider::new(&mut l.lean_accel_deg, 0.0..=3.0).text("lean when speeding up")).on_hover_text("Extra lean while speeding up or slowing down.");
                    if ui.button("Life defaults").clicked() {
                        *l = crate::procedural::LifeTuning::default();
                    }
                });
                egui::CollapsingHeader::new(egui::RichText::new("Clip editor: speed, trim and blend of any clip; save variants").color(theme::TEAL)).default_open(true).show(ui, |ui| {
                    clip_editor(ui, studio, lib.as_deref_mut());
                });
            }
            "Moveset" => crate::moveset::ui(ui, &mut mapping, studio, lib.as_deref()),
            "Corrections" => corrections_tab(ui, &mut corr),
            "Patterns" => {
                theme::section(ui, "Patterns across clips and models");
                ui.label(egui::RichText::new("Repeated fixes become library-wide rules; repeated notes show where the pipeline is wrong.").size(11.0).color(theme::MUTED));
                let notes = read_notes();
                for p in patterns(&corr, &notes) {
                    let bone = p["bone"].as_str().unwrap_or("-").to_owned();
                    if p["kind"] == "correction" {
                        let clips = p["clips"].as_array().map_or(0, |a| a.len());
                        let m = &p["mean_offset_deg"];
                        theme::stat(ui, &format!("{bone}: corrected in {clips} clips"), format!("{:.1} {:.1} {:.1}", m[0].as_f64().unwrap_or(0.0), m[1].as_f64().unwrap_or(0.0), m[2].as_f64().unwrap_or(0.0)), theme::WARN);
                        if p["promotable"] == true && theme::button(ui, "Promote to all clips", false).on_hover_text("Apply this fix to every animation, not just this one.").clicked() {
                            let o = [m[0].as_f64().unwrap_or(0.0) as f32, m[1].as_f64().unwrap_or(0.0) as f32, m[2].as_f64().unwrap_or(0.0) as f32];
                            corr.list.retain(|c| !(c.bone == bone && c.clip == "*"));
                            corr.list.push(Correction { clip: "*".into(), bone: bone.clone(), keys: vec![[0.0, o[0], o[1], o[2]]] });
                            corr.save();
                            studio.status = format!("Promoted the {bone} fix to every clip");
                        }
                    } else {
                        theme::stat(ui, &format!("{bone}: {}", p["category"].as_str().unwrap_or("")), format!("{} notes", p["count"]), theme::TEAL);
                    }
                }
                theme::section(ui, "Pose audit (Claude)");
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Introduced by our pipeline; inherited source issues are left out. Click to stage the worst case.").size(11.0).color(theme::MUTED));
                    if theme::button(ui, "Reload", false).on_hover_text("Read the latest audit from disk.").clicked() {
                        studio.audit = read_audit();
                    }
                });
                if let Some(models) = studio.audit["models"].as_object() {
                    for (m, clips) in models {
                        let n = clips.as_object().map_or(0, |c| c.len());
                        let bad = clips.as_object().map_or(0, |c| c.values().filter(|v| v["issues"].as_u64().unwrap_or(0) > 0).count());
                        theme::stat(ui, m, format!("{bad}/{n} clips flagged"), theme::MUTED);
                    }
                } else {
                    ui.label(egui::RichText::new("No audit yet: run Tools/blender/pose_audit.py").color(theme::WARN));
                }
                let mut stage = None;
                for p in audit_patterns(&studio.audit) {
                    let (check, bone) = (p["check"].as_str().unwrap_or(""), p["bone"].as_str().unwrap_or(""));
                    let text = format!("{} {check} {bone}: {} clips, worst {:.3} @ {}", p["model"].as_str().unwrap_or(""), p["clips"], p["worst"].as_f64().unwrap_or(0.0), p["at"].as_str().unwrap_or(""));
                    let color = if p["clips"].as_u64().unwrap_or(0) >= 3 { theme::BAD } else { theme::WARN };
                    if ui.add(egui::Label::new(egui::RichText::new(text).size(11.5).color(color)).sense(egui::Sense::click())).clicked() {
                        stage = Some(p.clone());
                    }
                }
                if let Some(p) = stage {
                    // "clip @t" -> stage that model, clip and normalized time, select the bone
                    let at = p["at"].as_str().unwrap_or("");
                    let (clip, t) = at.rsplit_once(" @").map_or((at, 0.0), |(c, t)| (c, t.parse::<f32>().unwrap_or(0.0)));
                    let model = p["model"].as_str().unwrap_or("");
                    let hero = matches!(model, "master.glb" | "fighter.glb");
                    let path = if hero { "packs/hero/fighter.glb".to_owned() } else { format!("packs/npc/{model}") };
                    if studio.model.as_deref() != Some(path.as_str()) {
                        studio.spawn_request = Some(path);
                    }
                    studio.hero_clips = hero;
                    let dur = if hero { lib.as_ref().map_or(1.0, |l| l.duration(clip)) } else { studio.own.as_ref().and_then(|o| o.3.get(clip)).map_or(1.0, |(_, d)| *d) };
                    studio.clip = Some(clip.to_owned());
                    studio.time = t * dur;
                    studio.playing = false;
                    studio.selected = p["bone"].as_str().and_then(|b| b.split("->").next()).filter(|b| b.starts_with("B_")).map(str::to_owned);
                    studio.status = format!("Audit: {} {} at {at}", p["check"].as_str().unwrap_or(""), p["bone"].as_str().unwrap_or(""));
                }
            }
            "Notes" => {
                theme::section(ui, "Notes sent to Claude");
                for n in read_notes().iter().rev().take(30) {
                    ui.label(egui::RichText::new(format!("{}  {} @ {:.2}s  {}", n["model"].as_str().unwrap_or(""), n["clip"].as_str().unwrap_or(""), n["time"].as_f64().unwrap_or(0.0), n["bone"].as_str().unwrap_or(""))).size(11.0).color(theme::MUTED));
                    ui.label(egui::RichText::new(n["text"].as_str().unwrap_or("")).color(theme::PAPER));
                    ui.separator();
                }
            }
            "HUD Layout" => {
                theme::section(ui, "HUD layout");
                let l = &mut *layout;
                for (label, v) in [("Health card", &mut l.vitals), ("Spells panel", &mut l.skills), ("Item slots", &mut l.items), ("Target card", &mut l.target)] {
                    ui.label(label);
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut v[0]).speed(1.0).prefix("x "));
                        ui.add(egui::DragValue::new(&mut v[1]).speed(1.0).prefix("y "));
                    });
                }
                ui.add(egui::Slider::new(&mut layout.scale, 0.6..=1.6).text("size")).on_hover_text("Size of the model.");
                ui.horizontal(|ui| {
                    if theme::button(ui, "Save layout", false).clicked() {
                        layout.save();
                        studio.status = "HUD layout saved to assets/ui/hud_layout.json".into();
                    }
                    if theme::button(ui, "Reset", false).clicked() {
                        *layout = HudLayout::default();
                    }
                });
                ui.label(egui::RichText::new("Close the Studio (F2) to see the HUD at its new place.").size(11.0).color(theme::MUTED));
            }
            _ => {
                theme::section(ui, "Engine");
                let d = |id: &bevy::diagnostic::DiagnosticPath| kp.diag.get(id).and_then(|d| d.smoothed());
                theme::stat(ui, "FPS", format!("{:.1}", d(&FrameTimeDiagnosticsPlugin::FPS).unwrap_or(0.0)), theme::GOOD);
                theme::stat(ui, "Frame time", format!("{:.2} ms", d(&FrameTimeDiagnosticsPlugin::FRAME_TIME).unwrap_or(0.0)), theme::PAPER);
                theme::stat(ui, "Entities", format!("{:.0}", d(&EntityCountDiagnosticsPlugin::ENTITY_COUNT).unwrap_or(0.0)), theme::PAPER);
                theme::stat(ui, "Physics bodies", format!("{}", bodies.iter().count()), theme::PAPER);
                theme::stat(ui, "Models in assets", format!("{}", studio.assets.len()), theme::PAPER);
                theme::stat(ui, "Hero clips", format!("{}", lib.as_ref().map_or(0, |l| l.clips.len())), theme::PAPER);
                let loading = studio.model.as_ref().map(|m| format!("{:?}", assets.get_load_state(&assets.load::<Gltf>(m.clone()))));
                theme::stat(ui, "Stage model load", loading.unwrap_or_else(|| "-".into()), theme::MUTED);
                theme::section(ui, "Simulation");
                theme::stat(ui, "Tick", format!("{}", tune.sim.world.tick), theme::PAPER);
                theme::stat(ui, "Fighters", format!("{}", tune.sim.world.fighters.len()), theme::PAPER);
                theme::stat(ui, "Projectiles", format!("{}", tune.sim.world.projectiles.len()), theme::PAPER);
                theme::stat(ui, "Obstacles", format!("{}", tune.sim.world.obstacles.len()), theme::PAPER);
                theme::stat(ui, "State hash", format!("{:016x}", tune.sim.world.state_hash()), theme::MUTED);
            }
        });
    });

    // timeline
    egui::Panel::bottom("studio_timeline").frame(theme::dock()).show_inside(&mut root, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().slider_width = 110.0;
            if theme::button(ui, if studio.playing { "Pause" } else { "Play" }, studio.playing).clicked() {
                studio.playing = !studio.playing;
            }
            ui.add(egui::Slider::new(&mut studio.speed, 0.05..=2.0).text("playback speed")).on_hover_text("1 = normal speed, 0.5 = half speed.");
            theme::checkbox(ui, &mut studio.rules, "Keep joints natural").on_hover_text("Stop joints bending past their natural limits and knees bending backwards.");
            theme::checkbox(ui, &mut studio.show_skeleton, "Skeleton");
            if theme::button(ui, "Reset view", false).clicked() { studio.reset_view = true; }
        });
        let dur = studio.duration;
        ui.spacing_mut().slider_width = (ui.available_width() - 160.0).max(80.0);
        if ui.add(egui::Slider::new(&mut studio.time, 0.0..=dur).text(format!("/ {dur:.2}s"))).dragged() {
            studio.playing = false;
        }
        ui.label(egui::RichText::new(format!("{}   |   {}   |   {}", studio.model.as_deref().unwrap_or("no model"), studio.clip.as_deref().unwrap_or("no clip"), studio.status)).size(11.0).color(theme::MUTED));
    });
    studio.viewport = Some(root.available_rect_before_wrap());
    Ok(())
}

fn inspect(ui: &mut egui::Ui, studio: &mut Studio, corr: &mut Corrections, feed: &mut crate::feed::DevFeed, tick: u64, wear: &mut crate::creator::Wear, playable: bool) {
    theme::section(ui, "Play test this model");
    theme::hint(ui, "Use the character on the stage as your player. Your animation mapping stays active.");
    ui.add_enabled_ui(playable, |ui| {
        if theme::button(ui, "Set as player model", false).clicked() {
            crate::creator::set_worn(wear, studio.model.clone());
            studio.status = "Player model saved".into();
        }
        if theme::button(ui, "Use model and play test", false).clicked() {
            crate::creator::set_worn(wear, studio.model.clone());
            studio.play_test();
        }
    });
    if !playable { theme::hint(ui, "Choose a game character with the shared skeleton to play test."); }
    theme::section(ui, "Stage");
    theme::hint(ui, "Turn and resize the model on the stage.");
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut studio.yaw, -180.0..=180.0).text("turn")).on_hover_text("Turn the model around (degrees).");
        ui.add(egui::Slider::new(&mut studio.scale, 0.2..=3.0).text("size")).on_hover_text("Size of the model.");
    });
    let n = studio.numbers.clone();
    theme::section(ui, "Joint ranges (final pose)");
    theme::hint(ui, "How far each joint is bent now, against its natural range. Red means outside it.");
    if let Some(js) = n["joints"].as_array() {
        for j in js {
            let ok = j["ok"] == true;
            theme::stat(ui, j["joint"].as_str().unwrap_or(""), format!("{:.1} deg  [{}, {}]", j["bend_deg"].as_f64().unwrap_or(0.0), j["range"][0], j["range"][1]), if ok { theme::PAPER } else { theme::BAD });
        }
    }
    if let Some(ls) = n["legs"].as_array() {
        for l in ls {
            let side = l["side"].as_str().unwrap_or("");
            let back = l["knee_backward"] == true;
            theme::stat(ui, &format!("{side} knee flexion"), format!("{:.1} deg{}", l["knee_flex_deg"].as_f64().unwrap_or(0.0), if back { "  BACKWARD" } else { "" }), if back { theme::BAD } else { theme::PAPER });
            theme::stat(ui, &format!("{side} ankle height"), format!("{:.3} m", l["ankle_height_m"].as_f64().unwrap_or(0.0)), theme::PAPER);
        }
    }
    if let Some(p) = n["pelvis_height_m"].as_f64() {
        theme::stat(ui, "Pelvis height", format!("{p:.3} m"), theme::PAPER);
    }
    theme::section(ui, "Selected bone");
    let Some(bone) = studio.selected.clone() else {
        ui.label(egui::RichText::new("Click a bone on the stage model.").color(theme::MUTED));
        return;
    };
    if let Some(s) = n["selected"].as_object() {
        theme::stat(ui, &bone, format!("{:.2} {:.2} {:.2}", s["world"][0].as_f64().unwrap_or(0.0), s["world"][1].as_f64().unwrap_or(0.0), s["world"][2].as_f64().unwrap_or(0.0)), theme::WARN);
        theme::stat(ui, "local rotation (deg)", format!("{:.1} {:.1} {:.1}", s["local_euler_deg"][0].as_f64().unwrap_or(0.0), s["local_euler_deg"][1].as_f64().unwrap_or(0.0), s["local_euler_deg"][2].as_f64().unwrap_or(0.0)), theme::PAPER);
    }
    theme::section(ui, "Correct this bone");
    ui.label(egui::RichText::new("Offset (deg) keyed at the current time of this clip; keys interpolate.").size(11.0).color(theme::MUTED));
    ui.add(egui::Slider::new(&mut studio.offset.x, -60.0..=60.0).text("x"));
    ui.add(egui::Slider::new(&mut studio.offset.y, -60.0..=60.0).text("y"));
    ui.add(egui::Slider::new(&mut studio.offset.z, -60.0..=60.0).text("z"));
    ui.horizontal(|ui| {
        let clip = studio.clip.clone().unwrap_or_default();
        if theme::button(ui, "Key offset", false).on_hover_text("Store this bone's turn at the current moment of the clip.").clicked() && !clip.is_empty() {
            let t = studio.time / studio.duration.max(0.01);
            let o = studio.offset;
            let entry = match corr.list.iter_mut().position(|c| c.clip == clip && c.bone == bone) {
                Some(i) => &mut corr.list[i],
                None => {
                    corr.list.push(Correction { clip: clip.clone(), bone: bone.clone(), keys: Vec::new() });
                    corr.list.last_mut().unwrap()
                }
            };
            entry.keys.retain(|k| (k[0] - t).abs() > 0.01);
            entry.keys.push([t, o.x, o.y, o.z]);
            entry.keys.sort_by(|a, b| a[0].total_cmp(&b[0]));
            corr.save();
            studio.status = format!("Keyed {bone} at {:.0}% of {clip}", t * 100.0);
        }
        if theme::button(ui, "Clear bone", false).on_hover_text("Remove this bone's fix.").clicked() {
            corr.list.retain(|c| !(c.clip == clip && c.bone == bone));
            corr.save();
        }
    });
    theme::section(ui, "Tell Claude what is wrong");
    egui::ComboBox::from_id_salt("note_cat").selected_text(CATEGORIES[studio.category]).show_ui(ui, |ui| {
        for (i, c) in CATEGORIES.iter().enumerate() {
            ui.selectable_value(&mut studio.category, i, *c);
        }
    });
    ui.add(egui::TextEdit::multiline(&mut studio.note).desired_rows(3).hint_text("e.g. left shoulder rolls inward at the punch extension"));
    if theme::button(ui, "Send note", false).on_hover_text("Send this note, with the pose numbers, to Claude.").clicked() && !studio.note.trim().is_empty() {
        let note = json!({"text": studio.note.trim(), "category": CATEGORIES[studio.category], "model": studio.model, "clip": studio.clip, "time": studio.time,
                          "duration": studio.duration, "bone": bone, "hero_clips": studio.hero_clips, "rules": studio.rules, "numbers": studio.numbers});
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(format!("{}/collab/studio_notes.jsonl", crate::ROOT)) {
            let _ = writeln!(f, "{note}");
        }
        feed.post("boss", &format!("@studio {} [{} | {} @ {:.2}s | {}] {}", CATEGORIES[studio.category], studio.model.as_deref().unwrap_or("-"), studio.clip.as_deref().unwrap_or("-"), studio.time, bone, studio.note.trim()), tick);
        studio.note.clear();
        studio.status = "Note sent with the full pose numbers".into();
    }
}

/// Edit the selected clip's speed, kept part and blend-in, or save it as a new named variant that the moveset
/// and the game can use like any clip. Edits apply live; Save writes assets/clip_edits.json.
fn clip_editor(ui: &mut egui::Ui, studio: &mut Studio, lib: Option<&mut AnimLibrary>) {
    let Some(lib) = lib else {
        ui.label("The hero library is still loading.");
        return;
    };
    let clip = studio.clip.clone().unwrap_or_default();
    if clip.is_empty() || lib.node(&clip).is_none() {
        ui.label(egui::RichText::new("Pick a hero clip on the left (Hero library ticked) to edit it.").color(theme::MUTED));
    } else {
        let mut e = lib.edit(&clip).clone();
        let before = e.clone();
        let base = lib.base(&clip).to_owned();
        ui.label(egui::RichText::new(if base == clip { clip.clone() } else { format!("{clip}  (variant of {base})") }).strong().color(theme::PAPER));
        ui.add(egui::Slider::new(&mut e.speed, 0.25..=3.0).text("playback speed")).on_hover_text("1 = normal speed, 0.5 = half speed.");
        ui.add(egui::Slider::new(&mut e.trim[0], 0.0..=0.9).text("cut start")).on_hover_text("Where the clip begins, as a fraction of its length (0 = the very start).");
        ui.add(egui::Slider::new(&mut e.trim[1], 0.1..=1.0).text("cut end")).on_hover_text("Where the clip stops, as a fraction of its length (1 = the very end).");
        e.trim[0] = e.trim[0].min(e.trim[1] - 0.05);
        let mut own_blend = e.blend_in.is_some();
        ui.horizontal(|ui| {
            ui.checkbox(&mut own_blend, "custom fade-in").on_hover_text("Give this clip its own fade-in time instead of the general one.");
            let mut b = e.blend_in.unwrap_or(0.12);
            if own_blend {
                ui.add(egui::Slider::new(&mut b, 0.0..=0.5).text("s"));
            }
            e.blend_in = own_blend.then_some(b);
        });
        if e != before {
            lib.edits.insert(clip.clone(), e.clone());
        }
        theme::stat(ui, "plays for", format!("{:.2} s", lib.duration(&clip)), theme::PAPER);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut studio.variant_name).hint_text("variant name").desired_width(160.0));
            let name = studio.variant_name.trim().to_owned();
            if theme::button(ui, "Save as variant", false).on_hover_text("Save a copy of this animation with your speed and cut points under a new name.").clicked() && !name.is_empty() && lib.node(&name).is_none() {
                lib.edits.insert(name.clone(), crate::anim::ClipEdit { base: Some(base.clone()), ..e.clone() });
                crate::anim::save_clip_edits(&lib.edits);
                studio.status = format!("Variant {name} saved; add it to a move in the Moveset tab");
                studio.clip = Some(name);
                studio.variant_name.clear();
            }
        });
        ui.horizontal(|ui| {
            if theme::button(ui, "Save edits", false).on_hover_text("Keep your speed and cut changes for this animation.").clicked() {
                crate::anim::save_clip_edits(&lib.edits);
                studio.status = "Clip edits saved to assets/clip_edits.json".into();
            }
            if theme::button(ui, "Reset this clip", false).on_hover_text("Undo your changes to this animation.").clicked() {
                if base == clip {
                    lib.edits.remove(&clip);
                } else {
                    lib.edits.insert(clip.clone(), crate::anim::ClipEdit { base: Some(base.clone()), ..default() });
                }
            }
        });
    }
    let variants: Vec<(String, String)> = lib.edits.iter().filter_map(|(n, e)| e.base.clone().map(|b| (n.clone(), b))).collect();
    if !variants.is_empty() {
        theme::section(ui, "Variants");
        theme::hint(ui, "A variant is a copy of an animation with your own speed and cut points.");
        let mut drop = None;
        for (n, b) in &variants {
            ui.horizontal(|ui| {
                if ui.selectable_label(studio.clip.as_deref() == Some(n.as_str()), format!("{n}  <- {b}")).clicked() {
                    studio.clip = Some(n.clone());
                    studio.time = 0.0;
                }
                if ui.small_button("delete").clicked() {
                    drop = Some(n.clone());
                }
            });
        }
        if let Some(n) = drop {
            lib.edits.remove(&n);
            crate::anim::save_clip_edits(&lib.edits);
        }
    }
}

fn corrections_tab(ui: &mut egui::Ui, corr: &mut Corrections) {
    theme::section(ui, "Corrections ledger");
    theme::hint(ui, "Saved fixes: small turns added to a bone during one animation (or all of them).");
    ui.label(egui::RichText::new("assets/corrections.json - applied live, under the joint and knee authority.").size(11.0).color(theme::MUTED));
    let mut drop = None;
    for (i, c) in corr.list.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{}  {}  ({} keys)", if c.clip == "*" { "ALL CLIPS" } else { &c.clip }, c.bone, c.keys.len())).size(11.5));
            if ui.small_button("remove").clicked() {
                drop = Some(i);
            }
        });
    }
    if let Some(i) = drop {
        corr.list.remove(i);
        corr.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correction_keys_interpolate_and_clamp() {
        let c = Correction { clip: "a".into(), bone: "b".into(), keys: vec![[0.2, 10.0, 0.0, 0.0], [0.6, 30.0, 0.0, 0.0]] };
        assert_eq!(c.offset_at(0.0).x, 10.0);
        assert!((c.offset_at(0.4).x - 20.0).abs() < 1e-4);
        assert_eq!(c.offset_at(0.9).x, 30.0);
        let ledger = Corrections { list: vec![c.clone(), Correction { clip: "*".into(), bone: "b".into(), keys: vec![[0.0, 1.0, 2.0, 3.0]] }] };
        assert!((ledger.offset("a", "b", 0.4) - Vec3::new(21.0, 2.0, 3.0)).length() < 1e-4, "clip fix plus the library rule");
        assert_eq!(ledger.offset("other", "b", 0.4), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn repeated_corrections_become_a_promotable_pattern() {
        let mk = |clip: &str| Correction { clip: clip.into(), bone: "B_L_Clavicle".into(), keys: vec![[0.0, 0.0, 8.0, 0.0]] };
        let p = patterns(&Corrections { list: vec![mk("run"), mk("walk")] }, &[json!({"bone": "B_L_Calf", "category": "knee / leg bend", "model": "m", "clip": "c"})]);
        assert_eq!(p[0]["promotable"], true);
        assert_eq!(p[0]["mean_offset_deg"][1], 8.0);
        assert_eq!(p[1]["count"], 1);
    }
}
