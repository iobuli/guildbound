//! A skinned Ranger with a constrained character motor, native animation and terrain support.
use crate::tuning::{Knob as K, Tuning};
use crate::{skeleton_ik as ik, terrain_motion as ground};
use avian3d::prelude::*;
use bevy::{
    gltf::Gltf,
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    ui::RelativeCursorPosition,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use std::{collections::HashMap, time::Duration};

const HALF: f32 = 0.85;
struct ModelChoice {
    label: &'static str,
    path: &'static str,
}

#[derive(Resource)]
struct ModelCatalog {
    choices: Vec<ModelChoice>,
    selected: usize,
    pending: i32,
}
impl Default for ModelCatalog {
    fn default() -> Self {
        Self {
            choices: vec![
                ModelChoice {
                    label: "Male Ranger",
                    path: "packs/npc/male_ranger.glb",
                },
                ModelChoice {
                    label: "Female Ranger",
                    path: "packs/npc/female_ranger.glb",
                },
                ModelChoice {
                    label: "Male Peasant",
                    path: "packs/npc/male_peasant.glb",
                },
                ModelChoice {
                    label: "Female Peasant",
                    path: "packs/npc/female_peasant.glb",
                },
            ],
            selected: 0,
            pending: 0,
        }
    }
}

#[derive(Resource)]
pub struct Settings {
    speed: f32,
    mass: f32,
    support: f32,
    crouch: f32,
    friction: f32,
    walking: bool,
    move_input: Vec3,
    running: bool,
    roll: bool,
    jump: bool,
    jump_held: bool,
    reset: bool,
    traces: bool,
    drag: Option<usize>,
    generation: u32,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_target_yaw: f32,
    camera_target_pitch: f32,
    camera_distance: f32,
    aim_direction: Vec3,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            speed: 1.3,
            mass: 75.0,
            support: 1.0,
            crouch: 0.0,
            friction: 0.8,
            walking: false,
            move_input: Vec3::ZERO,
            running: false,
            roll: false,
            jump: false,
            jump_held: false,
            reset: false,
            traces: true,
            drag: None,
            generation: 0,
            camera_yaw: 0.0,
            camera_pitch: 0.23,
            camera_target_yaw: 0.0,
            camera_target_pitch: 0.23,
            camera_distance: 6.5,
            aim_direction: Vec3::NEG_Z,
        }
    }
}

#[derive(Component, Default)]
struct Character {
    mover: crate::live_movement::Mover,
    phase: f32,
    last_position: Option<Vec3>,
    horizontal: Vec3,
    vy: f32,
    yaw: f32,
    direction: f32,
    dwell: f32,
    grounded: bool,
    charge: f32,
    landing: f32,
    acceleration: Vec3,
    run_blend: f32,
    roll: Option<Roll>,
    impact: f32,
    height: f32,
    yaw_rate: f32,
    charging: bool,
    push: f32,
    jump_power: f32,
    air_age: f32,
    air_origin: Vec3,
    air_velocity: Vec3,
    air_angles: [Vec2; 4],
    air_rates: [Vec2; 4],
}
#[derive(Clone, Copy)]
struct Roll {
    elapsed: f32,
    duration: f32,
    direction: Vec3,
    speed: f32,
}
fn smooth(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * u * (u * (u * 6.0 - 15.0) + 10.0)
}
#[derive(Component)]
struct ModelRoot;
#[derive(Component)]
struct Ready {
    nodes: Vec<Entity>,
    players: Vec<Entity>,
}
#[derive(Resource)]
struct ModelAsset(Handle<Gltf>);
#[derive(Component)]
struct Slider(usize);
#[derive(Component)]
struct Fill(usize);
#[derive(Component)]
struct Value(usize);
#[derive(Component)]
struct Action(usize);
#[derive(Component)]
struct ModelAction(i32);
#[derive(Component)]
struct ModelLabel;
#[derive(Component)]
struct Status;
#[derive(Component)]
struct PageLabel;
#[derive(Component)]
struct ProfileNote;

struct Leg {
    joints: [Entity; 3],
    lengths: [f32; 2],
    sole: f32,
    sole_up: Vec3,
    sole_points: [Vec3; 15],
    anchor: Vec3,
    lock: f32,
    planted: bool,
    raw: Vec3,
    final_pos: Vec3,
    error: f32,
    contact_confidence: f32,
}
#[derive(Resource)]
struct Rig {
    rest: Vec<(Entity, Transform)>,
    pelvis_base: Option<Vec3>,
    ground_anchor: Vec3,
    model: Entity,
    root: Entity,
    pelvis: Entity,
    spine: Entity,
    head: Entity,
    arms: [Entity; 2],
    hands: [crate::combat::Arm; 2],
    legs: [Leg; 2],
    motion: Option<(Entity, Transform)>,
    players: Vec<Entity>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    walk_duration: f32,
    stride: f32,
    current: Option<AnimationNodeIndex>,
    generation: u32,
    loads: [f32; 2],
    sway: Vec3,
    drop: f32,
    arm_angle: f32,
    torso_shift: Vec3,
    torso_lean: Vec2,
    body_height: f32,
    shoulder_width: f32,
    hip_width: f32,
    model_name: String,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Settings>()
        .init_resource::<ModelCatalog>()
        .init_resource::<Tuning>()
        .init_resource::<crate::reference::Reference>()
        .add_systems(Startup, spawn)
        .add_systems(
            Update,
            (
                controls.in_set(Stage::Input),
                switch_model,
                prepare,
                animate,
                paint,
            )
                .chain(),
        )
        .add_systems(FixedUpdate, motor.in_set(Stage::Motor))
        .add_systems(PostUpdate, reset_pose.before(bevy::app::AnimationSystems))
        .add_systems(
            PostUpdate,
            adapt
                .in_set(Stage::Pose)
                .after(bevy::app::AnimationSystems)
                .before(bevy::transform::TransformSystems::Propagate),
        )
        .add_systems(Update, (draw, camera, friction, profile_note));
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Stage {
    Input,
    Motor,
    Pose,
}

fn spawn(
    mut commands: Commands,
    assets: Res<AssetServer>,
    feel: Res<Tuning>,
    models: Res<ModelCatalog>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    feel.write_recommended();
    let x = -3.2;
    let y = ground::height(x, 0.0);
    let root = commands
        .spawn((
            Name::new("Ranger character controller"),
            Character {
                mover: crate::live_movement::Mover::new(std::f32::consts::FRAC_PI_2),
                yaw: std::f32::consts::FRAC_PI_2,
                direction: 1.0,
                grounded: true,
                height: HALF,
                ..default()
            },
            RigidBody::Kinematic,
            Collider::capsule(0.25, 1.2),
            Friction::new(0.8),
            CollisionLayers::new(2u32, 1u32 | 4u32 | 8u32),
            Transform::from_xyz(x, y + HALF, 0.0)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        ))
        .id();
    let initial = &models.choices[models.selected];
    let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(initial.path));
    let model = commands
        .spawn((
            Name::new(format!("{} from gb_live", initial.label)),
            ModelRoot,
            WorldAssetRoot(scene),
            Transform::from_xyz(0.0, -HALF, 0.0),
        ))
        .observe(ready)
        .id();
    commands.entity(root).add_child(model);
    commands.insert_resource(ModelAsset(assets.load(initial.path)));
    commands.spawn((
        Name::new("Ordinary physics ball"),
        RigidBody::Dynamic,
        Collider::sphere(0.25),
        Mass(8.0),
        Friction::new(0.8),
        CollisionLayers::new(2u32, 3u32),
        Restitution::new(0.6),
        Mesh3d(meshes.add(Sphere::new(0.25))),
        MeshMaterial3d(materials.add(Color::srgb(0.95, 0.43, 0.12))),
        Transform::from_xyz(0.0, ground::height(0.0, 1.4) + 2.0, 1.4),
    ));
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(15.0),
            flex_direction: FlexDirection::Column,
            row_gap: px(5.0),
            ..default()
        })
        .with_children(|ui| {
            ui.spawn(text("RANGER / COMBAT & MOVEMENT", 22.0));
            ui.spawn((
                Status,
                text("Loading the Ranger and its own walk clip...", 14.0),
            ));
            ui.spawn((ModelLabel, text("Model: Male Ranger", 14.0)));
            ui.spawn(Node {
                column_gap: px(5.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Button,
                    ModelAction(-1),
                    Node {
                        padding: UiRect::all(px(5.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.13, 0.22, 0.29)),
                ))
                .with_child(text("< Model (F6)", 12.0));
                row.spawn((
                    Button,
                    ModelAction(1),
                    Node {
                        padding: UiRect::all(px(5.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.13, 0.22, 0.29)),
                ))
                .with_child(text("Model > (F7)", 12.0));
            });
        });
    commands.spawn((Node{position_type:PositionType::Absolute,right:px(16.0),top:px(15.0),width:px(290.0),padding:UiRect::all(px(12.0)),
        flex_direction:FlexDirection::Column,row_gap:px(5.0),..default()},BackgroundColor(Color::srgba(0.035,0.055,0.075,0.94))))
        .with_children(|ui|{
            ui.spawn((PageLabel,text("",15.0)));
            for i in 0..5 {
                ui.spawn((Value(i),text("",13.0)));
                ui.spawn((Button,Slider(i),RelativeCursorPosition::default(),Node{width:percent(100.0),height:px(16.0),..default()},BackgroundColor(Color::srgb(0.13,0.2,0.25))))
                    .with_child((Fill(i),Node{height:percent(100.0),width:percent(50.0),..default()},BackgroundColor(Color::srgb(0.1,0.75,0.6))));
            }
            ui.spawn(Node{flex_wrap:FlexWrap::Wrap,column_gap:px(5.0),row_gap:px(5.0),..default()}).with_children(|buttons|{
                for (i,label) in ["Demo (Tab)","Jump tap","Reset (R / X)","Guides","Previous page","Next page","Save (F5)","Load (F9)"].iter().enumerate() {
                    buttons.spawn((Button,Action(i),Node{width:percent(48.0),padding:UiRect::all(px(5.0)),..default()},BackgroundColor(Color::srgb(0.13,0.22,0.29))))
                        .with_child(text(label,12.0));
                }
            });
            ui.spawn(text("WASD / stick: move | Shift / RT: run\nHold Space / A: charge; release: jump\nCtrl / B: roll | H: scratch head\nLMB / RB: punch | V: drop weapon\nMB4 / F / Y: disarm + equip\nHold MB5 / E / LB: two-hand grab\nAlternate A/D: build spin | W/S: carry\nRelease MB5 to throw",11.0));
            ui.spawn((ProfileNote,text("F5 saves values. RMB / right stick: orbit; wheel: zoom.",11.0)));
        });
    println!(
        "[ranger] loading {}; native walk; form-fitted controller",
        initial.path
    );
}
fn text(s: &str, size: f32) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font_size: bevy::text::FontSize::Px(size),
            ..default()
        },
        TextColor(Color::WHITE),
    )
}

