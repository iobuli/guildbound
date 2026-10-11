//! Guildbound dev build: the sparring floor, a dev panel for the boss, and a local remote API for Claude.
//! Run: `cargo run -p guildbound`. Remote: Bevy Remote Protocol on http://127.0.0.1:15702 (see Tools/gb.py).

mod anim;
mod camera;
mod feed;
mod panel;
mod remote;
mod sim_bridge;

use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

/// Repo root, for files the boss and the agents share (feed log, screenshots).
pub const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn main() {
    App::new()
        .insert_resource(ClearColor(camera::SKY))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 350.0, ..default() })
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Guildbound - dev".into(),
                        resolution: (1600, 900).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin { file_path: format!("{ROOT}/assets"), ..default() }),
        )
        .add_plugins(EguiPlugin::default())
        .add_plugins((remote::plugin(), RemoteHttpPlugin::default()))
        .insert_resource(sim_bridge::Sim::new())
        .init_resource::<feed::DevFeed>()
        .init_resource::<remote::RemoteActivity>()
        .init_resource::<panel::PanelState>()
        .init_resource::<anim::AnimSettings>()
        .init_resource::<anim::AnimRequests>()
        .init_resource::<anim::ClusterCursor>()
        .insert_resource(anim::Libraries::scan())
        .add_systems(Startup, (anim::load_library, sim_bridge::setup_scene, camera::spawn, announce))
        .add_systems(
            Update,
            (
                sim_bridge::read_player_input,
                sim_bridge::step_sim,
                sim_bridge::sync_views,
                anim::switch_library,
                anim::build_library,
                anim::drive_animation,
                anim::lean,
                sim_bridge::show_capsules,
                sim_bridge::draw_gizmos,
                camera::orbit,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (anim::lock_root_motion, anim::inertialize).chain().after(bevy::app::AnimationSystems).before(bevy::transform::TransformSystems::Propagate),
        )
        .add_systems(EguiPrimaryContextPass, panel::panel)
        .run();
}

fn announce(mut feed: ResMut<feed::DevFeed>) {
    feed.post("game", "Dev build up. Remote API on http://127.0.0.1:15702 (python -I Tools/gb.py state).", 0);
}
