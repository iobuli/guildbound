//! Guildbound dev build: the sparring floor, a dev panel for the boss, and a local remote API for Claude.
//! Run: `cargo run -p guildbound`. Remote: Bevy Remote Protocol on http://127.0.0.1:15702 (see Tools/gb.py).

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
        .insert_resource(ClearColor(Color::srgb(0.07, 0.09, 0.13)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Guildbound - dev".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins((remote::plugin(), RemoteHttpPlugin::default()))
        .insert_resource(sim_bridge::Sim::new())
        .init_resource::<feed::DevFeed>()
        .init_resource::<remote::RemoteActivity>()
        .init_resource::<panel::PanelState>()
        .add_systems(Startup, (sim_bridge::setup_scene, camera::spawn, announce))
        .add_systems(
            Update,
            (
                sim_bridge::read_player_input,
                sim_bridge::step_sim,
                sim_bridge::sync_views,
                sim_bridge::draw_gizmos,
                camera::orbit,
            )
                .chain(),
        )
        .add_systems(EguiPrimaryContextPass, panel::panel)
        .run();
}

fn announce(mut feed: ResMut<feed::DevFeed>) {
    feed.post("game", "Dev build up. Remote API on http://127.0.0.1:15702 (python -I Tools/gb.py state).", 0);
}
