//! Third-person action camera, reimagined from Snaiel's Godot Sekiro-style controller (MIT: follow pivot,
//! free look, lock-on framing, lazy auto-yaw) and the spring-arm idea (Godot docs), improved for our sim:
//! - Pivot at chest height with a shoulder offset (wider in combat) and a velocity look-ahead, smoothed
//!   frame-rate independently, so a 6 m/s sprint never drags the view.
//! - Free look (play mode: cursor locked, mouse steers; dev mode: middle-drag). Pitch can look up.
//! - Lazy follow: with no mouse input for a moment while moving, the camera drifts behind the travel
//!   direction in proportion to sideways motion (never fights the mouse).
//! - Lock-on: swings behind the player toward the target and pitches to hold the target in the upper third,
//!   pulling back as they separate.
//! - Spring arm: the boom is cast against static physics colliders and the sim's round obstacles (tree
//!   trunks, rocks, standing NPCs); it snaps in instantly and eases back out, so walls and trees never block.
//! - Feel: trauma-squared shake on hits, FOV widens with speed. Studio mode can take over the focus.
use avian3d::prelude::{RigidBody, SpatialQuery, SpatialQueryFilter};
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
    /// Base boom length (wheel zoom); combat and lock-on adjust from it.
    pub distance: f32,
    pub focus: Vec3,
    pub follow_player: bool,
    pub sensitivity: f32,
    /// Studio mode looks at this point instead of the player.
    pub studio_focus: Option<Vec3>,
    /// Current (collision-shortened) boom length and seconds since the last mouse look.
    boom: f32,
    idle_look: f32,
}