fn ready(
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
        .insert(Ready { nodes, players });
}

fn switch_model(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut models: ResMut<ModelCatalog>,
    mut settings: ResMut<Settings>,
    roots: Query<Entity, With<Character>>,
    current: Query<Entity, With<ModelRoot>>,
) {
    if models.pending == 0 {
        return;
    }
    let count = models.choices.len() as i32;
    models.selected = (models.selected as i32 + models.pending).rem_euclid(count) as usize;
    models.pending = 0;
    let choice = &models.choices[models.selected];
    for entity in &current {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<Rig>();
    let Ok(root) = roots.single() else { return };
    let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(choice.path));
    let model = commands
        .spawn((
            Name::new(format!("{} from gb_live", choice.label)),
            ModelRoot,
            WorldAssetRoot(scene),
            Transform::from_xyz(0.0, -HALF, 0.0),
        ))
        .observe(ready)
        .id();
    commands.entity(root).add_child(model);
    commands.insert_resource(ModelAsset(assets.load(choice.path)));
    settings.generation = settings.generation.wrapping_add(1);
    println!("[ranger] switching to {} ({})", choice.label, choice.path);
}

pub(crate) fn world_transform(world: &World, entity: Entity) -> Option<GlobalTransform> {
    fn resolve(world: &World, e: Entity, depth: u8) -> Option<GlobalTransform> {
        if depth > 80 {
            return None;
        }
        let local = *world.get::<Transform>(e)?;
        Some(match world.get::<ChildOf>(e) {
            Some(p) => resolve(world, p.parent(), depth + 1)?.mul_transform(local),
            None => GlobalTransform::from(local),
        })
    }
    resolve(world, entity, 0)
}

fn prepare(world: &mut World) {
    if world.contains_resource::<Rig>() {
        return;
    }
    let mut query = world.query_filtered::<(Entity, &Ready), With<ModelRoot>>();
    let Some((model, ready)) = query.iter(world).next() else {
        return;
    };
    let nodes = ready.nodes.clone();
    let players = ready.players.clone();
    if players.is_empty() {
        return;
    }
    let root = world.get::<ChildOf>(model).unwrap().parent();
    let names: HashMap<_, _> = nodes
        .iter()
        .filter_map(|&e| world.get::<Name>(e).map(|n| (n.as_str().to_string(), e)))
        .collect();
    let (graph, idle, walk, walk_duration) = {
        let asset = world.resource::<ModelAsset>();
        let gltfs = world.resource::<Assets<Gltf>>();
        let Some(gltf) = gltfs.get(&asset.0) else {
            return;
        };
        let Some(idle) = gltf.named_animations.get("npc_idle") else {
            return;
        };
        let Some(walk) = gltf.named_animations.get("npc_walk") else {
            return;
        };
        let Some(clip) = world.resource::<Assets<AnimationClip>>().get(walk) else {
            return;
        };
        let duration = clip.duration();
        let (graph, indices) = AnimationGraph::from_clips([idle.clone(), walk.clone()]);
        (graph, indices[0], indices[1], duration)
    };
    let scene_y = world_transform(world, model).unwrap().translation().y;
    let head_position = world_transform(world, names["B_Head"])
        .unwrap()
        .translation();
    let shoulder_width = world_transform(world, names["B_L_UpperArm"])
        .unwrap()
        .translation()
        .distance(
            world_transform(world, names["B_R_UpperArm"])
                .unwrap()
                .translation(),
        );
    let hip_width = world_transform(world, names["B_L_Thigh"])
        .unwrap()
        .translation()
        .distance(
            world_transform(world, names["B_R_Thigh"])
                .unwrap()
                .translation(),
        );
    let body_height = (head_position.y - scene_y + 0.13).clamp(1.4, 2.15);
    let fitted_half = (body_height * 0.5).clamp(0.72, 1.08);
    let collider_radius = (shoulder_width * 0.55).clamp(0.20, 0.34);
    let old_height = world.get::<Character>(root).unwrap().height;
    world.get_mut::<Position>(root).unwrap().y += fitted_half - old_height;
    {
        let mut character = world.get_mut::<Character>(root).unwrap();
        character.height = fitted_half;
    }
    *world.get_mut::<Collider>(root).unwrap() = Collider::capsule(
        collider_radius,
        (body_height - collider_radius * 2.0).max(0.55),
    );
    *world.get_mut::<Transform>(model).unwrap() = Transform::from_xyz(0.0, -fitted_half, 0.0);
    let model_name = world.resource::<ModelCatalog>().choices
        [world.resource::<ModelCatalog>().selected]
        .label
        .to_string();
    let leg = |side: &str| -> Leg {
        let joints = [
            names[&format!("B_{side}_Thigh")],
            names[&format!("B_{side}_Calf")],
            names[&format!("B_{side}_Foot")],
        ];
        let poses = joints.map(|e| world_transform(world, e).unwrap());
        let foot = poses[2].translation();
        let (_, rotation, _) = poses[2].to_scale_rotation_translation();
        let toe = names
            .get(&format!("B_{side}_Toe0"))
            .and_then(|&entity| world_transform(world, entity))
            .map(|pose| pose.translation())
            .unwrap_or(foot + Vec3::Z * 0.18);
        let forward = (toe - foot).with_y(0.0).normalize_or(Vec3::Z);
        let side_axis = Vec3::Y.cross(forward).normalize_or(Vec3::X);
        let toe_length = (toe - foot).with_y(0.0).length().clamp(0.12, 0.32);
        let heel_length = (toe_length * 0.42).clamp(0.055, 0.13);
        let half_width = (toe_length * 0.28).clamp(0.035, 0.075);
        let mut sole_points = [Vec3::ZERO; 15];
        for row in 0..5 {
            let along = -heel_length + (toe_length + heel_length) * row as f32 / 4.0;
            for column in 0..3 {
                let across = half_width * (column as f32 - 1.0);
                let contact = foot + forward * along + side_axis * across;
                let contact = contact.with_y(scene_y + 0.004);
                sole_points[row * 3 + column] = rotation.inverse() * (contact - foot);
            }
        }
        Leg {
            joints,
            lengths: [
                poses[0].translation().distance(poses[1].translation()),
                poses[1].translation().distance(foot),
            ],
            sole: (foot.y - scene_y).max(0.03),
            sole_up: rotation.inverse() * Vec3::Y,
            sole_points,
            anchor: foot,
            lock: 0.0,
            planted: false,
            raw: foot,
            final_pos: foot,
            error: 0.0,
            contact_confidence: 1.0,
        }
    };
    let legs = [leg("L"), leg("R")];
    let average_leg_reach = legs
        .iter()
        .map(|leg| leg.lengths.iter().sum::<f32>())
        .sum::<f32>()
        * 0.5;
    // Stride is a property of the sampled body, not of the Ranger asset or clip duration.
    // The selected animation supplies shape and cadence while the measured legs set reach.
    let fitted_stride = (average_leg_reach * 1.18).clamp(0.62, 1.28);
    let hands = std::array::from_fn(|i| {
        let side = if i == 0 { "L" } else { "R" };
        let joints = [
            names[&format!("B_{side}_UpperArm")],
            names[&format!("B_{side}_Forearm")],
            names[if i == 0 {
                "B_L_Forearm.001"
            } else {
                "B_R_Hand"
            }],
        ];
        let p = joints.map(|e| world_transform(world, e).unwrap().translation());
        let hand_pose = world_transform(world, joints[2]).unwrap();
        let hand_rotation = hand_pose.to_scale_rotation_translation().1;
        let finger = world_transform(world, names[&format!("B_{side}_Finger2")])
            .unwrap()
            .translation();
        let hand_axis = (finger - p[2]).normalize_or(Vec3::Y);
        let palm_offset = hand_rotation.inverse() * (finger - p[2]) * 0.62;
        let backhand_offset = palm_offset - hand_rotation.inverse() * Vec3::Y * 0.035;
        crate::combat::Arm {
            joints,
            lengths: [p[0].distance(p[1]), p[1].distance(p[2])],
            palm_axis: hand_rotation.inverse() * hand_axis,
            palm_offset,
            backhand_offset,
        }
    });
    crate::fingers::calibrate(world, &names, hands, names["B_Head"], names["B_Pelvis"]);
    crate::motion_library::calibrate(world, &names);
    let rest = nodes
        .iter()
        .filter_map(|&e| world.get::<Transform>(e).map(|t| (e, *t)))
        .collect();
    crate::paper_motion::calibrate(world, &nodes);
    let motion = names
        .get("Motion")
        .and_then(|&e| world.get::<Transform>(e).map(|t| (e, *t)));
    let handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
    for &p in &players {
        world.entity_mut(p).insert((
            AnimationGraphHandle(handle.clone()),
            AnimationTransitions::new(),
        ));
    }
    println!(
        "[ranger] {}: {} nodes, height {:.3}, shoulders {:.3}, hips {:.3}, legs {:?} / {:?}, soles {} / {}",
        model_name,
        nodes.len(),
        body_height,
        shoulder_width,
        hip_width,
        legs[0].lengths,
        legs[1].lengths,
        legs[0].sole,
        legs[1].sole
    );
    let profile = serde_json::json!({
        "model": model_name,
        "body_height_m": body_height,
        "controller_half_height_m": fitted_half,
        "collider_radius_m": collider_radius,
        "shoulder_width_m": shoulder_width,
        "hip_width_m": hip_width,
        "left_leg_m": legs[0].lengths,
        "right_leg_m": legs[1].lengths,
        "left_sole_offset_m": legs[0].sole,
        "right_sole_offset_m": legs[1].sole,
    });
    let _ = std::fs::create_dir_all("profiles").and_then(|_| {
        std::fs::write(
            format!(
                "profiles/rig_{}.json",
                model_name.to_ascii_lowercase().replace(' ', "_")
            ),
            serde_json::to_string_pretty(&profile).unwrap(),
        )
    });
    world.insert_resource(Rig {
        rest,
        pelvis_base: None,
        ground_anchor: Vec3::new(-3.2, ground::height(-3.2, 0.0), 0.0),
        model,
        root,
        pelvis: names["B_Pelvis"],
        spine: names["B_Spine2"],
        head: names["B_Head"],
        arms: [names["B_L_UpperArm"], names["B_R_UpperArm"]],
        hands,
        legs,
        motion,
        players,
        idle,
        walk,
        walk_duration,
        stride: fitted_stride,
        current: None,
        generation: 0,
        loads: [0.0; 2],
        sway: Vec3::ZERO,
        drop: 0.0,
        arm_angle: 0.0,
        torso_shift: Vec3::ZERO,
        torso_lean: Vec2::ZERO,
        body_height,
        shoulder_width,
        hip_width,
        model_name,
    });
}

