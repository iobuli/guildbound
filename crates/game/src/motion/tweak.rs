//! F2 pose tweak mode: freeze the player on one key of a bank track and hand-edit bones.
//! - PageUp / PageDown choose the track (the player's current action / locomotion tracks first,
//!   then every bank track by name), [ / ] choose the key.
//! - Click a bone (the nearest bone joint to the cursor ray) or step through the major bones with
//!   , and . ; the picked joint is marked with a gizmo.
//! - Arrow keys swing the bone about the camera's right / up axes, the mouse wheel twists it
//!   about its own length; 5° a step, 1° with Shift. Backspace resets the bone on this key.
//! - Ctrl+S writes the edits into `assets/anim/poses.json` `edits` (through
//!   `PoseBank::save_overrides`, starting from `PosesFile::current`); the bank's hot reload then
//!   applies them everywhere. Esc / F2 leaves (unsaved edits are kept until the next save or
//!   leaving, and logged).
//! While active, player input is ignored (`play::input`) and `tweak` is the only writer of the player's bone `Transform`s besides `compose`:
//! it runs after `MotionSet::Compose` and overwrites them with the frozen key pose (plus edits).
//! A small panel at the top right shows the track, key, bone and keys.
use super::bank::{Overrides, PoseBank, PosesFile};
use super::{MotionSet, Pose, Rig};
use crate::play::Player;
use crate::play::camera::MainCamera;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use gbui::prelude::*;
use std::collections::HashMap;

/// Swing / twist step (degrees) and the fine step with Shift.
const STEP: f32 = 5.0;
const FINE: f32 = 1.0;
/// Clicks pick the nearest joint within this distance of the cursor ray (m).
const PICK: f32 = 0.15;
/// The bones , and . step through.
const MAJOR: [&str; 21] = [
    "pelvis",
    "spine_01",
    "spine_02",
    "spine_03",
    "neck_01",
    "Head",
    "clavicle_l",
    "upperarm_l",
    "lowerarm_l",
    "hand_l",
    "clavicle_r",
    "upperarm_r",
    "lowerarm_r",
    "hand_r",
    "thigh_l",
    "calf_l",
    "foot_l",
    "thigh_r",
    "calf_r",
    "foot_r",
    "root",
];

pub fn plugin(app: &mut App) {
    app.init_resource::<Tweak>()
        .add_systems(Startup, spawn_panel)
        .add_systems(Update, (toggle, choose, pick, rotate, save, show_panel).chain())
        .add_systems(
            PostUpdate,
            freeze.after(MotionSet::Compose).before(TransformSystems::Propagate),
        );
}

/// The tool's state.
#[derive(Resource, Default)]
pub struct Tweak {
    pub on: bool,
    track: String,
    key: usize,
    bone: usize,
    /// Unsaved local rotations: (track, key) → bone index → rotation.
    edits: HashMap<(String, usize), HashMap<usize, Quat>>,
    status: String,
}

impl Tweak {
    fn pose(&self, bank: &PoseBank) -> Option<Pose> {
        let track = bank.tracks.get(&self.track)?;
        let key = track.keys.get(self.key.min(track.keys.len().saturating_sub(1)))?;
        let mut pose = key.pose.clone();
        if let Some(e) = self.edits.get(&(self.track.clone(), self.key)) {
            for (&i, &q) in e {
                if let Some(r) = pose.rot.get_mut(i) {
                    *r = q;
                }
            }
        }
        Some(pose)
    }
    fn unsaved(&self) -> usize {
        self.edits.values().map(HashMap::len).sum()
    }
}

/// Every bank track, sorted.
fn track_names(bank: &PoseBank) -> Vec<String> {
    let mut v: Vec<String> = bank.tracks.keys().cloned().collect();
    v.sort();
    v
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    bank: Option<Res<PoseBank>>,
    mut t: ResMut<Tweak>,
    players: Query<&crate::play::Action, With<Player>>,
) {
    let Some(bank) = bank else { return };
    let leave = t.on && keys.just_pressed(KeyCode::Escape);
    if !(keys.just_pressed(KeyCode::F2) || leave) {
        return;
    }
    t.on = !t.on && !leave;
    if t.on {
        if !bank.tracks.contains_key(&t.track) {
            // Start on the player's current action clip, else the idle.
            let current = players
                .iter()
                .next()
                .and_then(|a| a.layer.as_ref().map(|l| l.track.clone()))
                .filter(|n| bank.tracks.contains_key(n));
            t.track = current.unwrap_or_else(|| "ual1/Idle_Loop".to_string());
            t.key = 0;
        }
        if t.bone == 0 {
            t.bone = bank.skeleton.find("upperarm_r").unwrap_or(0);
        }
        t.status = "F2 tweak: click a bone, arrows / wheel rotate, Ctrl+S saves".into();
    } else if t.unsaved() > 0 {
        warn!("[tweak] left with {} unsaved bone edits (Ctrl+S saves)", t.unsaved());
    }
}

