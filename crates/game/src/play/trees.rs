//! Voxel trees you can chop down ("tree" interactables, spawned by `world` with `spawn_tree`).
//!
//! The trunk is 8 cm voxels (a bark shell around pale ringed wood), the canopy 20 cm voxels in a
//! few noisy blobs; both are drawn as smooth surfaces through the voxels (`smooth_mesh`: the
//! trunk smooth-shaded, the canopy faceted like the low-poly rocks), so they match the world while
//! staying destructible. Every `interact::UseBeat` (the axe landing) cuts a V-notch deeper into
//! the trunk on the user's side, at the height the blade lands on the first chop: the trunk is
//! remeshed, chips fly, the canopy and camera shake, the arm kicks back and a thunk plays. When
//! the notch is `FELL_DEPTH` of the radius deep, everything above it becomes one dynamic body on
//! `Phys::Debris` that tips away from the user; the stump stays. The fallen tree thuds and sheds
//! leaves when it lands, then after `BREAK_AFTER` breaks into `items` (logs along the trunk,
//! sticks and apples from the crown), and the tree grows back out of the stump once
//! `PropState.regrow` runs out (`interact` treats it as unavailable until then).
//! `chop` / `fell` lines in `staging/relations.log` give the blade's distance to the bark and the
//! notch depth, to calibrate the stand distance and the grip.
use super::camera::CameraShake;
use super::fx::{Burst, BurstKind, Sfx, SfxKind};
use super::interact::{PropState, UseBeat, Using};
use super::items::{ItemAssets, ItemKind, spawn_item};
use super::monitor::Monitor;
use super::tools::{Holding, blade};
use super::voxel::{VoxelGrid, hash3};
use super::{HALF_HEIGHT, Interactable, Phys, PlaySet, Player};
use crate::motion::{MotionLayers, Rig};
use crate::motion::bank::PoseBank;
use avian3d::prelude::*;
use bevy::prelude::*;

/// Size and look of one tree; `seed` varies the bark, canopy shape and colours.
#[derive(Clone, Copy, Debug)]
pub struct TreeSpec {
    pub radius: f32,
    pub height: f32,
    pub canopy: f32,
    pub seed: u32,
}

const TRUNK_CELL: f32 = 0.08;
const CANOPY_CELL: f32 = 0.2;
/// The trunk thins by this share toward the top and flares by `FLARE` at the ground, fading out
/// over `FLARE_HEIGHT`; it starts `SINK` below the root so slopes show no gap.
const TAPER: f32 = 0.25;
const FLARE: f32 = 0.35;
const FLARE_HEIGHT: f32 = 0.4;
const SINK: f32 = 0.2;
const BARK_DEPTH: f32 = 1.1 * TRUNK_CELL;
const RING_SPACING: f32 = 1.5 * TRUNK_CELL;

const BARK: u8 = 1;
const BARK_DARK: u8 = 2;
const WOOD: u8 = 3;
const RING: u8 = 4;
const TRUNK_PALETTE: [Color; 4] = [
    Color::srgb(0.36, 0.25, 0.16),
    Color::srgb(0.26, 0.18, 0.11),
    Color::srgb(0.88, 0.72, 0.50),
    Color::srgb(0.74, 0.56, 0.35),
];
const LEAF: u8 = 1;
const LEAF_LIGHT: u8 = 2;
const LEAF_DARK: u8 = 3;
const CANOPY_PALETTE: [Color; 3] = [
    Color::srgb(0.24, 0.45, 0.18),
    Color::srgb(0.34, 0.57, 0.22),
    Color::srgb(0.16, 0.34, 0.13),
];

