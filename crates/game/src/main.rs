//! Guildbound: stylized synthesized motion + physics combat.
//! `motion/` turns donor clips (UAL1, UAL2, Sekiro proto) into stylized extremes and composes
//! the final skeleton pose. `play/` owns the world, characters, combat and everything that
//! decides which motion layers are active.
//! Run: `cargo run -p guildbound`

mod motion;
mod play;

use avian3d::prelude::*;
use bevy::prelude::*;

/// Fixed physics tick (1/120 s).
pub const TICK_HZ: f64 = 120.0;

/// Absolute asset root, also used by code that reads files directly (gltf crate, JSON).
pub const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.55, 0.68, 0.82)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 400.0,
            ..default()
        })
        .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
        .add_plugins(
            DefaultPlugins
                .set(bevy::asset::AssetPlugin {
                    file_path: ASSETS.into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Guildbound".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(PhysicsPlugins::default().set(PhysicsInterpolationPlugin::interpolate_all()))
        .insert_resource(SubstepCount(8))
        .add_plugins((motion::plugin, play::plugin))
        .run();
}