fn choose(keys: Res<ButtonInput<KeyCode>>, bank: Option<Res<PoseBank>>, mut t: ResMut<Tweak>) {
    let Some(bank) = bank else { return };
    if !t.on {
        return;
    }
    let names = track_names(&bank);
    if names.is_empty() {
        return;
    }
    let at = names.iter().position(|n| *n == t.track).unwrap_or(0);
    if keys.just_pressed(KeyCode::PageDown) {
        t.track = names[(at + 1) % names.len()].clone();
        t.key = 0;
    }
    if keys.just_pressed(KeyCode::PageUp) {
        t.track = names[(at + names.len() - 1) % names.len()].clone();
        t.key = 0;
    }
    let count = bank.tracks.get(&t.track).map_or(1, |tr| tr.keys.len().max(1));
    if keys.just_pressed(KeyCode::BracketRight) {
        t.key = (t.key + 1) % count;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        t.key = (t.key + count - 1) % count;
    }
    t.key = t.key.min(count - 1);
    let major: Vec<usize> = MAJOR.iter().filter_map(|n| bank.skeleton.find(n)).collect();
    let i = major.iter().position(|&b| b == t.bone).unwrap_or(0);
    if keys.just_pressed(KeyCode::Period) {
        t.bone = major[(i + 1) % major.len()];
    }
    if keys.just_pressed(KeyCode::Comma) {
        t.bone = major[(i + major.len() - 1) % major.len()];
    }
}

/// Click picks the joint nearest the cursor ray.
#[allow(clippy::too_many_arguments)]
fn pick(
    mouse: Res<ButtonInput<MouseButton>>,
    mut t: ResMut<Tweak>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    rigs: Query<&Rig, With<Player>>,
    globals: Query<&GlobalTransform>,
    ui: Query<&Interaction>,
) {
    if !t.on || !mouse.just_pressed(MouseButton::Left) || ui.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let (Ok(window), Ok((camera, cam_tf)), Ok(rig)) = (windows.single(), cameras.single(), rigs.single()) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else { return };
    let Ok(ray) = camera.viewport_to_world(cam_tf, cursor) else { return };
    let best = rig
        .bones
        .iter()
        .enumerate()
        .filter(|(_, e)| **e != Entity::PLACEHOLDER)
        .filter_map(|(i, e)| globals.get(*e).ok().map(|g| (i, g.translation())))
        .map(|(i, p)| {
            let along = (p - ray.origin).dot(*ray.direction).max(0.0);
            (i, (ray.origin + *ray.direction * along).distance(p))
        })
        .filter(|(_, d)| *d < PICK)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((i, _)) = best {
        t.bone = i;
    }
}

#[allow(clippy::too_many_arguments)]
fn rotate(
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    bank: Option<Res<PoseBank>>,
    mut t: ResMut<Tweak>,
    cameras: Query<&GlobalTransform, With<MainCamera>>,
    rigs: Query<&Rig, With<Player>>,
    globals: Query<&GlobalTransform>,
) {
    let scroll: f32 = wheel.read().map(|w| w.y.signum()).sum();
    let Some(bank) = bank else { return };
    if !t.on {
        return;
    }
    let step = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) { FINE } else { STEP }.to_radians();
    let mut world_axis = Vec3::ZERO;
    let cam = cameras.single().map(|c| c.compute_transform().rotation).unwrap_or_default();
    let (right, up) = (cam * Vec3::X, cam * Vec3::Y);
    if keys.just_pressed(KeyCode::ArrowUp) {
        world_axis -= right;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        world_axis += right;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        world_axis -= up;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        world_axis += up;
    }
    let reset = keys.just_pressed(KeyCode::Backspace);
    if world_axis == Vec3::ZERO && scroll == 0.0 && !reset {
        return;
    }
    let Some(pose) = t.pose(&bank) else { return };
    let i = t.bone.min(pose.rot.len().saturating_sub(1));
    let k = (t.track.clone(), t.key);
    if reset {
        if let Some(e) = t.edits.get_mut(&k) {
            e.remove(&i);
        }
        return;
    }
    let local = pose.rot[i];
    // Parent's world rotation turns a world axis into the bone's parent frame.
    let parent_world = bank.skeleton.parent[i]
        .and_then(|p| rigs.single().ok().and_then(|r| r.bones.get(p).copied()))
        .and_then(|e| globals.get(e).ok())
        .map(|g| g.compute_transform().rotation)
        .unwrap_or_default();
    let mut next = local;
    if world_axis != Vec3::ZERO {
        let axis = (parent_world.inverse() * world_axis).normalize_or(Vec3::X);
        next = Quat::from_axis_angle(axis, step) * next;
    }
    if scroll != 0.0 {
        next *= Quat::from_rotation_y(step * scroll);
    }
    t.edits.entry(k).or_default().insert(i, next.normalize());
}

