//! PoseNet in the game (crates/posenet): the collector that feeds it, and the joint balls it drives.
//!
//! Collect: every clip of the library is sampled at 30 fps on hidden mannequins (no procedural layers on them,
//! so the data is the clips as authored) and written to assets/packs/_train/poses.bin; the trainer
//! (cargo run -p posenet --release) writes posenet.bin next to it, loaded here at start.
//!
//! Pose: in Possession a ball sits on every joint. The head, hand and foot balls (gold) and the pelvis ball
//! (teal) can be dragged; the network turns their places into a natural whole-body pose, and two-bone IK lands
//! hands and feet exactly on their balls. Capture key stores the balls; Generate moves them along smooth curves
//! between the keys and asks the network for every frame at 30 fps, giving a playable motion (saved to
//! assets/generated/NAME.json, the gb.possess motion format).
use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady};
use bevy_egui::egui;
use posenet::{BONES, EFFECTORS, Mlp, math, pose};
use serde_json::json;

use crate::anim::AnimLibrary;
use crate::motion_math::world_transform;
use crate::studio::{Studio, StudioRig};
use crate::ui_theme as theme;

const DATA: &str = "assets/packs/_train/poses.bin";
const WEIGHTS: &str = "assets/packs/_train/posenet.bin";
const FPS: f32 = 30.0;
const COLLECTORS: usize = 24;
const PELVIS_BALL: usize = 5;

struct Job {
    root: Entity,
    player: Option<Entity>,
    bones: Vec<Option<Entity>>,
    /// Clip index and frame; frame -1 lets the first seek land before reading.
    clip: Option<usize>,
    frame: i32,
}

#[derive(Resource, Default)]
pub struct Brain {
    pub net: Option<Mlp>,
    jobs: Vec<Job>,
    queue: Vec<usize>,
    clips: Vec<(String, AnimationNodeIndex, f32)>,
    frames: Vec<pose::Frame>,
    /// Network posing on the stage model.
    pub smart: bool,
    /// Head, left hand, right hand, left foot, right foot, pelvis: world positions.
    balls: Option<[Vec3; 6]>,
    yaw: f32,
    drag: Option<(usize, f32)>,
    pub show_balls: bool,
    pub balls_only: bool,
    hidden: bool,
    pub trails: bool,
    trail: Vec<VecDeque<Vec3>>,
    /// Captured keys: seconds after the previous key, and the balls in the rig's space.
    keys: Vec<(f32, [Vec3; 6])>,
    pub name: String,
    pub status: String,
    pending: Option<Action>,
    /// Start sampling the library (UI button or gb.brain {collect: true}).
    pub collect_request: bool,
}

#[derive(Component)]
struct Collector(usize);

pub fn plugin(app: &mut App) {
    let net = std::fs::read(format!("{}/{WEIGHTS}", crate::ROOT)).ok().and_then(|b| Mlp::from_bytes(&b));
    app.insert_resource(Brain { net, show_balls: true, name: "my_motion".into(), ..default() })
        .add_systems(Update, (collect_start, collect_seek, drag, draw, hide_meshes, run_pending))
        .add_systems(Last, collect_read)
        .add_systems(PostUpdate, apply.after(crate::possession::apply).before(crate::support::stage));
}

// ---------------------------------------------------------------- collector

fn start_collect(commands: &mut Commands, assets: &AssetServer, brain: &mut Brain, lib: &AnimLibrary) {
    brain.clips = lib.clips.iter().zip(&lib.durations).map(|((n, node), d)| (n.clone(), *node, *d)).collect();
    brain.queue = (0..brain.clips.len()).rev().collect();
    brain.frames.clear();
    let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset("packs/hero/mannequin.glb"));
    brain.jobs = (0..COLLECTORS).map(|i| {
        let root = commands.spawn((Collector(i), WorldAssetRoot(scene.clone()), Transform::from_xyz(i as f32 * 3.0, -400.0, 4000.0), Visibility::Hidden)).observe(on_collector_ready).id();
        Job { root, player: None, bones: vec![None; BONES.len()], clip: None, frame: -1 }
    }).collect();
    brain.status = format!("Sampling {} clips ...", brain.clips.len());
}

fn collect_start(mut commands: Commands, assets: Res<AssetServer>, mut brain: ResMut<Brain>, lib: Option<Res<AnimLibrary>>) {
    if !std::mem::take(&mut brain.collect_request) || !brain.jobs.is_empty() {
        return;
    }
    match lib.filter(|l| l.graph.is_some()) {
        Some(lib) => start_collect(&mut commands, &assets, &mut brain, &lib),
        None => brain.status = "The animation library has not loaded yet.".into(),
    }
}