/// Chopping: the clip, where its axe lands (s), and where the user stands from the trunk axis
/// (m): the clip's hand stops 0.54 m ahead of the feet and the blade reaches about 0.76 m
/// further, minus the trunk radius.
pub const CHOP_TRACK: &str = "ual2/TreeChopping_Loop";
const CHOP_DURATION: f32 = 0.967;
const CHOP_BEAT: f32 = 0.17;
const STAND: f32 = 1.45;
/// Notch: depth per chop (m), half-height at the deepest point (m) and how fast the opening
/// widens toward the bark, height limits (m above the root) and the default height.
const CHOP_DEPTH: f32 = 0.09;
const NOTCH_MIN: f32 = 0.05;
const NOTCH_SLOPE: f32 = 0.35;
const NOTCH_LOW: f32 = 0.5;
const NOTCH_HIGH: f32 = 1.4;
const NOTCH_DEFAULT: f32 = 1.0;
/// The tree goes over at this share of its radius cut.
const FELL_DEPTH: f32 = 1.3;
/// Arm and chest kick-back (rad/s) and camera shake per chop.
const RECOIL: f32 = 3.0;
const CHOP_SHAKE: f32 = 0.18;
/// Felling: tip speed (rad/s), tilt (rad from upright) that counts as landed, landing shake
/// and its fall-off distance (m), the longest wait for a landing (s), and when the tree is back.
const TIP_SPEED: f32 = 0.7;
const LANDED_TILT: f32 = 75.0 * std::f32::consts::PI / 180.0;
const LAND_SHAKE: f32 = 0.35;
const SHAKE_RANGE: f32 = 20.0;
const LAND_TIMEOUT: f32 = 6.0;
/// Seconds a landed tree lies before it breaks into items, and the spacing (m) of the logs it
/// breaks into along the fallen trunk.
const BREAK_AFTER: f32 = 2.5;
const LOG_SPACING: f32 = 0.85;
const REGROW: f32 = 40.0;
/// Growing back: duration (s) and the trunk's starting scale.
const GROW_TIME: f32 = 3.0;
const GROW_FROM: f32 = 0.3;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, chop.in_set(PlaySet::Act).after(super::interact::drive))
        .add_systems(Update, (settle_felled, regrow));
}

impl TreeSpec {
    /// Trunk radius at height `y` above the root.
    pub fn radius_at(&self, y: f32) -> f32 {
        let taper = 1.0 - TAPER * (y / self.height.max(0.1)).clamp(0.0, 1.0);
        let flare = FLARE * (1.0 - y.max(0.0) / FLARE_HEIGHT).max(0.0).powi(2);
        self.radius * (taper + flare)
    }

    fn canopy_center(&self) -> Vec3 {
        Vec3::Y * (self.height - 0.25 + self.canopy * 0.5)
    }
}

/// A notch being cut: height above the root, horizontal direction toward the user (root
/// space) and depth (m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Notch {
    pub height: f32,
    pub dir: Vec3,
    pub depth: f32,
}

/// Whether a trunk voxel centred at `p` (root space) lies in the notch of a trunk of `radius`:
/// a wedge across the full width, cut in from the user's side, opening toward the bark.
pub fn in_notch(p: Vec3, notch: &Notch, radius: f32) -> bool {
    let rel = p - Vec3::Y * notch.height;
    let into = radius - Vec2::new(rel.x, rel.z).dot(Vec2::new(notch.dir.x, notch.dir.z));
    into < notch.depth && rel.y.abs() < NOTCH_MIN + NOTCH_SLOPE * (notch.depth - into).max(0.0)
}

pub fn build_trunk(spec: &TreeSpec) -> VoxelGrid {
    let reach = spec.radius * (1.0 + FLARE) + TRUNK_CELL;
    let n = (2.0 * reach / TRUNK_CELL).ceil() as u32 + 1;
    let ny = ((spec.height + SINK) / TRUNK_CELL).ceil() as u32;
    let half = n as f32 * TRUNK_CELL / 2.0;
    let mut grid = VoxelGrid::new(UVec3::new(n, ny, n), TRUNK_CELL, Vec3::new(-half, -SINK, -half));
    let seed = spec.seed;
    grid.fill(|p, c| {
        let d = Vec2::new(p.x, p.z).length();
        // Vertical bark ridges: the roughness changes every third layer.
        let rough = (hash3(c.x, c.y / 3, c.z, seed) - 0.5) * 0.6 * TRUNK_CELL;
        let r = spec.radius_at(p.y) + rough;
        if d > r {
            0
        } else if d > r - BARK_DEPTH {
            if hash3(c.x, c.y, c.z, seed ^ 0x51) < 0.3 { BARK_DARK } else { BARK }
        } else if ((d / RING_SPACING) as i32) % 2 == 1 {
            RING
        } else {
            WOOD
        }
    });
    grid
}

