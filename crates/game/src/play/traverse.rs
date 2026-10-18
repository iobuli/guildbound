//! Traversal (`Update / PlaySet::Act`): owns a character's movement while its `Mover.mode` is
//! Swim, Climb or Ledge (the controller skips those modes and resumes on the way out).
//! - Water: a body whose ground lies deeper than `SWIM_DEPTH` under a `Water` surface swims:
//!   chest held `CHEST_BELOW` under the surface with a slow bob, the stick steers at `SWIM_SPEED`
//!   (sprint `SWIM_SPRINT`, spending stamina through `StaminaSpend`), and it wades out where
//!   the bottom comes up again. Diving: the crouch toggle (C) takes the body down toward the
//!   floor (`swim_height`) while `BREATH` lasts (it refills at the surface); jump clears the
//!   crouch and surfaces. Metres under water are `dive` deeds, on top `swim`.
//! - Climbing: jumping at a `Climbable` face whose top is out of reach grabs it. The stick moves
//!   up, down and along the wall (`CLIMB_SPEED`), costing stamina (more when moving); hands and
//!   feet take holds on a `HOLD` grid on the face (`select::HandTargets` / `FootTargets`, written
//!   here only, alternating as the climb advances). Jump kicks off the wall, dodge lets go, an
//!   empty stamina bar drops the climber, and moving past the wall's edge lets go too. At the
//!   top the climber hangs from the ledge, both hands on the edge (`ledge_hands`): sideways edges
//!   along it, down drops back to climbing, jump (or pushing up for `PULL_HOLD`) pulls over.
//! - Mantle: jumping at a `Climbable` block whose top is `MANTLE_LOW`–`MANTLE_HIGH` above the feet
//!   vaults onto it; reaching a wall's top climbs over. A mantle is an `Action` (`Owner::Traverse`,
//!   `ActionKind::Mantle`, `ual2/ClimbUp_1m`) with the root on a scripted two-phase path (rise to
//!   the top, then over the edge) in `MoveMode::Ledge`.
use super::deeds::Deed;
use super::select::{FootTargets, HandTargets};
use super::world::ground_height;
use super::{
    Action, ActionKind, ActionLayer, Climbable, Fighter, HALF_HEIGHT, Intent, MoveMode, Mover,
    Owner, Phys, PlaySet, StaminaSpend, Water,
};
use crate::motion::Mask;
use crate::motion::bank::PoseBank;
use avian3d::prelude::*;
use bevy::prelude::*;

/// Swim where the bottom is this far under the surface (m); wade out below `SWIM_DEPTH - SWIM_HYST`.
const SWIM_DEPTH: f32 = 1.3;
const SWIM_HYST: f32 = 0.15;
/// The chest floats this far under the surface (m); the root is `CHEST_HEIGHT` under the chest.
const CHEST_BELOW: f32 = 0.4;
const CHEST_HEIGHT: f32 = 0.35;
const SWIM_SPEED: f32 = 1.7;
const SWIM_SPRINT: f32 = 2.8;
const SWIM_SPRINT_COST: f32 = 10.0;
/// Bob amplitude (m) and rate (rad/s).
/// Diving: body centre this far above the floor at the bottom (m), vertical easing rate, head
/// height above the chest, breath (s) and how fast it comes back at the surface (s per s).
const DIVE_FLOOR: f32 = 0.75;
const DIVE_RATE: f32 = 1.6;
const HEAD_ABOVE_CHEST: f32 = 0.35;
const BREATH: f32 = 12.0;
const BREATH_REGAIN: f32 = 3.0;
const BOB: f32 = 0.035;
const BOB_RATE: f32 = 2.2;
/// Climbing: speed (m/s), stamina per second hanging / moving, distance kept from the face (m),
/// probe reach (m), hold grid (m).
const CLIMB_SPEED: f32 = 1.0;
const CLIMB_HANG_COST: f32 = 3.0;
const CLIMB_MOVE_COST: f32 = 9.0;
const WALL_GAP: f32 = 0.42;
const PROBE: f32 = 0.9;
const HOLD: f32 = 0.35;
/// Kick-off speeds (m/s): away from the face and up.
const KICK_OUT: f32 = 3.0;
const KICK_UP: f32 = 3.5;
/// A face is a climb when its top is more than this above the feet (m); a mantle between these.
const CLIMB_MIN: f32 = 1.5;
const MANTLE_LOW: f32 = 0.35;
const MANTLE_HIGH: f32 = 1.45;
/// Climbing mantles over the top once the hands are within this of it (m).
const TOP_REACH: f32 = 0.35;
/// Ledge: hands above the chest when reaching up (m), edging speed (m/s), and how long (s) the
/// stick pushes up before a hanging climber pulls over (jump pulls over at once).
const HANDS_ABOVE_CHEST: f32 = 0.75;
/// How far above the chest the hands actually reach in the hang (m): measured from the hand
/// bones (`ledge_hang` monitor lines), the arms stop well short of `HANDS_ABOVE_CHEST`.
const HANG_REACH: f32 = 0.37;
const LEDGE_SPEED: f32 = 0.6;
const PULL_HOLD: f32 = 0.5;

