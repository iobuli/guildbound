//! Kinematic capsule controller in `FixedUpdate / PlaySet::Move` using Avian `MoveAndSlide`:
//! ground/air modes, snappy accel/brake (Feel), turn rate, coyote time, variable jump, gravity,
//! ground snap + slope limit, step-up, `Mover.external` knockback decay, `root_motion`,
//! `move_scale`, `face`. Computes `Mover.accel` (smoothed), `landing`, `grounded`, `air_time`.
//! Does NOT touch `Mover.phase` (select owns it). Swim/Climb/Ledge/Ragdoll
//! modes are moved by `traverse` / `ragdoll` instead (skip them here).
//!
//! Position: the root `Transform` is written inside the fixed step (like Avian's
//! `move_and_slide_3d` example). Avian's interpolation records the eased start in `FixedFirst`
//! and the end in `FixedLast`, so render frames see a smooth root; `LinearVelocity` stays zero,
//! so the integrator never moves the kinematic body on its own. Teleports go through the
//! `Teleport` component so they also happen inside the fixed step.
//!
//! Frame/tick bookkeeping: `Intent.jump_pressed` is latched in `Update` (edges survive frames
//! with zero or several fixed ticks) and buffered for `JUMP_BUFFER`; `Mover.root_motion` is a
//! per-frame velocity request, so it is consumed by every tick of the frame and cleared after
//! the frame's LAST fixed tick.
//!
//! Characters whose model is not bound yet (no `motion::Rig`) are drawn as gizmo capsules.
use super::{CAPSULE_LENGTH, CAPSULE_RADIUS, Feel, Intent, MoveMode, Mover, Npc, Phys, PlaySet, Player};
use crate::motion::Rig;
use avian3d::prelude::*;
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};
use std::time::Duration;

/// Seconds a jump press stays buffered.
pub const JUMP_BUFFER: f32 = 0.12;
/// Upward velocity is multiplied by this once when the jump button is released early.
pub const JUMP_CUT: f32 = 0.45;
/// Steepest walkable slope (deg).
pub const SLOPE_LIMIT_DEG: f32 = 50.0;
/// Highest ledge the controller steps onto without a mantle (m).
pub const STEP_HEIGHT: f32 = 0.35;
/// Ground snap reach while grounded (m) and the touch-down probe while falling.
pub const SNAP_DISTANCE: f32 = 0.3;
const GROUND_PROBE: f32 = 0.05;
/// Knockback half-lives (s) on the ground and in the air.
pub const EXTERNAL_HALF_LIFE_GROUND: f32 = 0.25;
pub const EXTERNAL_HALF_LIFE_AIR: f32 = 1.0;
/// `Mover.accel` low-pass time constant (s).
const ACCEL_SMOOTHING: f32 = 0.08;
const MAX_ACCEL: f32 = 80.0;
/// Impact speed (m/s) that maps to `landing = 1`; seconds for `landing` to decay from 1 to 0.
const LANDING_FULL_SPEED: f32 = 12.0;
const LANDING_DECAY: f32 = 0.35;
/// Crouched speed relative to run speed.
const CROUCH_SPEED_SCALE: f32 = 0.45;
/// Analog tilt below this walks.
const WALK_TILT: f32 = 0.5;
/// Turn rate multiplier while airborne.
const AIR_TURN_SCALE: f32 = 0.6;
const MAX_FALL_SPEED: f32 = 45.0;
/// Jumping needs at least this much `move_scale` (actions that lock movement block jumps).
const JUMP_MIN_MOVE_SCALE: f32 = 0.5;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Mover, ControllerState>()
        .add_systems(Update, (latch_jump.after(PlaySet::Think), draw_unbound_capsules))
        .add_systems(FixedUpdate, move_characters.in_set(PlaySet::Move));
}

