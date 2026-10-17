//! World items: logs, sticks, stones, apples. `items` owns every `Item` entity.
//!
//! Drops pop out as small physics bodies on `Phys::Debris` (characters and ground probes ignore
//! them) and freeze in place (`Resting`, static) once they settle, so nothing rolls down a hill
//! forever. After `PICKUP_DELAY`, an item within `MAGNET_RADIUS` of the player is swapped for a
//! visual stand-in that flies to the chest (when the bag has room; otherwise `BagFull`) and is
//! offered to the bag with `ItemCollected`; `inventory` answers `ItemRejected` if it no longer
//! fits, and the item drops back at the player's feet. Sticks, stones and apples lie at rest on
//! the hills (`Spot`s, placed static on the slope), each coming back `RESPAWN` seconds after it
//! is collected.
use super::fx::{Sfx, SfxKind};
use super::inventory::Inventory;
use super::voxel::hash3;
use super::world::{COURTYARD_HALF, POND_CENTER, POND_RADIUS, ground_height, ground_normal};
use super::{HALF_HEIGHT, Phys, Player};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Log,
    Stick,
    Stone,
    Apple,
}

impl ItemKind {
    pub const ALL: [ItemKind; 4] = [ItemKind::Log, ItemKind::Stick, ItemKind::Stone, ItemKind::Apple];

