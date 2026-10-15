//! Player input hooks onto sim::api + Avian. Each action is a small callable function; the key bindings
//! only call them. Body [0] is the player, [1] the enemy dummy.
//! Joint targets come only from anim::proc (body::drive, every tick, from the body's mode/velocity);
//! input supplies wishes (v_want, heading), ProcState triggers and the jump/spin forces, never targets.
//!   WASD   move     sets v_want (walk 1.5 m/s, Shift run 4 m/s) relative to the camera; no forces. The gait
//!                   controller (phys_gait lean + foot placement, anim::gait pose) does the walking.
//!   Space  jump     starts a proc JumpState (crouch/push/air/land keyframes) + upward impulse, if a foot is down
//!   L      launch   sim::api::launch (ragdoll) -> recover after RECOVER_S (motor fades back in before touchdown)
//!   G held grab     sim::api::grab hand->enemy ankle, yaw torque on the pelvis; proc spin_footwork steps the feet
//! Keys are read in Update; held forces are applied in FixedUpdate so each 1/120 s tick gets exactly one.

use avian3d::prelude::*;
use bevy::prelude::*;
use guildbound_sim::api::{BodyMode, Sim};

use crate::body::{Bodies, Physical, SimRes};

pub const WALK_SPEED: f32 = 1.5;
pub const RUN_SPEED: f32 = 4.0;
pub const JUMP_IMPULSE: f32 = 260.0;
/// Launch velocity (m/s) applied to every segment.
pub const LAUNCH_V: Vec3 = Vec3::new(0.0, 5.5, -3.0);
pub const RECOVER_S: f32 = 0.8;
pub const GRAB_REACH: f32 = 0.7;
pub const SPIN_TORQUE: f32 = 500.0;

#[derive(Resource, Default)]
pub struct Actions {
    pub launched_for: Option<f32>,
    pub grabbing: bool,
    /// Latched in Update, consumed on the next fixed tick.
    dir: Vec3,
    jump: bool,
    launch: bool,
    grab: bool,
    release: bool,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Actions>()
        .add_systems(Update, keys)
        .add_systems(FixedUpdate, act);
}

/// WASD: wanted velocity and heading for body `i` (zero = stand). The gait controller moves the body.
pub fn set_move(bodies: &mut Bodies, i: usize, v_want: Vec3) {
    if let Some(v) = bodies.v_want.get_mut(i) {
        *v = Vec3::new(v_want.x, 0.0, v_want.z);
    }
    let h = if v_want.length_squared() > 1e-6 {
        Some((v_want.x as f64).atan2(v_want.z as f64))
    } else {
        None
    };
    if let Some(x) = bodies.heading_want.get_mut(i) {
        if h.is_some() {
            *x = h;
        }
    }
}

/// Space: proc jump keyframes + an upward impulse on the pelvis.
pub fn jump(bodies: &mut Bodies, i: usize, forces: &mut Query<Forces>) {
    let tick = bodies.ticks;
    let grounded = bodies
        .last
        .get(i)
        .map(|s| s.feet_down[0] || s.feet_down[1])
        .unwrap_or(false);
    if !grounded
        || bodies
            .procs
            .get(i)
            .map(|p| p.jump.is_some())
            .unwrap_or(true)
    {
        return;
    }
    bodies.procs[i].jump = Some(guildbound_sim::anim::proc::JumpState::new(tick));
    if let Ok(mut f) = forces.get_mut(bodies.list[i].links[0]) {
        f.apply_linear_impulse(Vec3::Y * JUMP_IMPULSE);
    }
}

/// L: launch self as a ragdoll (sim::api::launch: motor weight 0; the game applies launch_impulse next tick).
pub fn launch(sim: &mut Sim, p: &Physical) {
    sim.launch(
        p.id,
        guildbound_sim::math::Vec3::new(LAUNCH_V.x as f64, LAUNCH_V.y as f64, LAUNCH_V.z as f64),
    );
}

/// Air recovery: sim fades the motor back in (phys_air::recover_weight) to reach 1 before touchdown.
pub fn recover(sim: &mut Sim, p: &Physical) {
    sim.recover(p.id);
}