/// Body centre height hanging from a ledge at `top`: hands on the edge.
pub fn hang_y(top: f32) -> f32 {
    top - CHEST_HEIGHT - HANG_REACH + 0.02
}

/// Both hands on the ledge at `top`, a shoulder-width apart, just over the face's edge.
pub fn ledge_hands(body: Vec3, n: Vec3, right: Vec3, top: f32) -> [Vec3; 2] {
    let edge = Vec3::new(body.x, top + 0.02, body.z) - n * (WALL_GAP + 0.04);
    // [left, right]: facing the wall, the climber's left is −`right`.
    [edge - right * 0.22, edge + right * 0.22]
}

/// Mantle: duration (s) per metre of rise (plus a base), how far onto the top (m).
const MANTLE_BASE: f32 = 0.35;
const MANTLE_PER_M: f32 = 0.3;
const MANTLE_IN: f32 = 0.55;
const CLIMB_TRACK: &str = "ual2/ClimbUp_1m";

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (water, climb, mantle).chain().in_set(PlaySet::Act))
        .add_systems(Update, demo.after(PlaySet::Input).before(PlaySet::Act))
        .add_systems(Update, ledge_probe.after(PlaySet::Act));
}

/// Smoke-test driver: `GB_DIVE=1` dives once swimming (spawn in the pond). `GB_CLIMB=1` makes the player jump once at 2.5 s of game time and hold
/// forward from then on (spawn it facing a wall with `GB_SPAWN`).
fn demo(
    time: Res<Time>,
    mut armed: Local<Option<(bool, bool)>>,
    mut players: Query<&mut Intent, With<super::Player>>,
    movers: Query<&Mover, With<super::Player>>,
) {
    let (climb, dive) = *armed.get_or_insert_with(|| (std::env::var_os("GB_CLIMB").is_some(), std::env::var_os("GB_DIVE").is_some()));
    let (Ok(mut intent), Ok(mover)) = (players.single_mut(), movers.single()) else { return };
    // GB_DIVE: once swimming (spawn in the pond with GB_SPAWN), drift forward and dive.
    if dive && mover.mode == MoveMode::Swim {
        intent.move_dir = mover.forward() * 0.4;
        intent.crouch = time.elapsed_secs() > 2.0;
    }
    if !climb || time.elapsed_secs() < 2.5 {
        return;
    }
    intent.move_dir = mover.forward();
    if time.elapsed_secs() - time.delta_secs() < 2.5 {
        intent.jump_pressed = true;
    }
}

