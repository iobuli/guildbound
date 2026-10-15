//! Independent hand grips, a timed disarm, and a momentum-preserving circular throw.
use crate::tuning::{Knob as K, Tuning};
use crate::{ranger, skeleton_ik as ik, terrain_motion as ground};
use avian3d::prelude::*;
use bevy::{
    prelude::*,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use std::collections::HashMap;

const ENEMY_MODEL: &str = "packs/npc/female_ranger.glb";
const HANDLE: Vec3 = Vec3::new(0.0, -0.28, 0.0);

fn smooth(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

#[derive(Clone, Copy)]
pub(crate) struct Arm {
    pub joints: [Entity; 3],
    pub lengths: [f32; 2],
    pub palm_axis: Vec3,
    pub palm_offset: Vec3,
    pub backhand_offset: Vec3,
}
pub(crate) struct Actor {
    pub position: Vec3,
    pub yaw: f32,
    pub grounded: bool,
    pub mass: f32,
    pub friction: f32,
    pub generation: u32,
    pub arms: [Arm; 2],
    pub velocity: Vec3,
    pub rolling: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct Drive {
    pub velocity: Vec3,
    pub yaw: f32,
}

struct Grip {
    body: Entity,
    local: Vec3,
    anchor: Entity,
    joint: Entity,
    label: String,
    age: f32,
    load: f32,
    overload: f32,
}
struct Orbit {
    center: Vec3,
    radial: Vec3,
    omega: f32,
    drift: Vec3,
}
struct Disarm {
    time: f32,
    wrist: Vec3,
    detached: bool,
    released: bool,
}
struct Punch {
    time: f32,
    duration: f32,
    hand: usize,
    hit: bool,
    previous: Vec3,
}
#[derive(Resource)]
pub(crate) struct Combat {
    pub drive: Option<Drive>,
    pub focus: Option<Vec3>,
    pub frame_radius: f32,
    pub lean: f32,
    pub brace: f32,
    held: [bool; 2],
    pressed: [bool; 2],
    spin: f32,
    carry: f32,
    disarm_requested: bool,
    grips: [Option<Grip>; 2],
    orbit: Option<Orbit>,
    disarm: Option<Disarm>,
    targets: [Option<Vec3>; 2],
    last_targets: [Vec3; 2],
    weights: [f32; 2],
    generation: u32,
    release_speed: f32,
    release_time: f32,
    message: String,
    equipped: bool,
    drop_requested: bool,
    punch_requested: bool,
    punch: Option<Punch>,
    punch_buffer: f32,
    next_hand: usize,
    spin_pump: f32,
    spin_sign: f32,
    last_tap: i8,
    tap_age: f32,
}
impl Default for Combat {
    fn default() -> Self {
        Self {
            drive: None,
            focus: None,
            frame_radius: 0.0,
            lean: 0.0,
            brace: 0.0,
            held: [false; 2],
            pressed: [false; 2],
            spin: 0.0,
            carry: 0.0,
            disarm_requested: false,
            grips: [None, None],
            orbit: None,
            disarm: None,
            targets: [None; 2],
            last_targets: [Vec3::ZERO; 2],
            weights: [0.0; 2],
            generation: 0,
            release_speed: 0.0,
            release_time: 0.0,
            message: "MB4 disarms and equips. Hold MB5 for a two-handed grip; release to throw."
                .into(),
            equipped: false,
            drop_requested: false,
            punch_requested: false,
            punch: None,
            punch_buffer: 0.0,
            next_hand: 0,
            spin_pump: 0.0,
            spin_sign: 1.0,
            last_tap: 0,
            tap_age: 10.0,
        }
    }
}
#[derive(Component)]
struct Opponent;
#[derive(Component)]
struct Loaded {
    nodes: Vec<Entity>,
    players: Vec<Entity>,
}
#[derive(Component)]
struct CombatStatus;
struct Part {
    body: Entity,
    bone: Entity,
    parent: Option<usize>,
    center: Vec3,
    half_axis: Vec3,
    radius: f32,
    mass: f32,
    label: String,
}
#[derive(Resource)]
struct Enemy {
    root: Entity,
    rest: Vec<(Entity, Transform)>,
    players: Vec<Entity>,
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    parts: Vec<Part>,
    joints: Vec<Entity>,
    dynamic: bool,
    armed: bool,
    weapon: Entity,
    weapon_joint: Option<Entity>,
    wrist: Entity,
    arm: Arm,
    weapon_offset: Vec3,
    weapon_rotation: Quat,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Combat>()
        .add_systems(Startup, spawn)
        .add_systems(Update, (input.after(ranger::Stage::Input), hud, draw))
        .add_systems(FixedUpdate, step.before(ranger::Stage::Motor))
        .add_systems(
            PostUpdate,
            (prepare_enemy, pose_enemy)
                .chain()
                .after(bevy::app::AnimationSystems)
                .before(ranger::Stage::Pose),
        )
        .add_systems(
            PostUpdate,
            pose_hands
                .in_set(Stage::Hands)
                .after(ranger::Stage::Pose)
                .before(bevy::transform::TransformSystems::Propagate),
        );
}
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Stage {
    Hands,
}
impl Combat {
    pub(crate) fn finger_curl(&self) -> [f32; 2] {
        if self.grips[0].is_some() {
            return [1.0; 2];
        }
        let mut curl = [0.0, if self.equipped { 0.8 } else { 0.0 }];
        if let Some(p) = &self.punch {
            curl[p.hand] = 1.0;
        }
        if self.disarm.is_some() {
            curl = [0.65; 2];
        }
        curl
    }

    pub(crate) fn upper_body_active(&self) -> bool {
        self.punch.is_some()
            || self.disarm.is_some()
            || self.grips.iter().any(Option::is_some)
            || self.release_time > 0.0
    }

    pub(crate) fn punch_phase(&self) -> Option<(usize, f32)> {
        self.punch
            .as_ref()
            .map(|p| (p.hand, (p.time / p.duration).clamp(0.0, 1.0)))
    }
}

fn spawn(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(ENEMY_MODEL));
    commands.insert_resource(EnemyAsset(assets.load(ENEMY_MODEL)));
    commands
        .spawn((
            Name::new("Armed female Ranger opponent"),
            Opponent,
            WorldAssetRoot(scene),
            Transform::from_xyz(-2.55, ground::height(-2.55, 0.0), 0.0)
                .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
        ))
        .observe(loaded);
    // A separate prop: the characters themselves are the user's skinned Ranger models.
    let steel = materials.add(Color::srgb(0.72, 0.79, 0.84));
    let hilt = materials.add(Color::srgb(0.72, 0.39, 0.12));
    let weapon = commands
        .spawn((
            Name::new("Disarmable training sword"),
            Visibility::default(),
            RigidBody::Kinematic,
            Mass(1.1),
            Collider::cuboid(0.075, 0.76, 0.035),
            Friction::new(0.8),
            Restitution::new(0.05),
            CollisionLayers::new(8u32, 1u32 | 2u32),
            SweptCcd::NON_LINEAR,
            Transform::from_xyz(-2.43, 1.0, 0.0),
        ))
        .id();
    commands.entity(weapon).with_children(|p| {
        p.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.065, 0.57, 0.02))),
            MeshMaterial3d(steel),
            Transform::from_xyz(0.0, 0.08, 0.0),
        ));
        p.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.23, 0.035, 0.045))),
            MeshMaterial3d(hilt.clone()),
            Transform::from_xyz(0.0, -0.21, 0.0),
        ));
        p.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.022, 0.12))),
            MeshMaterial3d(hilt),
            Transform::from_xyz(0.0, -0.29, 0.0),
        ));
    });
    commands.insert_resource(WeaponEntity(weapon));
    commands.spawn((
        CombatStatus,
        Text::new("Loading combat opponent..."),
        TextFont {
            font_size: bevy::text::FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(122.0),
            max_width: px(650.0),
            ..default()
        },
    ));
}
#[derive(Resource)]
struct WeaponEntity(Entity);
#[derive(Resource)]
struct EnemyAsset(Handle<bevy::gltf::Gltf>);
fn loaded(
    event: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    players: Query<(), With<AnimationPlayer>>,
) {
    let nodes: Vec<_> = children.iter_descendants(event.entity).collect();
    let players = nodes
        .iter()
        .copied()
        .filter(|e| players.contains(*e))
        .collect();
    commands
        .entity(event.entity)
        .insert(Loaded { nodes, players });
}

