//! Kinematic capsule controller in `FixedUpdate / PlaySet::Move` using Avian `MoveAndSlide`:
//! ground/air modes, snappy accel/brake (Feel), turn rate, coyote time, variable jump, gravity,
//! slope limit, `Mover.external` knockback decay, `root_motion`, `move_scale`, `face`.
//! Computes `Mover.accel` (smoothed), `landing`, `grounded`, `air_time`.
//!
//! Support: the capsule floats `STEP_CLEARANCE` above the soles and only meets walls. A sphere
//! probe (`find_support`) finds the ground under the body each tick and sets its height, so
//! steps up to `STEP_HEIGHT` and drops down to `STEP_DOWN` keep the body grounded. Height jumps
//! the slope doesn't explain are steps: the body snaps, and `step_offset` eases the drawn root
//! over them (critically damped) so stairs read as steps, not teleports.
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
use super::{
    CAPSULE_LENGTH, CAPSULE_RADIUS, Feel, HALF_HEIGHT, Intent, MoveMode, Mover, Npc, Phys, PlaySet, Player,
    STEP_CLEARANCE,
};
use crate::motion::Rig;
use avian3d::prelude::*;
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};
use std::time::Duration;

/// Seconds a jump press stays buffered.
pub const JUMP_BUFFER: f32 = 0.18;
/// Upward velocity is multiplied by this once when the jump button is released early.
pub const JUMP_CUT: f32 = 0.45;
/// Steepest walkable slope (deg).
pub const SLOPE_LIMIT_DEG: f32 = 50.0;
/// Highest step the body walks onto without a mantle (m): the capsule's clearance.
pub const STEP_HEIGHT: f32 = STEP_CLEARANCE;
/// How far the ground may drop under the soles while staying grounded (stairs down, kerbs).
pub const STEP_DOWN: f32 = 0.45;
/// Touch-down tolerance under the soles while airborne.
const LAND_TOLERANCE: f32 = 0.05;
/// Support probe radius: how far the body's centre may hang past an edge and still stand.
pub const SUPPORT_RADIUS: f32 = 0.15;
/// Height changes the ground slope doesn't explain, above this (m), are steps.
const STEP_JUMP_MIN: f32 = 0.03;
/// Smoothing time (s) of the drawn root over a step.
const STEP_SMOOTH_TIME: f32 = 0.07;
const MAX_STEP_OFFSET: f32 = 0.6;
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
    /// Drawn root height minus the supported height (m), and its rate: eases out after a step.
    step_offset: f32,
    step_velocity: f32,
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

/// The part of a ground height change `rise` over the horizontal move `moved` that the ground
/// plane (`normal`) doesn't explain: a step, or 0 when it is just the slope.
pub fn step_residual(rise: f32, moved: Vec3, normal: Vec3) -> f32 {
    let n = if walkable(normal) { normal } else { Vec3::Y };
    let r = rise + (n.x * moved.x + n.z * moved.z) / n.y;
    if r.is_finite() && r.abs() > STEP_JUMP_MIN { r } else { 0.0 }
}

/// Critically damped ease of `x` (rate `v`) toward 0, stable for any `dt` (Unity's SmoothDamp).
pub fn ease_to_zero(x: f32, v: f32, smooth_time: f32, dt: f32) -> (f32, f32) {
    if !(x.is_finite() && v.is_finite()) {
        return (0.0, 0.0);
    }
    let dt = dt.max(0.0);
    let omega = 2.0 / smooth_time.max(1e-4);
    let e = omega * dt;
    let decay = 1.0 / (1.0 + e + 0.48 * e * e + 0.235 * e * e * e);
    let t = (v + omega * x) * dt;
    ((x + t) * decay, (v - omega * t) * decay)
}

/// Walkable ground under a body.
#[derive(Clone, Copy, Debug)]
struct Support {
    height: f32,
    normal: Vec3,
}