/// What traversal is doing with a body (added on demand).
#[derive(Component, Clone, Debug)]
pub enum Traverse {
    /// `swum`: metres toward the next `swim` / `dive` deed; `breath`: seconds left under water.
    Swim { clock: f32, swum: f32, breath: f32 },
    Climb(Climbing),
    Mantle(Mantle),
}

#[derive(Clone, Debug)]
pub struct Climbing {
    /// The face: an entity, its outward normal (horizontal) and its top (world y).
    wall: Entity,
    normal: Vec3,
    top: f32,
    /// Distance climbed (m), for the alternating holds.
    travelled: f32,
    /// Hanging from the top edge (hands on it); seconds the stick has pushed up while hanging.
    hanging: bool,
    pull: f32,
}

#[derive(Clone, Debug)]
pub struct Mantle {
    from: Vec3,
    to: Vec3,
    t: f32,
    duration: f32,
}

/// Root position `u` (0..1) along a mantle from `from` to `to`: rise first (to the top height,
/// staying at the face), then over the edge; each phase eased.
pub fn mantle_path(from: Vec3, to: Vec3, u: f32) -> Vec3 {
    let u = u.clamp(0.0, 1.0);
    let ease = |x: f32| x * x * (3.0 - 2.0 * x);
    let rise = ease((u / 0.6).min(1.0));
    let over = ease(((u - 0.45) / 0.55).clamp(0.0, 1.0));
    let y = from.y + (to.y - from.y) * rise;
    let xz = Vec2::new(from.x, from.z).lerp(Vec2::new(to.x, to.z), over);
    Vec3::new(xz.x, y, xz.y)
}

/// Hand and foot holds for a climber whose root is at `root`, on a face with outward `normal`,
/// after `travelled` m: hands a reach above the chest, feet below the hips, left and right
/// alternating every `HOLD` of climbing. Returns [left hand, right hand], [left foot, right foot].
pub fn holds(root: Vec3, normal: Vec3, travelled: f32) -> ([Vec3; 2], [Vec3; 2]) {
    let n = Vec3::new(normal.x, 0.0, normal.z).normalize_or(Vec3::Z);
    let right = Vec3::Y.cross(n).normalize_or(Vec3::X);
    let on_face = root - n * (WALL_GAP - 0.06);
    let cycle = (travelled / HOLD).rem_euclid(2.0);
    let lead = if cycle < 1.0 { 1.0 } else { -1.0 };
    let step = |base: f32, side: f32, up: bool| {
        // The left hand (side < 0) leads first.
        let raise = if (side < 0.0) == (lead > 0.0) {
            0.12
        } else {
            -0.05
        };
        on_face + Vec3::Y * (base + if up { raise } else { -raise }) + right * side
    };
    // Facing the wall (forward −n), the character's left is Y × (−n) = −right: the face's
    // "right" seen from outside is the climber's right.
    let hands = [step(0.75, -0.24, true), step(0.75, 0.24, true)];
    let feet = [step(-0.75, -0.16, false), step(-0.75, 0.16, false)];
    (hands, feet)
}

/// Body-centre height while swimming: bobbing at the surface, or near the floor when diving
/// (never above the surface height, never into the floor).
pub fn swim_height(surface: f32, floor: f32, diving: bool, clock: f32) -> f32 {
    let at_surface = surface - CHEST_BELOW - CHEST_HEIGHT + BOB * (clock * BOB_RATE).sin();
    if diving {
        (floor + DIVE_FLOOR).min(at_surface)
    } else {
        at_surface
    }
}

