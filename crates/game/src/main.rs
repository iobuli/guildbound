//! Guildbound: stylized synthesized motion + physics combat.
//! `motion/` turns donor clips (UAL1, UAL2, Sekiro proto) into stylized extremes and composes
//! the final skeleton pose. `play/` owns the world, characters, combat and everything that
//! decides which motion layers are active. `gbui` (crates/gbui) draws the HUD, themed by
//! `assets/ui/theme.json`. `gbperf` (F3) profiles frames and logs a baseline to
//! `staging/perf.log` every run.
//! Run: `cargo run -p guildbound`

mod motion;
mod play;

use avian3d::prelude::*;
use bevy::prelude::*;
use gbperf::PerfApp;
use bevy::winit::{UpdateMode, WinitSettings};
use std::time::Duration;

/// Fixed physics tick (1/120 s).
pub const TICK_HZ: f64 = 120.0;

/// Absolute asset root, also used by code that reads files directly (gltf crate, JSON).
pub const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn main() {
    App::new()
        .insert_resource(frame_pacing())
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
                        // GB_RES=WxH (e.g. 1920x1080) for recordings and screenshots at a higher resolution.
                        resolution: window_resolution().into(),
                        present_mode: bevy::window::PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(PhysicsPlugins::default().set(PhysicsInterpolationPlugin::interpolate_all()))
        // F7 (or GB_COLLIDERS=1) shows every collider as wireframe.
        .add_plugins(PhysicsDebugPlugin)
        .add_systems(Startup, |mut store: ResMut<GizmoConfigStore>| {
            store.config_mut::<PhysicsGizmos>().0.enabled = std::env::var_os("GB_COLLIDERS").is_some();
        })
        .add_systems(Update, |keys: Res<ButtonInput<KeyCode>>, mut store: ResMut<GizmoConfigStore>| {
            if keys.just_pressed(KeyCode::F7) {
                let c = &mut store.config_mut::<PhysicsGizmos>().0;
                c.enabled = !c.enabled;
            }
        })
        .insert_resource(SubstepCount(8))
        .add_plugins(gbui::GbuiPlugin {
            theme: concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/ui/theme.json").into(),
        })
        .add_plugins(gbperf::GbperfPlugin {
            log: concat!(env!("CARGO_MANIFEST_DIR"), "/../../staging/perf.log").into(),
            key: KeyCode::F3,
        })
        .add_plugins((motion::plugin, play::plugin))
        .perf_span(Update, "input", play::PlaySet::Input)
        .perf_span(Update, "think", play::PlaySet::Think)
        .perf_span(Update, "act", play::PlaySet::Act)
        .perf_span(Update, "select", play::PlaySet::Select)
        .perf_span(FixedUpdate, "move", play::PlaySet::Move)
        .perf_span(FixedPostUpdate, "physics", PhysicsSystems::StepSimulation)
        .perf_span(PostUpdate, "compose", motion::MotionSet::Compose)
        .perf_span(PostUpdate, "ui anim", gbui::GbuiSet::Animate)
        .perf_span(PostUpdate, "ui style", gbui::GbuiSet::Style)
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
