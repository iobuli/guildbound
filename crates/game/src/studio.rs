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
use std::collections::{BTreeMap, HashMap};
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
use crate::ui_theme::{self as theme, Kit};

/// Where models are staged: a cleared spot north-west of the courtyard.
pub const STAGE: Vec3 = Vec3::new(-30.0, 0.0, 30.0);
const CATEGORIES: [&str; 8] = ["foot sliding", "knee / leg bend", "arm / shoulder", "spine / head", "clipping", "timing", "scale / proportions", "other"];
const TABS: [&str; 6] = ["Inspect", "Corrections", "Patterns", "Notes", "HUD Layout", "Engine"];

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
        let v: Value = std::fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
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

    fn save(&self) {
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
    pub target: [f32; 2],
    pub scale: f32,
}

impl Default for HudLayout {
    fn default() -> Self {
        Self { vitals: [0.0, 0.0], skills: [0.0, 0.0], target: [0.0, 0.0], scale: 1.0 }
    }
}

impl HudLayout {
    fn path() -> String {
        format!("{}/assets/ui/hud_layout.json", crate::ROOT)
    }

    pub fn load() -> Self {
        let v: Value = std::fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!({}));
        let pair = |k: &str| [v[k][0].as_f64().unwrap_or(0.0) as f32, v[k][1].as_f64().unwrap_or(0.0) as f32];
        Self { vitals: pair("vitals"), skills: pair("skills"), target: pair("target"), scale: v["scale"].as_f64().unwrap_or(1.0) as f32 }
    }

    fn save(&self) {
        let _ = std::fs::create_dir_all(format!("{}/assets/ui", crate::ROOT));
        let _ = std::fs::write(Self::path(), serde_json::to_string_pretty(&json!({"vitals": self.vitals, "skills": self.skills, "target": self.target, "scale": self.scale})).unwrap_or_default());
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
    was_paused: bool,
    assets: Vec<String>,
    filter: String,
    clip_filter: String,
    pub model: Option<String>,
    entity: Option<Entity>,
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
    own: Option<(Handle<Gltf>, Option<Handle<AnimationGraph>>, BTreeMap<String, (AnimationNodeIndex, f32)>)>,
    graph_attached: Option<AssetId<AnimationGraph>>,
    pub numbers: Value,
    pub status: String,
    spawn_request: Option<String>,
    /// Remote open/close (gb.studio), handled like F2.
    pub toggle_request: bool,
    /// Last pose audit (collab/pose_audit.json), read when the Studio opens.
    audit: Value,
}

impl Studio {
    pub fn request_spawn(&mut self, path: String) {
        self.spawn_request = Some(path);
    }
}

impl Default for Studio {
    fn default() -> Self {
        Self {
            open: false, was_paused: false, assets: Vec::new(), filter: String::new(), clip_filter: String::new(), model: None, entity: None,
            clip: None, time: 0.0, duration: 1.0, playing: true, speed: 1.0, hero_clips: true, rules: true, yaw: 0.0, scale: 1.0,
            selected: None, note: String::new(), category: 0, offset: Vec3::ZERO, tab: 0, own: None, graph_attached: None,
            numbers: json!({}), status: "F2 closes the Studio. Pick a model on the left.".into(), spawn_request: None, toggle_request: false,
            audit: Value::Null,
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
    app.init_gizmo_group::<StudioGizmos>()
        .add_systems(Startup, gizmo_on_top)
        .add_plugins((FrameTimeDiagnosticsPlugin::default(), EntityCountDiagnosticsPlugin::default()))
        .insert_resource(Corrections::load())
        .insert_resource(HudLayout::load())
        .init_resource::<Studio>()
        .add_systems(Startup, scan)
        .add_systems(Update, (toggle, spawn_model, drive, pick, draw_skeleton).chain())
        .add_systems(bevy_egui::EguiPrimaryContextPass, ui);
}

fn scan(mut studio: ResMut<Studio>) {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else if p.extension().is_some_and(|x| x == "glb" || x == "gltf") {
                let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                if !rel.contains('#') && !rel.contains("/Exports/FBX") {
                    out.push(rel);
                }
            }
        }
    }
    let root = std::path::PathBuf::from(format!("{}/assets", crate::ROOT));
    let mut out = Vec::new();
    walk(&root.join("packs"), &root, &mut out);
    out.sort();
    studio.assets = out;
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut studio: ResMut<Studio>, mut sim: ResMut<Sim>, mut cam: Single<&mut crate::camera::OrbitCamera>, state: Res<State<crate::menu::AppState>>) {
    let requested = std::mem::take(&mut studio.toggle_request);
    if *state.get() != crate::menu::AppState::Playing || !(keys.just_pressed(KeyCode::F2) || requested) {
        return;
    }
    studio.open = !studio.open;
    if studio.open {
        studio.audit = read_audit();
        studio.was_paused = sim.paused;
        sim.paused = true;
        sim.clear_player_input();
        cam.studio_focus = Some(STAGE + Vec3::Y * 1.0);
        cam.distance = 3.6;
    } else {
        sim.paused = studio.was_paused;
        cam.studio_focus = None;
        cam.distance = 4.4;
    }
}