/// Request to place a character's feet at `feet` facing `yaw` and clear its movement state.
/// Consumed by the controller in the next fixed tick (waits while the character ragdolls).
#[derive(Component, Clone, Copy, Debug)]
pub struct Teleport {
    pub feet: Vec3,
    pub yaw: f32,
}

/// Per-character controller memory (added automatically with `Mover`).
#[derive(Component, Clone, Debug, Default)]
struct ControllerState {
    /// Input-driven horizontal velocity (before external and root motion).
    locomotion: Vec3,
    /// Last actual horizontal velocity (for `Mover.accel`).
    prev_velocity: Vec3,
    jump_latch: bool,
    jump_buffer: f32,
    /// Ascending from a jump (variable-height cut still possible).
    jumping: bool,
    /// The coyote / ground jump since the last touchdown is spent.
    jump_spent: bool,
    /// The controller moved this character last tick (false after traverse/ragdoll owned it).
    owned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Idle,
    Crouch,
    Walk,
    Run,
    Sprint,
}

/// Wrap an angle into (−π, π].
pub fn wrap_angle(a: f32) -> f32 {
    if !a.is_finite() {
        return 0.0;
    }
    let w = (a + PI).rem_euclid(TAU) - PI;
    if w <= -PI { w + TAU } else { w }
}

/// Yaw of a horizontal direction (forward = (sin yaw, 0, cos yaw)).
pub fn yaw_of(dir: Vec3) -> Option<f32> {
    let d = Vec2::new(dir.x, dir.z);
    (d.is_finite() && d.length_squared() > 1e-8).then(|| d.x.atan2(d.y))
}

/// Turn `yaw` toward `target` by at most `max_step` rad the short way round.
pub fn turn_toward(yaw: f32, target: f32, max_step: f32) -> f32 {
    let delta = wrap_angle(target - yaw);
    let step = delta.clamp(-max_step.max(0.0), max_step.max(0.0));
    wrap_angle(yaw + step)
}

/// Move `current` toward `target` at `accel` (speeding up) or `brake` (slowing / reversing)
/// m/s². Constant-rate, so it is frame-rate independent and never overshoots.
pub fn approach_velocity(current: Vec3, target: Vec3, accel: f32, brake: f32, dt: f32) -> Vec3 {
    if !target.is_finite() {
        return Vec3::ZERO;
    }
    if !current.is_finite() {
        return target;
    }
    let delta = target - current;
    let dist = delta.length();
    let slowing = target.length_squared() + 1e-6 < current.length_squared() || current.dot(target) < 0.0;
    let rate = if slowing { brake } else { accel }.max(0.0);
    let step = rate * dt.max(0.0);
    if dist <= step || dist < 1e-6 {
        target
    } else {
        current + delta * (step / dist)
    }
}

/// Exponential decay with a half-life (s).
pub fn decay_half_life(v: Vec3, half_life: f32, dt: f32) -> Vec3 {
    if !v.is_finite() {
        return Vec3::ZERO;
    }
    let out = v * 0.5f32.powf(dt.max(0.0) / half_life.max(1e-4));
    if out.length_squared() < 1e-6 { Vec3::ZERO } else { out }
}

/// Target ground speed (m/s, before `move_scale`) and gait for a stick magnitude 0..1.
pub fn target_speed(feel: &Feel, magnitude: f32, walk: bool, sprint: bool, crouch: bool) -> (f32, Gait) {
    let m = if magnitude.is_finite() { magnitude.clamp(0.0, 1.0) } else { 0.0 };
    let walk_speed = feel.walk_speed.max(0.0);
    let run_speed = feel.run_speed.max(walk_speed);
    if m < 0.05 {
        return (0.0, Gait::Idle);
    }
    if crouch {
        let full = run_speed * CROUCH_SPEED_SCALE;
        let slow = walk || m < WALK_TILT;
        return (if slow { full * 0.6 } else { full }, Gait::Crouch);
    }
    if walk {
        return (walk_speed, Gait::Walk);
    }
    if m < WALK_TILT {
        return (walk_speed * (m / WALK_TILT).max(0.35), Gait::Walk);
    }
    if sprint {
        return (feel.sprint_speed.max(run_speed), Gait::Sprint);
    }
    let t = ((m - WALK_TILT) / 0.4).clamp(0.0, 1.0);
    (walk_speed + (run_speed - walk_speed) * t, Gait::Run)
}