pub fn build_canopy(spec: &TreeSpec) -> VoxelGrid {
    let c = spec.canopy;
    let extent = c * 1.45;
    let n = (2.0 * extent / CANOPY_CELL).ceil() as u32;
    let half = n as f32 * CANOPY_CELL / 2.0;
    let mut grid = VoxelGrid::new(UVec3::splat(n), CANOPY_CELL, Vec3::splat(-half));
    let seed = spec.seed;
    let turn = hash3(1, 2, 3, seed) * std::f32::consts::TAU;
    let mut blobs = vec![(Vec3::ZERO, c)];
    for k in 0..4 {
        let a = turn + k as f32 * std::f32::consts::FRAC_PI_2;
        let lift = (hash3(k, 5, 7, seed) - 0.5) * 0.5 * c;
        let size = 0.6 * c * (0.85 + 0.25 * hash3(k, 9, 1, seed));
        blobs.push((Vec3::new(a.cos() * 0.55 * c, lift, a.sin() * 0.55 * c), size));
    }
    grid.fill(|p, cell| {
        if p.y < -0.55 * c {
            return 0;
        }
        let wobble = (hash3(cell.x, cell.y, cell.z, seed ^ 0x9A) - 0.5) * 1.5 * CANOPY_CELL;
        if !blobs.iter().any(|(o, r)| (p - *o).length() < r + wobble) {
            return 0;
        }
        let h = hash3(cell.x, cell.y, cell.z, seed ^ 0x3C);
        if p.y > 0.25 * c && h < 0.55 {
            LEAF_LIGHT
        } else if p.y < -0.2 * c && h < 0.6 {
            LEAF_DARK
        } else if h < 0.15 {
            LEAF_LIGHT
        } else {
            LEAF
        }
    });
    grid
}

/// Per-tree state on the interactable root.
#[derive(Component, Clone, Debug)]
pub struct Tree {
    spec: TreeSpec,
    trunk: Entity,
    canopy: Entity,
    collider: Entity,
    grid: VoxelGrid,
    trunk_mesh: Handle<Mesh>,
    canopy_mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    notch: Option<Notch>,
    felled: bool,
    /// 1 = grown; below 1 while growing back.
    grow: f32,
}

/// The part of a tree that fell (a dynamic body).
#[derive(Component, Clone, Copy, Debug)]
struct Felled {
    t: f32,
    /// `t` when it landed.
    landed: Option<f32>,
    canopy_at: Vec3,
    /// The log's extent along the body's local +Y (bottom, top) and the tree's seed.
    log: (f32, f32),
    seed: u32,
}

fn interactable() -> Interactable {
    Interactable {
        kind: "tree".into(),
        prompt: "Chop tree".into(),
        track: CHOP_TRACK.into(),
        duration: CHOP_DURATION,
        looping: true,
        stand: Vec3::new(0.0, 0.0, -STAND),
        face_yaw: 0.0,
        hands: [None, None],
        grip: None,
        range: 2.0,
        requires: Some("axe".into()),
        beat: Some(CHOP_BEAT),
        reach: None,
        radial: Some(STAND),
    }
}

fn spawn_canopy(
    commands: &mut Commands,
    root: Entity,
    mesh: Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    spec: &TreeSpec,
    scale: f32,
) -> Entity {
    commands
        .spawn((
            Name::new("Tree canopy"),
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(spec.canopy_center() * scale.max(GROW_FROM)).with_scale(Vec3::splat(scale)),
            ChildOf(root),
        ))
        .id()
}

fn trunk_collider(spec: &TreeSpec, height: f32) -> (Collider, Transform) {
    let height = height.max(0.1);
    (
        Collider::cylinder(spec.radius * 1.05, height),
        Transform::from_xyz(0.0, height / 2.0, 0.0),
    )
}