fn on_collector_ready(ready: On<WorldInstanceReady>, mut commands: Commands, children: Query<&Children>, nodes: Query<(&Name, &Transform)>, which: Query<&Collector>, mut brain: ResMut<Brain>, lib: Res<AnimLibrary>) {
    let Ok(c) = which.get(ready.entity) else { return };
    let added = crate::anim::add_targets(&mut commands, ready.entity, &children, &nodes);
    let Some(&player) = added.first() else { return };
    if let Some(graph) = &lib.graph {
        commands.entity(player).insert(AnimationGraphHandle(graph.clone()));
    }
    let job = &mut brain.jobs[c.0];
    job.player = Some(player);
    for e in added {
        if let Ok((n, _)) = nodes.get(e) && let Some(i) = BONES.iter().position(|b| *b == n.as_str()) {
            job.bones[i] = Some(e);
        }
    }
}

fn collect_seek(mut brain: ResMut<Brain>, mut players: Query<&mut AnimationPlayer>) {
    let brain = &mut *brain;
    for job in &mut brain.jobs {
        let Some(pe) = job.player else { continue };
        let Ok(mut player) = players.get_mut(pe) else { continue };
        if job.clip.is_none() {
            let Some(next) = brain.queue.pop() else { continue };
            job.clip = Some(next);
            job.frame = -1;
            player.stop_all();
        }
        let (_, node, duration) = brain.clips[job.clip.unwrap()];
        if !player.is_playing_animation(node) {
            player.play(node);
        }
        if let Some(a) = player.animation_mut(node) {
            a.set_speed(0.0).set_seek_time((job.frame.max(0) as f32 / FPS).min(duration));
        }
    }
}

fn collect_read(mut commands: Commands, mut brain: ResMut<Brain>, globals: Query<&GlobalTransform>, locals: Query<&Transform>) {
    let brain = &mut *brain;
    if brain.jobs.is_empty() {
        return;
    }
    for job in &mut brain.jobs {
        let Some(clip) = job.clip else { continue };
        if job.frame >= 0 && let Ok(root) = globals.get(job.root) {
            let inv = root.affine().inverse();
            let (_, root_rot, _) = root.to_scale_rotation_translation();
            let bones: Option<Vec<pose::BonePose>> = job.bones.iter().map(|b| {
                let e = (*b)?;
                let g = globals.get(e).ok()?;
                let (_, rot, pos) = g.to_scale_rotation_translation();
                let rig_rot = (root_rot.inverse() * rot).normalize();
                let local = locals.get(e).ok()?.rotation;
                Some(pose::BonePose { pos: inv.transform_point3(pos).to_array(), rot: rig_rot.to_array(), local: local.to_array() })
            }).collect();
            if let Some(bones) = bones {
                brain.frames.push(pose::Frame { clip: clip as u32, bones });
            }
        }
        job.frame += 1;
        if job.frame as f32 / FPS > brain.clips[clip].2 {
            job.clip = None;
        }
    }
    if brain.queue.is_empty() && brain.jobs.iter().all(|j| j.clip.is_none()) {
        let data = posenet::Dataset { clips: brain.clips.iter().map(|c| c.0.clone()).collect(), frames: std::mem::take(&mut brain.frames) };
        let path = format!("{}/{DATA}", crate::ROOT);
        let _ = std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap());
        let _ = std::fs::write(&path, data.to_bytes());
        for job in brain.jobs.drain(..) {
            commands.entity(job.root).despawn();
        }
        brain.status = format!("Saved {} poses from {} clips to {DATA}. Train with: cargo run -p posenet --release -- {DATA} {WEIGHTS}", data.frames.len(), data.clips.len());
    }
}

// ---------------------------------------------------------------- posing

struct Stage {
    root: Entity,
    map: BTreeMap<String, Entity>,
}

fn stage(world: &World) -> Option<Stage> {
    let root = world.resource::<Studio>().entity?;
    let bones = world.get::<StudioRig>(root)?.bones.clone();
    let map = bones.iter().filter_map(|&e| world.get::<Name>(e).map(|n| (n.as_str().to_owned(), e))).collect();
    Some(Stage { root, map })
}