fn spawn_model(mut commands: Commands, mut studio: ResMut<Studio>, assets: Res<AssetServer>) {
    let Some(path) = studio.spawn_request.take() else { return };
    if let Some(e) = studio.entity.take() {
        commands.entity(e).despawn();
    }
    let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(path.clone()));
    let e = commands
        .spawn((WorldAssetRoot(scene), Transform::from_translation(STAGE).with_rotation(Quat::from_rotation_y(studio.yaw.to_radians())).with_scale(Vec3::splat(studio.scale)), StudioRig { bones: Vec::new(), player: None, rules: studio.rules }))
        .observe(on_ready)
        .id();
    studio.entity = Some(e);
    studio.own = Some((assets.load(path.clone()), None, BTreeMap::new()));
    studio.graph_attached = None;
    studio.clip = None;
    studio.time = 0.0;
    studio.selected = None;
    studio.status = format!("Loaded {path}");
    studio.model = Some(path);
}

fn on_ready(ready: On<WorldInstanceReady>, children: Query<&Children>, players: Query<(), With<AnimationPlayer>>, targets: Query<(), With<AnimationTargetId>>, mut rigs: Query<&mut StudioRig>) {
    let Ok(mut rig) = rigs.get_mut(ready.entity) else { return };
    rig.bones.clear();
    for e in children.iter_descendants(ready.entity) {
        if players.contains(e) {
            rig.player = Some(e);
        }
        if targets.contains(e) {
            rig.bones.push(e);
        }
    }
}