/// Ground under a body whose capsule centre is `center`, from `STEP_HEIGHT` above the soles down
/// to `below` under them. A sphere probe takes the highest ground within `SUPPORT_RADIUS` (the
/// body can stand with its centre past an edge). An edge or corner contact counts when the face
/// just inside it is walkable; a ray straight down is the fallback when the sphere only finds
/// steep ground.
fn find_support(query: &SpatialQuery, center: Vec3, below: f32, filter: &SpatialQueryFilter) -> Option<Support> {
    let feet = center.y - HALF_HEIGHT;
    let accept = |height: f32, normal: Vec3| {
        (height.is_finite()
            && height >= feet - below - 1e-3
            && height <= feet + STEP_HEIGHT + 0.05
            && walkable(normal))
        .then(|| Support {
            height,
            normal: normal.normalize(),
        })
    };
    let probe = Collider::sphere(SUPPORT_RADIUS);
    let config = ShapeCastConfig::from_max_distance(HALF_HEIGHT - SUPPORT_RADIUS + below);
    if let Some(hit) = query.cast_shape(&probe, center, Quat::IDENTITY, Dir3::NEG_Y, &config, filter) {
        if let Some(support) = accept(hit.point1.y, hit.normal1) {
            return Some(support);
        }
        if hit.normal1.y > 0.02
            && let Ok(inward) = Dir3::new(-hit.normal1.with_y(0.0))
        {
            let origin = hit.point1 + inward * 0.03 + Vec3::Y * 0.1;
            if let Some(face) = query.cast_ray(origin, Dir3::NEG_Y, 0.2, true, filter)
                && let Some(support) = accept(origin.y - face.distance, face.normal)
            {
                return Some(support);
            }
        }
    }
    let ray = query.cast_ray(center, Dir3::NEG_Y, HALF_HEIGHT + below, true, filter)?;
    accept(center.y - ray.distance, ray.normal)
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

/// One move-and-slide call. While grounded the floating capsule only meets things taller than a
/// step, so every upward-facing hit is flattened into a vertical wall (the support probe owns the
/// height); in the air only steep hits are walls. Every wall normal is recorded.
fn slide(
    mas: &MoveAndSlide,
    collider: &Collider,
    from: Vec3,
    velocity: Vec3,
    dt: f32,
    grounded: bool,
    filter: &SpatialQueryFilter,
) -> Step {
    let config = MoveAndSlideConfig::default();
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
            if grounded
                && n.y > 0.0
                && let Ok(flat) = Dir3::new(n.with_y(0.0))
            {
                *hit.normal = flat;
                walls.push(flat.as_vec3());
            } else if !walkable(n) {
                walls.push(n);
            }
            MoveAndSlideHitResponse::Accept
        },
    );
    Step {
        position: if out.position.is_finite() { out.position } else { from },
        walls,
    }
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
        Option<&super::horse::Ridden>,
    )>,
) {
    let dt = time.delta_secs().clamp(1e-4, 1.0 / 30.0);
    // `root_motion` is a per-frame request: clear it after the frame's last fixed tick.
    let last_tick_of_frame = time.overstep() < time.timestep();
    let feel = &*feel;

    for (entity, collider, mut transform, mut mover, intent, mut state, teleport, ridden) in &mut bodies {
        if let Some(tp) = teleport
            && mover.mode != MoveMode::Ragdoll
        {
            commands.entity(entity).remove::<Teleport>();
            if tp.feet.is_finite() && tp.yaw.is_finite() {
                transform.translation = tp.feet + Vec3::Y * HALF_HEIGHT;
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

        if !state.owned {
            // Taking over from traverse / ragdoll / spawn: continue with their velocity.
            let v = if mover.velocity.is_finite() { mover.velocity } else { Vec3::ZERO };
            state.locomotion = v.with_y(0.0);
            state.prev_velocity = v.with_y(0.0);
            state.step_offset = 0.0;
            state.step_velocity = 0.0;
            state.owned = true;
        }
        // Physics runs on the supported height; `step_offset` only shifts the drawn root.
        let start = transform.translation - Vec3::Y * state.step_offset;
        if !start.is_finite() {
            continue;
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
        // Releasing the stick in flight preserves momentum; steering still brakes/reverses.
        let air_target = if !grounded && magnitude < 0.05 { state.locomotion } else { target_velocity };
        state.locomotion = approach_velocity(state.locomotion, air_target, accel, brake, dt);
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
            // Brief apex float followed by a decisive fall; early release keeps short hops crisp.
            let gravity_scale = if vy < 0.0 { 1.35 } else if state.jumping && vy < 1.4 && intent.jump_held { 0.65 } else { 1.0 };
            vy = (vy - feel.gravity.max(0.0) * gravity_scale * dt).max(-MAX_FALL_SPEED);
        }

        // --- Move: horizontal on the ground (the support sets the height), ballistic in the air.
        let horizontal = state.locomotion + external.with_y(0.0) + root_motion.with_y(0.0);
        let mut velocity = if grounded { horizontal } else { horizontal + Vec3::Y * vy };
        velocity.y += root_motion.y;
        let mut filter = movement_filter(entity);
        // A rider never meets its own horse.
        if let Some(r) = ridden {
            filter.excluded_entities.insert(r.1);
        }
        let Step { mut position, walls } = slide(&mas, collider, start, velocity, dt, grounded, &filter);

        // --- Support: stand on the ground under the body; steps become a drawn offset.
        let ascending = vy > 0.01 || root_motion.y > 0.1;
        let below = if grounded && !ascending { STEP_DOWN } else { LAND_TOLERANCE };
        let mut now_grounded = false;
        let mut normal = Vec3::Y;
        if !ascending && let Some(support) = find_support(&mas.spatial_query, position, below, &filter) {
            let supported = support.height + HALF_HEIGHT;
            if grounded {
                let step = step_residual(supported - position.y, position - start, ground_normal);
                state.step_offset = (state.step_offset - step).clamp(-MAX_STEP_OFFSET, MAX_STEP_OFFSET);
            }
            position.y = supported;
            now_grounded = true;
            normal = support.normal;
        }
        if now_grounded {
            let before = state.step_offset;
            (state.step_offset, state.step_velocity) =
                ease_to_zero(state.step_offset, state.step_velocity, STEP_SMOOTH_TIME, dt);
            mover.step_ease += state.step_offset - before;
        } else {
            // Airborne bodies are drawn where they are.
            position.y += state.step_offset;
            state.step_offset = 0.0;
            state.step_velocity = 0.0;
        }

        // --- Vertical bookkeeping, landing.
        let actual = (position - start) / dt;
        mover.landing = (mover.landing - dt / LANDING_DECAY).max(0.0);
        if now_grounded {
            if !was_grounded {
                let impact = -velocity.y.min(0.0);
                mover.landing = mover.landing.max(landing_impact(impact));
                mover.impact = impact;
            }
            vy = 0.0;
            state.jumping = false;
            state.jump_spent = false;
            mover.air_time = 0.0;
        } else {
            if was_grounded {
                mover.takeoff_speed = (position - start).with_y(0.0).length() / dt;
            }
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
            transform.translation = position + Vec3::Y * state.step_offset;
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
        let feet = p - Vec3::Y * HALF_HEIGHT;
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
    }

    #[test]
    fn slopes_are_not_steps() {
        // Walking 0.2 m along +x up a slope rising 0.5 m per metre: rise explained, no step.
        let n = Vec3::new(-0.5, 1.0, 0.0).normalize();
        let moved = Vec3::new(0.2, 0.0, 0.0);
        assert_eq!(step_residual(0.1, moved, n), 0.0);
        // A 0.25 m stair on flat ground is a step; so is one on top of the slope's own rise.
        assert!((step_residual(0.25, moved, Vec3::Y) - 0.25).abs() < 1e-6);
        assert!((step_residual(0.35, moved, n) - 0.25).abs() < 1e-5);
        assert!((step_residual(-0.25, moved, Vec3::Y) + 0.25).abs() < 1e-6);
        // Tiny residuals (terrain seams) are continuous ground.
        assert_eq!(step_residual(0.01, moved, Vec3::Y), 0.0);
        assert_eq!(step_residual(f32::NAN, moved, Vec3::Y), 0.0);
    }

    #[test]
    fn step_offset_eases_out_smoothly() {
        let dt = 1.0 / 64.0;
        let (mut x, mut v) = (-0.25, 0.0);
        let mut prev = x;
        for i in 0..64 {
            (x, v) = ease_to_zero(x, v, STEP_SMOOTH_TIME, dt);
            assert!(x.is_finite() && x <= 1e-4, "overshoot at tick {i}: {x}");
            assert!(x >= prev - 1e-6, "not monotonic at tick {i}");
            prev = x;
        }
        assert!(x.abs() < 1e-3, "{x}");
        // Frame-rate independent within a few millimetres, safe with NaN.
        let mut coarse = (-0.25, 0.0);
        let mut fine = (-0.25, 0.0);
        for _ in 0..4 {
            coarse = ease_to_zero(coarse.0, coarse.1, STEP_SMOOTH_TIME, 1.0 / 32.0);
        }
        for _ in 0..16 {
            fine = ease_to_zero(fine.0, fine.1, STEP_SMOOTH_TIME, 1.0 / 128.0);
        }
        assert!((coarse.0 - fine.0).abs() < 0.01, "{coarse:?} {fine:?}");
        assert_eq!(ease_to_zero(f32::NAN, 0.0, 0.1, dt), (0.0, 0.0));
    }
}