/// Swim where the water is deep; wade out where it isn't.
#[allow(clippy::type_complexity)]
fn water(
    time: Res<Time>,
    waters: Query<(&Transform, &Water), Without<Mover>>,
    mut bodies: Query<(
        Entity,
        &mut Transform,
        &mut Mover,
        &Intent,
        &Action,
        &Fighter,
        Option<&mut Traverse>,
    )>,
    mut spend: MessageWriter<StaminaSpend>,
    mut deeds: MessageWriter<Deed>,
    mut commands: Commands,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, mut tf, mut mover, intent, action, fighter, state) in &mut bodies {
        if matches!(
            state.as_deref(),
            Some(Traverse::Climb(_) | Traverse::Mantle(_))
        ) || mover.mode == MoveMode::Ragdoll
        {
            continue;
        }
        let p = tf.translation;
        let surface = waters.iter().find_map(|(wt, w)| {
            let local = p - wt.translation;
            (local.x.abs() <= w.half_extents.x && local.z.abs() <= w.half_extents.z)
                .then_some(wt.translation.y + w.half_extents.y)
        });
        let floor = ground_height(p.x, p.z);
        let depth = surface.map_or(0.0, |s| s - floor);
        let swimming = matches!(state.as_deref(), Some(Traverse::Swim { .. }));
        let feet = p.y - HALF_HEIGHT;
        let wants = surface.is_some_and(|s| depth > SWIM_DEPTH && feet < s - CHEST_HEIGHT);
        if !swimming
            && wants
            && matches!(mover.mode, MoveMode::Ground | MoveMode::Air)
            && action.is_free()
        {
            mover.mode = MoveMode::Swim;
            mover.velocity.y = 0.0;
            commands
                .entity(entity)
                .try_insert(Traverse::Swim { clock: 0.0, swum: 0.0, breath: BREATH });
            continue;
        }
        let Some(mut state) = state else { continue };
        let Traverse::Swim { clock, swum, breath } = &mut *state else {
            continue;
        };
        if depth < SWIM_DEPTH - SWIM_HYST || surface.is_none() || fighter.dead {
            mover.mode = MoveMode::Ground;
            commands.entity(entity).try_remove::<Traverse>();
            continue;
        }
        *clock += dt;
        let surface = surface.unwrap_or(p.y);
        let sprint = intent.sprint && fighter.stamina > 1.0;
        if sprint && intent.move_dir.length() > 0.1 {
            spend.write(StaminaSpend {
                who: entity,
                amount: SWIM_SPRINT_COST * dt,
            });
        }
        let speed = if sprint { SWIM_SPRINT } else { SWIM_SPEED };
        let want = Vec3::new(intent.move_dir.x, 0.0, intent.move_dir.z) * speed;
        let h = Vec3::new(mover.velocity.x, 0.0, mover.velocity.z);
        let h = h.lerp(want, (3.0 * dt).min(1.0));
        // Diving: the crouch toggle (C) takes the body down while breath lasts; jump clears it.
        let floor_here = ground_height(p.x, p.z);
        let head_under = p.y + CHEST_HEIGHT + HEAD_ABOVE_CHEST < surface;
        if head_under {
            *breath -= dt;
        } else {
            *breath = (*breath + BREATH_REGAIN * dt).min(BREATH);
        }
        let diving = intent.crouch && *breath > 0.0;
        let target_y = swim_height(surface, floor_here, diving, *clock);
        let rate = if diving || head_under { DIVE_RATE } else { 4.0 };
        let y = p.y + (target_y - p.y) * (rate * dt).min(1.0);
        let mut next = Vec3::new(p.x + h.x * dt, y, p.z + h.z * dt);
        // Never sink into the bowl's floor.
        let floor_next = ground_height(next.x, next.z);
        next.y = next.y.max(floor_next + HALF_HEIGHT * 0.6);
        mover.velocity = Vec3::new(h.x, (next.y - p.y) / dt.max(1e-4), h.z);
        if h.length() > 0.2 {
            mover.yaw = h.x.atan2(h.z);
            tf.rotation = Quat::from_rotation_y(mover.yaw);
        }
        *swum += Vec2::new(next.x - p.x, next.z - p.z).length() + if head_under { (next.y - p.y).abs() } else { 0.0 };
        if *swum >= 1.0 {
            *swum -= 1.0;
            deeds.write(Deed::new(entity, if head_under { "dive" } else { "swim" }, "", 1.0));
        }
        tf.translation = next;
        mover.grounded = false;
        mover.air_time = 0.0;
    }
}