/// Build the model's own clip graph once loaded; attach the chosen graph; advance and seek the timeline.
fn drive(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    time: Res<Time>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    lib: Option<Res<AnimLibrary>>,
    mut rigs: Query<&mut StudioRig>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let studio = &mut *studio;
    if let Some((h, graph, index)) = &mut studio.own
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
        if index.is_empty() {
            studio.hero_clips = true;
        }
    }
    let Some(e) = studio.entity else { return };
    let Ok(mut rig) = rigs.get_mut(e) else { return };
    rig.rules = studio.rules;
    let Some(pe) = rig.player else { return };
    let Ok(mut player) = players.get_mut(pe) else { return };
    let source: Option<(Handle<AnimationGraph>, Box<dyn Fn(&str) -> Option<(AnimationNodeIndex, f32)>>)> = if studio.hero_clips {
        lib.as_ref().and_then(|l| l.graph.clone()).map(|g| {
            let names: HashMap<String, (AnimationNodeIndex, f32)> = lib.as_ref().unwrap().clips.iter().map(|(n, node)| (n.clone(), (*node, lib.as_ref().unwrap().duration(n)))).collect();
            (g, Box::new(move |c: &str| names.get(c).copied()) as Box<dyn Fn(&str) -> Option<(AnimationNodeIndex, f32)>>)
        })
    } else {
        studio.own.as_ref().and_then(|(_, g, idx)| g.clone().map(|g| {
            let idx = idx.clone();
            (g, Box::new(move |c: &str| idx.get(c).copied()) as Box<dyn Fn(&str) -> Option<(AnimationNodeIndex, f32)>>)
        }))
    };
    let Some((graph, find)) = source else { return };
    if studio.graph_attached != Some(graph.id()) {
        commands.entity(pe).insert(AnimationGraphHandle(graph.clone()));
        studio.graph_attached = Some(graph.id());
        player.stop_all();
        return;
    }
    let Some((node, dur)) = studio.clip.as_deref().and_then(|c| find(c)) else {
        player.stop_all();
        return;
    };
    studio.duration = dur.max(0.01);
    if studio.playing {
        studio.time = (studio.time + time.delta_secs() * studio.speed).rem_euclid(studio.duration);
    }
    if !player.is_playing_animation(node) {
        player.stop_all();
        player.play(node);
    }
    if let Some(a) = player.animation_mut(node) {
        a.set_speed(0.0).set_seek_time(studio.time.min(studio.duration));
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

/// Skeleton overlay: teal bones, red where a joint is outside its range, gold for the selection.
fn draw_skeleton(studio: Res<Studio>, rigs: Query<&StudioRig>, bones: Query<(&Name, &GlobalTransform, Option<&ChildOf>)>, mut gizmos: Gizmos<StudioGizmos>) {
    if !studio.open {
        return;
    }
    let Some(rig) = studio.entity.and_then(|e| rigs.get(e).ok()) else { return };
    let bad: Vec<&str> = studio.numbers["violations"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
    for &b in &rig.bones {
        let Ok((name, g, parent)) = bones.get(b) else { continue };
        if name.as_str().contains("Finger") {
            continue;
        }
        if let Some(p) = parent.map(|c| c.parent()).filter(|pe| rig.bones.contains(pe)).and_then(|pe| bones.get(pe).ok()) {
            let col = if bad.contains(&name.as_str()) { Color::srgb(1.0, 0.3, 0.25) } else { Color::srgb(0.35, 0.86, 0.78) };
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
    for &(a, j, b, lo, hi) in crate::joints::RULES {
        let (Some(pa), Some(pj), Some(pb)) = (pos(a), pos(j), pos(b)) else { continue };
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
            let fwd = world.get::<Transform>(e).map_or(Vec3::Z, |t| t.rotation * Vec3::Z);
            let line = (a - h).normalize_or_zero();
            let off = (k - h) - line * (k - h).dot(line);
            let backward = off.dot(fwd) < -0.03;
            if backward || flex > crate::knee::MAX_FLEX_DEG + 0.5 {
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
    if let (Some(e), Some(clip)) = (s.entity, s.clip.clone()) && let Some(r) = world.get::<StudioRig>(e) {
        jobs.push((clip, s.time / s.duration.max(0.01), r.bones.clone()));
    }
    let corr = world.resource::<Corrections>().list.clone();
    let ledger = Corrections { list: corr };
    for (clip, t, bones) in jobs {
        for b in bones {
            let Some(name) = world.get::<Name>(b).map(|n| n.as_str().to_owned()) else { continue };
            let o = ledger.offset(&clip, &name, t);
            if o != Vec3::ZERO && let Some(mut tr) = world.get_mut::<Transform>(b) {
                tr.rotation = (tr.rotation * Quat::from_euler(EulerRot::XYZ, o.x.to_radians(), o.y.to_radians(), o.z.to_radians())).normalize();
            }
        }
    }
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
    std::fs::read_to_string(format!("{}/collab/pose_audit.json", crate::ROOT)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
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
    json!({"open": s.open, "model": s.model, "clip": s.clip, "time": s.time, "duration": s.duration, "hero_clips": s.hero_clips, "rules": s.rules,
           "selected": s.selected, "numbers": s.numbers, "corrections": corr.to_json()["corrections"], "patterns": patterns(corr, &notes),
           "notes": notes.iter().rev().take(20).collect::<Vec<_>>(), "assets": s.assets.len(), "audit_patterns": audit_patterns(&read_audit())})
}

#[allow(clippy::too_many_arguments)]
fn ui(
    mut contexts: EguiContexts,
    mut studio: ResMut<Studio>,
    mut corr: ResMut<Corrections>,
    mut layout: ResMut<HudLayout>,
    diag: Res<DiagnosticsStore>,
    kit: Option<Res<Kit>>,
    lib: Option<Res<AnimLibrary>>,
    sim: Res<Sim>,
    mut feed: ResMut<crate::feed::DevFeed>,
    assets: Res<AssetServer>,
    bodies: Query<(), With<avian3d::prelude::RigidBody>>,
) -> Result {
    if !studio.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    theme::style(&ctx);
    let kit = kit.as_deref();
    let studio = &mut *studio;
    let mut root = egui::Ui::new(ctx.clone(), "studio_root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.viewport_rect()));

    // toolbar
    egui::Panel::top("studio_bar").frame(theme::dock(kit)).show_inside(&mut root, |ui| {
        theme::dock_backdrop(ui, kit, ui.max_rect());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("GUILDBOUND STUDIO").size(15.0).strong().color(theme::TEAL));
            ui.separator();
            for (i, t) in TABS.iter().enumerate() {
                if theme::button(ui, kit, t, studio.tab == i).clicked() {
                    studio.tab = i;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let fps = diag.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
                ui.label(egui::RichText::new(format!("{fps:.0} FPS  |  F2 close")).color(theme::MUTED));
            });
        });
    });

    // asset browser
    egui::Panel::left("studio_assets").default_size(300.0).resizable(true).frame(theme::dock(kit)).show_inside(&mut root, |ui| {
        theme::dock_backdrop(ui, kit, ui.max_rect());
        theme::section(ui, "Assets");
        ui.add(egui::TextEdit::singleline(&mut studio.filter).hint_text("filter: hero, npc, Crate, Pine, Imp ..."));
        ui.label(egui::RichText::new(format!("{} models", studio.assets.len())).size(11.0).color(theme::MUTED));
        egui::ScrollArea::vertical().id_salt("assets").max_height(ui.available_height() * 0.5).show(ui, |ui| {
            let f = studio.filter.to_lowercase();
            let mut shown = 0;
            let mut pick = None;
            for a in studio.assets.iter().filter(|a| f.is_empty() || a.to_lowercase().contains(&f)) {
                shown += 1;
                if shown > 250 {
                    ui.label(egui::RichText::new("... refine the filter").color(theme::MUTED));
                    break;
                }
                let short = a.trim_start_matches("packs/").replace("_sources/", "");
                if ui.selectable_label(studio.model.as_deref() == Some(a.as_str()), egui::RichText::new(short).size(11.5)).clicked() {
                    pick = Some(a.clone());
                }
            }
            if let Some(p) = pick {
                studio.spawn_request = Some(p);
            }
        });
        theme::section(ui, "Clips");
        ui.horizontal(|ui| {
            ui.checkbox(&mut studio.hero_clips, "Hero library");
            ui.add(egui::TextEdit::singleline(&mut studio.clip_filter).hint_text("filter clips"));
        });
        let names: Vec<(String, f32)> = if studio.hero_clips {
            lib.as_ref().map(|l| l.clips.iter().map(|(n, _)| (n.clone(), l.duration(n))).collect()).unwrap_or_default()
        } else {
            studio.own.as_ref().map(|(_, _, idx)| idx.iter().map(|(n, (_, d))| (n.clone(), *d)).collect()).unwrap_or_default()
        };
        egui::ScrollArea::vertical().id_salt("clips").show(ui, |ui| {
            let f = studio.clip_filter.to_lowercase();
            for (n, d) in names.iter().filter(|(n, _)| f.is_empty() || n.to_lowercase().contains(&f)) {
                if ui.selectable_label(studio.clip.as_deref() == Some(n.as_str()), egui::RichText::new(format!("{n}   {d:.2}s")).size(11.5)).clicked() {
                    studio.clip = Some(n.clone());
                    studio.time = 0.0;
                }
            }
        });
    });

    // inspector / tabs
    egui::Panel::right("studio_inspect").default_size(360.0).resizable(true).frame(theme::dock(kit)).show_inside(&mut root, |ui| {
        theme::dock_backdrop(ui, kit, ui.max_rect());
        egui::ScrollArea::vertical().show(ui, |ui| match studio.tab {
            0 => inspect(ui, studio, &mut corr, kit, &mut feed, sim.world.tick),
            1 => corrections_tab(ui, &mut corr),
            2 => {
                theme::section(ui, "Patterns across clips and models");
                ui.label(egui::RichText::new("Repeated fixes become library-wide rules; repeated notes show where the pipeline is wrong.").size(11.0).color(theme::MUTED));
                let notes = read_notes();
                for p in patterns(&corr, &notes) {
                    let bone = p["bone"].as_str().unwrap_or("-").to_owned();
                    if p["kind"] == "correction" {
                        let clips = p["clips"].as_array().map_or(0, |a| a.len());
                        let m = &p["mean_offset_deg"];
                        theme::stat(ui, &format!("{bone}: corrected in {clips} clips"), format!("{:.1} {:.1} {:.1}", m[0].as_f64().unwrap_or(0.0), m[1].as_f64().unwrap_or(0.0), m[2].as_f64().unwrap_or(0.0)), theme::WARN);
                        if p["promotable"] == true && theme::button(ui, kit, "Promote to all clips", false).clicked() {
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
                    if theme::button(ui, kit, "Reload", false).clicked() {
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
                    let dur = if hero { lib.as_ref().map_or(1.0, |l| l.duration(clip)) } else { studio.own.as_ref().and_then(|(_, _, idx)| idx.get(clip)).map_or(1.0, |(_, d)| *d) };
                    studio.clip = Some(clip.to_owned());
                    studio.time = t * dur;
                    studio.playing = false;
                    studio.selected = p["bone"].as_str().and_then(|b| b.split("->").next()).filter(|b| b.starts_with("B_")).map(str::to_owned);
                    studio.status = format!("Audit: {} {} at {at}", p["check"].as_str().unwrap_or(""), p["bone"].as_str().unwrap_or(""));
                }
            }
            3 => {
                theme::section(ui, "Notes sent to Claude");
                for n in read_notes().iter().rev().take(30) {
                    ui.label(egui::RichText::new(format!("{}  {} @ {:.2}s  {}", n["model"].as_str().unwrap_or(""), n["clip"].as_str().unwrap_or(""), n["time"].as_f64().unwrap_or(0.0), n["bone"].as_str().unwrap_or(""))).size(11.0).color(theme::MUTED));
                    ui.label(egui::RichText::new(n["text"].as_str().unwrap_or("")).color(theme::PAPER));
                    ui.separator();
                }
            }
            4 => {
                theme::section(ui, "HUD layout");
                let l = &mut *layout;
                for (label, v) in [("Vitals card", &mut l.vitals), ("Skill bar", &mut l.skills), ("Target card", &mut l.target)] {
                    ui.label(label);
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut v[0]).speed(1.0).prefix("x "));
                        ui.add(egui::DragValue::new(&mut v[1]).speed(1.0).prefix("y "));
                    });
                }
                ui.add(egui::Slider::new(&mut layout.scale, 0.6..=1.6).text("scale"));
                ui.horizontal(|ui| {
                    if theme::button(ui, kit, "Save layout", false).clicked() {
                        layout.save();
                        studio.status = "HUD layout saved to assets/ui/hud_layout.json".into();
                    }
                    if theme::button(ui, kit, "Reset", false).clicked() {
                        *layout = HudLayout::default();
                    }
                });
                ui.label(egui::RichText::new("Close the Studio (F2) to see the HUD at its new place.").size(11.0).color(theme::MUTED));
            }
            _ => {
                theme::section(ui, "Engine");
                let d = |id: &bevy::diagnostic::DiagnosticPath| diag.get(id).and_then(|d| d.smoothed());
                theme::stat(ui, "FPS", format!("{:.1}", d(&FrameTimeDiagnosticsPlugin::FPS).unwrap_or(0.0)), theme::GOOD);
                theme::stat(ui, "Frame time", format!("{:.2} ms", d(&FrameTimeDiagnosticsPlugin::FRAME_TIME).unwrap_or(0.0)), theme::PAPER);
                theme::stat(ui, "Entities", format!("{:.0}", d(&EntityCountDiagnosticsPlugin::ENTITY_COUNT).unwrap_or(0.0)), theme::PAPER);
                theme::stat(ui, "Physics bodies", format!("{}", bodies.iter().count()), theme::PAPER);
                theme::stat(ui, "Models in assets", format!("{}", studio.assets.len()), theme::PAPER);
                theme::stat(ui, "Hero clips", format!("{}", lib.as_ref().map_or(0, |l| l.clips.len())), theme::PAPER);
                let loading = studio.model.as_ref().map(|m| format!("{:?}", assets.get_load_state(&assets.load::<Gltf>(m.clone()))));
                theme::stat(ui, "Stage model load", loading.unwrap_or_else(|| "-".into()), theme::MUTED);
                theme::section(ui, "Simulation");
                theme::stat(ui, "Tick", format!("{}", sim.world.tick), theme::PAPER);
                theme::stat(ui, "Fighters", format!("{}", sim.world.fighters.len()), theme::PAPER);
                theme::stat(ui, "Projectiles", format!("{}", sim.world.projectiles.len()), theme::PAPER);
                theme::stat(ui, "Obstacles", format!("{}", sim.world.obstacles.len()), theme::PAPER);
                theme::stat(ui, "State hash", format!("{:016x}", sim.world.state_hash()), theme::MUTED);
            }
        });
    });

    // timeline
    egui::Panel::bottom("studio_timeline").frame(theme::dock(kit)).show_inside(&mut root, |ui| {
        theme::dock_backdrop(ui, kit, ui.max_rect());
        ui.horizontal(|ui| {
            if theme::button(ui, kit, if studio.playing { "Pause" } else { "Play" }, studio.playing).clicked() {
                studio.playing = !studio.playing;
            }
            ui.add(egui::Slider::new(&mut studio.speed, 0.05..=2.0).text("speed"));
            let dur = studio.duration;
            ui.spacing_mut().slider_width = (ui.available_width() - 260.0).max(120.0);
            if ui.add(egui::Slider::new(&mut studio.time, 0.0..=dur).text(format!("/ {dur:.2}s"))).dragged() {
                studio.playing = false;
            }
            ui.checkbox(&mut studio.rules, "Authority rules");
        });
        ui.label(egui::RichText::new(format!("{}   |   {}   |   {}", studio.model.as_deref().unwrap_or("no model"), studio.clip.as_deref().unwrap_or("no clip"), studio.status)).size(11.0).color(theme::MUTED));
    });
    Ok(())
}

fn inspect(ui: &mut egui::Ui, studio: &mut Studio, corr: &mut Corrections, kit: Option<&Kit>, feed: &mut crate::feed::DevFeed, tick: u64) {
    theme::section(ui, "Stage");
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(&mut studio.yaw, -180.0..=180.0).text("yaw"));
        ui.add(egui::Slider::new(&mut studio.scale, 0.2..=3.0).text("scale"));
    });
    let n = studio.numbers.clone();
    theme::section(ui, "Joint ranges (final pose)");
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
        if theme::button(ui, kit, "Key offset", false).clicked() && !clip.is_empty() {
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
        if theme::button(ui, kit, "Clear bone", false).clicked() {
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
    if theme::button(ui, kit, "Send note", false).clicked() && !studio.note.trim().is_empty() {
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

fn corrections_tab(ui: &mut egui::Ui, corr: &mut Corrections) {
    theme::section(ui, "Corrections ledger");
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