fn bone_pos(world: &World, s: &Stage, name: &str) -> Option<Vec3> {
    world_transform(world, *s.map.get(name)?).map(|t| t.translation())
}

/// The balls where the joints are now, and the facing from the hips (rig space).
fn balls_from_pose(world: &World, s: &Stage) -> Option<([Vec3; 6], f32)> {
    let mut b = [Vec3::ZERO; 6];
    for (k, &i) in EFFECTORS.iter().enumerate() {
        b[k] = bone_pos(world, s, BONES[i])?;
    }
    b[PELVIS_BALL] = bone_pos(world, s, "B_Pelvis")?;
    let root = world_transform(world, s.root)?;
    let inv = root.affine().inverse();
    let (l, r) = (inv.transform_point3(bone_pos(world, s, "B_L_Thigh")?), inv.transform_point3(bone_pos(world, s, "B_R_Thigh")?));
    let side = l - r;
    let fwd = Vec3::new(side.x, 0.0, side.z).cross(Vec3::Y);
    Some((b, fwd.x.atan2(fwd.z)))
}

/// The network's pose for balls given in the rig's space: (pelvis rotation in rig space, local rotations).
fn predict(net: &Mlp, rig: &[Vec3; 6], yaw: f32) -> (Quat, Vec<Quat>) {
    let eff: [math::V3; 5] = std::array::from_fn(|i| rig[i].to_array());
    let out = net.run(&pose::features(&eff, rig[PELVIS_BALL].to_array(), yaw));
    let (pelvis, locals) = pose::decode(&out, yaw);
    (Quat::from_array(pelvis), locals.into_iter().map(Quat::from_array).collect())
}

/// Write a predicted pose onto the stage rig, then land hands and feet on their balls with two-bone IK.
fn pose_rig(world: &mut World, s: &Stage, rig_balls: &[Vec3; 6], pelvis_rig: Quat, locals: &[Quat]) {
    let Some(root) = world_transform(world, s.root) else { return };
    let (_, root_rot, _) = root.to_scale_rotation_translation();
    for (i, name) in BONES.iter().enumerate().skip(1) {
        if let Some(&e) = s.map.get(*name) && let Some(mut t) = world.get_mut::<Transform>(e) {
            t.rotation = locals[i].normalize();
        }
    }
    if let Some(&pelvis) = s.map.get("B_Pelvis") && let Some(parent) = world.get::<ChildOf>(pelvis).map(|p| p.parent()) && let Some(pg) = world_transform(world, parent) {
        let (_, parent_rot, _) = pg.to_scale_rotation_translation();
        let world_pos = root.transform_point(rig_balls[PELVIS_BALL]);
        if let Some(mut t) = world.get_mut::<Transform>(pelvis) {
            t.rotation = (parent_rot.inverse() * root_rot * pelvis_rig).normalize();
            t.translation = pg.affine().inverse().transform_point3(world_pos);
        }
    }
    // hands and feet exactly on their balls; the pole keeps the network's own elbow and knee direction
    for (chain, ball) in [(["B_L_UpperArm", "B_L_Forearm", "B_L_Forearm.001"], 1), (["B_R_UpperArm", "B_R_Forearm", "B_R_Hand"], 2), (["B_L_Thigh", "B_L_Calf", "B_L_Foot"], 3), (["B_R_Thigh", "B_R_Calf", "B_R_Foot"], 4)] {
        let joints: Option<Vec<Entity>> = chain.iter().map(|n| s.map.get(*n).copied()).collect();
        let Some(j) = joints else { continue };
        let p: Option<Vec<Vec3>> = j.iter().map(|&e| world_transform(world, e).map(|t| t.translation())).collect();
        let Some(p) = p else { continue };
        let lengths = [p[0].distance(p[1]), p[1].distance(p[2])];
        if lengths.iter().any(|l| !(0.05..1.0).contains(l)) {
            continue;
        }
        let pole = p[1] - (p[0] + p[2]) * 0.5;
        crate::ik::apply(world, [j[0], j[1], j[2]], root.transform_point(rig_balls[ball]), pole, pole, lengths);
    }
}