/// The face ahead of a body at chest height, if any: (entity, point, outward normal).
fn face_ahead(spatial: &SpatialQuery, from: Vec3, forward: Vec3) -> Option<(Entity, Vec3, Vec3)> {
    let filter = SpatialQueryFilter::from_mask(Phys::World);
    let dir = Dir3::new(Vec3::new(forward.x, 0.0, forward.z)).ok()?;
    spatial
        .cast_ray(from, dir, PROBE, true, &filter)
        .map(|hit| {
            (
                hit.entity,
                from + dir * hit.distance,
                Vec3::new(hit.normal.x, 0.0, hit.normal.z).normalize_or(-dir.as_vec3()),
            )
        })
}

#[allow(clippy::type_complexity)]
fn climb(
    time: Res<Time>,
    spatial: SpatialQuery,
    bank: Option<Res<PoseBank>>,
    climbables: Query<&Climbable>,
    mut bodies: Query<(
        Entity,
        &mut Transform,
        &mut Mover,
        &Intent,
        &mut Action,
        &Fighter,
        Option<&mut Traverse>,
    )>,
    mut spend: MessageWriter<StaminaSpend>,
    mut deeds: MessageWriter<Deed>,
    mut commands: Commands,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, mut tf, mut mover, intent, mut action, fighter, state) in &mut bodies {
        let feet = tf.translation.y - HALF_HEIGHT;
        let chest = tf.translation + Vec3::Y * CHEST_HEIGHT;
        let climbing = matches!(state.as_deref(), Some(Traverse::Climb(_)));
        if state.is_none() {
            {
                // Grab a tall face, or mantle onto a waist-high one, when jumping at it.
                if !intent.jump_pressed
                    || !action.is_free()
                    || !matches!(mover.mode, MoveMode::Ground | MoveMode::Air)
                {
                    continue;
                }
                let Some((wall, point, normal)) = face_ahead(&spatial, chest, mover.forward())
                    .or_else(|| {
                        face_ahead(&spatial, tf.translation - Vec3::Y * 0.3, mover.forward())
                    })
                else {
                    continue;
                };
                let Ok(c) = climbables.get(wall) else {
                    continue;
                };
                let rise = c.ledge - feet;
                if (MANTLE_LOW..=MANTLE_HIGH).contains(&rise) {
                    start_mantle(
                        &mut commands,
                        entity,
                        &mut action,
                        &mut mover,
                        &tf,
                        point,
                        normal,
                        c.ledge,
                        bank.as_deref(),
                    );
                } else if rise > CLIMB_MIN && fighter.stamina > 5.0 {
                    let yaw = (-normal.x).atan2(-normal.z);
                    tf.translation =
                        Vec3::new(point.x, tf.translation.y + 0.15, point.z) + normal * WALL_GAP;
                    tf.rotation = Quat::from_rotation_y(yaw);
                    mover.yaw = yaw;
                    mover.mode = MoveMode::Climb;
                    mover.velocity = Vec3::ZERO;
                    mover.grounded = false;
                    commands
                        .entity(entity)
                        .try_insert(Traverse::Climb(Climbing {
                            wall,
                            normal,
                            top: c.ledge,
                            travelled: 0.0,
                            hanging: false,
                            pull: 0.0,
                        }));
                }
            }
        } else if climbing {
            {
                let Some(mut st) = state else { continue };
                let Traverse::Climb(c) = &mut *st else {
                    continue;
                };
                let n = c.normal;
                let right = Vec3::Y.cross(n).normalize_or(Vec3::X);
                let let_go = |commands: &mut Commands, mover: &mut Mover, v: Vec3| {
                    mover.mode = MoveMode::Air;
                    mover.velocity = v;
                    commands
                        .entity(entity)
                        .try_remove::<(Traverse, HandTargets, FootTargets)>();
                };
                if fighter.dead || mover.mode == MoveMode::Ragdoll {
                    commands
                        .entity(entity)
                        .try_remove::<(Traverse, HandTargets, FootTargets)>();
                    continue;
                }
                let pull_up = c.hanging && (intent.jump_pressed || c.pull >= PULL_HOLD);
                if pull_up {
                    let point = tf.translation - n * WALL_GAP;
                    let top = c.top;
                    commands.entity(entity).try_remove::<(HandTargets, FootTargets)>();
                    start_mantle(&mut commands, entity, &mut action, &mut mover, &tf, point, n, top, bank.as_deref());
                    continue;
                }
                if intent.jump_pressed {
                    let_go(&mut commands, &mut mover, n * KICK_OUT + Vec3::Y * KICK_UP);
                    continue;
                }
                if intent.dodge_pressed || fighter.stamina <= 0.0 {
                    let_go(&mut commands, &mut mover, n * 0.5);
                    continue;
                }
                // Stick: forward (into the wall) climbs, back descends, sideways shimmies. The
                // character faces the wall, so `side` measures toward its left (−right of the face).
                let stick = intent.move_dir;
                let up = stick.dot(-n);
                let side = stick.dot(-right);
                if c.hanging {
                    // On the ledge: push up to pull over, down to drop back onto the face.
                    c.pull = if up > 0.5 { c.pull + dt } else { 0.0 };
                    if up < -0.5 {
                        c.hanging = false;
                    }
                }
                let v = if c.hanging {
                    -right * side * LEDGE_SPEED
                } else {
                    Vec3::Y * up * CLIMB_SPEED - right * side * CLIMB_SPEED * 0.8
                };
                let moving = v.length() > 0.1;
                spend.write(StaminaSpend {
                    who: entity,
                    amount: if moving {
                        CLIMB_MOVE_COST
                    } else {
                        CLIMB_HANG_COST
                    } * dt,
                });
                let mut next = tf.translation + v * dt;
                // Down to the ground: step off.
                if next.y - HALF_HEIGHT <= ground_height(next.x, next.z) + 0.02 && up < 0.0 {
                    mover.mode = MoveMode::Ground;
                    commands
                        .entity(entity)
                        .try_remove::<(Traverse, HandTargets, FootTargets)>();
                    continue;
                }
                // Stay on the face; past its edge, let go.
                match face_ahead(&spatial, next + Vec3::Y * CHEST_HEIGHT, -n) {
                    Some((wall, point, normal)) if wall == c.wall => {
                        next = Vec3::new(point.x, next.y, point.z) + normal * WALL_GAP;
                        c.normal = normal;
                    }
                    _ => {
                        let_go(&mut commands, &mut mover, n * 0.5);
                        continue;
                    }
                }
                let before = c.travelled;
                c.travelled += (next - tf.translation).length();
                if c.travelled.floor() > before.floor() {
                    deeds.write(Deed::new(entity, "climb", "", 1.0));
                }
                mover.velocity = (next - tf.translation) / dt.max(1e-4);
                tf.translation = next;
                // The top: take the ledge and hang from it (both hands on the edge).
                let hands_y = next.y + CHEST_HEIGHT + HANDS_ABOVE_CHEST;
                if !c.hanging && hands_y + TOP_REACH >= c.top && up >= 0.0 {
                    c.hanging = true;
                    c.pull = 0.0;
                }
                let (mut hands, feet_holds) = holds(next, n, c.travelled);
                if c.hanging {
                    tf.translation.y = hang_y(c.top);
                    hands = ledge_hands(tf.translation, n, right, c.top);
                }
                commands.entity(entity).try_insert((
                    HandTargets {
                        hands: [Some((hands[0], 1.0)), Some((hands[1], 1.0))],
                    },
                    FootTargets {
                        feet: [Some((feet_holds[0], 0.9)), Some((feet_holds[1], 0.9))],
                    },
                ));
            }
        }
    }
}