    pub fn name(self) -> &'static str {
        match self {
            ItemKind::Log => "Log",
            ItemKind::Stick => "Stick",
            ItemKind::Stone => "Stone",
            ItemKind::Apple => "Apple",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ItemKind::Log => "A length of fresh wood. Burns long, builds well.",
            ItemKind::Stick => "A dry branch. Kindling, or a handle.",
            ItemKind::Stone => "A fist-sized stone. Good for ringing a fire.",
            ItemKind::Apple => "Crisp and sweet. Better roasted.",
        }
    }

    /// Most of this kind one bag slot holds.
    pub fn stack(self) -> u32 {
        match self {
            ItemKind::Log | ItemKind::Apple => 20,
            ItemKind::Stick | ItemKind::Stone => 50,
        }
    }

    /// Tile colour in the bag.
    pub fn color(self) -> Color {
        match self {
            ItemKind::Log => Color::srgb(0.55, 0.38, 0.22),
            ItemKind::Stick => Color::srgb(0.40, 0.29, 0.18),
            ItemKind::Stone => Color::srgb(0.52, 0.52, 0.50),
            ItemKind::Apple => Color::srgb(0.74, 0.16, 0.12),
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// An item lying (or flying) in the world.
#[derive(Component, Clone, Copy, Debug)]
pub struct Item {
    pub kind: ItemKind,
    /// Seconds since it was dropped.
    age: f32,
}

/// A visual-only stand-in for an item on its way into the player's bag (the physical item is
/// despawned, so no physics state is left fighting the flight).
#[derive(Component, Clone, Copy, Debug)]
struct Flying {
    kind: ItemKind,
    t: f32,
    from: Vec3,
    scale: Vec3,
    to: Entity,
}

/// A place on the hills where an item lies; it comes back after being collected.
#[derive(Component, Clone, Copy, Debug)]
struct Spot {
    kind: ItemKind,
    place: Transform,
    item: Option<Entity>,
    timer: f32,
}

/// A dropped item that has come to rest and was frozen in place (`RigidBody::Static`), so
/// round things can't roll down the hills forever.
#[derive(Component)]
struct Resting;

/// An item reached `who` and is offered to its bag.
#[derive(Message, Clone, Copy, Debug)]
pub struct ItemCollected {
    pub who: Entity,
    pub kind: ItemKind,
    pub count: u32,
}

/// The bag could not take `count` of `kind`: drop them back at `who`'s feet.
#[derive(Message, Clone, Copy, Debug)]
pub struct ItemRejected {
    pub who: Entity,
    pub kind: ItemKind,
    pub count: u32,
}

/// Something in the magnet's reach has no room in `who`'s bag (sent at most every
/// `FULL_NOTICE` seconds), so `inventory` can say so.
#[derive(Message, Clone, Copy, Debug)]
pub struct BagFull {
    pub who: Entity,
}

/// Hill tree trunks (world positions) to scatter apples under; inserted by `world`.
#[derive(Resource, Clone, Debug, Default)]
pub struct HillTrees(pub Vec<Vec3>);

#[derive(Resource)]
pub struct ItemAssets {
    meshes: [Handle<Mesh>; 4],
    materials: [Handle<StandardMaterial>; 4],
    stone: Collider,
    stem: (Handle<Mesh>, Handle<StandardMaterial>),
}

const LOG_RADIUS: f32 = 0.12;
pub const LOG_LENGTH: f32 = 0.7;
const STICK_RADIUS: f32 = 0.025;
const STICK_LENGTH: f32 = 0.55;
const STONE_RADIUS: f32 = 0.09;
const APPLE_RADIUS: f32 = 0.06;
/// Seconds before a drop can be collected (it pops out and settles first), magnet reach (m),
/// flight time (s) and the arc height of the flight (m).
const PICKUP_DELAY: f32 = 0.6;
const MAGNET_RADIUS: f32 = 2.5;
const FLY_TIME: f32 = 0.35;
const FLY_ARC: f32 = 0.35;
/// Where on the player items fly to (m above the feet).
const CHEST: f32 = 1.2;
/// Seconds before a collected hill item comes back.
const RESPAWN: f32 = 60.0;
/// A drop freezes in place once it is older than `SETTLE_MIN` and slower than `SETTLE_SPEED`
/// (m/s) and `SETTLE_SPIN` (rad/s), or anyway at `SETTLE_MAX` (s); one that fell more than
/// `KILL_DEPTH` (m) under the terrain is removed.
const SETTLE_MIN: f32 = 1.5;
const SETTLE_SPEED: f32 = 0.15;
const SETTLE_SPIN: f32 = 0.5;
const SETTLE_MAX: f32 = 8.0;
const KILL_DEPTH: f32 = 5.0;
/// Seconds between "bag full" notices while something in reach doesn't fit.
const FULL_NOTICE: f32 = 3.0;
/// Hill scatter: counts, ring (m from the centre), keep-outs and flatness.
const STICKS: u32 = 14;
const STONES: u32 = 12;
const SCATTER_MIN: f32 = COURTYARD_HALF + 3.0;
const SCATTER_MAX: f32 = 42.0;
const MIN_FLATNESS: f32 = 0.85;

pub fn plugin(app: &mut App) {
    app.add_message::<ItemCollected>()
        .add_message::<ItemRejected>()
        .add_message::<BagFull>()
        .init_resource::<HillTrees>()
        .add_systems(Startup, load)
        .add_systems(Update, (scatter, magnet, fly, rejected, settle, respawn).chain());
}

/// Bark sides and ringed pale end caps, by normal.
fn log_mesh() -> Mesh {
    let mut mesh = Cylinder::new(LOG_RADIUS, LOG_LENGTH).mesh().resolution(9).build();
    let positions: Vec<[f32; 3]> = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|p| p.as_float3())
        .map(<[_]>::to_vec)
        .unwrap_or_default();
    let normals: Vec<[f32; 3]> = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(bevy::mesh::VertexAttributeValues::Float32x3(n)) => n.clone(),
        _ => vec![[0.0, 1.0, 0.0]; positions.len()],
    };
    let bark = Color::srgb(0.36, 0.25, 0.16).to_linear();
    let wood = Color::srgb(0.86, 0.70, 0.48).to_linear();
    let colors: Vec<[f32; 4]> = positions
        .iter()
        .zip(&normals)
        .enumerate()
        .map(|(i, (p, n))| {
            if n[1].abs() > 0.5 {
                let r = Vec2::new(p[0], p[2]).length() / LOG_RADIUS;
                let k = if r > 0.85 { 0.45 } else if ((r * 3.0) as i32) % 2 == 1 { 0.85 } else { 1.0 };
                [wood.red * k, wood.green * k, wood.blue * k, 1.0]
            } else {
                let k = 0.85 + 0.3 * hash3(i as i32, 0, 0, 7);
                [bark.red * k, bark.green * k, bark.blue * k, 1.0]
            }
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let mut solid = |c: Color, rough: f32| {
        mats.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: rough,
            ..default()
        })
    };
    let stone_mesh = Sphere::new(STONE_RADIUS)
        .mesh()
        .ico(1)
        .unwrap_or_else(|_| Sphere::new(STONE_RADIUS).into())
        .scaled_by(Vec3::new(1.0, 0.65, 0.85));
    let stone = Collider::convex_hull_from_mesh(&stone_mesh).unwrap_or_else(|| Collider::sphere(STONE_RADIUS * 0.7));
    let materials = [
        solid(Color::WHITE, 0.9),
        solid(Color::srgb(0.33, 0.23, 0.14), 0.9),
        solid(Color::srgb(0.50, 0.49, 0.47), 0.95),
        solid(Color::srgb(0.78, 0.15, 0.10), 0.45),
    ];
    let stem = (
        meshes.add(Cylinder::new(0.006, 0.04)),
        solid(Color::srgb(0.30, 0.22, 0.12), 0.9),
    );
    let assets = ItemAssets {
        meshes: [
            meshes.add(log_mesh()),
            meshes.add(Cylinder::new(STICK_RADIUS, STICK_LENGTH).mesh().resolution(6)),
            meshes.add(stone_mesh),
            meshes.add(Sphere::new(APPLE_RADIUS).mesh().uv(12, 8)),
        ],
        materials,
        stone,
        stem,
    };
    commands.insert_resource(assets);
}