pub fn apply(world: &mut World) {
    let b = world.resource::<Brain>();
    if !b.smart || b.net.is_none() || !world.resource::<Studio>().open || world.resource::<crate::possession::Possession>().motion.is_some() {
        return;
    }
    let Some(s) = stage(world) else { return };
    let Some(root) = world_transform(world, s.root) else { return };
    if world.resource::<Brain>().balls.is_none() {
        let Some((balls, yaw)) = balls_from_pose(world, &s) else { return };
        let mut b = world.resource_mut::<Brain>();
        b.balls = Some(balls);
        b.yaw = yaw;
    }
    let b = world.resource::<Brain>();
    let inv = root.affine().inverse();
    let rig: [Vec3; 6] = b.balls.unwrap().map(|p| inv.transform_point3(p));
    let (pelvis, locals) = predict(b.net.as_ref().unwrap(), &rig, b.yaw);
    pose_rig(world, &s, &rig, pelvis, &locals);
}

/// Left-drag a ball: it moves in the plane facing the camera.
fn drag(
    mut brain: ResMut<Brain>,
    studio: Res<Studio>,
    mouse: Res<ButtonInput<MouseButton>>,
    wants: Res<bevy_egui::input::EguiWantsInput>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    cam: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
) {
    let Some(balls) = brain.balls.filter(|_| brain.smart && studio.open) else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let Ok(ray) = cam.0.viewport_to_world(cam.1, cursor) else { return };
    if mouse.just_pressed(MouseButton::Left) && !wants.wants_any_pointer_input() {
        brain.drag = balls.iter().enumerate().filter_map(|(i, &p)| {
            let screen = cam.0.world_to_viewport(cam.1, p).ok()?;
            (screen.distance(cursor) < 22.0).then(|| (i, (p - ray.origin).dot(*ray.direction), screen.distance(cursor)))
        }).min_by(|a, b| a.2.total_cmp(&b.2)).map(|(i, d, _)| (i, d));
    }
    if !mouse.pressed(MouseButton::Left) {
        brain.drag = None;
    }
    if let Some((i, depth)) = brain.drag {
        let p = ray.origin + *ray.direction * depth;
        if let Some(b) = brain.balls.as_mut() {
            b[i] = p;
        }
    }
}

fn draw(mut brain: ResMut<Brain>, studio: Res<Studio>, rigs: Query<&StudioRig>, globals: Query<&GlobalTransform>, names: Query<&Name>, mut gizmos: Gizmos<crate::studio::StudioGizmos>) {
    if !studio.open || !brain.show_balls {
        return;
    }
    let Some(rig) = studio.entity.and_then(|e| rigs.get(e).ok()) else { return };
    let body: Vec<(usize, Vec3)> = rig.bones.iter().filter_map(|&e| {
        let i = BONES.iter().position(|b| names.get(e).is_ok_and(|n| n.as_str() == *b))?;
        Some((i, globals.get(e).ok()?.translation()))
    }).collect();
    // a ball on every joint, sized by the joint (hips and chest larger)
    for &(i, p) in &body {
        let r = if i <= 3 || matches!(i, 14 | 18) { 0.045 } else { 0.03 };
        gizmos.sphere(Isometry3d::from_translation(p), r, Color::srgba(0.35, 0.86, 0.78, 0.9));
    }
    // trails of the head, hands and feet (the last 1.5 s), to read arcs and spacing
    if brain.trails {
        if brain.trail.len() != EFFECTORS.len() {
            brain.trail = vec![VecDeque::new(); EFFECTORS.len()];
        }
        for (k, &i) in EFFECTORS.iter().enumerate() {
            if let Some(&(_, p)) = body.iter().find(|(b, _)| *b == i) {
                let t = &mut brain.trail[k];
                t.push_back(p);
                if t.len() > 90 {
                    t.pop_front();
                }
                gizmos.linestrip(t.iter().copied(), Color::srgba(1.0, 0.8, 0.3, 0.7));
            }
        }
    }
    if let Some(balls) = brain.balls.filter(|_| brain.smart) {
        for (i, p) in balls.iter().enumerate() {
            let c = if i == PELVIS_BALL { Color::srgb(0.2, 0.9, 0.8) } else { Color::srgb(1.0, 0.78, 0.25) };
            let held = brain.drag.is_some_and(|d| d.0 == i);
            gizmos.sphere(Isometry3d::from_translation(*p), if held { 0.09 } else { 0.07 }, c);
        }
    }
}

/// "Balls only": hide the stage model's meshes so just the joint balls show.
fn hide_meshes(mut brain: ResMut<Brain>, studio: Res<Studio>, children: Query<&Children>, mut meshes: Query<&mut Visibility, With<Mesh3d>>) {
    let want = brain.balls_only && studio.open;
    let Some(root) = studio.entity else { return };
    if want == brain.hidden && !want {
        return;
    }
    for e in children.iter_descendants(root) {
        if let Ok(mut v) = meshes.get_mut(e) {
            *v = if want { Visibility::Hidden } else { Visibility::Inherited };
        }
    }
    brain.hidden = want;
}

