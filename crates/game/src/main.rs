//! Athorina Stage 1: a focused character controller and animation laboratory.
//! `motion/` retargets the local clip library; `play/` owns the grey stage and impact wall.
//! Run: `cargo run -p guildbound`

mod motion;
mod play;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::winit::{UpdateMode, WinitSettings};
use std::time::Duration;

/// Fixed physics tick (1/120 s).
pub const TICK_HZ: f64 = 120.0;

/// Absolute asset root, also used by code that reads files directly (gltf crate, JSON).
pub const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn main() {
    App::new()
        .insert_resource(frame_pacing())
        .insert_resource(ClearColor(Color::srgb(0.32, 0.34, 0.37)))
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
                        title: "Athorina | Stage 1".into(),
                        // GB_RES=WxH (e.g. 1920x1080) for recordings and screenshots at a higher resolution.
                        resolution: window_resolution().into(),
                        present_mode: bevy::window::PresentMode::AutoVsync,
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

/// Keep fast GPUs from rendering unlimited frames, including while the game is in the background.
/// Input is collected between ticks; the focused tick defaults to 60 Hz (GB_MAX_FPS=30..240).
fn frame_pacing() -> WinitSettings {
    let fps = std::env::var("GB_MAX_FPS").ok().and_then(|s| s.parse::<u32>().ok()).unwrap_or(60).clamp(30, 240);
    let paced = |hz: u32| UpdateMode::Reactive {
        wait: Duration::from_secs_f64(1.0 / hz as f64),
        react_to_device_events: false,
        react_to_user_events: false,
        react_to_window_events: false,
    };
    WinitSettings { focused_mode: paced(fps), unfocused_mode: paced(15) }
}

/// Window size: `GB_RES=WxH` when set and sane, else 1280x720.
fn window_resolution() -> (u32, u32) {
    std::env::var("GB_RES")
        .ok()
        .and_then(|v| {
            let (w, h) = v.trim().split_once(['x', 'X'])?;
            Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?))
        })
        .filter(|&(w, h)| (320..=7680).contains(&w) && (240..=4320).contains(&h))
        .unwrap_or((1280, 720))
}