fn prepare_enemy(world: &mut World) {
    if world.contains_resource::<Enemy>() {
        return;
    }
    let mut q = world.query_filtered::<(Entity, &Loaded), With<Opponent>>();
    let Some((root, loaded)) = q.iter(world).next() else {
        return;
    };
    if loaded.players.is_empty() {
        return;
    }
    let nodes = loaded.nodes.clone();
    let players = loaded.players.clone();
    let names: HashMap<_, _> = nodes
        .iter()
        .filter_map(|&e| world.get::<Name>(e).map(|n| (n.to_string(), e)))
        .collect();
    let asset = world.resource::<EnemyAsset>().0.clone();
    let gltfs = world.resource::<Assets<bevy::gltf::Gltf>>();
    let Some(gltf) = gltfs.get(&asset) else {
        return;
    };
    let Some(idle) = gltf.named_animations.get("npc_idle") else {
        return;
    };
    let (graph, indices) = AnimationGraph::from_clips([idle.clone()]);
    let idle = indices[0];
    let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
    let rest = nodes
        .iter()
        .filter_map(|&e| world.get::<Transform>(e).map(|t| (e, *t)))
        .collect();
    let specs = [
        ("B_Pelvis", "B_Spine", None, 0.125, 16.0, "pelvis"),
        ("B_Spine2", "B_Neck", Some(0), 0.15, 20.0, "chest"),
        ("B_Head", "", Some(1), 0.10, 5.0, "head"),
        (
            "B_L_UpperArm",
            "B_L_Forearm",
            Some(1),
            0.055,
            3.0,
            "left upper arm",
        ),
        (
            "B_L_Forearm",
            "B_L_Forearm.001",
            Some(3),
            0.045,
            2.0,
            "left forearm",
        ),
        (
            "B_R_UpperArm",
            "B_R_Forearm",
            Some(1),
            0.055,
            3.0,
            "right upper arm",
        ),
        (
            "B_R_Forearm",
            "B_R_Hand",
            Some(5),
            0.045,
            2.0,
            "right forearm",
        ),
        ("B_L_Thigh", "B_L_Calf", Some(0), 0.075, 8.0, "left thigh"),
        ("B_L_Calf", "B_L_Foot", Some(7), 0.06, 5.0, "left shin"),
        ("B_R_Thigh", "B_R_Calf", Some(0), 0.075, 8.0, "right thigh"),
        ("B_R_Calf", "B_R_Foot", Some(9), 0.06, 5.0, "right shin"),
        ("B_L_Foot", "B_L_Toe0", Some(8), 0.055, 1.2, "left foot"),
        ("B_R_Foot", "B_R_Toe0", Some(10), 0.055, 1.2, "right foot"),
    ];
    let mut parts = Vec::new();
    for (bone, end, parent, radius, mass, label) in specs {
        let bone = names[bone];
        let pose = ranger::world_transform(world, bone).unwrap();
        let (_, rotation, start) = pose.to_scale_rotation_translation();
        let end = names
            .get(end)
            .and_then(|&e| ranger::world_transform(world, e))
            .map(|p| p.translation())
            .unwrap_or(start + Vec3::Y * 0.18);
        let delta = rotation.inverse() * (end - start);
        let center = delta * 0.5;
        let half_axis = delta.normalize_or(Vec3::Y) * (delta.length() * 0.5 - radius).max(0.005);
        let mass = mass * 65.0 / 79.4;
        let position = start + rotation * center;
        let body = world
            .spawn((
                Name::new(format!("Opponent {label}")),
                RigidBody::Kinematic,
                Collider::capsule_endpoints(radius, -half_axis, half_axis),
                Mass(mass),
                Friction::new(0.8),
                LinearDamping(0.015),
                AngularDamping(0.08),
                MaxAngularSpeed(30.0),
                SleepingDisabled,
                SweptCcd::NON_LINEAR,
                CollisionLayers::new(4u32, 1u32 | 2u32),
                Position(position),
                Rotation(rotation),
                Transform::from_translation(position).with_rotation(rotation),
            ))
            .id();
        parts.push(Part {
            body,
            bone,
            parent,
            center,
            half_axis,
            radius,
            mass,
            label: label.into(),
        });
    }
    for &p in &players {
        world
            .entity_mut(p)
            .insert(AnimationGraphHandle(graph.clone()));
        world
            .get_mut::<AnimationPlayer>(p)
            .unwrap()
            .play(idle)
            .repeat();
    }
    let wrist = names["B_R_Hand"];
    let (_, rotation, _) = ranger::world_transform(world, wrist)
        .unwrap()
        .to_scale_rotation_translation();
    let joints = [names["B_R_UpperArm"], names["B_R_Forearm"], wrist];
    let p = joints.map(|e| ranger::world_transform(world, e).unwrap().translation());
    let enemy = Enemy {
        root,
        rest,
        players,
        graph,
        idle,
        parts,
        joints: Vec::new(),
        dynamic: false,
        armed: true,
        weapon: world.resource::<WeaponEntity>().0,
        weapon_joint: None,
        wrist,
        arm: Arm {
            joints,
            lengths: [p[0].distance(p[1]), p[1].distance(p[2])],
            palm_axis: Vec3::Y,
            palm_offset: Vec3::Y * 0.08,
            backhand_offset: Vec3::new(0.0, 0.08, -0.035),
        },
        weapon_offset: rotation.inverse() * Vec3::Y * 0.28,
        weapon_rotation: rotation.inverse(),
    };
    world.insert_resource(enemy);
    println!(
        "[combat] female Ranger ready: 13 fitted body parts, independent grips, disarmable sword"
    );
}

fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    ui: Query<&Interaction, With<Button>>,
    time: Res<Time>,
    mut combat: ResMut<Combat>,
) {
    combat.tap_age += time.delta_secs();
    if combat.grips[0].is_some() {
        let tap = if keys.just_pressed(KeyCode::KeyD) {
            1
        } else if keys.just_pressed(KeyCode::KeyA) {
            -1
        } else {
            0
        };
        if tap != 0 && (tap != combat.last_tap || combat.tap_age > 0.8) {
            if combat.tap_age > 0.8 || combat.last_tap == 0 {
                combat.spin_sign = tap as f32;
            }
            combat.spin_pump = (combat.spin_pump + 0.32).min(1.0);
            combat.last_tap = tap;
            combat.tap_age = 0.0;
        }
    } else {
        combat.spin_pump = 0.0;
        combat.last_tap = 0;
    }
    let mut held = [
        mouse.pressed(MouseButton::Forward) || keys.pressed(KeyCode::KeyE),
        false,
    ];
    let mut pressed = [
        mouse.just_pressed(MouseButton::Forward) || keys.just_pressed(KeyCode::KeyE),
        false,
    ];
    let mut disarm = mouse.just_pressed(MouseButton::Back) || keys.just_pressed(KeyCode::KeyF);
    let mut punch =
        mouse.just_pressed(MouseButton::Left) && !ui.iter().any(|i| *i != Interaction::None);
    let mut drop_weapon = keys.just_pressed(KeyCode::KeyV);
    let mut spin =
        (keys.pressed(KeyCode::KeyD) as u8 as f32) - (keys.pressed(KeyCode::KeyA) as u8 as f32);
    let mut carry =
        (keys.pressed(KeyCode::KeyW) as u8 as f32) - (keys.pressed(KeyCode::KeyS) as u8 as f32);
    for pad in &pads {
        held[0] |= pad.pressed(GamepadButton::LeftTrigger);
        pressed[0] |= pad.just_pressed(GamepadButton::LeftTrigger);
        punch |= pad.just_pressed(GamepadButton::RightTrigger);
        drop_weapon |= pad.just_pressed(GamepadButton::DPadDown);
        disarm |= pad.just_pressed(GamepadButton::North);
        let x = pad.left_stick().x;
        if pad.left_stick().y.abs() > carry.abs() {
            carry = pad.left_stick().y;
        }
        if x.abs() > spin.abs() {
            spin = if x.abs() > 0.15 {
                (x.abs() - 0.15) / 0.85 * x.signum()
            } else {
                0.0
            };
        }
    }
    combat.held = held;
    for i in 0..2 {
        combat.pressed[i] |= pressed[i];
    }
    combat.disarm_requested |= disarm;
    combat.punch_requested |= punch;
    combat.drop_requested |= drop_weapon;
    combat.spin = spin;
    combat.carry = carry;
}