// ---------------------------------------------------------------- keys and generation

/// Catmull-Rom through the key balls (ends clamped), the smooth arcs the network fills.
fn curve(keys: &[[Vec3; 6]], seg: usize, u: f32) -> [Vec3; 6] {
    let at = |i: isize| keys[i.clamp(0, keys.len() as isize - 1) as usize];
    let (p0, p1, p2, p3) = (at(seg as isize - 1), at(seg as isize), at(seg as isize + 1), at(seg as isize + 2));
    let (u2, u3) = (u * u, u * u * u);
    std::array::from_fn(|k| 0.5 * ((2.0 * p1[k]) + (p2[k] - p0[k]) * u + (2.0 * p0[k] - 5.0 * p1[k] + 4.0 * p2[k] - p3[k]) * u2 + (3.0 * p1[k] - p0[k] - 3.0 * p2[k] + p3[k]) * u3))
}

/// Build the motion: the network's pose every 1/30 s along the curves, as rest-relative bone turns.
fn generate(world: &mut World) -> Result<serde_json::Value, String> {
    let s = stage(world).ok_or("put a model on the stage first")?;
    let root = s.root;
    let rest = world.get::<crate::procedural::RestPose>(root).ok_or("model not ready")?;
    let (rest_rot, rest_pos) = (rest.0.clone(), rest.1.clone());
    let b = world.resource::<Brain>();
    let net = b.net.as_ref().ok_or("no trained network yet")?;
    if b.keys.len() < 2 {
        return Err("capture at least two keys".into());
    }
    let times: Vec<f32> = b.keys.iter().scan(0.0, |t, k| { *t += k.0; Some(*t) }).map(|t| t - b.keys[0].0).collect();
    let pts: Vec<[Vec3; 6]> = b.keys.iter().map(|k| k.1).collect();
    let total = *times.last().unwrap();
    let frames = ((total * FPS).round() as usize).clamp(1, 239);
    let pelvis = *s.map.get("B_Pelvis").ok_or("no pelvis")?;
    let parent = world.get::<ChildOf>(pelvis).map(|p| p.parent()).ok_or("no pelvis parent")?;
    let root_g = world_transform(world, root).ok_or("no root")?;
    let parent_g = world_transform(world, parent).ok_or("no parent")?;
    // the pelvis parent in rig space (the root-motion bone is pinned at rest)
    let parent_rig = root_g.affine().inverse() * parent_g.affine();
    let (_, parent_rig_rot, _) = parent_rig.to_scale_rotation_translation();
    let rest_pelvis_rig = parent_rig.transform_point3(rest_pos[&pelvis]);
    let mut out = Vec::new();
    for f in 0..=frames {
        let t = total * f as f32 / frames as f32;
        let seg = times.windows(2).position(|w| t <= w[1]).unwrap_or(times.len() - 2);
        let u = ((t - times[seg]) / (times[seg + 1] - times[seg]).max(1e-4)).clamp(0.0, 1.0);
        let rig = curve(&pts, seg, u);
        let (pelvis_rig, locals) = predict(net, &rig, b.yaw);
        let mut bones = serde_json::Map::new();
        for (i, name) in BONES.iter().enumerate() {
            let Some(&e) = s.map.get(*name) else { continue };
            let local = if i == 0 { (parent_rig_rot.inverse() * pelvis_rig).normalize() } else { locals[i] };
            let off = (rest_rot[&e].inverse() * local).normalize();
            let (x, y, z) = off.to_euler(EulerRot::XYZ);
            bones.insert((*name).to_owned(), json!([x.to_degrees(), y.to_degrees(), z.to_degrees()]));
        }
        let pelvis_m = rig[PELVIS_BALL] - rest_pelvis_rig;
        out.push(json!({"time": t, "bones": bones, "pelvis_m": pelvis_m.to_array(), "hands_m": {"left": rig[1].to_array(), "right": rig[2].to_array()}}));
    }
    Ok(json!({"keys": out, "loop": false}))
}