/// Monitor: while hanging, how far each hand bone sits from the ledge's top (m; + above), so
/// the hand IK on the edge can be checked from `staging/relations.log` (`ledge_hang` lines).
fn ledge_probe(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    mut monitor: ResMut<super::monitor::Monitor>,
    bodies: Query<(Option<&Name>, &Traverse, &crate::motion::Rig)>,
    globals: Query<&GlobalTransform>,
    mut last: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last < 0.25 {
        return;
    }
    let Some(bank) = bank else { return };
    for (name, state, rig) in &bodies {
        let Traverse::Climb(c) = state else { continue };
        if !c.hanging {
            continue;
        }
        let hand = |side: usize| {
            rig.bones
                .get(bank.bones.hand[side])
                .and_then(|e| globals.get(*e).ok())
                .map(|g| g.translation().y - c.top)
        };
        if let (Some(l), Some(r)) = (hand(0), hand(1)) {
            *last = now;
            let who = name.map_or("?", |n| n.as_str());
            monitor.write(&format!("{now:.2} {who} ledge_hang - {:.3} Climb l={l:.3} r={r:.3}", l.abs().max(r.abs())));
        }
    }
}

/// Begin a mantle onto the top at `top` (world y) over the edge at `point` (face normal `n`).
#[allow(clippy::too_many_arguments)]
fn start_mantle(
    commands: &mut Commands,
    entity: Entity,
    action: &mut Action,
    mover: &mut Mover,
    tf: &Transform,
    point: Vec3,
    n: Vec3,
    top: f32,
    bank: Option<&PoseBank>,
) {
    let from = tf.translation;
    let onto = Vec3::new(point.x, 0.0, point.z) - n * MANTLE_IN;
    let to = Vec3::new(onto.x, top + HALF_HEIGHT, onto.z);
    let rise = (to.y - from.y).max(0.0);
    let duration = MANTLE_BASE + MANTLE_PER_M * rise;
    let yaw = (-n.x).atan2(-n.z);
    action.start(
        ActionKind::Mantle,
        Owner::Traverse,
        Some(ActionLayer::new(CLIMB_TRACK, 0.0, Mask::Full)),
    );
    mover.mode = MoveMode::Ledge;
    mover.yaw = yaw;
    mover.velocity = Vec3::ZERO;
    let _ = bank;
    commands.entity(entity).try_insert(Traverse::Mantle(Mantle {
        from,
        to,
        t: 0.0,
        duration,
    }));
}