fn collider(kind: ItemKind, assets: &ItemAssets) -> Collider {
    match kind {
        ItemKind::Log => Collider::cylinder(LOG_RADIUS, LOG_LENGTH),
        ItemKind::Stick => Collider::cuboid(STICK_RADIUS * 2.0, STICK_LENGTH, STICK_RADIUS * 2.0),
        ItemKind::Stone => assets.stone.clone(),
        ItemKind::Apple => Collider::sphere(APPLE_RADIUS),
    }
}

/// Spawn a loose item; it can be collected once it has been out for `PICKUP_DELAY`.
pub fn spawn_item(
    commands: &mut Commands,
    assets: &ItemAssets,
    kind: ItemKind,
    at: Vec3,
    rotation: Quat,
    velocity: Vec3,
) -> Entity {
    let i = kind.index();
    let mut e = commands.spawn((
        Name::new(kind.name()),
        Item { kind, age: 0.0 },
        Mesh3d(assets.meshes[i].clone()),
        MeshMaterial3d(assets.materials[i].clone()),
        Transform::from_translation(at).with_rotation(rotation),
        RigidBody::Dynamic,
        collider(kind, assets),
        CollisionLayers::new(Phys::Debris, [Phys::World, Phys::Debris]),
        LinearVelocity(if velocity.is_finite() { velocity } else { Vec3::ZERO }),
        LinearDamping(0.2),
        AngularDamping(if kind == ItemKind::Log { 0.8 } else { 2.0 }),
        Friction::new(0.9),
    ));
    if kind == ItemKind::Apple {
        let (mesh, material) = assets.stem.clone();
        e.with_child((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, APPLE_RADIUS + 0.015, 0.0),
        ));
    }
    e.id()
}

/// Height (m) of an item's centre above the ground it rests on (sticks and logs lie down).
fn rest_height(kind: ItemKind) -> f32 {
    match kind {
        ItemKind::Log => LOG_RADIUS,
        ItemKind::Stick => STICK_RADIUS,
        ItemKind::Stone => STONE_RADIUS * 0.65,
        ItemKind::Apple => APPLE_RADIUS,
    }
}

/// An item resting on the ground at `ground`, tilted onto the slope `normal`, turned `yaw`.
pub fn placed(kind: ItemKind, ground: Vec3, normal: Vec3, yaw: f32) -> Transform {
    let normal = normal.normalize_or(Vec3::Y);
    let tilt = Quat::from_rotation_arc(Vec3::Y, normal);
    let lying = if matches!(kind, ItemKind::Stick | ItemKind::Log) {
        Quat::from_rotation_z(PI / 2.0)
    } else {
        Quat::IDENTITY
    };
    Transform::from_translation(ground + normal * rest_height(kind))
        .with_rotation(tilt * Quat::from_rotation_y(yaw) * lying)
}

/// An item placed at rest (static, collectible at once): hill items and their respawns.
fn spawn_placed(commands: &mut Commands, assets: &ItemAssets, kind: ItemKind, place: Transform) -> Entity {
    let i = kind.index();
    let mut e = commands.spawn((
        Name::new(kind.name()),
        Item { kind, age: PICKUP_DELAY },
        Resting,
        Mesh3d(assets.meshes[i].clone()),
        MeshMaterial3d(assets.materials[i].clone()),
        place,
        RigidBody::Static,
        collider(kind, assets),
        CollisionLayers::new(Phys::Debris, [Phys::World, Phys::Debris]),
    ));
    if kind == ItemKind::Apple {
        let (mesh, material) = assets.stem.clone();
        e.with_child((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, APPLE_RADIUS + 0.015, 0.0),
        ));
    }
    e.id()
}