/// Ctrl+S: merge the edits into the poses file's overrides and write it.
fn save(
    keys: Res<ButtonInput<KeyCode>>,
    bank: Option<Res<PoseBank>>,
    file: Option<Res<PosesFile>>,
    mut t: ResMut<Tweak>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if !t.on || !ctrl || !keys.just_pressed(KeyCode::KeyS) {
        return;
    }
    let (Some(bank), Some(file)) = (bank, file) else { return };
    let mut overrides: Overrides = file.current.clone();
    let mut n = 0;
    for ((track, key), bones) in &t.edits {
        for (&i, q) in bones {
            let name = bank.skeleton.names[i].clone();
            overrides
                .edits
                .entry(track.clone())
                .or_default()
                .entry(key.to_string())
                .or_default()
                .insert(name, q.to_array());
            n += 1;
        }
    }
    t.status = match bank.save_overrides(&overrides) {
        Ok(()) => {
            t.edits.clear();
            format!("saved {n} bone edits to poses.json")
        }
        Err(e) => format!("save failed: {e}"),
    };
    info!("[tweak] {}", t.status);
}

/// Hold the player in the chosen key pose (after `compose`, before propagation).
fn freeze(
    bank: Option<Res<PoseBank>>,
    t: Res<Tweak>,
    rigs: Query<&Rig, With<Player>>,
    mut bones: Query<&mut Transform, With<super::RigBone>>,
    globals: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    let Some(bank) = bank else { return };
    if !t.on {
        return;
    }
    let (Ok(rig), Some(pose)) = (rigs.single(), t.pose(&bank)) else { return };
    for (i, &e) in rig.bones.iter().enumerate() {
        let Ok(mut tf) = bones.get_mut(e) else { continue };
        if let Some(q) = pose.rot.get(i) {
            tf.rotation = *q;
        }
        if i == bank.bones.pelvis {
            tf.translation = bank.skeleton.rest[i].translation + pose.pelvis;
        }
    }
    if let Some(g) = rig.bones.get(t.bone).and_then(|e| globals.get(*e).ok()) {
        gizmos.sphere(Isometry3d::from_translation(g.translation()), 0.05, Color::srgb(1.0, 0.8, 0.2));
    }
}

#[derive(Component)]
struct Panel;
#[derive(Component)]
struct PanelText;

fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            Panel,
            panel(
                "bag",
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    right: px(18.0),
                    top: px(18.0),
                    padding: UiRect::all(px(10.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4.0),
                    ..default()
                },
            ),
            GlobalZIndex(45),
        ))
        .with_children(|p| {
            p.spawn(label("▌POSE TWEAK", "title"));
            p.spawn((PanelText, label("", "text")));
            p.spawn(label(
                "PgUp/PgDn track  [ ] key  click or , . bone\narrows swing  wheel twist  Shift fine  Bksp reset\nCtrl+S save  Esc/F2 leave",
                "text.dim",
            ));
        });
}

fn show_panel(
    t: Res<Tweak>,
    bank: Option<Res<PoseBank>>,
    mut roots: Query<&mut Node, With<Panel>>,
    mut texts: Query<&mut Text, With<PanelText>>,
) {
    for mut n in &mut roots {
        let want = if t.on { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
    let Some(bank) = bank else { return };
    if !t.on {
        return;
    }
    let keys = bank.tracks.get(&t.track).map_or(0, |tr| tr.keys.len());
    let time = bank.tracks.get(&t.track).and_then(|tr| tr.keys.get(t.key)).map_or(0.0, |k| k.time);
    let bone = bank.skeleton.names.get(t.bone).cloned().unwrap_or_default();
    let line = format!(
        "{}\nkey {}/{} at {:.2}s   bone {}\nunsaved {}   {}",
        t.track,
        t.key + 1,
        keys,
        time,
        bone,
        t.unsaved(),
        t.status
    );
    for mut text in &mut texts {
        if text.0 != line {
            text.0 = line.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_override_the_key_pose_and_major_bones_exist() {
        let (bank, _) = PoseBank::build_with(std::path::Path::new("__no_poses__.json")).expect("bank");
        for n in MAJOR {
            assert!(bank.skeleton.find(n).is_some(), "{n}");
        }
        let mut t = Tweak {
            track: "ual1/Punch_Jab".into(),
            key: 1,
            ..default()
        };
        let base = t.pose(&bank).expect("pose");
        let hand = bank.skeleton.find("hand_r").unwrap();
        let q = Quat::from_rotation_x(0.7);
        t.edits.entry((t.track.clone(), 1)).or_default().insert(hand, q);
        let edited = t.pose(&bank).unwrap();
        assert!(edited.rot[hand].angle_between(q) < 1e-5);
        assert_eq!(edited.rot[0], base.rot[0], "other bones untouched");
        assert_eq!(t.unsaved(), 1);
    }
}