pub fn ui(ui: &mut egui::Ui, b: &mut Brain) {
    let mut act = None;
    theme::section(ui, "Joint balls and smart posing");
    theme::hint(ui, "A ball sits on every joint. With smart posing on, drag the gold balls (head, hands, feet) or the teal pelvis ball and the network moves the whole body naturally.");
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut b.show_balls, "Joint balls").on_hover_text("Draw a ball on every joint.");
        ui.checkbox(&mut b.balls_only, "Balls only").on_hover_text("Hide the model so only the joint balls show.");
        ui.checkbox(&mut b.trails, "Trails").on_hover_text("Draw the path of the head, hands and feet over the last moments: shows arcs and timing.");
    });
    match &b.net {
        Some(_) => {
            ui.horizontal(|ui| {
                if ui.checkbox(&mut b.smart, "Smart posing").on_hover_text("The network poses the whole body from the balls.").changed() {
                    b.balls = None;
                }
                if b.smart && theme::button(ui, "Reset balls", false).on_hover_text("Put the balls back on the current pose.").clicked() {
                    b.balls = None;
                }
            });
        }
        None => theme::hint(ui, "The network is not trained yet. Build the pose data, then train it (Claude runs the trainer)."),
    }
    let busy = !b.jobs.is_empty();
    ui.add_enabled_ui(!busy, |ui| {
        if theme::button(ui, "Build pose data", false).on_hover_text("Sample every animation in the library for the network to learn from (takes about half a minute).").clicked() {
            b.collect_request = true;
        }
    });

    theme::section(ui, &format!("Key poses ({})", b.keys.len()));
    theme::hint(ui, "Pose with the balls, capture a key, move the balls, capture again. Generate fills every frame in between.");
    ui.horizontal(|ui| {
        if theme::button(ui, "Capture key", false).on_hover_text("Store the balls as the next key pose.").clicked() {
            act = Some(Action::Capture);
        }
        if theme::button(ui, "Clear keys", false).clicked() {
            b.keys.clear();
        }
    });
    let mut remove = None;
    for (i, k) in b.keys.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("key {}", i + 1));
            if i > 0 {
                ui.add(egui::Slider::new(&mut k.0, 0.1..=3.0).text("s after the last")).on_hover_text("Time from the previous key.");
            }
            if theme::button(ui, "x", false).on_hover_text("Remove this key.").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        b.keys.remove(i);
    }
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut b.name).desired_width(140.0));
        if theme::button(ui, "Generate", true).on_hover_text("Build and play the motion; it is saved to assets/generated.").clicked() {
            act = Some(Action::Generate);
        }
    });
    if !b.status.is_empty() {
        ui.label(egui::RichText::new(&b.status).size(11.5).color(theme::MUTED));
    }
    if act.is_some() {
        b.pending = act;
    }
}

enum Action {
    Capture,
    Generate,
}

/// UI actions that need the whole world.
fn run_pending(world: &mut World) {
    let Some(action) = world.resource_mut::<Brain>().pending.take() else { return };
    match action {
        Action::Capture => {
            let Some(s) = stage(world) else { return };
            let Some(root) = world_transform(world, s.root) else { return };
            let balls = match world.resource::<Brain>().balls {
                Some(b) => b,
                None => match balls_from_pose(world, &s) { Some((b, yaw)) => { world.resource_mut::<Brain>().yaw = yaw; b } None => return },
            };
            let inv = root.affine().inverse();
            let mut b = world.resource_mut::<Brain>();
            b.keys.push((0.5, balls.map(|p| inv.transform_point3(p))));
            b.status = format!("{} keys", b.keys.len());
        }
        Action::Generate => {
            let result = generate(world).and_then(|v| {
                let known: Vec<String> = stage(world).map(|s| s.map.keys().cloned().collect()).unwrap_or_default();
                let motion = crate::generated_motion::Motion::parse(&v, &known)?;
                Ok((v, motion))
            });
            let name: String = world.resource::<Brain>().name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
            match result {
                Ok((v, motion)) => {
                    let dir = format!("{}/assets/generated", crate::ROOT);
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(format!("{dir}/{name}.json"), serde_json::to_string_pretty(&v).unwrap_or_default());
                    let keys = motion.keys.len();
                    world.resource_mut::<crate::possession::Possession>().motion = Some(motion);
                    let mut s = world.resource_mut::<Studio>();
                    s.time = 0.0;
                    s.playing = true;
                    world.resource_mut::<Brain>().status = format!("Generated {keys} frames, saved assets/generated/{name}.json");
                }
                Err(e) => world.resource_mut::<Brain>().status = format!("Could not generate: {e}"),
            }
        }
    }
}