/// Hill scatter positions for `count` items of one kind, `salt` keeping kinds apart.
pub fn scatter_points(count: u32, salt: u32) -> Vec<Vec3> {
    let mut out = Vec::new();
    let mut i = 0;
    while (out.len() as u32) < count && i < count * 12 {
        let a = i as f32 * 2.399_963 + salt as f32 * 0.7;
        let r = SCATTER_MIN + (SCATTER_MAX - SCATTER_MIN) * hash3(i as i32, salt as i32, 3, 11);
        i += 1;
        let (x, z) = (a.cos() * r, a.sin() * r);
        let pond = Vec2::new(x, z).distance(POND_CENTER) < POND_RADIUS + 2.5;
        let yard = x.abs().max(z.abs()) < COURTYARD_HALF + 1.0;
        if pond || yard || ground_normal(x, z).y < MIN_FLATNESS {
            continue;
        }
        out.push(Vec3::new(x, ground_height(x, z), z));
    }
    out
}

/// Once, when the assets exist: the hill items and their spots.
fn scatter(
    mut commands: Commands,
    assets: Option<Res<ItemAssets>>,
    trees: Res<HillTrees>,
    mut done: Local<bool>,
) {
    let Some(assets) = assets else { return };
    if *done {
        return;
    }
    *done = true;
    let mut spots: Vec<(ItemKind, Vec3)> = Vec::new();
    spots.extend(scatter_points(STICKS, 1).into_iter().map(|p| (ItemKind::Stick, p)));
    spots.extend(scatter_points(STONES, 2).into_iter().map(|p| (ItemKind::Stone, p)));
    for (k, trunk) in trees.0.iter().enumerate() {
        let apples = 1 + (hash3(k as i32, 4, 9, 13) * 2.99) as i32;
        for j in 0..apples {
            let a = hash3(k as i32, j, 1, 17) * TAU;
            let r = 0.7 + hash3(k as i32, j, 2, 17) * 0.9;
            let (x, z) = (trunk.x + a.cos() * r, trunk.z + a.sin() * r);
            spots.push((ItemKind::Apple, Vec3::new(x, ground_height(x, z), z)));
        }
    }
    for (n, (kind, at)) in spots.into_iter().enumerate() {
        let yaw = hash3(n as i32, 5, 5, 19) * TAU;
        let place = placed(kind, at, ground_normal(at.x, at.z), yaw);
        let item = spawn_placed(&mut commands, &assets, kind, place);
        commands.spawn((
            Name::new("Item spot"),
            Spot {
                kind,
                place,
                item: Some(item),
                timer: 0.0,
            },
        ));
    }
}

fn chest(player: &Transform) -> Vec3 {
    player.translation + Vec3::Y * (CHEST - HALF_HEIGHT)
}

/// Settled items near the player start flying to it, if the bag can take them (counting the
/// ones already on their way).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn magnet(
    mut commands: Commands,
    time: Res<Time>,
    assets: Option<Res<ItemAssets>>,
    mut items: Query<(Entity, &mut Item, &Transform)>,
    players: Query<(Entity, &Transform, &Inventory), (With<Player>, Without<Item>)>,
    flying: Query<&Flying>,
    mut full: MessageWriter<BagFull>,
    mut notice: Local<f32>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    *notice = (*notice - dt).max(0.0);
    for (_, mut item, _) in &mut items {
        item.age += dt;
    }
    let (Some(assets), Ok((player, ptf, bag))) = (assets, players.single()) else {
        return;
    };
    let reach = chest(ptf);
    let mut incoming = bag.clone();
    for f in &flying {
        incoming.add(f.kind, 1);
    }
    for (entity, item, tf) in &items {
        if item.age < PICKUP_DELAY || tf.translation.distance(reach) > MAGNET_RADIUS {
            continue;
        }
        if !incoming.fits(item.kind, 1) {
            if *notice <= 0.0 {
                *notice = FULL_NOTICE;
                full.write(BagFull { who: player });
            }
            continue;
        }
        incoming.add(item.kind, 1);
        let i = item.kind.index();
        commands.entity(entity).try_despawn();
        commands.spawn((
            Name::new("Collecting"),
            Flying {
                kind: item.kind,
                t: 0.0,
                from: tf.translation,
                scale: tf.scale,
                to: player,
            },
            Mesh3d(assets.meshes[i].clone()),
            MeshMaterial3d(assets.materials[i].clone()),
            *tf,
        ));
    }
}