fn move_kinematic(world: &mut World, entity: Entity, position: Vec3, rotation: Quat, dt: f32) {
    let from = world.get::<Position>(entity).unwrap().0;
    let current = world.get::<Rotation>(entity).unwrap().0;
    world.get_mut::<LinearVelocity>(entity).unwrap().0 =
        ((position - from) / dt).clamp_length_max(12.0);
    let delta = (rotation * current.inverse()).normalize();
    let (axis, angle) = delta.to_axis_angle();
    let angle =
        (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    world.get_mut::<AngularVelocity>(entity).unwrap().0 =
        (axis * angle / dt).clamp_length_max(20.0);
}

fn activate(world: &mut World, enemy: &mut Enemy) {
    if enemy.dynamic {
        return;
    }
    for &player in &enemy.players {
        world.entity_mut(player).remove::<AnimationGraphHandle>();
    }
    for part in &enemy.parts {
        let (_, rotation, anchor) = ranger::world_transform(world, part.bone)
            .unwrap()
            .to_scale_rotation_translation();
        let position = anchor + rotation * part.center;
        world.entity_mut(part.body).insert((
            RigidBody::Dynamic,
            Position(position),
            Rotation(rotation),
            Transform::from_translation(position).with_rotation(rotation),
        ));
    }
    for (i, part) in enemy.parts.iter().enumerate() {
        let Some(parent) = part.parent else {
            continue;
        };
        let parent = &enemy.parts[parent];
        let anchor = ranger::world_transform(world, part.bone)
            .unwrap()
            .translation();
        let joint = if i == 8 || i == 10 {
            let p0 = ranger::world_transform(world, parent.bone)
                .unwrap()
                .translation();
            let p2 = ranger::world_transform(world, enemy.parts[if i == 8 { 11 } else { 12 }].bone)
                .unwrap()
                .translation();
            let bend = (anchor - p0)
                .normalize_or(Vec3::NEG_Y)
                .angle_between((p2 - anchor).normalize_or(Vec3::NEG_Y));
            let forward = world.get::<Transform>(enemy.root).unwrap().rotation * Vec3::Z;
            world
                .spawn((
                    RevoluteJoint::new(parent.body, part.body)
                        .with_anchor(anchor)
                        .with_hinge_axis(Vec3::Y.cross(forward))
                        .with_angle_limits(-bend, 2.35 - bend),
                    JointCollisionDisabled,
                ))
                .id()
        } else {
            let limit = if i <= 2 {
                0.65
            } else if i >= 11 {
                0.6
            } else {
                1.7
            };
            world
                .spawn((
                    SphericalJoint::new(parent.body, part.body)
                        .with_anchor(anchor)
                        .with_basis(Quat::IDENTITY)
                        .with_swing_limits(0.0, limit)
                        .with_twist_limits(-0.75, 0.75),
                    JointCollisionDisabled,
                ))
                .id()
        };
        enemy.joints.push(joint);
    }
    if enemy.armed {
        world.entity_mut(enemy.weapon).insert(RigidBody::Dynamic);
        let anchor = weapon_handle(world, enemy);
        enemy.weapon_joint = Some(
            world
                .spawn((
                    FixedJoint::new(enemy.parts[6].body, enemy.weapon)
                        .with_anchor(anchor)
                        .with_basis(Quat::IDENTITY),
                    JointCollisionDisabled,
                ))
                .id(),
        );
    }
    enemy.dynamic = true;
}

fn attach(world: &mut World, body: Entity, point: Vec3, label: String) -> Grip {
    let position = world.get::<Position>(body).unwrap().0;
    let rotation = world.get::<Rotation>(body).unwrap().0;
    let local = rotation.inverse() * (point - position);
    let anchor = world
        .spawn((
            Name::new("Independent hand grip anchor"),
            RigidBody::Kinematic,
            Position(point),
            Rotation::IDENTITY,
            Transform::from_translation(point),
        ))
        .id();
    let mut constraint = SphericalJoint::new(anchor, body)
        .with_local_anchor1(Vec3::ZERO)
        .with_local_anchor2(local);
    constraint.point_compliance = 2e-6;
    let joint = world
        .spawn((constraint, JointForces::new(), JointCollisionDisabled))
        .id();
    Grip {
        body,
        local,
        anchor,
        joint,
        label,
        age: 0.0,
        load: 0.0,
        overload: 0.0,
    }
}
fn detach(world: &mut World, grip: Grip) {
    world.despawn(grip.joint);
    world.despawn(grip.anchor);
    // Deliberately leave both velocities untouched. The constraint has already supplied the momentum.
}
fn grip_point(world: &World, grip: &Grip) -> Vec3 {
    world.get::<Position>(grip.body).unwrap().0
        + world.get::<Rotation>(grip.body).unwrap().0 * grip.local
}
fn weapon_handle(world: &World, enemy: &Enemy) -> Vec3 {
    world.get::<Position>(enemy.weapon).unwrap().0
        + world.get::<Rotation>(enemy.weapon).unwrap().0 * HANDLE
}
fn com(world: &World, enemy: &Enemy) -> (Vec3, Vec3, f32) {
    let mut position = Vec3::ZERO;
    let mut velocity = Vec3::ZERO;
    let mut total = 0.0;
    for p in &enemy.parts {
        position += world.get::<Position>(p.body).unwrap().0 * p.mass;
        velocity += world.get::<LinearVelocity>(p.body).unwrap().0 * p.mass;
        total += p.mass;
    }
    (position / total, velocity / total, total)
}

fn reset(world: &mut World, enemy: &mut Enemy, combat: &mut Combat) {
    for grip in &mut combat.grips {
        if let Some(grip) = grip.take() {
            detach(world, grip);
        }
    }
    for joint in enemy.joints.drain(..) {
        world.despawn(joint);
    }
    if let Some(joint) = enemy.weapon_joint.take() {
        world.despawn(joint);
    }
    for &(entity, transform) in &enemy.rest {
        *world.get_mut::<Transform>(entity).unwrap() = transform;
    }
    for part in &enemy.parts {
        let (_, rotation, p) = ranger::world_transform(world, part.bone)
            .unwrap()
            .to_scale_rotation_translation();
        let p = p + rotation * part.center;
        world.entity_mut(part.body).insert((
            RigidBody::Kinematic,
            Position(p),
            Rotation(rotation),
            LinearVelocity::ZERO,
            AngularVelocity::ZERO,
            Transform::from_translation(p).with_rotation(rotation),
        ));
    }
    for &player in &enemy.players {
        world
            .entity_mut(player)
            .insert(AnimationGraphHandle(enemy.graph.clone()));
        world
            .get_mut::<AnimationPlayer>(player)
            .unwrap()
            .play(enemy.idle)
            .repeat();
    }
    let (_, rotation, p) = ranger::world_transform(world, enemy.wrist)
        .unwrap()
        .to_scale_rotation_translation();
    let p = p + rotation * enemy.weapon_offset;
    let rotation = rotation * enemy.weapon_rotation;
    world.entity_mut(enemy.weapon).insert((
        RigidBody::Kinematic,
        Position(p),
        Rotation(rotation),
        LinearVelocity::ZERO,
        AngularVelocity::ZERO,
        Transform::from_translation(p).with_rotation(rotation),
    ));
    enemy.dynamic = false;
    enemy.armed = true;
    *combat = Combat {
        generation: combat.generation,
        ..default()
    };
}

fn surface(world: &World, part: &Part, point: Vec3) -> Vec3 {
    let center = world.get::<Position>(part.body).unwrap().0;
    let rotation = world.get::<Rotation>(part.body).unwrap().0;
    let a = center - rotation * part.half_axis;
    let line = rotation * part.half_axis * 2.0;
    let t = ((point - a).dot(line) / line.length_squared().max(1e-6)).clamp(0.0, 1.0);
    let axis = a + line * t;
    axis + (point - axis).normalize_or(Vec3::Z) * part.radius
}

fn closest_pair(world: &World, enemy: &Enemy, actor: &Actor, space: f32) -> Option<(usize, Vec3)> {
    let palms = actor.arms.map(|a| {
        ranger::world_transform(world, a.joints[2])
            .unwrap()
            .translation()
    });
    let center = (palms[0] + palms[1]) * 0.5;
    enemy
        .parts
        .iter()
        .enumerate()
        .filter_map(|(i, part)| {
            let point = surface(world, part, center);
            let reachable = actor.arms.iter().all(|a| {
                ranger::world_transform(world, a.joints[0])
                    .unwrap()
                    .translation()
                    .distance(point)
                    < a.lengths.iter().sum::<f32>() * 0.98 - space * 0.2
            });
            (reachable && point.distance(center) < 0.85).then_some((
                i,
                point,
                point.distance_squared(center),
            ))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(i, p, _)| (i, p))
}

fn release_body(world: &mut World, enemy: &Enemy, combat: &mut Combat) {
    if let Some(grip) = combat.grips[0].take() {
        detach(world, grip);
        combat.release_speed = com(world, enemy).1.length();
        combat.release_time = 1.8;
    }
    combat.orbit = None;
    combat.drive = None;
}

fn step(world: &mut World) {
    let Some(actor) = ranger::combat_actor(world) else {
        return;
    };
    let Some(mut enemy) = world.remove_resource::<Enemy>() else {
        return;
    };
    let mut combat = world.remove_resource::<Combat>().unwrap();
    let feel = world.resource::<Tuning>().clone();
    let dt = world.resource::<Time>().delta_secs().max(1e-6);
    combat.drive = None;
    combat.lean = 0.0;
    combat.brace = 0.0;
    combat.targets = [None; 2];
    combat.spin_pump *= (-0.7 * dt).exp();
    combat.release_time = (combat.release_time - dt).max(0.0);
    combat.punch_buffer = (combat.punch_buffer - dt).max(0.0);
    if actor.generation != combat.generation {
        combat.generation = actor.generation;
        reset(world, &mut enemy, &mut combat);
        world
            .entity_mut(enemy.weapon)
            .insert(CollisionLayers::new(8u32, 1u32 | 2u32));
        world.insert_resource(enemy);
        world.insert_resource(combat);
        return;
    }
    if !enemy.dynamic {
        for part in &enemy.parts {
            let (_, rotation, p) = ranger::world_transform(world, part.bone)
                .unwrap()
                .to_scale_rotation_translation();
            move_kinematic(world, part.body, p + rotation * part.center, rotation, dt);
        }
    }
    let forward = Quat::from_rotation_y(actor.yaw) * Vec3::Z;
    let side = Vec3::Y.cross(forward);
    if std::mem::take(&mut combat.drop_requested) && combat.equipped {
        combat.equipped = false;
        world
            .entity_mut(enemy.weapon)
            .insert((RigidBody::Dynamic, CollisionLayers::new(8u32, 1u32 | 2u32)));
        combat.message = "Weapon dropped with its current hand velocity.".into();
    }
    if std::mem::take(&mut combat.disarm_requested) && !actor.rolling {
        if combat.equipped {
            combat.message = "Sword equipped. V / D-pad down drops it.".into();
        } else if combat.grips[0].is_some() {
            combat.message = "Release the body before disarming.".into();
        } else if !enemy.armed {
            combat.message = "No held weapon. R resets the opponent.".into();
        } else if combat.disarm.is_none() {
            let wrist = ranger::world_transform(world, enemy.wrist)
                .unwrap()
                .translation();
            let reachable = actor.arms.iter().all(|a| {
                ranger::world_transform(world, a.joints[0])
                    .unwrap()
                    .translation()
                    .distance(wrist)
                    < a.lengths.iter().sum::<f32>() * 0.99
            });
            if reachable && actor.grounded {
                combat.disarm = Some(Disarm {
                    time: 0.0,
                    wrist,
                    detached: false,
                    released: false,
                });
                combat.punch = None;
                combat.message =
                    "Disarm: control wrist, strip weapon, equip it. Movement stays available."
                        .into();
            } else {
                combat.message = "Step within both hands' reach, then MB4 / F / Y.".into();
            }
        }
    }
    if std::mem::take(&mut combat.pressed[0])
        && combat.grips[0].is_none()
        && combat.disarm.is_none()
        && actor.grounded
        && !actor.rolling
    {
        if let Some((part, point)) = closest_pair(world, &enemy, &actor, feel.get(K::HandSpace)) {
            activate(world, &mut enemy);
            combat.grips[0] = Some(attach(
                world,
                enemy.parts[part].body,
                point,
                format!("{} / both hands", enemy.parts[part].label),
            ));
            let radius = feel.get(K::StepRadius);
            combat.orbit = Some(Orbit {
                center: actor.position - forward * radius,
                radial: forward * radius,
                omega: 0.0,
                drift: actor.velocity,
            });
            combat.punch = None;
            combat.message="Both hands on one ball joint. A/D spins; W/S moves the pivot; release MB5 to throw.".into();
        } else {
            combat.message =
                "No shared body surface in both hands' reach. Step closer, then MB5.".into();
        }
    }
    if combat.grips[0].is_some() && (!combat.held[0] || !actor.grounded || actor.rolling) {
        release_body(world, &enemy, &mut combat);
        combat.message =
            "Thrown: constraint removed; actual linear and angular momentum retained.".into();
    }
    if combat.grips[0].is_some() {
        let orbit = combat.orbit.as_mut().unwrap();
        let radius = orbit.radial.length();
        let mut inertia = actor.mass * radius * radius;
        let mut radial_mass = actor.mass * radius;
        for part in &enemy.parts {
            let r = (world.get::<Position>(part.body).unwrap().0 - orbit.center)
                .with_y(0.0)
                .length();
            inertia += part.mass * (r * r + part.half_axis.length_squared() / 3.0);
            radial_mass += part.mass * r;
        }
        let traction = actor.friction * actor.mass * 9.81;
        let limit = (traction / radial_mass.max(1.0)).sqrt().min(4.0);
        let centripetal = orbit.omega * orbit.omega * radial_mass;
        let enemy_pull = orbit.omega * orbit.omega * (radial_mass - actor.mass * radius);
        combat.lean = -(enemy_pull / (actor.mass * 9.81)).atan().min(0.6) * 0.5;
        combat.brace = 0.04 + 0.045 * (orbit.omega.abs() / limit.max(0.01));
        let alpha = radius
            * (traction * traction - centripetal * centripetal)
                .max(0.0)
                .sqrt()
            / inertia.max(1.0);
        let command = if combat.spin_pump > 0.08 {
            combat.spin_sign * combat.spin_pump
        } else {
            combat.spin
        };
        let wanted = if command.abs() > 0.01 {
            command * alpha * feel.get(K::SpinPower)
        } else {
            -orbit.omega * feel.get(K::SpinDrag)
        };
        orbit.omega = (orbit.omega + wanted.clamp(-alpha, alpha) * dt).clamp(-limit, limit);
        orbit.radial = Quat::from_rotation_y(orbit.omega * dt) * orbit.radial;
        orbit.drift *= (-4.0 * dt).exp();
        let current_radius = orbit.radial.length();
        orbit.radial = orbit.radial.normalize_or(forward)
            * (current_radius
                + (feel.get(K::StepRadius) - current_radius) * (1.0 - (-6.0 * dt).exp()));
        orbit.center += ground::safe_travel(
            actor.position,
            (forward * combat.carry * feel.get(K::CarryMove) + orbit.drift) * dt,
            0.25,
        );
        let wanted = orbit.center + orbit.radial;
        let velocity =
            ground::safe_travel(actor.position, (wanted - actor.position).with_y(0.0), 0.25) / dt;
        combat.drive = Some(Drive {
            velocity: velocity.clamp_length_max(3.0),
            yaw: orbit.radial.x.atan2(orbit.radial.z),
        });
        let radial = orbit.radial.normalize_or(forward);
        let shoulders = actor.arms.map(|a| {
            ranger::world_transform(world, a.joints[0])
                .unwrap()
                .translation()
        });
        let reach = actor
            .arms
            .iter()
            .map(|a| a.lengths.iter().sum::<f32>())
            .fold(f32::MAX, f32::min)
            * 0.78;
        let goal = (shoulders[0] + shoulders[1]) * 0.5 + radial * reach - Vec3::Y * 0.11;
        let grip = combat.grips[0].as_mut().unwrap();
        let point = world.get::<Position>(grip.anchor).unwrap().0;
        move_kinematic(
            world,
            grip.anchor,
            point.lerp(goal, 1.0 - (-feel.get(K::HandFollow) * dt).exp()),
            Quat::IDENTITY,
            dt,
        );
        grip.age += dt;
        let force = world
            .get::<JointForces>(grip.joint)
            .map(|f| f.force().length())
            .unwrap_or(0.0);
        grip.load += (force - grip.load) * (1.0 - (-8.0 * dt).exp());
        let point = grip_point(world, grip);
        let too_far = actor.arms.iter().any(|a| {
            ranger::world_transform(world, a.joints[0])
                .unwrap()
                .translation()
                .distance(point)
                > a.lengths.iter().sum::<f32>() + 0.15
        });
        if grip.age > 0.3 && (grip.load > feel.get(K::GripForce) || too_far) {
            grip.overload += dt;
        } else {
            grip.overload = 0.0;
        }
        let part = enemy.parts.iter().find(|p| p.body == grip.body).unwrap();
        for i in 0..2 {
            combat.targets[i] = Some(surface(
                world,
                part,
                point + side * feel.get(K::HandSpace) * if i == 0 { 0.5 } else { -0.5 },
            ));
        }
        if grip.overload > 0.12 {
            release_body(world, &enemy, &mut combat);
            combat.message = "Grip overloaded; released without adding a throw impulse.".into();
        }
    }
    if let Some(mut disarm) = combat.disarm.take() {
        disarm.time += dt;
        let u = ((disarm.time - 0.18) / 0.4).clamp(0.0, 1.0);
        let ease = u * u * (3.0 - 2.0 * u);
        combat.targets[0] = Some(disarm.wrist + Vec3::Y * 0.035 * ease);
        if !disarm.detached && disarm.time >= 0.58 {
            enemy.armed = false;
            if let Some(j) = enemy.weapon_joint.take() {
                world.despawn(j);
            }
            world.entity_mut(enemy.weapon).insert(RigidBody::Dynamic);
            let point = weapon_handle(world, &enemy);
            combat.grips[1] = Some(attach(world, enemy.weapon, point, "sword hilt".into()));
            disarm.detached = true;
        }
        if !disarm.detached {
            combat.targets[1] = Some(disarm.wrist + side * 0.04 + Vec3::Y * 0.03 * ease);
        } else if !disarm.released {
            let u = ((disarm.time - 0.58) / 0.4).clamp(0.0, 1.0);
            let u = u * u * (3.0 - 2.0 * u);
            let arm = actor.arms[1];
            let shoulder = ranger::world_transform(world, arm.joints[0])
                .unwrap()
                .translation();
            let guard = shoulder + forward * 0.2 - Vec3::Y * 0.2;
            let goal = disarm.wrist.lerp(guard, u);
            if let Some(grip) = &combat.grips[1] {
                move_kinematic(world, grip.anchor, goal, Quat::IDENTITY, dt);
                combat.targets[1] = Some(grip_point(world, grip));
            }
            if disarm.time >= 0.98 {
                if let Some(grip) = combat.grips[1].take() {
                    detach(world, grip);
                }
                combat.equipped = true;
                world
                    .entity_mut(enemy.weapon)
                    .insert((RigidBody::Kinematic, CollisionLayers::new(8u32, 0u32)));
                disarm.released = true;
                combat.message =
                    "Disarmed and equipped. Move freely; LMB punches, V drops the weapon.".into();
            }
        }
        if disarm.time < 1.2 {
            combat.disarm = Some(disarm);
        }
    }
    if enemy.armed && !enemy.dynamic {
        let (_, rotation, wrist) = ranger::world_transform(world, enemy.wrist)
            .unwrap()
            .to_scale_rotation_translation();
        let (wrist, twist) = combat
            .disarm
            .as_ref()
            .map(|d| {
                let u = ((d.time - 0.18) / 0.4).clamp(0.0, 1.0);
                (
                    d.wrist + Vec3::Y * 0.035 * u,
                    Quat::from_axis_angle(forward, 1.35 * u),
                )
            })
            .unwrap_or((wrist, Quat::IDENTITY));
        let rotation = twist * rotation * enemy.weapon_rotation;
        move_kinematic(world, enemy.weapon, wrist - rotation * HANDLE, rotation, dt);
    }
    if combat.equipped && combat.grips[0].is_none() && !actor.rolling && combat.disarm.is_none() {
        let shoulder = ranger::world_transform(world, actor.arms[1].joints[0])
            .unwrap()
            .translation();
        combat.targets[1] = Some(shoulder + forward * 0.22 - Vec3::Y * 0.22);
    }
    let punch_pressed = std::mem::take(&mut combat.punch_requested);
    if punch_pressed && combat.punch.is_some() {
        // The combat framework's input buffer: remember intent through active/recovery frames.
        combat.punch_buffer = 0.22;
    }
    if punch_pressed
        && combat.punch.is_none()
        && combat.grips[0].is_none()
        && combat.disarm.is_none()
        && !actor.rolling
        && actor.grounded
    {
        let hand = if combat.equipped { 0 } else { combat.next_hand };
        combat.next_hand = 1 - hand;
        combat.punch = Some(Punch {
            time: 0.0,
            duration: feel.get(K::PunchTime),
            hand,
            hit: false,
            previous: {
                let arm = actor.arms[hand];
                let pose = ranger::world_transform(world, arm.joints[2]).unwrap();
                let (_, rotation, wrist) = pose.to_scale_rotation_translation();
                wrist + rotation * arm.palm_offset.lerp(arm.backhand_offset, 0.12)
            },
        });
    }
    if let Some(mut punch) = combat.punch.take() {
        punch.time += dt;
        let u = (punch.time / punch.duration).min(1.0);
        let arm = actor.arms[punch.hand];
        let shoulder = ranger::world_transform(world, arm.joints[0])
            .unwrap()
            .translation();
        let guard = shoulder + forward * 0.16 - Vec3::Y * 0.1;
        let (start, peak, end, source_extension) = {
            let reference = world.resource::<crate::reference::Reference>();
            (
                0.2 + (reference.strike_start - 0.2) * feel.get(K::Reference),
                0.48 + (reference.strike_peak - 0.48) * feel.get(K::Reference),
                0.65 + (reference.strike_end - 0.65) * feel.get(K::Reference),
                reference.strike_extension,
            )
        };
        let measured_reach = arm.lengths.iter().sum::<f32>() + arm.palm_offset.length();
        let source_scaled = source_extension.min(measured_reach * 1.04);
        let reach = source_scaled.lerp(measured_reach * 0.98, feel.get(K::DimensionAdapt));
        let extension = if u < start {
            -0.08 * (u / start.max(0.01))
        } else if u < peak {
            let t = (u - start) / (peak - start).max(0.01);
            -0.08 + t * t * (3.0 - 2.0 * t) * (reach - 0.08)
        } else {
            let t = (u - peak) / (1.0 - peak);
            (1.0 - t * t * (3.0 - 2.0 * t)) * (reach - 0.16)
        };
        let line_target = guard + forward * extension;
        let head = &enemy.parts[2];
        let head_position = world.get::<Position>(head.body).unwrap().0;
        let head_rotation = world.get::<Rotation>(head.body).unwrap().0;
        let axis = head_rotation * head.half_axis;
        let crown = if axis.y >= 0.0 {
            head_position + axis
        } else {
            head_position - axis
        } + Vec3::Y * head.radius;
        let chin = if axis.y >= 0.0 {
            head_position - axis
        } else {
            head_position + axis
        } - Vec3::Y * head.radius;
        let target_landmark = chin.lerp(crown, 0.12);
        let landmark_weight = feel.get(K::HandTarget)
            * smooth(((u - start) / (peak - start).max(0.01)).clamp(0.0, 1.0))
            * smooth(((end - u) / (end - peak).max(0.01)).clamp(0.0, 1.0));
        let hand_contact = arm.palm_offset.lerp(arm.backhand_offset, 0.12);
        let fist_offset = forward * hand_contact.length();
        let wanted_wrist = target_landmark - fist_offset;
        let reachable_wrist = shoulder
            + (wanted_wrist - shoulder).clamp_length_max(arm.lengths.iter().sum::<f32>() * 0.985);
        let target = line_target.lerp(reachable_wrist, landmark_weight);
        let contact_point = target + fist_offset;
        combat.targets[punch.hand] = Some(target);
        combat.lean += 0.10 * (std::f32::consts::PI * u).sin();
        if !punch.hit && (start..end.max(start + 0.01)).contains(&u) {
            let mut impact = None;
            for j in 0..=12 {
                let p = punch.previous.lerp(contact_point, j as f32 / 12.0);
                if let Some((i, _)) = enemy
                    .parts
                    .iter()
                    .enumerate()
                    .map(|(i, part)| (i, surface(world, part, p).distance(p)))
                    .filter(|(_, d)| *d < 0.07)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                {
                    impact = Some((i, p));
                    break;
                }
            }
            if let Some((i, point)) = impact {
                activate(world, &mut enemy);
                let impulse = (forward + Vec3::Y * 0.1).normalize() * feel.get(K::PunchImpulse);
                let mut forces = world.query::<Forces>();
                if let Ok(mut force) = forces.get_mut(world, enemy.parts[i].body) {
                    force.apply_linear_impulse_at_point(impulse, point);
                }
                punch.hit = true;
                combat.message = format!(
                    "Punch hit {}: {:.1} Ns",
                    enemy.parts[i].label,
                    feel.get(K::PunchImpulse)
                );
            }
        }
        punch.previous = contact_point;
        if u < 1.0 && !actor.rolling {
            combat.punch = Some(punch);
        } else if combat.punch_buffer > 0.0 && !actor.rolling {
            combat.punch_requested = true;
            combat.punch_buffer = 0.0;
        }
    }
    let (position, _, _) = com(world, &enemy);
    let active = combat.grips[0].is_some() || combat.disarm.is_some() || combat.release_time > 0.0;
    combat.focus = Some(if active {
        actor.position.lerp(position, 0.42)
    } else {
        actor.position
    });
    combat.frame_radius = if active {
        (position - actor.position).with_y(0.0).length().min(5.0)
    } else {
        0.0
    };
    world.insert_resource(enemy);
    world.insert_resource(combat);
}

fn pose_enemy(world: &mut World) {
    let Some(enemy) = world.remove_resource::<Enemy>() else {
        return;
    };
    if enemy.dynamic {
        for (i, part) in enemy.parts.iter().enumerate() {
            let rotation = world.get::<Rotation>(part.body).unwrap().0;
            if i == 0 {
                let anchor = world.get::<Position>(part.body).unwrap().0 - rotation * part.center;
                let parent = world.get::<ChildOf>(part.bone).unwrap().parent();
                let inverse = ranger::world_transform(world, parent)
                    .unwrap()
                    .affine()
                    .inverse();
                world.get_mut::<Transform>(part.bone).unwrap().translation =
                    inverse.transform_point3(anchor);
            }
            ik::rotate_world(world, part.bone, rotation);
        }
    } else if enemy.armed || world.resource::<Combat>().disarm.is_some() {
        let root = world.get::<Transform>(enemy.root).unwrap();
        let fwd = root.rotation * Vec3::Z;
        let target = world
            .resource::<Combat>()
            .disarm
            .as_ref()
            .map(|d| d.wrist + Vec3::Y * 0.035)
            .unwrap_or(root.translation + Vec3::Y * 1.22 + fwd * 0.2 - Vec3::Y.cross(fwd) * 0.08);
        ik::apply(
            world,
            enemy.arm.joints,
            target,
            Vec3::NEG_Y,
            fwd,
            enemy.arm.lengths,
        );
    }
    world.insert_resource(enemy);
}

fn pose_hands(world: &mut World) {
    let Some(actor) = ranger::combat_actor(world) else {
        return;
    };
    let mut combat = world.remove_resource::<Combat>().unwrap();
    let dt = world.resource::<Time>().delta_secs().min(0.05);
    let forward = Quat::from_rotation_y(actor.yaw) * Vec3::Z;
    for i in 0..2 {
        let wanted = if combat.targets[i].is_some() {
            1.0
        } else {
            0.0
        };
        if let Some(target) = combat.targets[i] {
            combat.last_targets[i] = target;
        }
        combat.weights[i] += (wanted - combat.weights[i]) * (1.0 - (-16.0 * dt).exp());
        if combat.weights[i] < 0.001 {
            continue;
        }
        let arm = actor.arms[i];
        let pose = arm
            .joints
            .map(|e| ranger::world_transform(world, e).unwrap());
        let shoulder = pose[0].translation();
        let elbow = pose[1].translation();
        let wrist = pose[2].translation();
        let target = wrist.lerp(combat.last_targets[i], combat.weights[i]);
        let axis = (target - shoulder).normalize_or(forward);
        let pole =
            (elbow - shoulder - axis * (elbow - shoulder).dot(axis)).normalize_or(Vec3::NEG_Y);
        ik::apply(world, arm.joints, target, pole, Vec3::NEG_Y, arm.lengths);
        // Aim the calibrated palm direction into the grip without assuming identity bone roll.
        let aim_center = combat.grips[0]
            .as_ref()
            .or(combat.grips[i].as_ref())
            .as_ref()
            .map(|g| world.get::<Position>(g.body).unwrap().0)
            .or_else(|| {
                combat.disarm.as_ref().map(|_| {
                    let enemy = world.resource::<Enemy>();
                    world
                        .get::<Position>(if i == 0 {
                            enemy.parts[6].body
                        } else {
                            enemy.weapon
                        })
                        .unwrap()
                        .0
                })
            });
        if let Some(center) = aim_center {
            let (_, rotation, _) = ranger::world_transform(world, arm.joints[2])
                .unwrap()
                .to_scale_rotation_translation();
            let aim = (center - target).normalize_or(forward);
            let before = rotation * arm.palm_axis;
            let (axis, angle) = Quat::from_rotation_arc(before, aim).to_axis_angle();
            ik::rotate_world(
                world,
                arm.joints[2],
                Quat::from_axis_angle(axis, angle.min(70f32.to_radians()) * combat.weights[i])
                    * rotation,
            );
        }
    }
    if combat.equipped {
        let dt = dt.max(1e-6);
        let enemy = world.resource::<Enemy>();
        let weapon = enemy.weapon;
        let stowed = combat.grips[0].is_some() || actor.rolling;
        let frame = ranger::combat_frame(world).unwrap();
        let frame_rotation = frame.to_scale_rotation_translation().1;
        let rotation = frame_rotation * Quat::from_rotation_z(if stowed { -0.35 } else { -0.15 });
        let point = if stowed {
            let shoulders = actor.arms.map(|a| {
                ranger::world_transform(world, a.joints[0])
                    .unwrap()
                    .translation()
            });
            (shoulders[0] + shoulders[1]) * 0.5 - frame_rotation * Vec3::Z * 0.18
        } else {
            ranger::world_transform(world, actor.arms[1].joints[2])
                .unwrap()
                .translation()
        };
        let position = point - rotation * HANDLE;
        let previous = world.get::<Position>(weapon).unwrap().0;
        let old_rotation = world.get::<Rotation>(weapon).unwrap().0;
        let (axis, angle) = (rotation * old_rotation.inverse()).to_axis_angle();
        let angle =
            (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        world.entity_mut(weapon).insert((
            Position(position),
            Rotation(rotation),
            LinearVelocity(((position - previous) / dt).clamp_length_max(12.0)),
            AngularVelocity((axis * angle / dt).clamp_length_max(25.0)),
            Transform::from_translation(position).with_rotation(rotation),
        ));
    }
    world.insert_resource(combat);
}

fn hud(combat: Res<Combat>, mut texts: Query<&mut Text, With<CombatStatus>>) {
    let hands: Vec<_> = (0..2)
        .map(|i| {
            combat.grips[0]
                .as_ref()
                .or(combat.grips[i].as_ref())
                .as_ref()
                .map(|g| format!("{} ({:.0} N)", g.label, g.load))
                .unwrap_or(if combat.equipped && i == 1 {
                    "sword equipped".into()
                } else {
                    "free".into()
                })
        })
        .collect();
    let omega = combat.orbit.as_ref().map(|o| o.omega).unwrap_or(0.0);
    if let Ok(mut text) = texts.single_mut() {
        text.0 = format!(
            "Left: {} | Right: {}\nSpin {:.2} rad/s | last release {:.2} m/s\n{}",
            hands[0], hands[1], omega, combat.release_speed, combat.message
        );
    }
}
fn draw(combat: Res<Combat>, positions: Query<&Position>, mut gizmos: Gizmos) {
    for i in 0..2 {
        if let Some(grip) = &combat.grips[i] {
            if let Ok(position) = positions.get(grip.anchor) {
                gizmos.cross(
                    Isometry3d::from_translation(position.0),
                    0.07,
                    if i == 0 {
                        Color::srgb(0.15, 1.0, 0.5)
                    } else {
                        Color::srgb(1.0, 0.6, 0.1)
                    },
                );
            }
        }
    }
    if let Some(orbit) = &combat.orbit {
        gizmos.linestrip(
            (0..=64).map(|i| {
                let a = i as f32 * std::f32::consts::TAU / 64.0;
                let radius = orbit.radial.length();
                let p = orbit.center + Vec3::new(a.sin() * radius, 0.0, a.cos() * radius);
                p.with_y(ground::height(p.x, p.z) + 0.02)
            }),
            Color::srgb(1.0, 0.65, 0.15),
        );
    }
}