fn mantle(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    mut bodies: Query<(
        Entity,
        &mut Transform,
        &mut Mover,
        &mut Action,
        &mut Traverse,
    )>,
    mut deeds: MessageWriter<Deed>,
    mut commands: Commands,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let clip = bank.as_deref().map_or(0.67, |b| b.duration(CLIMB_TRACK));
    for (entity, mut tf, mut mover, mut action, mut state) in &mut bodies {
        let Traverse::Mantle(m) = &mut *state else {
            continue;
        };
        // Another owner (a hit reaction) took the body: drop the mantle where it is.
        if action.owner != Owner::Traverse || action.kind != ActionKind::Mantle {
            mover.mode = MoveMode::Air;
            commands.entity(entity).try_remove::<Traverse>();
            continue;
        }
        m.t += dt;
        let u = m.t / m.duration.max(1e-3);
        let next = mantle_path(m.from, m.to, u);
        mover.velocity = (next - tf.translation) / dt.max(1e-4);
        tf.translation = next;
        tf.rotation = Quat::from_rotation_y(mover.yaw);
        if let Some(layer) = action.layer.as_mut() {
            // The pull-up half of the climbing cycle, once over the mantle.
            layer.time = (u.min(1.0) * 0.5) * clip;
        }
        action.elapsed = m.t;
        if u >= 1.0 {
            action.clear();
            mover.mode = MoveMode::Ground;
            mover.velocity = Vec3::ZERO;
            mover.grounded = true;
            deeds.write(Deed::new(entity, "mantle", "", 1.0));
            commands.entity(entity).try_remove::<Traverse>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hanging_hands_sit_on_the_edge() {
        let top = 3.0;
        let body = Vec3::new(1.0, hang_y(top), 0.42);
        let (n, right) = (Vec3::Z, Vec3::X);
        let hands = ledge_hands(body, n, right, top);
        for h in hands {
            assert!((h.y - top).abs() < 0.05, "on the edge: {h}");
            assert!(h.z < body.z - WALL_GAP, "over the face, not in the air: {h}");
        }
        assert!((hands[0] - hands[1]).length() > 0.3);
        // From the hang the hands reach the edge.
        assert!((body.y + CHEST_HEIGHT + HANG_REACH - top).abs() < 0.05);
    }

    #[test]
    fn diving_goes_down_but_not_into_the_floor() {
        let (surface, floor) = (-0.35, -2.45);
        let top = swim_height(surface, floor, false, 0.0);
        let down = swim_height(surface, floor, true, 0.0);
        assert!(down < top - 0.5, "{down} vs {top}");
        assert!(down + CHEST_HEIGHT + HEAD_ABOVE_CHEST < surface, "the head goes under");
        assert!(down >= floor + DIVE_FLOOR - 1e-5);
        // Shallow water: diving can't go above the surface height.
        assert!(swim_height(surface, top - 0.1, true, 0.0) <= top);
    }

    #[test]
    fn mantle_rises_before_it_crosses_and_lands_on_target() {
        let (from, to) = (Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 2.2, 1.0));
        assert_eq!(mantle_path(from, to, 0.0), from);
        assert!(mantle_path(from, to, 1.0).distance(to) < 1e-5);
        let early = mantle_path(from, to, 0.4);
        assert!(
            early.y > from.y + 0.6 && early.z < 0.01,
            "rises at the face first: {early}"
        );
        let mut prev = from;
        for i in 1..=50 {
            let p = mantle_path(from, to, i as f32 / 50.0);
            assert!(p.y >= prev.y - 1e-5 && p.z >= prev.z - 1e-5, "monotonic");
            prev = p;
        }
    }

    #[test]
    fn holds_sit_on_the_face_and_alternate() {
        let root = Vec3::new(0.0, 2.0, 0.42);
        let n = Vec3::Z;
        let (h0, f0) = holds(root, n, 0.1);
        let (h1, _) = holds(root, n, 0.1 + HOLD);
        for p in h0.iter().chain(&f0) {
            assert!((p.z - 0.06).abs() < 1e-4, "on the face: {p}");
        }
        assert!(h0[0].y > h0[1].y && h1[0].y < h1[1].y, "hands lead in turn");
        assert!(h0.iter().all(|h| h.y > root.y) && f0.iter().all(|f| f.y < root.y));
        assert!((h0[0].x - h0[1].x).abs() > 0.4, "hands apart");
    }

    /// Hands and feet go on the climber's own sides: its left is +X of a body facing the wall.
    #[test]
    fn climb_holds_are_on_the_matching_side() {
        for normal in [Vec3::Z, Vec3::X, Vec3::new(-0.6, 0.0, 0.8)] {
            let n = normal.normalize();
            let yaw = (-n.x).atan2(-n.z);
            let left = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
            let root = Vec3::new(1.0, 2.0, 3.0);
            let (hands, feet) = holds(root, n, 0.1);
            assert!((hands[0] - root).dot(left) > 0.1 && (hands[1] - root).dot(left) < -0.1);
            assert!((feet[0] - root).dot(left) > 0.05 && (feet[1] - root).dot(left) < -0.05);
            let right = Vec3::Y.cross(n);
            let edge = ledge_hands(root, n, right, 3.0);
            assert!((edge[0] - root).dot(left) > 0.1 && (edge[1] - root).dot(left) < -0.1);
        }
    }
}