/// Spawn a choppable voxel tree with its root on the ground at `at`. `material` should be a
/// white, vertex-coloured material.
pub fn spawn_tree(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    at: Vec3,
    spec: TreeSpec,
) -> Entity {
    let grid = build_trunk(&spec);
    let trunk_mesh = meshes.add(grid.smooth_mesh(&TRUNK_PALETTE, spec.seed, false));
    let canopy_mesh = meshes.add(build_canopy(&spec).smooth_mesh(&CANOPY_PALETTE, spec.seed, true));
    let root = commands
        .spawn((
            Name::new("Tree"),
            Transform::from_translation(at),
            Visibility::default(),
            RigidBody::Static,
            interactable(),
        ))
        .id();
    let trunk = commands
        .spawn((
            Name::new("Tree trunk"),
            Mesh3d(trunk_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            ChildOf(root),
        ))
        .id();
    let collider = commands
        .spawn((
            Name::new("Tree trunk collider"),
            trunk_collider(&spec, spec.height),
            CollisionLayers::new(Phys::World, LayerMask::ALL),
            ChildOf(root),
        ))
        .id();
    let canopy = spawn_canopy(commands, root, canopy_mesh.clone(), material, &spec, 1.0);
    commands.entity(root).insert(Tree {
        spec,
        trunk,
        canopy,
        collider,
        grid,
        trunk_mesh,
        canopy_mesh,
        material: material.clone(),
        notch: None,
        felled: false,
        grow: 1.0,
    });
    root
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn chop(
    mut commands: Commands,
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut beats: MessageReader<UseBeat>,
    mut trees: Query<(&Transform, &mut Tree, &mut PropState)>,
    mut users: Query<
        (
            &Transform,
            Option<&Holding>,
            Option<&Name>,
            Has<Player>,
            Option<&mut MotionLayers>,
            Option<&Rig>,
            Option<&Using>,
        ),
        Without<Tree>,
    >,
    globals: Query<&GlobalTransform>,
    mut bursts: MessageWriter<Burst>,
    mut sounds: MessageWriter<Sfx>,
    mut shake: ResMut<CameraShake>,
    mut monitor: ResMut<Monitor>,
) {
    let now = time.elapsed_secs();
    for beat in beats.read() {
        let Ok((root, mut tree, mut prop)) = trees.get_mut(beat.target) else {
            continue;
        };
        if tree.felled {
            continue;
        }
        let Ok((user, holding, name, player, layers, rig, using)) = users.get_mut(beat.user) else {
            continue;
        };
        let to_local = |w: Vec3| root.rotation.inverse() * (w - root.translation);
        let edge = holding.and_then(|h| globals.get(h.item).ok()).map(blade).filter(|b| b.is_finite());
        let spec = tree.spec;
        let mut notch = tree.notch.unwrap_or_else(|| Notch {
            height: edge.map_or(NOTCH_DEFAULT, |b| to_local(b).y).clamp(NOTCH_LOW, NOTCH_HIGH),
            dir: to_local(user.translation).with_y(0.0).normalize_or(Vec3::NEG_Z),
            depth: 0.0,
        });
        let radius = spec.radius_at(notch.height);
        notch.depth += CHOP_DEPTH;
        tree.notch = Some(notch);
        let (removed, mean) = tree.grid.carve(|p| in_notch(p, &notch, radius));
        if removed > 0 {
            let _ = meshes.insert(&tree.trunk_mesh, tree.grid.smooth_mesh(&TRUNK_PALETTE, spec.seed, false));
        }

        // Feel: chips, canopy, camera, arm, sound.
        let toward_user = root.rotation * notch.dir;
        let cut_at = if removed > 0 {
            root.transform_point(mean)
        } else {
            root.transform_point(Vec3::Y * notch.height + notch.dir * radius)
        };
        bursts.write(Burst {
            kind: BurstKind::Chips,
            at: cut_at + toward_user * 0.05,
            dir: toward_user,
            count: 10,
        });
        prop.shake = 1.0;
        if player {
            shake.add(CHOP_SHAKE);
        }
        if let (Some(mut layers), Some(bank)) = (layers, bank.as_deref()) {
            let left = user.rotation * Vec3::X;
            layers.impulses.push((bank.bones.lowerarm[1], -left * RECOIL));
            layers.impulses.push((bank.bones.spine[1], -left * RECOIL * 0.5));
        }
        sounds.write(Sfx {
            kind: SfxKind::Chop,
            at: cut_at,
            volume: 0.9,
        });

        let who = name.map_or("?", |n| n.as_str());
        let reach = edge.map_or("-".to_string(), |b| {
            let l = to_local(b);
            format!("{:.3}", Vec2::new(l.x, l.z).length() - spec.radius_at(l.y))
        });
        let blade_y = edge.map_or("-".to_string(), |b| format!("{:.2}", to_local(b).y));
        // The right hand in the user's own frame (feet origin, +Z forward), to compare with the
        // clip's impact pose (0, 1.06, 0.54), and the clip time of the strike.
        let hand = rig
            .zip(bank.as_deref())
            .and_then(|(rig, bank)| rig.bones.get(bank.bones.hand[1]))
            .and_then(|bone| globals.get(*bone).ok())
            .map_or("-".to_string(), |g| {
                let feet = user.translation - Vec3::Y * HALF_HEIGHT;
                let l = user.rotation.inverse() * (g.translation() - feet);
                format!("({:.2},{:.2},{:.2})", l.x, l.y, l.z)
            });
        let clip_t = using.map_or("-".to_string(), |u| format!("{:.2}", u.t % CHOP_DURATION));
        monitor.write(&format!(
            "{now:.2} {who} chop - {reach} Ground depth={:.2} blade_y={blade_y} hand={hand} clip_t={clip_t}",
            notch.depth
        ));

        if notch.depth >= FELL_DEPTH * radius {
            fell(&mut commands, &mut meshes, root, &mut tree, notch, radius);
            prop.regrow = REGROW;
            prop.shake = 0.0;
            bursts.write(Burst {
                kind: BurstKind::Leaves,
                at: root.transform_point(spec.canopy_center()),
                dir: -toward_user,
                count: 20,
            });
            monitor.write(&format!("{now:.2} {who} fell - {:.2} Ground cut_y={:.2}", notch.depth, notch.height));
        }
    }
}

/// Split everything above the notch into a dynamic body tipping away from the user.
fn fell(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    root: &Transform,
    tree: &mut Tree,
    notch: Notch,
    radius: f32,
) {
    let spec = tree.spec;
    let cut = tree.grid.layer_of(notch.height);
    let top = tree.grid.split_above(cut);
    let cut_y = tree.grid.origin.y + cut as f32 * tree.grid.cell;
    let _ = meshes.insert(&tree.trunk_mesh, tree.grid.smooth_mesh(&TRUNK_PALETTE, spec.seed, false));
    commands.entity(tree.collider).insert(trunk_collider(&spec, cut_y));

    let away = -(root.rotation * notch.dir).with_y(0.0).normalize_or(root.rotation * Vec3::Z);
    let axis = Vec3::Y.cross(away).normalize_or(Vec3::X);
    let log_bottom = cut_y + 0.03;
    let log_len = (spec.height - log_bottom).max(0.2);
    let canopy_at = spec.canopy_center();
    let collider = Collider::compound(vec![
        (
            Vec3::Y * (log_bottom + log_len / 2.0),
            Quat::IDENTITY,
            Collider::cylinder(spec.radius_at(log_bottom + log_len / 2.0), log_len),
        ),
        (canopy_at, Quat::IDENTITY, Collider::sphere(spec.canopy * 0.75)),
    ]);
    // Tip about the far edge of the cut: the body's centre swings around it.
    let pivot = root.transform_point(Vec3::Y * cut_y - notch.dir * radius);
    let centre = root.transform_point(canopy_at * 0.85);
    let spin = axis * TIP_SPEED;
    let body = commands
        .spawn((
            Name::new("Felled tree"),
            *root,
            Visibility::default(),
            RigidBody::Dynamic,
            collider,
            CollisionLayers::new(Phys::Debris, [Phys::World, Phys::Debris]),
            AngularVelocity(spin),
            LinearVelocity(spin.cross(centre - pivot)),
            AngularDamping(0.2),
            Felled {
                t: 0.0,
                landed: None,
                canopy_at,
                log: (log_bottom, log_bottom + log_len),
                seed: spec.seed,
            },
        ))
        .id();
    commands.spawn((
        Name::new("Felled trunk"),
        Mesh3d(meshes.add(top.smooth_mesh(&TRUNK_PALETTE, spec.seed, false))),
        MeshMaterial3d(tree.material.clone()),
        Transform::IDENTITY,
        ChildOf(body),
    ));
    commands.entity(tree.canopy).insert(ChildOf(body));
    tree.felled = true;
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn settle_felled(
    mut commands: Commands,
    time: Res<Time>,
    items: Option<Res<ItemAssets>>,
    mut shake: ResMut<CameraShake>,
    mut bursts: MessageWriter<Burst>,
    mut sounds: MessageWriter<Sfx>,
    mut monitor: ResMut<Monitor>,
    player: Query<&Transform, (With<Player>, Without<Felled>)>,
    mut bodies: Query<(Entity, &Transform, &mut Felled)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let player = player.single().ok().map(|p| p.translation);
    for (entity, tf, mut felled) in &mut bodies {
        felled.t += dt;
        let tilt = (tf.rotation * Vec3::Y).y.clamp(-1.0, 1.0).acos();
        if felled.landed.is_none() && (tilt > LANDED_TILT || felled.t > LAND_TIMEOUT) {
            felled.landed = Some(felled.t);
            let canopy = tf.transform_point(felled.canopy_at);
            sounds.write(Sfx {
                kind: SfxKind::TreeFall,
                at: canopy,
                volume: 1.2,
            });
            bursts.write(Burst {
                kind: BurstKind::Leaves,
                at: canopy,
                dir: Vec3::Y,
                count: 50,
            });
            if let Some(p) = player {
                shake.add(LAND_SHAKE * (1.0 - p.distance(canopy) / SHAKE_RANGE).clamp(0.0, 1.0));
            }
        }
        let Some(landed) = felled.landed else { continue };
        if felled.t - landed < BREAK_AFTER {
            continue;
        }
        // Break into logs along the trunk and sticks / apples from the crown.
        let Some(items) = items.as_deref() else { continue };
        let (bottom, top) = felled.log;
        let logs = ((top - bottom) / LOG_SPACING).floor().max(1.0) as i32;
        for i in 0..logs {
            let y = bottom + (i as f32 + 0.5) * (top - bottom) / logs as f32;
            let at = tf.transform_point(Vec3::Y * y) + Vec3::Y * 0.15;
            let pop = Vec3::new(hash3(i, 1, 0, felled.seed) - 0.5, 0.0, hash3(i, 2, 0, felled.seed) - 0.5);
            spawn_item(&mut commands, items, ItemKind::Log, at, tf.rotation, pop * 1.5 + Vec3::Y * 2.5);
            bursts.write(Burst {
                kind: BurstKind::Chips,
                at,
                dir: Vec3::Y,
                count: 6,
            });
        }
        let crown = tf.transform_point(felled.canopy_at);
        let sticks = 2 + (hash3(3, 3, 3, felled.seed) * 1.99) as i32;
        let apples = (hash3(4, 4, 4, felled.seed) * 2.99) as i32;
        for (k, kind) in std::iter::repeat_n(ItemKind::Stick, sticks as usize)
            .chain(std::iter::repeat_n(ItemKind::Apple, apples as usize))
            .enumerate()
        {
            let k = k as i32;
            let offset = Vec3::new(hash3(k, 5, 1, felled.seed) - 0.5, 0.3, hash3(k, 6, 1, felled.seed) - 0.5);
            let spin = Quat::from_rotation_y(hash3(k, 7, 1, felled.seed) * std::f32::consts::TAU)
                * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
            spawn_item(&mut commands, items, kind, crown + offset * 1.6, spin, offset * 2.0 + Vec3::Y * 2.0);
        }
        bursts.write(Burst {
            kind: BurstKind::Leaves,
            at: crown,
            dir: Vec3::Y,
            count: 40,
        });
        sounds.write(Sfx {
            kind: SfxKind::Chop,
            at: crown,
            volume: 0.8,
        });
        monitor.write(&format!(
            "{:.2} tree drop - {logs} - sticks={sticks} apples={apples}",
            time.elapsed_secs()
        ));
        commands.entity(entity).try_despawn();
    }
}

fn ease_out_back(x: f32) -> f32 {
    let (c1, c3) = (1.70158, 2.70158);
    let x = x.clamp(0.0, 1.0) - 1.0;
    1.0 + c3 * x * x * x + c1 * x * x
}

/// Regrowth after the regrow timer, the grow-in, and the canopy's chop shake. `trees` is the only
/// writer of a standing tree's part transforms (trunk, collider, canopy); `interact` only decays
/// `PropState.shake`.
fn regrow(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut trees: Query<(Entity, &mut Tree, &PropState)>,
    mut parts: Query<&mut Transform, Without<Tree>>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let now = time.elapsed_secs();
    for (root, mut tree, prop) in &mut trees {
        let spec = tree.spec;
        if tree.felled && prop.regrow <= 0.0 {
            tree.grid = build_trunk(&spec);
            let _ = meshes.insert(&tree.trunk_mesh, tree.grid.smooth_mesh(&TRUNK_PALETTE, spec.seed, false));
            // Full-size collider, scaled with the trunk below while it grows in.
            commands.entity(tree.collider).insert(trunk_collider(&spec, spec.height));
            let (mesh, material) = (tree.canopy_mesh.clone(), tree.material.clone());
            tree.canopy = spawn_canopy(&mut commands, root, mesh, &material, &spec, 0.01);
            tree.notch = None;
            tree.felled = false;
            tree.grow = 0.0;
        }
        if tree.felled {
            // The canopy now belongs to the falling body.
            continue;
        }
        let growing = tree.grow < 1.0;
        if growing {
            tree.grow = (tree.grow + dt / GROW_TIME).min(1.0);
        }
        let s = if tree.grow < 1.0 { ease_out_back(tree.grow) } else { 1.0 };
        let trunk_scale = GROW_FROM + (1.0 - GROW_FROM) * s;
        if growing {
            if let Ok(mut tf) = parts.get_mut(tree.trunk) {
                tf.scale = Vec3::splat(trunk_scale);
            }
            if let Ok(mut tf) = parts.get_mut(tree.collider) {
                tf.translation = Vec3::Y * (spec.height * trunk_scale / 2.0);
                tf.scale = Vec3::splat(trunk_scale);
            }
        }
        if let Ok(mut tf) = parts.get_mut(tree.canopy) {
            let shake = prop.shake;
            tf.translation = spec.canopy_center() * trunk_scale;
            tf.scale = Vec3::splat(s.max(0.01));
            tf.rotation = Quat::from_rotation_z(0.06 * shake * (17.0 * now).sin())
                * Quat::from_rotation_x(0.04 * shake * (13.0 * now).cos());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> TreeSpec {
        TreeSpec {
            radius: 0.33,
            height: 3.2,
            canopy: 1.6,
            seed: 7,
        }
    }

    #[test]
    fn trunk_has_bark_outside_and_wood_inside() {
        let s = spec();
        let g = build_trunk(&s);
        assert!(g.count() > 1000);
        let at = |p: Vec3| {
            let c = ((p - g.origin) / g.cell).floor().as_ivec3();
            g.get(c.x, c.y, c.z)
        };
        assert!(matches!(at(Vec3::new(0.0, 1.0, 0.0)), WOOD | RING), "heartwood");
        let edge = s.radius_at(1.0) - 0.5 * BARK_DEPTH;
        assert!(matches!(at(Vec3::new(edge, 1.0, 0.0)), BARK | BARK_DARK), "bark shell");
        assert_eq!(at(Vec3::new(s.radius * 1.3, 1.0, 0.0)), 0);
        assert!(s.radius_at(0.0) > s.radius_at(1.0) && s.radius_at(1.0) > s.radius_at(3.0));
        assert!(build_canopy(&s).count() > 500);
    }

    #[test]
    fn notch_deepens_from_the_user_side_and_fells() {
        let s = spec();
        let mut g = build_trunk(&s);
        let total = g.count();
        let r = s.radius_at(1.0);
        let mut notch = Notch {
            height: 1.0,
            dir: Vec3::NEG_Z,
            depth: 0.0,
        };
        let mut removed = Vec::new();
        let mut chops = 0;
        while notch.depth < FELL_DEPTH * r {
            notch.depth += CHOP_DEPTH;
            chops += 1;
            removed.push(g.carve(|p| in_notch(p, &notch, r)).0);
        }
        assert!((4..=7).contains(&chops), "{chops} chops to fell");
        assert!(removed.iter().all(|&n| n > 0), "{removed:?}");
        // Cut on the user's (−Z) side only; the far side (hinge) stands.
        assert!(in_notch(Vec3::new(0.0, 1.0, -r + 0.02), &notch, r));
        assert!(!in_notch(Vec3::new(0.0, 1.0, r - 0.02), &notch, r));
        assert!(!in_notch(Vec3::new(0.0, 1.5, -r + 0.02), &notch, r), "above the opening");
        let before = g.count();
        assert!(before < total);
        let top = g.split_above(g.layer_of(1.0));
        assert_eq!(top.count() + g.count(), before);
    }
}