pub fn spawn(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::default(),
        Projection::Perspective(PerspectiveProjection { fov: 55f32.to_radians(), ..default() }),
        // HDR + bloom so emissive magic glows
        bevy::camera::Hdr,
        bevy::post_process::bloom::Bloom::NATURAL,
        OrbitCamera { yaw: 0.0, pitch: 0.32, distance: 4.4, focus: Vec3::new(0.0, 1.45, 0.0), follow_player: true, sensitivity: 0.0025, studio_focus: None, boom: 4.4, idle_look: 0.0 },
        DistanceFog { color: SKY, falloff: FogFalloff::Linear { start: 40.0, end: 140.0 }, ..default() },
    ));
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Exponential smoothing factor for a rate (1/s) over dt: frame-rate independent.
fn ease(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// First hit distance of a ray against vertical cylinders (sim obstacles: centre, radius; 4 m tall).
pub fn ray_vs_cylinders(origin: Vec3, dir: Vec3, max: f32, cylinders: &[(gs::V2, f32)]) -> Option<f32> {
    let (ox, oz, dx, dz) = (origin.x, origin.z, dir.x, dir.z);
    let a = dx * dx + dz * dz;
    if a < 1e-8 {
        return None;
    }
    let mut best: Option<f32> = None;
    for &(c, r) in cylinders {
        let (px, pz) = (ox - c.x, oz - c.z);
        let b = 2.0 * (px * dx + pz * dz);
        let cc = px * px + pz * pz - r * r;
        let disc = b * b - 4.0 * a * cc;
        if disc < 0.0 {
            continue;
        }
        let t = (-b - disc.sqrt()) / (2.0 * a);
        let y = origin.y + dir.y * t;
        if t > 0.0 && t < max && (0.0..4.0).contains(&y) && best.is_none_or(|bt| t < bt) {
            best = Some(t);
        }
    }
    best
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
    spatial: SpatialQuery,
    bodies: Query<&RigidBody>,
) {
    let in_menu = *state.get() == crate::menu::AppState::MainMenu;
    let (mut orbit, mut transform, mut projection) = cam.into_inner();
    let (win, mut cursor) = window.into_inner();
    let dt = time.delta_secs().min(0.05);
    let ended = sim.world.fighters[0].hp <= 0.0;
    let studio = orbit.studio_focus.is_some();
    let play = panel.hidden && win.focused && !in_menu && !ended && !studio;

    // play mode locks the cursor; dev and studio modes free it
    let want = if play { CursorGrabMode::Locked } else { CursorGrabMode::None };
    if cursor.grab_mode != want {
        cursor.grab_mode = want;
        cursor.visible = !play;
    }

    let over_panel = !panel.hidden && win.cursor_position().is_some_and(|p| p.x <= panel.width);
    let looking = play || (!over_panel && buttons.pressed(MouseButton::Middle));
    let me = &sim.world.fighters[0];
    let lock = me.lock_target.and_then(|j| sim.world.fighters.get(j)).filter(|t| t.hp > 0.0);
    if in_menu {
        // menu backdrop: a slow cinematic orbit of the live arena
        orbit.yaw += 0.12 * dt;
    } else if looking && motion.delta.length_squared() > 0.0 {
        orbit.yaw -= motion.delta.x * orbit.sensitivity;
        orbit.pitch = (orbit.pitch + motion.delta.y * orbit.sensitivity).clamp(-0.55, 1.25);
        orbit.idle_look = 0.0;
    } else {
        orbit.idle_look += dt;
    }
    if !in_menu && win.focused && !over_panel && scroll.delta.y != 0.0 {
        orbit.distance = (orbit.distance * (1.0 - 0.1 * scroll.delta.y.signum())).clamp(1.8, 30.0);
    }

    let combat = sim.world.in_combat(0);
    let vel = Vec3::new(me.velocity.x, 0.0, me.velocity.z);
    let p = Vec3::new(me.position.x, 1.45 + me.height, me.position.z);
    let (target_focus, mut distance) = if let Some(f) = orbit.studio_focus {
        (f, orbit.distance)
    } else {
        match lock {
            Some(t) => {
                // behind the player, toward the target; hold the target high in frame, pull back with separation
                let to = Vec3::new(t.position.x - me.position.x, 0.0, t.position.z - me.position.z);
                let sep = to.length();
                if sep > 0.05 {
                    let want_yaw = (-to.x).atan2(-to.z);
                    let err = wrap(want_yaw - orbit.yaw);
                    if err.abs() > 0.05 {
                        orbit.yaw += (err - 0.05 * err.signum()) * ease(4.0, dt);
                    }
                    // pitch so the target's head sits in the upper third: look down more when close
                    let want_pitch = (0.42 - 0.035 * sep).clamp(0.12, 0.45) + 0.4 * ((me.height - t.height) / sep.max(1.0));
                    orbit.pitch += (want_pitch - orbit.pitch) * ease(3.0, dt);
                }
                let head = Vec3::new(t.position.x, 1.55 + t.height, t.position.z);
                (p.lerp(head, 0.3), orbit.distance.max(3.4 + sep * 0.45))
            }
            None if orbit.follow_player => {
                // lazy follow: after 0.6 s without mouse look, drift behind the travel direction
                let speed = vel.length();
                if !in_menu && orbit.idle_look > 0.6 && speed > 0.6 {
                    let behind = (-vel.x).atan2(-vel.z);
                    let err = wrap(behind - orbit.yaw);
                    let side = err.sin().abs();
                    orbit.yaw += err * ease(1.2 * side * (speed / 4.0).min(1.5), dt);
                }
                // look-ahead so a sprint leads the frame instead of trailing it
                (p + vel * 0.12, orbit.distance)
            }
            None => (Vec3::new(0.0, 1.0, 0.0), orbit.distance),
        }
    };
    if combat && lock.is_none() && !studio {
        distance *= 0.9;
    }
    orbit.focus = orbit.focus.lerp(target_focus, ease(14.0, dt));

    // hit shake as trauma: each contact adds trauma (0.1 m of profile shake = 1.0) that decays over 0.35 s;
    // the offset is trauma squared, so light hits barely move the view and deathblows punch
    let now = time.elapsed_secs();
    let contact_now = sim.world.tick as f32 * gs::STEP;
    let trauma: f32 = sim
        .recent
        .iter()
        .filter(|(c, _)| c.attacker == 0 || c.defender == 0)
        .map(|(c, at)| {
            let age = contact_now - at;
            let t = crate::feedback::profile(c.outcome).shake_m / 0.1;
            if age < 0.35 { t * (1.0 - age / 0.35) } else { 0.0 }
        })
        .sum::<f32>()
        .min(1.0);
    let wobble = Vec3::new((now * 37.0).sin() + 0.5 * (now * 91.0).sin(), (now * 43.0).sin(), (now * 29.0).cos() + 0.5 * (now * 77.0).cos());
    let shake = wobble * 0.14 * trauma * trauma * if sim.paused || in_menu { 0.0 } else { feedback.shake_scale };

    // boom direction and a shoulder offset to the right of the view (wider in combat)
    let (sy, cy) = orbit.yaw.sin_cos();
    let (sp, cp) = orbit.pitch.sin_cos();
    let back = Vec3::new(sy * cp, sp, cy * cp);
    let right = Vec3::new(cy, 0.0, -sy);
    let shoulder = if studio || in_menu { 0.0 } else if combat { 0.55 } else { 0.35 };
    let pivot = orbit.focus + right * shoulder;

    // spring arm: shortest clear boom against static colliders and sim obstacles; in fast, out slow
    let clear = {
        let dir = Dir3::new(back).unwrap_or(Dir3::Z);
        let filter = SpatialQueryFilter::default();
        let statics = |e: Entity| bodies.get(e).is_ok_and(|b| *b == RigidBody::Static);
        let phys = spatial.cast_ray_predicate(pivot, dir, distance, true, &filter, &statics).map(|h| h.distance);
        let trunks = ray_vs_cylinders(pivot, back, distance, &sim.world.obstacles);
        let hit = match (phys, trunks) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        hit.map_or(distance, |d| (d - 0.3).max(0.6))
    };
    orbit.boom = if clear < orbit.boom { clear } else { orbit.boom + (clear - orbit.boom) * ease(3.0, dt) };

    let eye = pivot + back * orbit.boom + shake;
    *transform = Transform::from_translation(eye).looking_at(pivot + shake * 0.5, Vec3::Y);

    // speed FOV: 55 deg standing, up to about 64 at a sprint
    if let Projection::Perspective(pp) = projection.as_mut() {
        let speed = vel.length() / sim.world.tuning.run_speed.max(0.1);
        let want = (55.0 + 6.0 * speed.clamp(0.0, 1.6)).to_radians();
        pp.fov += (want - pp.fov) * ease(5.0, dt);
    }
}

/// Lock-on target: the living fighter nearest the screen centre within 25 m in front of the camera.
/// `side` = Some(+1/-1) picks the nearest target to the right/left of the current one (mouse flick).
pub fn pick_target(sim: &gs::World, cam: &Transform, current: Option<usize>, side: Option<f32>) -> Option<usize> {
    let fwd = cam.forward().as_vec3();
    let right = cam.right().as_vec3();
    let me = sim.fighters[0].position;
    let score = |j: usize| {
        let f = &sim.fighters[j];
        let to = Vec3::new(f.position.x, 1.2 + f.height, f.position.z) - cam.translation;
        let d = (f.position - me).length();
        let ahead = to.normalize_or_zero().dot(fwd);
        (d <= 25.0 && ahead > 0.3).then(|| (j, ahead, to.normalize_or_zero().dot(right)))
    };
    let cands: Vec<_> = (1..sim.fighters.len()).filter(|&j| sim.fighters[j].hp > 0.0).filter_map(score).collect();
    match (side, current) {
        (Some(s), Some(cur)) => {
            let cur_x = cands.iter().find(|c| c.0 == cur).map_or(0.0, |c| c.2);
            cands.iter().filter(|c| c.0 != cur && (c.2 - cur_x) * s > 0.0).min_by(|a, b| (a.2 - cur_x).abs().total_cmp(&(b.2 - cur_x).abs())).map(|c| c.0).or(Some(cur))
        }
        _ => cands.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|c| c.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spring_arm_stops_at_the_first_trunk() {
        let trees = [(gs::V2::new(0.0, 3.0), 0.5), (gs::V2::new(0.0, 6.0), 0.5)];
        let hit = ray_vs_cylinders(Vec3::new(0.0, 1.5, 0.0), Vec3::Z, 10.0, &trees).unwrap();
        assert!((hit - 2.5).abs() < 1e-4, "{hit}");
        assert!(ray_vs_cylinders(Vec3::new(0.0, 1.5, 0.0), Vec3::X, 10.0, &trees).is_none());
        assert!(ray_vs_cylinders(Vec3::new(0.0, 5.0, 0.0), Vec3::Z, 10.0, &trees).is_none(), "above the trunks");
    }
}