/// G: grab the enemy's nearest ankle with the player's right hand if within reach. Returns true when grabbed.
pub fn grab(
    sim: &mut Sim,
    player: &Physical,
    enemy: &Physical,
    q: &Query<(&Position, &Rotation)>,
) -> bool {
    use guildbound_sim::api::JointId;
    let Some(hand_i) = player.names.iter().position(|n| *n == "B_R_Forearm") else {
        return false;
    };
    let Ok((hp, hr)) = q.get(player.links[hand_i]) else {
        return false;
    };
    let hand = hp.0 + hr.0 * Vec3::new(0.0, -0.13, 0.0);
    let best = enemy
        .feet
        .iter()
        .flatten()
        .filter_map(|&i| {
            q.get(enemy.links[i])
                .ok()
                .map(|(p, _)| (i, p.0.distance(hand)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    match best {
        Some((ankle, d)) if d <= GRAB_REACH => {
            sim.grab(
                player.id,
                JointId(hand_i as u16),
                enemy.id,
                JointId(ankle as u16),
            );
            true
        }
        _ => false,
    }
}

/// Spin while holding: yaw torque on the player's pelvis; the enemy follows through the link (stays physical).
/// The feet step around the pair's COM via anim::proc spin_footwork (Grabbing mode).
pub fn spin(player: &Physical, forces: &mut Query<Forces>) {
    if let Ok(mut f) = forces.get_mut(player.links[0]) {
        f.apply_torque(Vec3::Y * SPIN_TORQUE);
    }
}

pub fn release_grab(sim: &mut Sim, player: &Physical) {
    sim.release_grab(player.id);
}

fn keys(
    k: Res<ButtonInput<KeyCode>>,
    cam: Query<&Transform, With<Camera3d>>,
    mut a: ResMut<Actions>,
) {
    // Camera-relative: W = away from the camera along the floor.
    let (fwd, right) = cam
        .iter()
        .next()
        .map(|t| {
            let f = Vec3::new(t.forward().x, 0.0, t.forward().z).normalize_or(Vec3::NEG_Z);
            (f, Vec3::new(-f.z, 0.0, f.x))
        })
        .unwrap_or((Vec3::NEG_Z, Vec3::X));
    let mut dir = Vec3::ZERO;
    if k.pressed(KeyCode::KeyW) {
        dir += fwd;
    }
    if k.pressed(KeyCode::KeyS) {
        dir -= fwd;
    }
    if k.pressed(KeyCode::KeyA) {
        dir -= right;
    }
    if k.pressed(KeyCode::KeyD) {
        dir += right;
    }
    let speed = if k.pressed(KeyCode::ShiftLeft) || k.pressed(KeyCode::ShiftRight) {
        RUN_SPEED
    } else {
        WALK_SPEED
    };
    a.dir = dir.normalize_or_zero() * speed;
    a.jump |= k.just_pressed(KeyCode::Space);
    a.launch |= k.just_pressed(KeyCode::KeyL);
    a.grab |= k.just_pressed(KeyCode::KeyG);
    a.release |= k.just_released(KeyCode::KeyG);
}

fn act(
    time: Res<Time>,
    mut sim: ResMut<SimRes>,
    mut bodies: ResMut<Bodies>,
    mut a: ResMut<Actions>,
    mut set: ParamSet<(Query<(&Position, &Rotation)>, Query<Forces>)>,
) {
    if bodies.list.len() < 2 {
        return;
    }
    // The sim drops a grab on its own (break force, launch): follow it.
    if a.grabbing
        && bodies
            .last
            .first()
            .map(|s| s.mode != BodyMode::Grabbing)
            .unwrap_or(true)
    {
        a.grabbing = false;
    }
    if std::mem::take(&mut a.grab) && !a.grabbing {
        a.grabbing = grab(&mut sim.sim, &bodies.list[0], &bodies.list[1], &set.p0());
    }
    let mut forces = set.p1();
    let v = if a.launched_for.is_none() {
        a.dir
    } else {
        Vec3::ZERO
    };
    set_move(&mut bodies, 0, v);
    set_move(&mut bodies, 1, Vec3::ZERO); // dummy just stands
    if std::mem::take(&mut a.jump) && a.launched_for.is_none() {
        jump(&mut bodies, 0, &mut forces);
    }
    if std::mem::take(&mut a.launch) && a.launched_for.is_none() {
        if a.grabbing {
            release_grab(&mut sim.sim, &bodies.list[0]);
            a.grabbing = false;
        }
        launch(&mut sim.sim, &bodies.list[0]);
        a.launched_for = Some(0.0);
    }
    if let Some(t) = a.launched_for.as_mut() {
        *t += time.delta_secs();
        if *t >= RECOVER_S {
            recover(&mut sim.sim, &bodies.list[0]);
            a.launched_for = None;
        }
    }
    if a.grabbing {
        spin(&bodies.list[0], &mut forces);
    }
    if std::mem::take(&mut a.release) && a.grabbing {
        release_grab(&mut sim.sim, &bodies.list[0]);
        a.grabbing = false;
    }
}
