//! Guildbound dev build: the sparring floor, a dev panel for the boss, and a local remote API for Claude.
//! Run: `cargo run -p guildbound`. Remote: Bevy Remote Protocol on http://127.0.0.1:15702 (see Tools/gb.py).

mod anim;
mod arena;
mod camera;
mod catalog;
mod controller;
mod creator;
mod editor;
mod feed;
mod feedback;
mod hud;
mod ik;
mod joints;
mod knee;
mod guard_ik;
mod loot_view;
mod magic_view;
mod menu;
mod motion_timing;
mod motion_math;
mod npc;
mod locomotion;
mod panel;
mod possession;
mod procedural;
mod props;
mod ragdoll;
mod rig_profile;
mod generated_motion;
mod body_clearance;
mod reference_pose;
mod remote;
mod sfx;
mod sim_bridge;
mod studio;
mod ui_theme;
mod support;
mod terrain;

use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

/// Repo root, for files the boss and the agents share (feed log, screenshots).
pub const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// A JSON file under the repo root; Null when missing or malformed (indexing Null yields Null).
pub fn read_json(rel: &str) -> serde_json::Value {
    std::fs::read_to_string(format!("{ROOT}/{rel}")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn main() {
    App::new()
        .insert_resource(ClearColor(camera::SKY))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 350.0, ..default() })
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Guildbound - beta".into(),
                        resolution: (1600, 900).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin { file_path: format!("{ROOT}/assets"), ..default() }),
        )
        .add_plugins(EguiPlugin::default())
        .add_plugins((studio::plugin, creator::plugin, editor::plugin))
        .add_plugins(possession::plugin)
        .add_plugins(reference_pose::plugin)
        .add_plugins(bevy_inspector_egui::quick::WorldInspectorPlugin::new().run_if(bevy::input::common_conditions::input_toggle_active(false, KeyCode::F1)))
        .add_plugins(bevy_lunex::UiLunexPlugins)
        .add_plugins(avian3d::prelude::PhysicsPlugins::new(PostUpdate))
        .init_state::<menu::AppState>()
        .add_plugins((remote::plugin(), RemoteHttpPlugin::default()))
        .insert_resource(sim_bridge::Sim::new())
        .init_resource::<feed::DevFeed>()
        .init_resource::<remote::RemoteActivity>()
        .init_resource::<panel::PanelState>()
        .init_resource::<anim::AnimSettings>()
        .init_resource::<controller::LocomotionMap>()
        .init_resource::<feedback::FeedbackSettings>()
        .init_resource::<anim::AnimRequests>()
        .init_resource::<anim::ClusterCursor>()
        .init_resource::<ragdoll::Ragdolls>()
        .init_resource::<support::Support>()
        .init_resource::<support::SupportSettings>()
        .init_resource::<support::StageSupport>()
        .init_resource::<guard_ik::GuardIk>()
        .init_resource::<sfx::SfxSettings>()
        .init_resource::<npc::NpcLibraries>()
        .insert_resource(anim::Libraries::scan())
        .add_systems(Startup, (anim::load_library, sim_bridge::setup_scene, arena::spawn, terrain::spawn, camera::spawn, menu::spawn_ui_camera, sfx::load, hud::load_icons, announce))
        .add_systems(PostStartup, (props::spawn, magic_view::setup, npc::spawn))
        .add_systems(OnEnter(menu::AppState::MainMenu), menu::spawn_menu)
        .add_systems(OnExit(menu::AppState::MainMenu), menu::despawn_menu)
        .add_systems(
            Update,
            (
                menu::toggle_menu,
                menu::trace_close_requests,
                sim_bridge::read_player_input.run_if(in_state(menu::AppState::Playing).and_then(camera::on_player)),
                sim_bridge::step_sim,
                sim_bridge::ensure_views,
                props::drive_bodies,
                sim_bridge::sync_views,
                anim::switch_library,
                anim::build_library,
                anim::drive_animation,
                anim::lean,
                sim_bridge::show_capsules,
                sim_bridge::draw_gizmos,
                loot_view::sync,
                loot_view::bob,
                magic_view::draw_magic,
                camera::orbit,
                sfx::play,
                npc::build_graphs,
                npc::drive,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (anim::inertialize, anim::lock_root_motion, generated_motion::apply, procedural::bone_lengths, anim::procedural_layers, support::apply, guard_ik::apply, procedural::fighters, studio::apply_corrections, possession::apply, support::stage, generated_motion::plant_feet, joints::apply, body_clearance::apply, joints::apply, knee::apply, studio::measure, possession::measure, ragdoll::prepare, props::physics_clock).chain().after(bevy::app::AnimationSystems).before(avian3d::prelude::PhysicsSystems::First),
        )
        .add_systems(PostUpdate, ragdoll::apply.after(avian3d::prelude::PhysicsSystems::Last).before(bevy::transform::TransformSystems::Propagate))
        .add_systems(PostUpdate, npc::life.after(bevy::app::AnimationSystems).before(avian3d::prelude::PhysicsSystems::First))
        .add_systems(PostUpdate, possession::physical_drive.after(possession::measure).before(ragdoll::prepare))
        .add_systems(PostUpdate, possession::physical_skin.after(avian3d::prelude::PhysicsSystems::Last).before(bevy::transform::TransformSystems::Propagate))
        .add_systems(PostUpdate, magic_view::sync_orbs.after(bevy::transform::TransformSystems::Propagate))
        .add_systems(EguiPrimaryContextPass, panel::panel)
        .run();
}

fn announce(mut feed: ResMut<feed::DevFeed>) {
    feed.post("game", "Dev build up. Remote API on http://127.0.0.1:15702 (python -I Tools/gb.py state).", 0);
}