/// Flying items arc into the player's chest, shrinking; on arrival they are offered to the bag.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut flying: Query<(Entity, &mut Flying, &mut Transform)>,
    targets: Query<&Transform, (With<Player>, Without<Flying>)>,
    mut collected: MessageWriter<ItemCollected>,
    mut sounds: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, mut fly, mut tf) in &mut flying {
        let Ok(target) = targets.get(fly.to) else {
            commands.entity(entity).try_despawn();
            continue;
        };
        fly.t += dt / FLY_TIME;
        let s = fly.t.clamp(0.0, 1.0);
        let eased = s * s;
        let to = chest(target);
        tf.translation = fly.from.lerp(to, eased) + Vec3::Y * (FLY_ARC * (PI * s).sin());
        tf.scale = fly.scale * (1.0 - 0.7 * s);
        if fly.t >= 1.0 {
            collected.write(ItemCollected {
                who: fly.to,
                kind: fly.kind,
                count: 1,
            });
            sounds.write(Sfx {
                kind: SfxKind::Collect,
                at: to,
                volume: 0.6,
            });
            commands.entity(entity).try_despawn();
        }
    }
}

/// What the bag refused drops back at the collector's feet.
fn rejected(
    mut commands: Commands,
    assets: Option<Res<ItemAssets>>,
    mut refused: MessageReader<ItemRejected>,
    who: Query<&Transform>,
) {
    let Some(assets) = assets else {
        refused.clear();
        return;
    };
    for r in refused.read() {
        let Ok(tf) = who.get(r.who) else { continue };
        let forward = tf.rotation * Vec3::Z;
        for k in 0..r.count.min(20) {
            let at = tf.translation + forward * 0.6 + Vec3::Y * (0.2 * k as f32 - HALF_HEIGHT + 0.4);
            spawn_item(&mut commands, &assets, r.kind, at, Quat::IDENTITY, forward * 1.5 + Vec3::Y * 2.0);
        }
    }
}

/// Dropped items freeze in place once they come to rest (or after `SETTLE_MAX`); items that fell
/// through the world are removed.
#[allow(clippy::type_complexity)]
fn settle(
    mut commands: Commands,
    items: Query<(Entity, &Item, &Transform, Option<&LinearVelocity>, Option<&AngularVelocity>), Without<Resting>>,
) {
    for (entity, item, tf, linear, angular) in &items {
        let p = tf.translation;
        if !p.is_finite() || p.y < ground_height(p.x, p.z) - KILL_DEPTH {
            commands.entity(entity).try_despawn();
            continue;
        }
        let speed = linear.map_or(0.0, |v| v.0.length());
        let spin = angular.map_or(0.0, |w| w.0.length());
        let still = item.age > SETTLE_MIN && speed < SETTLE_SPEED && spin < SETTLE_SPIN;
        if still || item.age > SETTLE_MAX {
            commands.entity(entity).try_insert((RigidBody::Static, Resting));
        }
    }
}

/// Collected hill items come back after `RESPAWN` seconds.
fn respawn(
    mut commands: Commands,
    time: Res<Time>,
    assets: Option<Res<ItemAssets>>,
    mut spots: Query<&mut Spot>,
    alive: Query<(), With<Item>>,
) {
    let Some(assets) = assets else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for mut spot in &mut spots {
        match spot.item {
            Some(e) if alive.contains(e) => {}
            Some(_) => {
                spot.item = None;
                spot.timer = RESPAWN;
            }
            None => {
                spot.timer -= dt;
                if spot.timer <= 0.0 {
                    spot.item = Some(spawn_placed(&mut commands, &assets, spot.kind, spot.place));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hill_scatter_avoids_pond_and_courtyard() {
        for (count, salt) in [(STICKS, 1), (STONES, 2)] {
            let points = scatter_points(count, salt);
            assert_eq!(points.len() as u32, count);
            for p in points {
                assert!(Vec2::new(p.x, p.z).distance(POND_CENTER) >= POND_RADIUS + 2.5);
                assert!(p.x.abs().max(p.z.abs()) >= COURTYARD_HALF + 1.0);
                assert!((p.y - ground_height(p.x, p.z)).abs() < 1e-4);
            }
        }
        assert_ne!(scatter_points(5, 1), scatter_points(5, 2));
    }

    #[test]
    fn kinds_are_described() {
        for kind in ItemKind::ALL {
            assert!(!kind.name().is_empty() && !kind.description().is_empty());
            assert!(kind.stack() > 0);
            assert_eq!(ItemKind::ALL[kind.index()], kind);
        }
    }
}
