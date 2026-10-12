//! Third-person combat camera.
//! - Play mode (Tab hides the dev panel): cursor locked, the mouse steers the camera directly.
//! - Dev mode (panel visible): cursor free for the UI, middle-drag orbits. The wheel zooms in both.
//! - Lock-on: swings behind the player aimed at the target, frames both, pulls back as they separate.
//! - Feel: short shake on hits (stronger on counter-hits, guard breaks, parries), FOV widens with speed.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use guildbound_sim as gs;

use crate::panel::PanelState;
use crate::sim_bridge::Sim;

/// Sky and fog colour (also the clear colour).
pub const SKY: Color = Color::srgb(0.62, 0.71, 0.80);

#[derive(Component)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub focus: Vec3,
    pub follow_player: bool,
    pub sensitivity: f32,
}

pub fn spawn(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::default(),
        Projection::Perspective(PerspectiveProjection { fov: 55f32.to_radians(), ..default() }),
        // HDR + bloom so emissive magic glows
        bevy::camera::Hdr,
        bevy::post_process::bloom::Bloom::NATURAL,
        OrbitCamera { yaw: 0.0, pitch: 0.38, distance: 6.5, focus: Vec3::ZERO, follow_player: true, sensitivity: 0.0025 },
        DistanceFog { color: SKY, falloff: FogFalloff::Linear { start: 30.0, end: 110.0 }, ..default() },
    ));
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

pub fn orbit(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    panel: Res<PanelState>,
    sim: Res<Sim>,
    time: Res<Time>,
    cam: Single<(&mut OrbitCamera, &mut Transform, &mut Projection)>,
    state: Res<State<crate::menu::AppState>>,
    feedback: Res<crate::feedback::FeedbackSettings>,
) {
    let in_menu = *state.get() == crate::menu::AppState::MainMenu;
    let (mut orbit, mut transform, mut projection) = cam.into_inner();
    let (win, mut cursor) = window.into_inner();
    let dt = time.delta_secs().min(0.05);
    let ended = sim.world.fighters[0].hp <= 0.0 || sim.world.fighters[1].hp <= 0.0;
    let play = panel.hidden && win.focused && !in_menu && !ended;

    // play mode locks the cursor; dev mode frees it for the panel
    let want = if play { CursorGrabMode::Locked } else { CursorGrabMode::None };
    if cursor.grab_mode != want {
        cursor.grab_mode = want;
        cursor.visible = !play;
    }

    let over_panel = !panel.hidden && win.cursor_position().is_some_and(|p| p.x <= panel.width);
    if in_menu {
        // menu backdrop: a slow cinematic orbit of the live arena
        orbit.yaw += 0.12 * dt;
    } else if play || (!over_panel && buttons.pressed(MouseButton::Middle)) {
        orbit.yaw -= motion.delta.x * orbit.sensitivity;
        orbit.pitch = (orbit.pitch + motion.delta.y * orbit.sensitivity).clamp(0.05, 1.3);
    }
    if !in_menu && win.focused && !over_panel && scroll.delta.y != 0.0 {
        orbit.distance = (orbit.distance * (1.0 - 0.1 * scroll.delta.y.signum())).clamp(2.5, 30.0);
    }

    let me = &sim.world.fighters[0];
    let p = Vec3::new(me.position.x, 1.0, me.position.z);
    let lock = me.lock_target.and_then(|j| sim.world.fighters.get(j));
    let (target_focus, distance) = match lock {
        Some(t) => {
            // behind the player, looking at the target; frame both and pull back with separation
            let to = Vec3::new(t.position.x - me.position.x, 0.0, t.position.z - me.position.z);
            let sep = to.length();
            if sep > 0.05 {
                let want_yaw = (-to.x).atan2(-to.z);
                // soft follow with a small dead zone, so lock-on guides the camera instead of yanking it
                let err = wrap(want_yaw - orbit.yaw);
                let k = 1.0 - (-3.0 * dt).exp();
                if err.abs() > 0.06 {
                    orbit.yaw += (err - 0.06 * err.signum()) * k;
                }
            }
            // aim between the player and the target's head
            let head = Vec3::new(t.position.x, 1.55 + t.height, t.position.z);
            (p.lerp(head, 0.35), orbit.distance.max(4.5 + sep * 0.5))
        }
        None if orbit.follow_player => (p, orbit.distance),
        None => (Vec3::new(0.0, 0.5, 0.0), orbit.distance),
    };
    let k = 1.0 - (-8.0 * dt).exp();
    orbit.focus = orbit.focus.lerp(target_focus, k);

    // hit shake: recent contacts, decaying over 0.18 s
    let now = time.elapsed_secs();
    let contact_now = sim.world.tick as f32 * gs::STEP;
    let trauma: f32 = sim
        .recent
        .iter()
        .filter(|(c, _)| c.attacker == 0 || c.defender == 0)
        .map(|(c, at)| {
            let age = contact_now - at;
            let strength = crate::feedback::profile(c.outcome).shake_m;
            if age < 0.18 { strength * (1.0 - age / 0.18) } else { 0.0 }
        })
        .sum::<f32>()
        .min(0.2);
    let shake = Vec3::new((now * 71.0).sin(), (now * 53.0).sin(), (now * 61.0).cos()) * if sim.paused || in_menu { 0.0 } else { trauma * feedback.shake_scale };

    let (sy, cy) = orbit.yaw.sin_cos();
    let (sp, cp) = orbit.pitch.sin_cos();
    let eye = orbit.focus + Vec3::new(sy * cp, sp, cy * cp) * distance + shake;
    *transform = Transform::from_translation(eye).looking_at(orbit.focus + shake * 0.5, Vec3::Y);

    // speed FOV: 55 deg standing, up to about 63 at a full run
    if let Projection::Perspective(pp) = projection.as_mut() {
        let speed = me.velocity.length() / sim.world.tuning.run_speed.max(0.1);
        let want = (55.0 + 8.0 * speed.clamp(0.0, 1.2)).to_radians();
        pp.fov += (want - pp.fov) * (1.0 - (-5.0 * dt).exp());
    }
}