fn reset_pose(world: &mut World) {
    let Some(rig) = world.remove_resource::<Rig>() else {
        return;
    };
    for &(entity, rest) in &rig.rest {
        if let Some(mut t) = world.get_mut::<Transform>(entity) {
            *t = rest;
        }
    }
    if let Some(c) = world.get::<Character>(rig.root) {
        let height = c.height;
        *world.get_mut::<Transform>(rig.model).unwrap() = Transform::from_xyz(0.0, -height, 0.0);
    }
    world.insert_resource(rig);
}

fn animate(world: &mut World) {
    let Some(mut rig) = world.remove_resource::<Rig>() else {
        return;
    };
    let settings = world.resource::<Settings>();
    let generation = settings.generation;
    let blend = world.resource::<Tuning>().get(K::Blend);
    let character = world.get::<Character>(rig.root).unwrap();
    let moving = character.horizontal.length() > 0.03 && character.grounded;
    let phase = character.phase;
    let node = if moving { rig.walk } else { rig.idle };
    for &entity in &rig.players {
        let mut query = world.query::<(&mut AnimationPlayer, &mut AnimationTransitions)>();
        if let Ok((mut player, mut transitions)) = query.get_mut(world, entity) {
            if rig.current != Some(node) || rig.generation != generation {
                transitions
                    .play(&mut player, node, Duration::from_secs_f32(blend))
                    .repeat();
            }
            if let Some(active) = player.animation_mut(node) {
                if moving {
                    active
                        .set_speed(0.0)
                        .set_seek_time(phase * rig.walk_duration);
                } else {
                    active.set_speed(1.0);
                }
            }
        }
    }
    if rig.generation != generation {
        for leg in &mut rig.legs {
            leg.planted = false;
            leg.lock = 0.0;
        }
        rig.sway = Vec3::ZERO;
        rig.drop = 0.0;
        rig.arm_angle = 0.0;
        rig.torso_shift = Vec3::ZERO;
        rig.torso_lean = Vec2::ZERO;
        rig.generation = generation;
    }
    rig.current = Some(node);
    world.insert_resource(rig);
}

