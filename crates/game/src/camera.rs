//! Orbit camera: right-drag orbits, wheel zooms. Ignores the mouse over the dev panel.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::panel::PanelState;
use crate::sim_bridge::Sim;

#[derive(Component)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub focus: Vec3,
    pub follow_player: bool,
}

pub fn spawn(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::default(),
        OrbitCamera { yaw: 0.0, pitch: 0.75, distance: 13.0, focus: Vec3::ZERO, follow_player: false },
    ));
}

pub fn orbit(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window, With<PrimaryWindow>>,
    panel: Res<PanelState>,
    sim: Res<Sim>,
    time: Res<Time>,
    cam: Single<(&mut OrbitCamera, &mut Transform)>,
) {
    let (mut orbit, mut transform) = cam.into_inner();
    let over_panel = window.cursor_position().is_some_and(|p| p.x <= panel.width);
    if !over_panel {
        if buttons.pressed(MouseButton::Right) {
            orbit.yaw -= motion.delta.x * 0.005;
            orbit.pitch = (orbit.pitch + motion.delta.y * 0.005).clamp(0.08, 1.5);
        }
        if scroll.delta.y != 0.0 {
            orbit.distance = (orbit.distance * (1.0 - 0.1 * scroll.delta.y.signum())).clamp(3.0, 40.0);
        }
    }
    let target = if orbit.follow_player {
        let p = sim.world.fighters[0].position;
        Vec3::new(p.x, 0.9, p.z)
    } else {
        Vec3::new(0.0, 0.5, 0.0)
    };
    let k = 1.0 - (-8.0 * time.delta_secs()).exp();
    orbit.focus = orbit.focus.lerp(target, k);
    let (sy, cy) = orbit.yaw.sin_cos();
    let (sp, cp) = orbit.pitch.sin_cos();
    let eye = orbit.focus + Vec3::new(sy * cp, sp, cy * cp) * orbit.distance;
    *transform = Transform::from_translation(eye).looking_at(orbit.focus, Vec3::Y);
}
