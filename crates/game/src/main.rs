//! Ranger character motor, native animation and terrain support.
//! Use --comparison for the earlier weight comparison, --sandbox for player/dummy physics.
//! Run: `cargo run -p guildbound`  (window)

mod body;
mod combat;
mod comparison;
mod fingers;
mod input;
mod live_movement;
mod motion_library;
mod overlay;
mod paper_motion;
mod picker;
mod ranger;
mod reference;
mod rig;
mod skeleton_ik;
mod source;
mod terrain_motion;
mod tuning;
mod voxel;

use avian3d::prelude::*;
use bevy::prelude::*;

/// Fixed tick shared by Avian and sim::api (1/120 s).
pub const TICK_HZ: f64 = 120.0;

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.55, 0.68, 0.82)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 400.0,
            ..default()
        })
        .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
        .add_plugins(
            DefaultPlugins
                .set(bevy::asset::AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Guildbound - overhaul".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(PhysicsPlugins::default().set(PhysicsInterpolationPlugin::interpolate_all()))
        .insert_resource(SubstepCount(12))
        .add_systems(Startup, setup_scene);
    if std::env::args().any(|a| a == "--sandbox") {
        app.add_plugins((body::plugin, overlay::plugin, input::plugin, picker::plugin));
    } else if std::env::args().any(|a| a == "--comparison") {
        app.add_plugins((body::plugin, overlay::plugin, comparison::plugin));
    } else {
        app.add_plugins((
            ranger::plugin,
            combat::plugin,
            fingers::plugin,
            motion_library::plugin,
            paper_motion::plugin,
        ));
    }
    app.run();
}

/// Camera, light and the 1 m thick floor slab (top face at y = 0).
pub fn setup_scene(
    mut commands: Commands,
    bench: Option<Res<comparison::Settings>>,
    ranger: Option<Res<ranger::Settings>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let camera = if ranger.is_some() {
        Transform::from_xyz(3.8, 2.6, 7.2).looking_at(Vec3::new(0.0, 0.85, 0.0), Vec3::Y)
    } else if bench.is_some() {
        Transform::from_xyz(1.0, 4.2, 11.5).looking_at(Vec3::new(0.0, 0.7, 0.0), Vec3::Y)
    } else {
        Transform::from_xyz(2.4, 1.5, 3.2).looking_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y)
    };
    commands.spawn((Camera3d::default(), camera));
    commands.spawn((
        DirectionalLight {
            illuminance: 9000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(3.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    if bench.is_some() || ranger.is_some() {
        terrain_motion::spawn_ground(&mut commands, &mut meshes, &mut mats);
        return;
    }
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(20.0, 1.0, 20.0),
        Friction::new(0.8),
        rig::floor_layers(),
        Mesh3d(meshes.add(Cuboid::new(20.0, 1.0, 20.0))),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.45, 0.40),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
}