/// Landing impact 0..1 from the downward speed at touchdown.
pub fn landing_impact(impact_speed: f32) -> f32 {
    if impact_speed.is_finite() {
        (impact_speed / LANDING_FULL_SPEED).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Whether a surface normal is walkable ground.
pub fn walkable(normal: Vec3) -> bool {
    normal.is_finite() && normal.y >= SLOPE_LIMIT_DEG.to_radians().cos()
}

/// Horizontal velocity lifted onto the ground plane (keeps horizontal speed; slopes add y).
pub fn slope_tangent(horizontal: Vec3, normal: Vec3) -> Vec3 {
    if !walkable(normal) {
        return horizontal.with_y(0.0);
    }
    let y = -(normal.x * horizontal.x + normal.z * horizontal.z) / normal.y;
    Vec3::new(horizontal.x, y, horizontal.z)
}

fn latch_jump(mut q: Query<(&Intent, &mut ControllerState)>) {
    for (intent, mut state) in &mut q {
        if intent.jump_pressed {
            state.jump_latch = true;
        }
    }
}

/// Result of one controller tick.
struct Step {
    position: Vec3,
    walls: Vec<Vec3>,
}

/// Shared filter: world geometry and other characters, never ourselves.
fn movement_filter(entity: Entity) -> SpatialQueryFilter {
    SpatialQueryFilter::from_mask([Phys::World, Phys::Character]).with_excluded_entities([entity])
}

/// One move-and-slide call. Steep (non-walkable) slopes are treated as vertical walls while
/// grounded, so the capsule never creeps up them; every non-walkable hit normal is recorded.
#[allow(clippy::too_many_arguments)]
fn slide(
    mas: &MoveAndSlide,
    collider: &Collider,
    from: Vec3,
    velocity: Vec3,
    dt: f32,
    ground_plane: Option<Vec3>,
    grounded: bool,
    filter: &SpatialQueryFilter,
) -> Step {
    let mut config = MoveAndSlideConfig::default();
    if let Some(n) = ground_plane.and_then(|n| Dir3::new(n).ok()) {
        config.planes.push(n);
    }
    let mut walls = Vec::new();
    let out = mas.move_and_slide(
        collider,
        from,
        Quat::IDENTITY,
        velocity,
        Duration::from_secs_f32(dt),
        &config,
        filter,
        |hit| {
            let n = hit.normal.as_vec3();
            if !walkable(n) {
                if grounded
                    && n.y > 0.0
                    && let Ok(flat) = Dir3::new(n.with_y(0.0))
                {
                    *hit.normal = flat;
                }
                walls.push(hit.normal.as_vec3());
            }
            MoveAndSlideHitResponse::Accept
        },
    );
    Step {
        position: if out.position.is_finite() { out.position } else { from },
        walls,
    }
}

/// Quake-style step-up: lift, move horizontally, drop back onto walkable ground.
fn try_step_up(
    mas: &MoveAndSlide,
    collider: &Collider,
    from: Vec3,
    horizontal: Vec3,
    dt: f32,
    skin: f32,
    filter: &SpatialQueryFilter,
) -> Option<Vec3> {
    let lift = mas
        .cast_move(collider, from, Quat::IDENTITY, Vec3::Y * STEP_HEIGHT, skin, filter)
        .map_or(STEP_HEIGHT, |h| h.distance);
    if lift < 0.05 {
        return None;
    }
    let raised = from + Vec3::Y * lift;
    let moved = slide(mas, collider, raised, horizontal.with_y(0.0), dt, None, true, filter);
    let drop = mas.cast_move(
        collider,
        moved.position,
        Quat::IDENTITY,
        Vec3::NEG_Y * (lift + GROUND_PROBE),
        skin,
        filter,
    )?;
    if !walkable(drop.normal1) {
        return None;
    }
    let landed = moved.position - Vec3::Y * drop.distance;
    (landed.is_finite() && landed.y >= from.y - 0.02 && landed.y <= from.y + STEP_HEIGHT + 0.02).then_some(landed)
}

#[allow(clippy::type_complexity)]
fn move_characters(
    time: Res<Time<Fixed>>,
    feel: Res<Feel>,
    mas: MoveAndSlide,
    mut commands: Commands,
    mut bodies: Query<(
        Entity,
        &Collider,
        &mut Transform,
        &mut Mover,
        &Intent,
        &mut ControllerState,
        Option<&Teleport>,
    )>,
) {
    let dt = time.delta_secs().clamp(1e-4, 1.0 / 30.0);
    // `root_motion` is a per-frame request: clear it after the frame's last fixed tick.
    let last_tick_of_frame = time.overstep() < time.timestep();
    let skin = mas.length_unit.0 * MoveAndSlideConfig::default().skin_width;
    let feel = &*feel;

    for (entity, collider, mut transform, mut mover, intent, mut state, teleport) in &mut bodies {
        if let Some(tp) = teleport
            && mover.mode != MoveMode::Ragdoll
        {
            commands.entity(entity).remove::<Teleport>();
            if tp.feet.is_finite() && tp.yaw.is_finite() {
                transform.translation = tp.feet + Vec3::Y * super::HALF_HEIGHT;
                transform.rotation = Quat::from_rotation_y(tp.yaw);
                *mover = Mover {
                    yaw: tp.yaw,
                    ..default()
                };
                *state = ControllerState::default();
                continue;
            }
        }

        if matches!(
            mover.mode,
            MoveMode::Swim | MoveMode::Climb | MoveMode::Ledge | MoveMode::Ragdoll
        ) {
            state.owned = false;
            state.jump_latch = false;
            state.jump_buffer = 0.0;
            state.jumping = false;
            if last_tick_of_frame {
                mover.root_motion = Vec3::ZERO;
            }
            continue;
        }

        let start = transform.translation;
        if !start.is_finite() {
            continue;
        }
        if !state.owned {
            // Taking over from traverse / ragdoll / spawn: continue with their velocity.
            let v = if mover.velocity.is_finite() { mover.velocity } else { Vec3::ZERO };
            state.locomotion = v.with_y(0.0);
            state.prev_velocity = v.with_y(0.0);
            state.owned = true;
        }
        let was_grounded = mover.grounded;
        let mut grounded = was_grounded;
        let ground_normal = if walkable(mover.ground_normal) { mover.ground_normal } else { Vec3::Y };
        let move_scale = if mover.move_scale.is_finite() { mover.move_scale.clamp(0.0, 1.0) } else { 1.0 };
        let root_motion = if mover.root_motion.is_finite() { mover.root_motion } else { Vec3::ZERO };
        let mut external = if mover.external.is_finite() { mover.external } else { Vec3::ZERO };
        let mut vy = if mover.velocity.y.is_finite() { mover.velocity.y } else { 0.0 };

        // --- Input.
        let input = if intent.move_dir.is_finite() {
            intent.move_dir.with_y(0.0).clamp_length_max(1.0)
        } else {
            Vec3::ZERO
        };
        let magnitude = input.length();
        mover.crouch = intent.crouch;
        let (speed, gait) = target_speed(feel, magnitude, intent.walk, intent.sprint, mover.crouch);

        // --- Turning.
        let face = mover.face.filter(|f| f.is_finite());
        let wanted_yaw = face.or_else(|| (magnitude > 0.05).then(|| yaw_of(input)).flatten());
        let yaw = if mover.yaw.is_finite() { mover.yaw } else { 0.0 };
        let new_yaw = match wanted_yaw {
            Some(target) => {
                let mut rate = feel.turn_rate.max(0.0).to_radians();
                if face.is_none() {
                    rate *= move_scale;
                }
                if !grounded {
                    rate *= AIR_TURN_SCALE;
                }
                turn_toward(yaw, target, rate * dt)
            }
            None => yaw,
        };
        mover.yaw_rate = wrap_angle(new_yaw - yaw) / dt;
        mover.yaw = new_yaw;

        // --- Horizontal locomotion.
        let target_velocity = input.normalize_or_zero() * speed * move_scale;
        let (accel, brake) = if grounded {
            (feel.accel, feel.brake)
        } else {
            let air = feel.air_control.clamp(0.0, 1.0);
            (feel.accel * air, feel.brake * air)
        };
        state.locomotion = approach_velocity(state.locomotion, target_velocity, accel, brake, dt);
        mover.sprinting = gait == Gait::Sprint && target_velocity.length_squared() > 0.01;

        // --- External: vertical part launches, horizontal part decays.
        if external.y.abs() > 1e-3 {
            vy += external.y;
            external.y = 0.0;
            if vy > 0.0 {
                grounded = false;
                state.jumping = false;
                state.jump_spent = true;
            }
        }
        let half_life = if grounded { EXTERNAL_HALF_LIFE_GROUND } else { EXTERNAL_HALF_LIFE_AIR };
        external = decay_half_life(external, half_life, dt);

        // --- Jump (buffered, coyote, variable height).
        state.jump_buffer = (state.jump_buffer - dt).max(0.0);
        if state.jump_latch {
            state.jump_latch = false;
            state.jump_buffer = JUMP_BUFFER;
        }
        if grounded {
            vy = 0.0;
        }
        let coyote = !grounded && !state.jump_spent && mover.air_time <= feel.coyote_time;
        if state.jump_buffer > 0.0 && (grounded || coyote) && move_scale >= JUMP_MIN_MOVE_SCALE {
            vy = feel.jump_speed.max(0.0);
            grounded = false;
            state.jumping = true;
            state.jump_spent = true;
            state.jump_buffer = 0.0;
            mover.crouch = false;
        }
        if state.jumping {
            if vy <= 0.0 {
                state.jumping = false;
            } else if !intent.jump_held {
                vy *= JUMP_CUT;
                state.jumping = false;
            }
        }
        if !grounded {
            vy = (vy - feel.gravity.max(0.0) * dt).max(-MAX_FALL_SPEED);
        }

        // --- Move.
        let horizontal = state.locomotion + external.with_y(0.0) + root_motion.with_y(0.0);
        let mut velocity = if grounded {
            slope_tangent(horizontal, ground_normal)
        } else {
            horizontal + Vec3::Y * vy
        };
        velocity.y += root_motion.y;
        let filter = movement_filter(entity);
        let plane = grounded.then_some(ground_normal);
        let Step { mut position, mut walls } = slide(&mas, collider, start, velocity, dt, plane, grounded, &filter);

        // --- Step-up when blocked on the ground.
        let wanted_h = Vec2::new(velocity.x, velocity.z) * dt;
        let achieved_h = Vec2::new(position.x - start.x, position.z - start.z);
        let mut stepped = false;
        if grounded
            && !walls.is_empty()
            && wanted_h.length() > 1e-3
            && achieved_h.length() < wanted_h.length() * 0.75
            && let Some(up) = try_step_up(&mas, collider, start, velocity, dt, skin, &filter)
            && Vec2::new(up.x - start.x, up.z - start.z).length() > achieved_h.length() + 1e-3
        {
            position = up;
            stepped = true;
            walls.clear();
        }

        // --- Ground probe + snap.
        let ascending = vy > 0.01 || root_motion.y > 0.1;
        let reach = if (grounded || stepped) && !ascending { SNAP_DISTANCE } else { GROUND_PROBE };
        let mut now_grounded = false;
        let mut normal = Vec3::Y;
        if !ascending
            && let Some(hit) = mas.cast_move(collider, position, Quat::IDENTITY, Vec3::NEG_Y * reach, skin, &filter)
            && walkable(hit.normal1)
        {
            position.y -= hit.distance;
            now_grounded = true;
            normal = hit.normal1.normalize_or(Vec3::Y);
        }

        // --- Vertical bookkeeping, landing.
        let actual = (position - start) / dt;
        mover.landing = (mover.landing - dt / LANDING_DECAY).max(0.0);
        if now_grounded {
            if !was_grounded {
                let impact = -velocity.y.min(0.0);
                mover.landing = mover.landing.max(landing_impact(impact));
            }
            vy = 0.0;
            state.jumping = false;
            state.jump_spent = false;
            mover.air_time = 0.0;
        } else {
            mover.air_time += dt;
            if !grounded {
                // Bonk on ceilings / slide on steep slopes: keep only what actually happened.
                if vy > 0.0 && actual.y < vy * 0.5 {
                    vy = actual.y.max(0.0);
                } else if vy < 0.0 && !walls.is_empty() {
                    vy = vy.max(actual.y.min(0.0));
                }
            } else {
                // Walked off a ledge: start falling from rest (coyote time applies).
                vy = vy.min(0.0);
            }
        }

        // --- External stops on walls.
        for n in &walls {
            let into = external.dot(*n);
            if into < 0.0 {
                external -= *n * into;
            }
        }

        // --- Write back.
        let horizontal_actual = actual.with_y(0.0);
        let raw_accel = ((horizontal_actual - state.prev_velocity) / dt).clamp_length_max(MAX_ACCEL);
        if raw_accel.is_finite() && (position - start).length() < 3.0 {
            mover.accel = mover.accel.lerp(raw_accel, 1.0 - (-dt / ACCEL_SMOOTHING).exp());
        }
        if !mover.accel.is_finite() {
            mover.accel = Vec3::ZERO;
        }
        state.prev_velocity = horizontal_actual;
        mover.velocity = Vec3::new(horizontal_actual.x, vy, horizontal_actual.z);
        mover.external = external;
        mover.grounded = now_grounded;
        mover.ground_normal = normal;
        mover.mode = if now_grounded { MoveMode::Ground } else { MoveMode::Air };
        if last_tick_of_frame {
            mover.root_motion = Vec3::ZERO;
        }
        if position.is_finite() {
            transform.translation = position;
        }
        transform.rotation = Quat::from_rotation_y(mover.yaw);
    }
}

/// Debug: gizmo capsules for characters whose model is not bound yet.
fn draw_unbound_capsules(
    mut gizmos: Gizmos,
    q: Query<(&Transform, &Mover, Has<Player>, Has<Npc>), Without<Rig>>,
) {
    let capsule = Capsule3d::new(CAPSULE_RADIUS, CAPSULE_LENGTH);
    for (transform, mover, player, npc) in &q {
        let color = if player {
            Color::srgb(0.3, 0.9, 0.5)
        } else if npc {
            Color::srgb(0.95, 0.35, 0.3)
        } else {
            Color::srgb(0.8, 0.8, 0.8)
        };
        let p = transform.translation;
        gizmos.primitive_3d(&capsule, Isometry3d::new(p, transform.rotation), color);
        let chest = p + Vec3::Y * 0.35;
        gizmos.arrow(chest, chest + mover.forward() * 0.7, color);
        let feet = p - Vec3::Y * super::HALF_HEIGHT;
        let ring = if mover.grounded { color } else { Color::srgb(1.0, 0.9, 0.2) };
        gizmos.circle(Isometry3d::new(feet + Vec3::Y * 0.02, Quat::from_rotation_x(-PI / 2.0)), CAPSULE_RADIUS, ring);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approach_is_frame_rate_independent_and_exact() {
        let target = Vec3::new(4.2, 0.0, 0.0);
        let run = |dt: f32, secs: f32| {
            let mut v = Vec3::ZERO;
            for _ in 0..(secs / dt).round() as usize {
                v = approach_velocity(v, target, 28.0, 34.0, dt);
            }
            v
        };
        let a = run(1.0 / 120.0, 0.1);
        let b = run(1.0 / 30.0, 0.1);
        assert!((a.x - 2.8).abs() < 1e-3 && (a.x - b.x).abs() < 1e-3, "{a} {b}");
        // reaches the target exactly, never overshoots
        assert_eq!(run(1.0 / 120.0, 1.0), target);
        // braking uses the brake rate
        let v = approach_velocity(target, Vec3::ZERO, 28.0, 34.0, 0.05);
        assert!((v.x - (4.2 - 34.0 * 0.05)).abs() < 1e-4);
        assert_eq!(approach_velocity(Vec3::NAN, target, 1.0, 1.0, 0.1), target);
    }

    #[test]
    fn turning_wraps_the_short_way() {
        let near_pi = PI - 0.1;
        let out = turn_toward(near_pi, -near_pi, 0.05);
        // crossing ±π goes forward through π, not back through 0
        assert!((wrap_angle(out - near_pi) - 0.05).abs() < 1e-5, "{out}");
        assert!((turn_toward(0.0, 0.3, 1.0) - 0.3).abs() < 1e-6);
        assert!((wrap_angle(3.0 * PI) - PI).abs() < 1e-5);
        assert_eq!(yaw_of(Vec3::Z), Some(0.0));
        assert!((yaw_of(Vec3::X).unwrap() - PI / 2.0).abs() < 1e-6);
        assert_eq!(yaw_of(Vec3::ZERO), None);
    }

    #[test]
    fn half_life_halves() {
        let v = Vec3::new(8.0, 0.0, 0.0);
        let mut a = v;
        for _ in 0..30 {
            a = decay_half_life(a, 0.25, 0.25 / 30.0);
        }
        assert!((a.x - 4.0).abs() < 1e-3);
        assert_eq!(decay_half_life(Vec3::splat(1e-4), 0.25, 0.01), Vec3::ZERO);
    }

    #[test]
    fn gait_tiers() {
        let feel = Feel::default();
        assert_eq!(target_speed(&feel, 0.0, false, false, false), (0.0, Gait::Idle));
        assert_eq!(target_speed(&feel, 1.0, false, false, false), (feel.run_speed, Gait::Run));
        assert_eq!(target_speed(&feel, 1.0, true, false, false), (feel.walk_speed, Gait::Walk));
        assert_eq!(target_speed(&feel, 1.0, false, true, false), (feel.sprint_speed, Gait::Sprint));
        let (slow, g) = target_speed(&feel, 0.3, false, true, false);
        assert_eq!(g, Gait::Walk);
        assert!(slow < feel.walk_speed);
        let (c, g) = target_speed(&feel, 1.0, false, true, true);
        assert_eq!(g, Gait::Crouch);
        assert!(c < feel.run_speed && c > 0.0);
    }

    #[test]
    fn landing_and_slopes() {
        assert_eq!(landing_impact(6.0), 0.5);
        assert_eq!(landing_impact(30.0), 1.0);
        assert_eq!(landing_impact(f32::NAN), 0.0);
        assert!(walkable(Vec3::Y));
        assert!(!walkable(Vec3::new(1.0, 0.5, 0.0).normalize()));
        let n = Vec3::new(0.3, 1.0, 0.1).normalize();
        let t = slope_tangent(Vec3::new(2.0, 0.0, -1.0), n);
        assert!(t.dot(n).abs() < 1e-5);
        assert_eq!((t.x, t.z), (2.0, -1.0));
    }
}