fn motor(
    time: Res<Time>,
    mut settings: ResMut<Settings>,
    rig: Option<Res<Rig>>,
    combat: Res<crate::combat::Combat>,
    feel: Res<Tuning>,
    reference: Res<crate::reference::Reference>,
    mut query: Query<(
        &mut Character,
        &mut Position,
        &mut Rotation,
        &mut LinearVelocity,
        &mut Collider,
    )>,
) {
    let Some(rig) = rig else {
        return;
    };
    let Ok((mut c, mut position, mut rotation, mut velocity, mut collider)) =
        query.get_mut(rig.root)
    else {
        return;
    };
    let dt = time.delta_secs();
    if std::mem::take(&mut settings.reset) {
        let fitted_half = (rig.body_height * 0.5).clamp(0.72, 1.08);
        let collider_radius = (rig.shoulder_width * 0.55).clamp(0.20, 0.34);
        *position = Position(Vec3::new(
            -3.2,
            ground::height(-3.2, 0.0) + fitted_half,
            0.0,
        ));
        *c = Character {
            mover: crate::live_movement::Mover::new(std::f32::consts::FRAC_PI_2),
            direction: 1.0,
            yaw: std::f32::consts::FRAC_PI_2,
            grounded: true,
            height: fitted_half,
            ..default()
        };
        velocity.0 = Vec3::ZERO;
        rotation.0 = Quat::from_rotation_y(c.yaw);
        settings.generation += 1;
        *collider = Collider::capsule(
            collider_radius,
            (rig.body_height - collider_radius * 2.0).max(0.55),
        );
        return;
    }
    if std::mem::take(&mut settings.jump)
        && c.grounded
        && c.charge <= 0.0
        && c.roll.is_none()
        && combat.drive.is_none()
    {
        c.charging = true;
        c.charge = dt;
        c.push = 0.0;
    }
    if settings.roll && c.grounded && c.roll.is_none() && combat.drive.is_none() {
        let direction = settings.move_input.normalize_or(
            c.horizontal
                .normalize_or(Quat::from_rotation_y(c.yaw) * Vec3::Z),
        );
        c.roll = Some(Roll {
            elapsed: 0.0,
            duration: feel.get(K::RollTime),
            direction,
            speed: c.horizontal.length().max(feel.get(K::RollSpeed)),
        });
        settings.roll = false;
        c.charge = 0.0;
        c.charging = false;
        c.push = 0.0;
    }
    if let Some(mut roll) = c.roll {
        roll.elapsed += dt;
        c.roll = if roll.elapsed < roll.duration {
            Some(roll)
        } else {
            None
        };
    }
    c.landing = (c.landing - dt).max(0.0);
    if c.charging {
        c.charge = (c.charge + dt).min(feel.get(K::ChargeTime));
        if !settings.jump_held {
            c.charging = false;
            c.push = feel.get(K::Prepare) * 0.45;
            c.jump_power = feel.get(K::MinJump)
                + (1.0 - feel.get(K::MinJump)) * smooth(c.charge / feel.get(K::ChargeTime));
        }
    }
    if c.push > 0.0 {
        c.push -= dt;
        if c.push <= 0.0 {
            c.vy = (feel.get(K::Jump) * c.jump_power / settings.mass).min(7.0);
            let launch = settings
                .aim_direction
                .with_y(0.0)
                .normalize_or(Quat::from_rotation_y(c.yaw) * Vec3::Z);
            c.horizontal = c.horizontal * feel.get(K::MomentumCarry)
                + launch * feel.get(K::AimLaunch) * c.jump_power;
            c.mover.velocity = c.horizontal;
            c.mover.yaw = launch.x.atan2(launch.z);
            c.yaw = c.mover.yaw;
            c.grounded = false;
            c.charge = 0.0;
            c.air_age = 0.0;
            c.air_origin = position.0;
            c.air_velocity = c.horizontal + Vec3::Y * c.vy;
            c.air_rates = [
                Vec2::new(-0.35, 0.06),
                Vec2::new(-0.28, -0.06),
                Vec2::new(0.4, 0.0),
                Vec2::new(0.35, 0.0),
            ];
        }
    }
    let remaining = if c.direction > 0.0 {
        3.2 - position.x
    } else {
        position.x + 3.2
    };
    let mut tuning = guildbound_sim::Tuning::default();
    tuning.drag_accel = feel.get(K::Acceleration) * 75.0 / settings.mass;
    tuning.turn_rate = feel.get(K::Turn);
    tuning.coast_half_life *= settings.mass / 75.0;
    let normal = ground::normal(position.x, position.z);
    let gravity_tangent = Vec3::NEG_Y * 9.81 + normal * (9.81 * normal.y);
    let grip = settings.friction * 9.81 * normal.y.max(0.0);
    let braking = (grip - gravity_tangent.length()).max(0.1);
    if settings.walking && c.dwell > 0.0 {
        c.dwell -= dt;
        if c.dwell <= 0.0 {
            c.direction = -c.direction;
        }
    } else if settings.walking && remaining < 0.055 && c.horizontal.length() < 0.25 {
        c.dwell = 0.45;
    }
    let want = if c.charge > 0.0 || c.landing > 0.0 {
        None
    } else if settings.walking && c.dwell <= 0.0 {
        Some((
            Vec3::X * c.direction,
            settings
                .speed
                .min(remaining.max(0.0) * std::f32::consts::LN_2 / tuning.coast_half_life)
                .min((2.0 * braking * remaining.max(0.0)).sqrt()),
        ))
    } else if !settings.walking && settings.move_input.length_squared() > 1e-6 {
        Some((
            settings.move_input.normalize_or_zero(),
            (if settings.running {
                feel.get(K::Run)
            } else {
                settings.speed
            }) * settings.move_input.length().min(1.0),
        ))
    } else {
        None
    };
    if let Some(last) = c.last_position.filter(|_| c.grounded) {
        c.phase = crate::live_movement::advance_phase(
            c.phase,
            (position.0 - last).with_y(0.0).length(),
            ((rig.stride * (1.0 + 0.35 * c.run_blend))
                * (1.0 - feel.get(K::Reference) * c.run_blend)
                + rig.legs[0].lengths.iter().sum::<f32>()
                    * reference.stride_ratio
                    * feel.get(K::Reference)
                    * c.run_blend)
                * feel.get(K::Stride)
                / feel.get(K::Cadence),
        );
    }
    c.last_position = Some(position.0);
    let mut predicted = position.0;
    let old_velocity = c.horizontal;
    if c.charge > 0.0 {
        c.mover.velocity = c.horizontal;
        let aim = settings
            .aim_direction
            .with_y(0.0)
            .normalize_or(Quat::from_rotation_y(c.yaw) * Vec3::Z);
        c.mover.yaw = aim.x.atan2(aim.z);
    } else if let Some(roll) = c.roll {
        let u = roll.elapsed / roll.duration;
        c.mover.velocity = roll.direction * roll.speed * (1.0 - 0.7 * smooth((u - 0.7) / 0.3));
        c.mover.yaw = roll.direction.x.atan2(roll.direction.z);
    } else if let Some(drive) = combat.drive {
        c.mover.velocity = drive.velocity;
        c.mover.yaw = drive.yaw;
    } else if c.grounded {
        if c.landing > 0.0 {
            tuning.coast_half_life = 0.05 / feel.get(K::PlantGrip);
        }
        crate::live_movement::step(&mut c.mover, &mut predicted, want, &tuning, dt);
    }
    if c.grounded {
        // Coulomb friction bounds the live motor's requested change in momentum.
        let requested = (c.mover.velocity - old_velocity) / dt.max(1e-6);
        let tangent_acceleration = requested
            .with_y(-(normal.x * requested.x + normal.z * requested.z) / normal.y.max(0.1));
        let actual =
            gravity_tangent + (tangent_acceleration - gravity_tangent).clamp_length_max(grip);
        c.mover.velocity = old_velocity + actual.with_y(0.0) * dt;
    }
    // Keep direct control inside the visible terrain patch.
    c.mover.velocity.x = c
        .mover
        .velocity
        .x
        .clamp((-8.5 - position.x) / dt, (8.5 - position.x) / dt);
    c.mover.velocity.z = c
        .mover
        .velocity
        .z
        .clamp((-8.5 - position.z) / dt, (8.5 - position.z) / dt);
    c.horizontal = c.mover.velocity;
    if c.grounded {
        let allowed = ground::safe_travel(
            position.0,
            c.horizontal * dt,
            if c.roll.is_some() { 0.16 } else { 0.3 },
        );
        c.horizontal = allowed / dt;
        c.mover.velocity = c.horizontal;
    }
    let run_target = if c.roll.is_none() {
        ((c.horizontal.length() - feel.get(K::Walk))
            / (feel.get(K::Run) - feel.get(K::Walk)).max(0.1))
        .clamp(0.0, 1.0)
    } else {
        0.0
    };
    c.run_blend =
        c.run_blend + (run_target - c.run_blend) * (1.0 - (-dt / feel.get(K::Blend)).exp());
    let acceleration = (c.horizontal - old_velocity) / dt.max(1e-6);
    c.acceleration = c.acceleration.lerp(acceleration, 1.0 - (-12.0 * dt).exp());
    let yaw_delta = (c.mover.yaw - c.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    c.yaw_rate = c.yaw_rate + (yaw_delta / dt - c.yaw_rate) * (1.0 - (-10.0 * dt).exp());
    c.yaw = c.mover.yaw;
    rotation.0 = Quat::from_rotation_y(c.yaw);
    let height = if c.roll.is_some() { 0.42 } else { HALF };
    if height != c.height {
        position.y += height - c.height;
        *collider = if c.roll.is_some() {
            Collider::sphere(0.42)
        } else {
            Collider::capsule(0.25, 1.2)
        };
        c.height = height;
    }
    let next = position.0 + c.horizontal * dt;
    let next_floor = ground::height(next.x, next.z);
    if c.grounded && next_floor < ground::height(position.x, position.z) - 0.12 {
        c.grounded = false;
        c.vy = 0.0;
    }
    let mut vertical_velocity;
    if c.grounded {
        c.vy = (next_floor + c.height - position.y) / dt.max(1e-6);
        vertical_velocity = c.vy;
        c.air_age = 0.0;
    } else {
        if c.air_age == 0.0 {
            c.air_origin = position.0;
            c.air_velocity = c.horizontal + Vec3::Y * c.vy;
        }
        c.air_age += dt;
        let previous = c.vy;
        c.vy -= 9.81 * dt;
        vertical_velocity = (previous + c.vy) * 0.5;
        if position.y + c.vy * dt - c.height <= next_floor && c.vy <= 0.0 {
            c.impact = -c.vy;
            c.vy = (next_floor + c.height - position.y) / dt;
            vertical_velocity = c.vy;
            c.grounded = true;
            c.landing = feel.get(K::Landing);
            if c.roll.is_none()
                && (settings.roll || c.horizontal.length() > feel.get(K::RollThreshold))
                && (c.horizontal.length() > 0.2 || settings.roll)
            {
                c.roll = Some(Roll {
                    elapsed: 0.0,
                    duration: feel.get(K::RollTime),
                    direction: c
                        .horizontal
                        .normalize_or(Quat::from_rotation_y(c.yaw) * Vec3::Z),
                    speed: c.horizontal.length().max(feel.get(K::RollSpeed)),
                });
            }
            settings.roll = false;
        }
    }
    for i in 0..4 {
        let angle = c.air_angles[i];
        let rate = c.air_rates[i];
        let length = if i < 2 {
            rig.hands[i].lengths.iter().sum::<f32>()
        } else {
            rig.legs[i - 2].lengths[1]
        };
        let omega = (9.81 / length.max(0.15)).sqrt();
        let acceleration = angle * (-omega * omega) - rate * (2.2 * omega);
        let next_rate = if c.grounded {
            rate * (-20.0 * dt).exp()
        } else {
            rate + acceleration * dt
        };
        let next_angle = if c.grounded {
            angle * (-20.0 * dt).exp()
        } else {
            (angle + next_rate * dt).clamp(Vec2::splat(-0.16), Vec2::splat(0.16))
        };
        c.air_rates[i] = next_rate;
        c.air_angles[i] = next_angle;
    }
    velocity.0 = c.horizontal + Vec3::Y * vertical_velocity;
}

fn adapt(world: &mut World) {
    let Some(mut rig) = world.remove_resource::<Rig>() else {
        return;
    };
    let dt = world.resource::<Time>().delta_secs().min(0.05);
    let feel = world.resource::<Tuning>().clone();
    let reference = world.resource::<crate::reference::Reference>().clone();
    let (support, crouch, mass) = {
        let s = world.resource::<Settings>();
        (s.support, s.crouch, s.mass)
    };
    let (
        root_pos,
        yaw,
        grounded,
        charge,
        landing,
        acceleration,
        run,
        roll,
        height,
        phase,
        vy,
        impact,
    ) = {
        let c = world.get::<Character>(rig.root).unwrap();
        (
            world.get::<Position>(rig.root).unwrap().0,
            c.yaw,
            c.grounded,
            c.charge,
            c.landing,
            c.acceleration,
            c.run_blend,
            c.roll,
            c.height,
            c.phase,
            c.vy,
            c.impact,
        )
    };
    *world.get_mut::<Transform>(rig.model).unwrap() = Transform::from_xyz(0.0, -height, 0.0);
    let (yaw_rate, speed, move_input) = {
        let c = world.get::<Character>(rig.root).unwrap();
        (
            c.yaw_rate,
            c.horizontal.length(),
            world.resource::<Settings>().move_input,
        )
    };
    if let Some((e, rest)) = rig.motion {
        if let Some(mut t) = world.get_mut::<Transform>(e) {
            t.translation = rest.translation;
        }
    }
    let forward = Quat::from_rotation_y(yaw) * Vec3::Z;
    let source_floor = root_pos.y - height;
    let (charging, push, power, air_angles) = {
        let c = world.get::<Character>(rig.root).unwrap();
        (c.charging, c.push, c.jump_power, c.air_angles)
    };
    let firmness = feel.get(K::Firmness);
    {
        let mut pelvis = world.get_mut::<Transform>(rig.pelvis).unwrap();
        let baseline = *rig.pelvis_base.get_or_insert(pelvis.translation);
        pelvis.translation = baseline
            + (pelvis.translation - baseline).clamp_length_max(0.035) * (1.0 - 0.65 * firmness);
    }
    if let Some(roll) = roll {
        let tuck = (std::f32::consts::PI * roll.elapsed / roll.duration)
            .sin()
            .max(0.0)
            .powf(0.6);
        let side = Vec3::Y.cross(forward);
        for (bone, angle) in [(rig.pelvis, 0.35), (rig.spine, 0.85), (rig.head, 0.25)] {
            let (_, rotation, _) = world_transform(world, bone)
                .unwrap()
                .to_scale_rotation_translation();
            ik::rotate_world(
                world,
                bone,
                Quat::from_axis_angle(side, angle * tuck) * rotation,
            );
        }
    }
    let jump_height = (source_floor - ground::height(root_pos.x, root_pos.z)).max(0.0);
    let charge_u = if charging {
        (charge / feel.get(K::ChargeTime)).clamp(0.0, 1.0)
    } else {
        power
    };
    let push_u = if push > 0.0 {
        smooth(1.0 - push / (feel.get(K::Prepare) * 0.45))
    } else {
        0.0
    };
    let charge_ease = if charging {
        0.45 + 0.55 * smooth(charge_u)
    } else if push > 0.0 {
        (0.45 + 0.55 * charge_u) * (1.0 - push_u)
    } else {
        0.0
    };
    let brace = world.resource::<crate::combat::Combat>().brace;
    let braking = (-acceleration.dot(forward) / 8.0).clamp(0.0, 1.0) * (speed / 0.6).min(1.0);
    let recovery = landing / feel.get(K::Landing);
    let pose_drop = crouch
        + 0.18 * charge_ease
        + recovery * (0.07 + impact * 0.018).min(0.22)
        + brace
        + run * 0.012
        + braking * feel.get(K::BrakeBend);
    let mut required_drop = 0.0f32;
    let mut targets = [Vec3::ZERO; 2];
    let mut rotations = [Quat::IDENTITY; 2];
    let step_direction = world.resource::<crate::combat::Combat>().drive.map(|d| {
        forward
            .lerp(
                d.velocity.normalize_or(forward),
                (d.velocity.length() / 0.2).min(1.0),
            )
            .normalize_or(forward)
    });
    for (i, leg) in rig.legs.iter_mut().enumerate() {
        let pose = world_transform(world, leg.joints[2]).unwrap();
        let (_, rotation, _) = pose.to_scale_rotation_translation();
        leg.raw = pose.translation();
        let native_height = (leg.raw.y - source_floor - leg.sole).max(0.0);
        let swing = smooth(native_height / 0.10);
        leg.raw.y += feel.get(K::FootLift) * swing;
        let influence = feel.get(K::Reference);
        let leg_length = leg.lengths.iter().sum::<f32>();
        let stride = rig.stride * (1.0 + 0.35 * run) * (1.0 - influence)
            + leg_length * reference.stride_ratio * influence;
        let duty = 0.5 + (reference.duty - 0.5) * influence;
        let u = (phase + i as f32 * 0.5).fract();
        let (z, lift) = if u < duty {
            (stride * duty * (0.5 - u / duty), 0.0)
        } else {
            let t = (u - duty) / (1.0 - duty);
            (
                -stride * duty * 0.5 + stride * duty * smooth(t),
                (std::f32::consts::PI * t).sin(),
            )
        };
        let hip = world_transform(world, leg.joints[0]).unwrap().translation();
        let lateral = (hip - root_pos).dot(Vec3::Y.cross(forward));
        let generated = root_pos.with_y(source_floor + leg.sole)
            + Vec3::Y.cross(forward) * lateral
            + forward * z
            + Vec3::Y
                * lift
                * feel.get(K::RunKnee)
                * (0.75 + reference.knee_peak * 0.18 * influence);
        leg.raw = leg.raw.lerp(generated, run);
        if lift > 0.0 {
            // Look ahead only during swing; planted contacts stay authoritative.
            let preview = feel.get(K::FootPredict);
            leg.raw += forward * speed * preview * lift * (0.18 + 0.22 * run);
            let preview_floor = ground::height(leg.raw.x, leg.raw.z);
            leg.raw.y = leg
                .raw
                .y
                .max(preview_floor + leg.sole + feel.get(K::SoleClearance));
        }
        if let Some(direction) = step_direction {
            let offset = leg.raw - root_pos;
            leg.raw += (direction - forward) * offset.dot(forward);
        }
        let height = leg.raw.y - source_floor - leg.sole;
        let phase_contact = u < duty;
        let contact = grounded
            && roll.is_none()
            && (landing > 0.0
                || if run > 0.12 {
                    phase_contact
                } else {
                    height < if leg.planted { 0.065 } else { 0.035 }
                });
        if !leg.planted && contact {
            leg.anchor = Vec3::new(
                leg.raw.x,
                ground::height(leg.raw.x, leg.raw.z) + leg.sole,
                leg.raw.z,
            );
            leg.planted = true;
        }
        let horizontal_error = (leg.raw - leg.anchor).with_y(0.0).length();
        let release_reach = leg.lengths.iter().sum::<f32>() * 0.72;
        if leg.planted && (!contact || horizontal_error > release_reach) {
            leg.planted = false;
        }
        let want = if leg.planted { support } else { 0.0 };
        let lock_response = if want > leg.lock { 24.0 } else { 14.0 };
        leg.lock += (want - leg.lock) * (1.0 - (-lock_response * dt).exp());
        let mut target = leg.raw.lerp(leg.anchor, leg.lock);
        if !grounded {
            leg.planted = false;
            leg.lock = 0.0;
            let time_to_ground = (vy + (vy * vy + 2.0 * 9.81 * jump_height).sqrt()) / 9.81;
            let prepare_land = if vy < 0.0 {
                smooth(1.0 - time_to_ground / 0.2)
            } else {
                0.0
            };
            target =
                leg.raw + Vec3::Y * (jump_height.min(feel.get(K::AirTuck)) * (1.0 - prepare_land));
        }
        if let Some(roll) = roll {
            let hip = world_transform(world, leg.joints[0]).unwrap().translation();
            let tucked = hip + forward * 0.23 - Vec3::Y * 0.3;
            target = leg.raw.lerp(
                tucked,
                (std::f32::consts::PI * roll.elapsed / roll.duration)
                    .sin()
                    .max(0.0)
                    .powf(0.6),
            );
            leg.planted = false;
            leg.lock = 0.0;
        }
        let correction = Quat::from_rotation_arc(
            (rotation * leg.sole_up).normalize_or(Vec3::Y),
            ground::normal(target.x, target.z),
        );
        let (axis, angle) = correction.to_axis_angle();
        let sole_rotation = (Quat::from_axis_angle(axis, angle.min(30f32.to_radians()) * support)
            * rotation)
            .normalize();
        rotations[i] = sole_rotation;
        if grounded && roll.is_none() {
            // HumanRig-style dimension adaptation: use the measured Ranger sole, not a point ray.
            // Dense samples are local to each active foot, so terrain cost stays bounded.
            let sample_count = feel.get(K::SoleDensity).round().clamp(3.0, 15.0) as usize;
            let mut required_y = f32::NEG_INFINITY;
            let mut low = f32::INFINITY;
            let mut high = f32::NEG_INFINITY;
            let mut normal_agreement = 0.0;
            for sample in 0..sample_count {
                let index = if sample_count == 1 {
                    7
                } else {
                    sample * 14 / (sample_count - 1)
                };
                let offset = sole_rotation * leg.sole_points[index];
                let x = target.x + offset.x;
                let z = target.z + offset.z;
                let floor = ground::height(x, z);
                required_y = required_y.max(floor - offset.y + feel.get(K::SoleClearance));
                low = low.min(floor);
                high = high.max(floor);
                normal_agreement += ground::normal(x, z)
                    .dot(ground::normal(target.x, target.z))
                    .max(0.0);
            }
            let spread = (high - low).max(0.0);
            let agreement = normal_agreement / sample_count as f32;
            let raw_confidence = agreement * (1.0 - spread / 0.18).clamp(0.0, 1.0);
            leg.contact_confidence +=
                (raw_confidence - leg.contact_confidence) * (1.0 - (-18.0 * dt).exp());
            let compliance = feel.get(K::ContactCompliance);
            let correction_weight = support * (1.0 - compliance * (1.0 - leg.contact_confidence));
            target.y += (required_y.max(target.y) - target.y) * correction_weight;
            if leg.planted {
                leg.anchor.y = target.y;
            }
        } else {
            leg.contact_confidence += (0.0 - leg.contact_confidence) * (1.0 - (-8.0 * dt).exp());
        }
        targets[i] = target;
        if grounded && roll.is_none() {
            let hip = world_transform(world, leg.joints[0]).unwrap().translation();
            let delta = hip - target;
            let reach = leg.lengths.iter().sum::<f32>() - 0.003;
            let vertical = (reach * reach - Vec2::new(delta.x, delta.z).length_squared())
                .max(0.0)
                .sqrt();
            required_drop = required_drop.max((delta.y - vertical).max(0.0));
        }
    }
    // Move the pelvis laterally toward the loaded foot before solving either leg.
    let planted = [rig.legs[0].planted, rig.legs[1].planted];
    let pelvis = world_transform(world, rig.pelvis).unwrap().translation();
    let p0 = rig.legs[0].anchor;
    let p1 = rig.legs[1].anchor;
    let line = (p0 - p1).with_y(0.0);
    let balance_point =
        pelvis - acceleration * (pelvis.y - ground::height(pelvis.x, pelvis.z)).max(0.0) / 9.81;
    let share = match planted {
        [true, false] => [1.0, 0.0],
        [false, true] => [0.0, 1.0],
        [true, true] => {
            let t = ((balance_point - p1).with_y(0.0).dot(line) / line.length_squared().max(1e-6))
                .clamp(0.08, 0.92);
            [t, 1.0 - t]
        }
        _ => [0.5, 0.5],
    };
    // Quasi-static support estimate, rather than pretending to measure forces from animated limbs.
    let normal_force = if grounded {
        mass * 9.81 * ground::normal(root_pos.x, root_pos.z).y
    } else {
        0.0
    };
    rig.loads = share.map(|w| w * normal_force);
    let side = Vec3::Y.cross(forward).normalize_or(Vec3::X);
    let cop = p0 * share[0] + p1 * share[1];
    let lateral = if grounded && planted.iter().any(|&p| p) {
        (cop - pelvis).dot(side).clamp(-0.075, 0.075) * support
    } else {
        0.0
    };
    rig.sway = rig.sway.lerp(
        side * lateral * (1.0 - 0.45 * firmness),
        1.0 - (-feel.get(K::GroundAnchor) * dt).exp(),
    );
    if grounded {
        let connection = if planted.iter().any(|p| *p) {
            cop.with_y(ground::height(cop.x, cop.z))
        } else {
            root_pos.with_y(ground::height(root_pos.x, root_pos.z))
        };
        rig.ground_anchor = rig
            .ground_anchor
            .lerp(connection, 1.0 - (-feel.get(K::GroundAnchor) * dt).exp());
    }
    let drop_target = pose_drop.max(required_drop).min(0.25);
    rig.drop += (drop_target - rig.drop) * (1.0 - (-feel.get(K::ReturnRate) * dt).exp());
    rig.drop = rig.drop.max(required_drop.min(0.25));
    // Convert the world drop through the parent frame; source bone rolls are not assumed to be identity.
    let parent = world.get::<ChildOf>(rig.pelvis).unwrap().parent();
    let parent_matrix = world_transform(world, parent).unwrap().affine();
    world.get_mut::<Transform>(rig.pelvis).unwrap().translation += parent_matrix
        .inverse()
        .transform_vector3(Vec3::NEG_Y * rig.drop + rig.sway);

    // Treat the torso as the movable mass above the pelvis. The desired upper-body projection
    // follows the support centre plus h*a/g, which is the zero-moment relation for acceleration.
    let head_position = world_transform(world, rig.head)
        .map(|pose| pose.translation())
        .unwrap_or(pelvis + Vec3::Y * 0.65);
    let torso_height = (head_position.y - pelvis.y).abs().clamp(0.42, 0.9);
    let support_shift = if grounded && planted.iter().any(|&value| value) {
        (cop - pelvis).with_y(0.0) * feel.get(K::SupportLean)
    } else {
        Vec3::ZERO
    };
    let acceleration_shift =
        acceleration.with_y(0.0) * (torso_height / 9.81) * feel.get(K::AccelLean);
    let desired_shift = (support_shift + acceleration_shift).clamp_length_max(0.16) * support;
    let response = feel.get(K::LeanResponse);
    rig.torso_shift = rig
        .torso_shift
        .lerp(desired_shift, 1.0 - (-response * dt).exp());
    let desired_lean = Vec2::new(
        rig.torso_shift.dot(forward).atan2(torso_height),
        rig.torso_shift.dot(side).atan2(torso_height),
    );
    rig.torso_lean = rig
        .torso_lean
        .lerp(desired_lean, 1.0 - (-response * 0.8 * dt).exp());

    // Translate the chest as well as rotating it. Rotation alone reads as bending at the waist;
    // this shifts the visible upper-body mass over the loaded foot.
    if let Some(spine_parent) = world.get::<ChildOf>(rig.spine).map(ChildOf::parent) {
        if let Some(parent_pose) = world_transform(world, spine_parent) {
            world.get_mut::<Transform>(rig.spine).unwrap().translation += parent_pose
                .affine()
                .inverse()
                .transform_vector3(rig.torso_shift * feel.get(K::UpperShift));
        }
    }
    let action_lean = world.resource::<crate::combat::Combat>().lean
        + feel.get(K::RunLean) * run
        + charge_ease * 0.08;
    let braking_lean = if braking > 0.01 {
        acceleration.dot(forward).atan2(9.81) * feel.get(K::BrakeLean)
    } else {
        0.0
    };
    if let Some(spine) = world_transform(world, rig.spine) {
        let (_, rotation, _) = spine.to_scale_rotation_translation();
        let turn_bank = -(speed * yaw_rate / 9.81).atan() * feel.get(K::TurnBank);
        ik::rotate_world(
            world,
            rig.spine,
            Quat::from_axis_angle(forward, (turn_bank - rig.torso_lean.y).clamp(-0.42, 0.42))
                * Quat::from_axis_angle(
                    side,
                    (rig.torso_lean.x + braking_lean + action_lean).clamp(-0.42, 0.42),
                )
                * rotation,
        );
    }
    // The native walk already supplies opposing arm/leg phase. Add only the jump's counter-swing.
    if grounded
        && roll.is_none()
        && move_input.length_squared() > 0.01
        && world.resource::<crate::combat::Combat>().drive.is_none()
    {
        let desired = move_input.x.atan2(move_input.z);
        let delta = (desired - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        for (bone, weight) in [(rig.spine, 0.22), (rig.head, feel.get(K::HeadLead))] {
            let (_, rotation, _) = world_transform(world, bone)
                .unwrap()
                .to_scale_rotation_translation();
            ik::rotate_world(
                world,
                bone,
                Quat::from_rotation_y(delta.clamp(-0.85, 0.85) * weight) * rotation,
            );
        }
    }
    let arm_target = if charge > 0.0 {
        0.25 * charge_ease - 0.85 * push_u
    } else if !grounded {
        -0.55 * (jump_height / 0.3).clamp(0.0, 1.0)
    } else {
        0.0
    };
    rig.arm_angle += (arm_target - rig.arm_angle) * (1.0 - (-9.0 * dt).exp());
    let arm_counter = feel.get(K::ArmCounter);
    for (i, arm) in rig.arms.into_iter().enumerate() {
        if let Some(pose) = world_transform(world, arm) {
            let (_, rotation, _) = pose.to_scale_rotation_translation();
            ik::rotate_world(
                world,
                arm,
                Quat::from_axis_angle(forward, rig.torso_lean.y * arm_counter)
                    * Quat::from_axis_angle(
                        side,
                        rig.arm_angle - rig.torso_lean.x * arm_counter * 0.45
                            + run
                                * (0.18 + 0.08 * (1.0 - firmness))
                                * (std::f32::consts::TAU * phase + i as f32 * std::f32::consts::PI)
                                    .sin(),
                    )
                    * rotation,
            );
        }
        if run > 0.001 {
            let elbow = rig.hands[i].joints[1];
            let (_, rotation, _) = world_transform(world, elbow)
                .unwrap()
                .to_scale_rotation_translation();
            let arm_phase = std::f32::consts::TAU * phase + i as f32 * std::f32::consts::PI;
            let source_hint = (reference.elbow_bend * 0.12).clamp(0.08, 0.18);
            let relaxed_bend = 0.08 + source_hint + 0.16 * (0.5 + 0.5 * arm_phase.cos());
            ik::rotate_world(
                world,
                elbow,
                Quat::from_axis_angle(side, -relaxed_bend * run) * rotation,
            );
        }
    }
    for (i, leg) in rig.legs.iter_mut().enumerate() {
        let hip = world_transform(world, leg.joints[0]).unwrap().translation();
        let knee = world_transform(world, leg.joints[1]).unwrap().translation();
        let axis = (targets[i] - hip).normalize_or(Vec3::NEG_Y);
        let off = knee - hip - axis * (knee - hip).dot(axis);
        let fwd = (forward - axis * forward.dot(axis)).normalize_or(forward);
        let blend = (off.length() / 0.06).clamp(0.0, 1.0);
        let mut pole = (off.normalize_or(fwd) * blend + fwd * (1.0 - blend)).normalize_or(fwd);
        if pole.dot(fwd) < 0.0 && off.length() < 0.03 {
            pole = fwd;
        }
        if support > 0.0 || !grounded || rig.drop > 0.0 {
            ik::apply(world, leg.joints, targets[i], pole, forward, leg.lengths);
            ik::rotate_world(world, leg.joints[2], rotations[i]);
        }
        let positions = leg
            .joints
            .map(|e| world_transform(world, e).unwrap().translation());
        leg.final_pos = positions[2];
        leg.error = leg.final_pos.distance(targets[i]);
    }
    if !grounded && roll.is_none() {
        let t = (vy + (vy * vy + 2.0 * 9.81 * jump_height).sqrt()) / 9.81;
        let weight = feel.get(K::AirLoose) * smooth((t / 0.18).min(1.0));
        let bones = [
            rig.arms[0],
            rig.arms[1],
            rig.legs[0].joints[1],
            rig.legs[1].joints[1],
        ];
        for i in 0..4 {
            let (_, rotation, _) = world_transform(world, bones[i])
                .unwrap()
                .to_scale_rotation_translation();
            let angle = air_angles[i] * weight;
            ik::rotate_world(
                world,
                bones[i],
                Quat::from_axis_angle(side, if i >= 2 { angle.x.max(0.0) } else { angle.x })
                    * Quat::from_axis_angle(forward, angle.y)
                    * rotation,
            );
        }
        for leg in &mut rig.legs {
            leg.final_pos = world_transform(world, leg.joints[2]).unwrap().translation();
        }
    }
    if let Some(roll) = roll {
        let u = (roll.elapsed / roll.duration).clamp(0.0, 1.0);
        let tuck = (std::f32::consts::PI * u).sin().max(0.0).powf(0.6);
        for (i, arm) in rig.hands.iter().enumerate() {
            let wrist = world_transform(world, arm.joints[2]).unwrap().translation();
            let target = root_pos
                + Vec3::Y * (1.0 - height)
                + forward * 0.18
                + side * if i == 0 { 0.13 } else { -0.13 };
            ik::apply(
                world,
                arm.joints,
                wrist.lerp(target, tuck),
                Vec3::NEG_Y,
                forward,
                arm.lengths,
            );
        }
        let pelvis = world_transform(world, rig.pelvis).unwrap().translation();
        let model_pose = world_transform(world, rig.model).unwrap();
        let rotation = Quat::from_axis_angle(
            Vec3::Y.cross(roll.direction).normalize_or(side),
            std::f32::consts::TAU * smooth(u),
        );
        let pivot = pelvis.lerp(root_pos.with_y(source_floor + 0.48), tuck);
        let mut origin = pivot - rotation * (pelvis - model_pose.translation());
        let mut clearance = 0.0f32;
        let points = std::iter::once((rig.head, 0.12))
            .chain(std::iter::once((rig.pelvis, 0.14)))
            .chain(rig.hands.iter().flat_map(|a| a.joints.map(|e| (e, 0.06))))
            .chain(rig.legs.iter().flat_map(|l| l.joints.map(|e| (e, 0.065))));
        for (bone, radius) in points {
            let p = world_transform(world, bone).unwrap().translation();
            let p = pivot + rotation * (p - pelvis);
            clearance = clearance.max(ground::height(p.x, p.z) + radius - p.y);
        }
        origin.y += clearance;
        let parent = world.get::<ChildOf>(rig.model).unwrap().parent();
        let parent_pose = world_transform(world, parent).unwrap();
        let parent_rotation = parent_pose.to_scale_rotation_translation().1;
        let mut model = world.get_mut::<Transform>(rig.model).unwrap();
        model.translation = parent_pose.affine().inverse().transform_point3(origin);
        model.rotation = (parent_rotation.inverse() * rotation * parent_rotation).normalize();
        drop(model);
        for leg in &mut rig.legs {
            leg.final_pos = world_transform(world, leg.joints[2]).unwrap().translation();
        }
    }
    world.insert_resource(rig);
}

fn controls(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    cameras: Query<&Transform, With<Camera3d>>,
    mut settings: ResMut<Settings>,
    mut feel: ResMut<Tuning>,
    mut models: ResMut<ModelCatalog>,
    sliders: Query<(&Slider, &Interaction, &RelativeCursorPosition)>,
    buttons: Query<(&Action, &Interaction), Changed<Interaction>>,
    model_buttons: Query<(&ModelAction, &Interaction), Changed<Interaction>>,
) {
    let mouse_aim = mouse.pressed(MouseButton::Right)
        || (keys.pressed(KeyCode::Space) && settings.drag.is_none());
    if mouse_aim {
        let sensitivity = feel.get(K::MouseSensitivity);
        settings.camera_target_yaw -= motion.delta.x * sensitivity;
        settings.camera_target_pitch =
            (settings.camera_target_pitch + motion.delta.y * sensitivity * 0.68).clamp(0.08, 1.18);
    }
    let camera_alpha = 1.0 - (-feel.get(K::CameraSmooth) * time.delta_secs()).exp();
    settings.camera_yaw += (settings.camera_target_yaw - settings.camera_yaw) * camera_alpha;
    settings.camera_pitch += (settings.camera_target_pitch - settings.camera_pitch) * camera_alpha;
    settings.camera_distance = (settings.camera_distance - scroll.delta.y * 0.4).clamp(3.0, 12.0);
    let mut stick = Vec2::new(
        (keys.pressed(KeyCode::KeyD) as u8 as f32) - (keys.pressed(KeyCode::KeyA) as u8 as f32),
        (keys.pressed(KeyCode::KeyW) as u8 as f32) - (keys.pressed(KeyCode::KeyS) as u8 as f32),
    );
    let mut running = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let mut jump = keys.just_pressed(KeyCode::Space);
    let mut jump_held = keys.pressed(KeyCode::Space);
    let mut roll =
        keys.just_pressed(KeyCode::ControlLeft) || keys.just_pressed(KeyCode::ControlRight);
    let mut reset = keys.just_pressed(KeyCode::KeyR);
    let mut demo = keys.just_pressed(KeyCode::Tab);
    if keys.just_pressed(KeyCode::F6) {
        models.pending = -1;
    }
    if keys.just_pressed(KeyCode::F7) {
        models.pending = 1;
    }
    for gamepad in &gamepads {
        let view = gamepad.right_stick();
        if view.length() > 0.15 {
            settings.camera_target_yaw -= view.x * time.delta_secs() * 2.0;
            settings.camera_target_pitch =
                (settings.camera_target_pitch - view.y * time.delta_secs()).clamp(0.08, 1.18);
        }
        let raw = gamepad.left_stick();
        let magnitude = ((raw.length() - 0.15) / 0.85).clamp(0.0, 1.0);
        let analog = raw.normalize_or_zero() * magnitude;
        if analog.length_squared() > stick.length_squared() {
            stick = analog;
        }
        running |= gamepad.get(GamepadButton::RightTrigger2).unwrap_or(0.0) > 0.4;
        jump |= gamepad.just_pressed(GamepadButton::South);
        jump_held |= gamepad.pressed(GamepadButton::South);
        roll |= gamepad.just_pressed(GamepadButton::East);
        reset |= gamepad.just_pressed(GamepadButton::West);
        demo |= gamepad.just_pressed(GamepadButton::Start);
    }
    let forward = cameras
        .iter()
        .next()
        .map(|t| {
            Vec3::from(t.forward())
                .with_y(0.0)
                .normalize_or(Vec3::NEG_Z)
        })
        .unwrap_or(Vec3::NEG_Z);
    settings.aim_direction = forward;
    settings.move_input =
        (forward * stick.y + forward.cross(Vec3::Y) * stick.x).clamp_length_max(1.0);
    settings.running = running;
    if demo {
        settings.walking = !settings.walking;
        settings.reset |= settings.walking;
    }
    if settings.move_input.length_squared() > 1e-6 {
        settings.walking = false;
    }
    if jump {
        settings.jump = true;
    }
    settings.jump_held = jump_held;
    settings.roll |= roll;
    if keys.just_pressed(KeyCode::F5) {
        feel.save();
    }
    if keys.just_pressed(KeyCode::F9) {
        feel.load();
    }
    if reset {
        settings.reset = true;
    }
    if !mouse.pressed(MouseButton::Left) {
        settings.drag = None;
    }
    for (slider, interaction, cursor) in &sliders {
        if mouse.pressed(MouseButton::Left)
            && mouse.just_pressed(MouseButton::Left)
            && *interaction == Interaction::Pressed
        {
            settings.drag = Some(slider.0);
        }
        if settings.drag == Some(slider.0) {
            if let Some(p) = cursor.normalized {
                let v = (p.x + 0.5).clamp(0.0, 1.0);
                feel.set(slider.0, v);
            }
        }
    }
    for (action, interaction) in &buttons {
        if *interaction == Interaction::Pressed {
            match action.0 {
                0 => {
                    settings.walking = !settings.walking;
                    settings.reset |= settings.walking;
                }
                1 => settings.jump = true,
                2 => settings.reset = true,
                3 => settings.traces = !settings.traces,
                4 => {
                    feel.page =
                        (feel.page + crate::tuning::PAGES.len() - 1) % crate::tuning::PAGES.len()
                }
                5 => feel.page = (feel.page + 1) % crate::tuning::PAGES.len(),
                6 => feel.save(),
                _ => feel.load(),
            }
        }
    }
    for (action, interaction) in &model_buttons {
        if *interaction == Interaction::Pressed {
            models.pending = action.0;
        }
    }
    settings.speed = feel.get(K::Walk);
    settings.mass = feel.get(K::Mass);
    settings.friction = feel.get(K::Friction);
    settings.support = feel.get(K::Support);
    settings.crouch = feel.get(K::Crouch);
}

fn paint(
    settings: Res<Settings>,
    feel: Res<Tuning>,
    models: Res<ModelCatalog>,
    rig: Option<Res<Rig>>,
    characters: Query<&Character>,
    mut fills: Query<(&Fill, &mut Node)>,
    mut values: Query<
        (&Value, &mut Text),
        (Without<Status>, Without<PageLabel>, Without<ModelLabel>),
    >,
    mut status: Query<
        &mut Text,
        (
            With<Status>,
            Without<Value>,
            Without<PageLabel>,
            Without<ModelLabel>,
        ),
    >,
    mut pages: Query<
        &mut Text,
        (
            With<PageLabel>,
            Without<Value>,
            Without<Status>,
            Without<ModelLabel>,
        ),
    >,
    mut model_labels: Query<
        &mut Text,
        (
            With<ModelLabel>,
            Without<Value>,
            Without<Status>,
            Without<PageLabel>,
        ),
    >,
) {
    for (fill, mut n) in &mut fills {
        n.width = percent(feel.fraction(fill.0) * 100.0);
    }
    for (value, mut t) in &mut values {
        t.0 = feel.label(value.0);
    }
    if let Ok(mut page) = pages.single_mut() {
        page.0 = format!(
            "{}/{}  {}",
            feel.page + 1,
            crate::tuning::PAGES.len(),
            crate::tuning::PAGES[feel.page]
        );
    }
    if let Ok(mut label) = model_labels.single_mut() {
        label.0 = format!("Model: {}", models.choices[models.selected].label);
    }
    if let (Some(rig), Ok(mut t)) = (rig, status.single_mut()) {
        if let Ok(c) = characters.get(rig.root) {
            t.0 = format!(
                "{} / {}\nH {:.2} m | shoulders {:.2} m | hips {:.2} m | stride {:.2} m\nSpeed {:.2} m/s | {}\nEstimated support: L {:.0} N / R {:.0} N | grip {:.0} N",
                rig.model_name,
                if settings.walking {
                    "walking demo"
                } else {
                    "WASD / gamepad control"
                },
                rig.body_height,
                rig.shoulder_width,
                rig.hip_width,
                rig.stride,
                c.horizontal.length(),
                if c.roll.is_some() {
                    "rolling"
                } else if c.charge > 0.0 {
                    "preparing jump"
                } else if !c.grounded {
                    "airborne"
                } else if c.landing > 0.0 {
                    "landing / plant"
                } else if c.run_blend > 0.5 {
                    "running"
                } else {
                    "grounded"
                },
                rig.loads[0],
                rig.loads[1],
                settings.friction * rig.loads.iter().sum::<f32>()
            );
        }
    }
}

fn friction(settings: Res<Settings>, mut bodies: Query<&mut Friction>) {
    if !settings.is_changed() {
        return;
    }
    for mut material in &mut bodies {
        *material = Friction::new(settings.friction);
    }
}

fn draw(
    settings: Res<Settings>,
    feel: Res<Tuning>,
    rig: Option<Res<Rig>>,
    transforms: Query<&GlobalTransform>,
    characters: Query<&Character>,
    mut gizmos: Gizmos,
) {
    let points = (0..=80).map(|i| {
        let x = -3.5 + i as f32 * 7.0 / 80.0;
        Vec3::new(x, ground::height(x, 0.0) + 0.015, 0.0)
    });
    gizmos.linestrip(points, Color::srgb(0.1, 0.8, 0.8));
    if !settings.traces {
        return;
    }
    if let Some(rig) = rig {
        if let Ok(pelvis) = transforms.get(rig.pelvis) {
            gizmos.line(
                rig.ground_anchor,
                pelvis.translation(),
                Color::srgb(0.25, 1.0, 0.65),
            );
            gizmos.cross(
                Isometry3d::from_translation(rig.ground_anchor),
                0.07,
                Color::srgb(0.25, 1.0, 0.65),
            );
        }
        if let (Ok(root), Ok(c)) = (transforms.get(rig.root), characters.get(rig.root)) {
            if c.charging || c.push > 0.0 {
                let charge_power = if c.charging {
                    feel.get(K::MinJump)
                        + (1.0 - feel.get(K::MinJump)) * smooth(c.charge / feel.get(K::ChargeTime))
                } else {
                    c.jump_power
                };
                let velocity = c.horizontal * feel.get(K::MomentumCarry)
                    + settings.aim_direction * feel.get(K::AimLaunch) * charge_power
                    + Vec3::Y * (feel.get(K::Jump) * charge_power / settings.mass).min(7.0);
                let mut arc = Vec::new();
                for i in 0..=60 {
                    let t = i as f32 * 0.035;
                    let p = root.translation() + velocity * t - Vec3::Y * (0.5 * 9.81 * t * t);
                    if i > 0 && p.y - c.height < ground::height(p.x, p.z) {
                        break;
                    }
                    arc.push(p);
                }
                gizmos.linestrip(arc, Color::srgb(1.0, 0.8, 0.15));
            } else if !c.grounded {
                let mut arc = Vec::new();
                for i in 0..=60 {
                    let t = i as f32 * 0.04;
                    let p = c.air_origin + c.air_velocity * t - Vec3::Y * (0.5 * 9.81 * t * t);
                    if i > 0 && p.y - c.height < ground::height(p.x, p.z) {
                        break;
                    }
                    arc.push(p);
                }
                gizmos.linestrip(arc, Color::srgb(0.9, 0.65, 1.0));
            }
            let direction = c
                .horizontal
                .normalize_or(Quat::from_rotation_y(c.yaw) * Vec3::Z);
            let mut p = root.translation();
            let mut path = vec![p.with_y(ground::height(p.x, p.z) + 0.03)];
            for _ in 0..24 {
                let delta = ground::safe_travel(p, direction * 0.1, 0.16);
                if delta.length_squared() < 1e-6 {
                    break;
                }
                p += delta;
                path.push(p.with_y(ground::height(p.x, p.z) + 0.03));
            }
            gizmos.linestrip(path, Color::srgb(1.0, 0.55, 0.15));
        }
        if let (Ok(head), Ok(pelvis)) = (transforms.get(rig.head), transforms.get(rig.pelvis)) {
            gizmos.line(
                head.translation(),
                pelvis.translation(),
                Color::srgb(1.0, 0.85, 0.2),
            );
            for leg in &rig.legs {
                gizmos.line(
                    pelvis.translation(),
                    leg.final_pos,
                    Color::srgb(0.6, 0.8, 1.0),
                );
            }
        }
        for leg in &rig.legs {
            let hit = Vec3::new(
                leg.final_pos.x,
                ground::height(leg.final_pos.x, leg.final_pos.z),
                leg.final_pos.z,
            );
            gizmos.line(
                leg.final_pos + Vec3::Y * 0.35,
                hit,
                Color::srgb(0.1, 0.75, 1.0),
            );
            if leg.planted {
                gizmos.cross(
                    Isometry3d::from_translation(leg.anchor),
                    0.055,
                    Color::srgb(0.2, 1.0, 0.2),
                );
            }
            if let Ok(foot_pose) = transforms.get(leg.joints[2]) {
                let (_, rotation, origin) = foot_pose.to_scale_rotation_translation();
                let sample_count = feel.get(K::SoleDensity).round().clamp(3.0, 15.0) as usize;
                let color = Color::srgb(
                    1.0 - leg.contact_confidence * 0.7,
                    0.35 + leg.contact_confidence * 0.65,
                    0.25,
                );
                for sample in 0..sample_count {
                    let index = sample * 14 / (sample_count - 1).max(1);
                    let contact = origin + rotation * leg.sole_points[index];
                    let floor = contact.with_y(ground::height(contact.x, contact.z));
                    gizmos.line(contact, floor, color);
                    gizmos.cross(Isometry3d::from_translation(floor), 0.012, color);
                }
            }
        }
    }
}

fn camera(
    time: Res<Time>,
    feel: Res<Tuning>,
    rig: Option<Res<Rig>>,
    positions: Query<&Transform, Without<Camera3d>>,
    characters: Query<&Character>,
    combat: Res<crate::combat::Combat>,
    settings: Res<Settings>,
    mut cameras: Query<&mut Transform, With<Camera3d>>,
) {
    let Some(rig) = rig else {
        return;
    };
    // Interpolated render transform, not the 120 Hz physics Position, so the camera doesn't stutter.
    let Ok(position) = positions.get(rig.root).map(|t| t.translation) else {
        return;
    };
    let floor = ground::height(position.x, position.z);
    let focus = combat.focus.unwrap_or(position);
    let lead = characters
        .get(rig.root)
        .map(|character| {
            let direction = if character.charging || character.push > 0.0 {
                settings.aim_direction
            } else {
                character.horizontal.normalize_or_zero()
            };
            direction
                * feel.get(K::CameraLead)
                * (character.horizontal.length() / 3.5).clamp(0.35, 1.0)
        })
        .unwrap_or(Vec3::ZERO);
    let x = focus.x + lead.x;
    let z = focus.z + lead.z;
    for mut camera in &mut cameras {
        let distance = settings.camera_distance + combat.frame_radius * 0.55;
        let pivot = Vec3::new(x, floor + 0.85, z);
        let wanted = pivot
            + Vec3::new(
                settings.camera_yaw.sin() * settings.camera_pitch.cos(),
                settings.camera_pitch.sin(),
                settings.camera_yaw.cos() * settings.camera_pitch.cos(),
            ) * distance;
        camera.translation = camera
            .translation
            .lerp(wanted, 1.0 - (-10.0 * time.delta_secs()).exp());
        let bias = Vec3::new(settings.camera_yaw.cos(), 0.0, -settings.camera_yaw.sin()) * 0.55;
        camera.look_at(pivot + bias, Vec3::Y);
    }
}

fn profile_note(
    feel: Res<Tuning>,
    reference: Res<crate::reference::Reference>,
    mut notes: Query<&mut Text, With<ProfileNote>>,
) {
    if let Ok(mut note) = notes.single_mut() {
        note.0 = format!(
            "{}\n{}\nRMB: orbit | wheel: zoom",
            feel.note, reference.note
        );
    }
}

pub(crate) fn combat_frame(world: &World) -> Option<GlobalTransform> {
    world_transform(world, world.get_resource::<Rig>()?.model)
}

pub(crate) struct LibraryContext {
    pub phase: f32,
    pub charge: f32,
    pub push: f32,
    pub landing: f32,
    pub roll: Option<f32>,
}

pub(crate) fn library_context(world: &World) -> Option<LibraryContext> {
    let rig = world.get_resource::<Rig>()?;
    let c = world.get::<Character>(rig.root)?;
    let feel = world.resource::<Tuning>();
    Some(LibraryContext {
        phase: c.phase,
        charge: (c.charge / feel.get(K::ChargeTime)).clamp(0.0, 1.0),
        push: if c.push > 0.0 {
            (c.push / (feel.get(K::Prepare) * 0.45).max(0.01)).clamp(0.0, 1.0)
        } else {
            0.0
        },
        landing: (c.landing / feel.get(K::Landing).max(0.01)).clamp(0.0, 1.0),
        roll: c
            .roll
            .map(|roll| (roll.elapsed / roll.duration).clamp(0.0, 1.0)),
    })
}

pub(crate) fn combat_actor(world: &World) -> Option<crate::combat::Actor> {
    let rig = world.get_resource::<Rig>()?;
    let c = world.get::<Character>(rig.root)?;
    let settings = world.resource::<Settings>();
    Some(crate::combat::Actor {
        position: world.get::<Position>(rig.root)?.0,
        yaw: c.yaw,
        grounded: c.grounded,
        mass: settings.mass,
        friction: settings.friction,
        generation: settings.generation,
        arms: rig.hands,
        velocity: c.horizontal,
        rolling: c.roll.is_some(),
    })
}
