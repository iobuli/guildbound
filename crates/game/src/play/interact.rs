//! World interaction. Every character keeps a `Focus`: the best `Interactable` in reach, scored
//! by distance and facing (the player's facing is the camera's). For the player it also sets
//! `InteractPrompt`, and `draw_markers` rings every usable thing nearby, so you can see what E
//! does and where you'd stand.
//!
//! E starts an `ActionKind::Interact` action owned by `interact`: the character walks into
//! place (`Mover.root_motion`, input locked by `move_scale`, turned by `face`), then plays the
//! interactable's track as a full-body action layer with hand IK targets. One-shots end with
//! the clip (bank duration when known); looping uses run until E, jump, dodge, attack or a fresh
//! move input. A hit taking over the `Action` drops the use. Uses leave marks on the world
//! (`PropState`): chest lid and lever toggle, crops come back after a while.
//!
//! Other modules react through messages instead of code here: `UseEffect` when a one-shot
//! reaches its effect point (`tools` moves the axe between table and hand), `UseBeat` each time
//! a looping use lands (`trees` cuts the trunk); landing also freezes the clip for
//! `Feel.hitstop`. A loop lands at its `Interactable.beat` clip time, except a tool use, which
//! lands when the held tool's blade stops after a fast swing (`Swing`; the beat is the fallback). What E offers depends on the user's held tool (`offer`): a use
//! that `requires` a tool the user lacks plays a head shake instead. Radial interactables
//! (trees) are used from whichever side the user comes from.
use super::controller::{wrap_angle, yaw_of};
use super::select::HandTargets;
use super::tools::{Holding, Tool, blade};
use super::{
    Action, ActionKind, ActionLayer, Feel, HALF_HEIGHT, InteractPrompt, Intent, Interactable, MoveMode, Mover,
    Owner, PlaySet, Player,
};
use crate::motion::Mask;
use crate::motion::bank::PoseBank;
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

/// Walking speed into place (m/s); it slows over the last `APPROACH_SLOW` metres.
const APPROACH_SPEED: f32 = 1.4;
const APPROACH_SLOW: f32 = 0.25;
/// Distance (m) and heading error (rad) that count as in place.
const IN_PLACE: f32 = 0.06;
const IN_PLACE_YAW: f32 = 0.15;
/// Give up walking into place after this long (s); start anyway when this close (m).
const APPROACH_TIMEOUT: f32 = 2.5;
const CLOSE_ENOUGH: f32 = 0.5;
/// Heading error (rad) above which the approach turns on the spot before walking, and below
/// which it walks at full speed: a pivot, then a step, instead of a sideways slide.
const PIVOT_STOP: f32 = 1.6;
const PIVOT_GO: f32 = 0.6;
/// Holding in place during a use: correction time (s) and speed cap (m/s).
const HOLD_TIME: f32 = 0.12;
const HOLD_SPEED: f32 = 0.6;
/// Stick tilt that counts as released / as a new move that cancels a looping use.
const MOVE_RELEASED: f32 = 0.15;
const MOVE_CANCEL: f32 = 0.35;
/// One-shot uses change the world at this fraction of the clip.
const EFFECT_AT: f32 = 0.45;
/// Harvested crops and the taken table item come back after this long (s).
const REGROW_TIME: f32 = 12.0;
/// Things within this distance of the player get a marker (m).
const MARKER_RANGE: f32 = 14.0;
/// Head shake when E is pressed without the required tool: clip and longest play time (s).
const REFUSE_TRACK: &str = "ual2/Idle_No_Loop";
const REFUSE_TIME: f32 = 1.3;
/// Ramp (s) of hand targets around an `Interactable.reach` window.
const REACH_RAMP: f32 = 0.12;
/// Tool strikes: the blade must have moved faster than this (m/s) and then slowed below this
/// share of its peak; without one, the cycle lands `STRIKE_LATE` s after its clip `beat`.
const STRIKE_FAST: f32 = 3.0;
const STRIKE_STOP: f32 = 0.3;
const STRIKE_LATE: f32 = 0.25;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Interactable, PropState>()
        .add_message::<UseEffect>()
        .add_message::<UseBeat>()
        .add_systems(Update, (focus, drive).chain().in_set(PlaySet::Act))
        .add_systems(Update, (animate_props.before(drive), draw_markers));
}

/// A one-shot use reached its effect point.
#[derive(Message, Clone, Copy, Debug)]
pub struct UseEffect {
    pub user: Entity,
    pub target: Entity,
}

/// A looping use crossed its `Interactable.beat` (the axe lands).
#[derive(Message, Clone, Copy, Debug)]
pub struct UseBeat {
    pub user: Entity,
    pub target: Entity,
}

/// What E does with an interactable for a given user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Offer {
    Nothing,
    Use(String),
    /// Missing the required tool: the prompt says so and E shakes the head.
    Refuse(String),
}

impl Offer {
    pub fn prompt(&self) -> Option<&str> {
        match self {
            Offer::Nothing => None,
            Offer::Use(p) | Offer::Refuse(p) => Some(p),
        }
    }
}

/// The offer for a user holding `held`. The table holds the axe (`PropState.on` = taken):
/// pick it up empty-handed, put it back while holding it.
pub fn offer(it: &Interactable, prop: &PropState, held: Option<Tool>) -> Offer {
    if !available(it, prop) {
        return Offer::Nothing;
    }
    if it.kind == "armor_stand" {
        // `PropState.on` = the set is worn (by the player); the stand takes it back.
        return Offer::Use(if prop.on { "Take off armor".to_string() } else { it.prompt.clone() });
    }
    if it.kind == "table" {
        return match (prop.on, held) {
            (false, None) => Offer::Use(it.prompt.clone()),
            (true, Some(Tool::Axe)) => Offer::Use("Put down axe".to_string()),
            _ => Offer::Nothing,
        };
    }
    match &it.requires {
        Some(tool) if held.map(Tool::name) != Some(tool.as_str()) => {
            Offer::Refuse(format!("{} (need {tool})", it.prompt))
        }
        _ => Offer::Use(it.prompt.clone()),
    }
}

/// Where the user stands for `it` (feet) and the yaw it faces, coming from `feet`.
pub fn stand_and_face(root: &Transform, it: &Interactable, feet: Vec3) -> (Vec3, f32) {
    match it.radial {
        Some(r) if r.is_finite() => {
            let back = root.rotation * Vec3::NEG_Z;
            let away = (feet - root.translation).with_y(0.0).normalize_or(back);
            let stand = root.translation + away * r;
            (stand, yaw_of(-away).unwrap_or(0.0))
        }
        _ => (root.translation + root.rotation * it.stand, face_yaw(root, it)),
    }
}

/// The interactable a character would use with E right now.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Focus(pub Option<Entity>);

/// A character's interaction in progress; removed when it ends or a hit takes the `Action`.
#[derive(Component, Clone, Copy, Debug)]
pub struct Using {
    pub target: Entity,
    pub phase: Phase,
    /// Seconds in the current phase.
    pub t: f32,
    /// Move input was released since the use began (a key still held from walking up to the
    /// object must not cancel it at once).
    armed: bool,
    /// The one-shot effect has been applied.
    applied: bool,
    /// Stand point and facing, fixed when the use starts.
    stand: Vec3,
    face: f32,
    /// Seconds the clip stays frozen after a beat (hitstop).
    freeze: f32,
    /// Tool uses: the swing being watched for its strike.
    swing: Swing,
}

/// Watches a held tool's blade for the moment it stops after swinging fast: the strike. The
/// clip clock alone misses it, because stylized timing, springs and exaggeration move the
/// visible impact away from the raw clip's.
#[derive(Clone, Copy, Debug, Default)]
pub struct Swing {
    prev: Option<Vec3>,
    peak: f32,
    /// Clip cycle that already struck.
    struck: Option<i64>,
}

impl Swing {
    /// Feed the blade point (world) after `dt` s, in clip cycle `cycle` at `t` s into it. True
    /// once per cycle: when the blade slows below `STRIKE_STOP` of a peak above `STRIKE_FAST`,
    /// or at `fallback` s into the cycle if it never did.
    pub fn step(&mut self, blade: Option<Vec3>, dt: f32, cycle: i64, t: f32, fallback: Option<f32>) -> bool {
        if self.struck.is_some_and(|c| c < cycle) {
            self.struck = None;
            self.peak = 0.0;
        }
        let speed = match (self.prev, blade) {
            (Some(a), Some(b)) if dt > 1e-4 && a.is_finite() && b.is_finite() => (b - a).length() / dt,
            _ => 0.0,
        };
        self.prev = blade;
        if self.struck == Some(cycle) {
            return false;
        }
        self.peak = self.peak.max(speed);
        let stopped = self.peak > STRIKE_FAST && speed < STRIKE_STOP * self.peak;
        let late = fallback.is_some_and(|f| t >= f);
        if stopped || late {
            self.struck = Some(cycle);
            self.peak = 0.0;
        }
        stopped || late
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Approach,
    Use,
    /// Head shake: the required tool is missing.
    Refuse,
}

/// What an interactable remembers between uses.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PropState {
    /// Toggle (chest open, lever thrown).
    pub on: bool,
    /// Seconds until a harvested / taken thing is back (0 = present).
    pub regrow: f32,
    /// `on` eased 0..1 for the animation.
    amount: f32,
    /// Canopy shake energy (set by `trees` on every chop, decays here).
    pub shake: f32,
    /// The owning module has nothing to offer right now (an armour stand with no plates).
    pub disabled: bool,
}

/// Whether the thing can be used now (harvested crops grow back, felled trees regrow, owners can
/// switch a prop off with `disabled`).
pub fn available(it: &Interactable, prop: &PropState) -> bool {
    !prop.disabled && !(matches!(it.kind.as_str(), "crop" | "tree") && prop.regrow > 0.0)
}

/// Focus score (lower is better) of an interactable centred at `center`, used from `stand`, for
/// a character at `feet` looking along `view`; `None` when out of reach or behind.
pub fn focus_score(feet: Vec3, view: Vec3, center: Vec3, stand: Vec3, range: f32) -> Option<f32> {
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    let d = flat(stand - feet).length().min(flat(center - feet).length());
    if !d.is_finite() || d > range || (feet.y - stand.y).abs() > 1.2 {
        return None;
    }
    let facing = flat(center - feet).normalize_or_zero().dot(flat(view).normalize_or_zero());
    if facing < -0.2 && d > 0.8 {
        return None;
    }
    Some(d - 0.8 * facing)
}

/// Hand IK weight over a use: in after the reach starts, out before the clip ends. `reach`
/// (clip seconds) overrides when the hands hold fully; they ramp over `REACH_RAMP` around it.
fn hand_weight(t: f32, duration: f32, looping: bool, reach: Option<(f32, f32)>) -> f32 {
    let smooth = |a: f32, b: f32, x: f32| {
        let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    if let Some((from, until)) = reach.filter(|r| r.0.is_finite() && r.1.is_finite()) {
        return smooth(from - REACH_RAMP, from, t) * (1.0 - smooth(until, until + REACH_RAMP, t));
    }
    if looping {
        smooth(0.0, 0.3, t)
    } else {
        let u = t / duration.max(1e-3);
        smooth(0.15, 0.35, u) * (1.0 - smooth(0.7, 0.9, u))
    }
}

/// World yaw the user faces: the interactable's `face_yaw` turned with its root.
fn face_yaw(root: &Transform, it: &Interactable) -> f32 {
    let (root_yaw, _, _) = root.rotation.to_euler(EulerRot::YXZ);
    wrap_angle(it.face_yaw + root_yaw)
}

#[allow(clippy::type_complexity)]
fn focus(
    mut commands: Commands,
    mut prompt: ResMut<InteractPrompt>,
    characters: Query<(
        Entity,
        &Transform,
        &Mover,
        &Intent,
        &Action,
        Option<&Focus>,
        Option<&Using>,
        Option<&Holding>,
        Has<Player>,
    )>,
    interactables: Query<(Entity, &Transform, &Interactable, &PropState), Without<Mover>>,
) {
    let busy: Vec<Entity> = characters.iter().filter_map(|c| c.6.map(|u| u.target)).collect();
    let mut player_prompt = None;
    for (entity, tf, mover, intent, action, focus, using, holding, player) in &characters {
        let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
        let look = (intent.look - feet).with_y(0.0);
        let view = if player && look.is_finite() && look.length_squared() > 1e-4 { look } else { mover.forward() };
        let held = holding.map(|h| h.tool);
        let best = interactables
            .iter()
            .filter(|(e, ..)| !busy.contains(e) || using.is_some_and(|u| u.target == *e))
            .filter_map(|(e, itf, it, prop)| {
                let offer = offer(it, prop, held);
                offer.prompt()?;
                let (stand, _) = stand_and_face(itf, it, feet);
                focus_score(feet, view, itf.translation, stand, it.range).map(|s| (e, s, offer))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let wanted = best.as_ref().map(|(e, ..)| *e);
        if focus.map(|f| f.0) != Some(wanted) {
            commands.entity(entity).insert(Focus(wanted));
        }
        if player {
            player_prompt = match using {
                Some(u) if u.phase == Phase::Use => interactables
                    .get(u.target)
                    .ok()
                    .filter(|(_, _, it, _)| it.looping)
                    .map(|_| "Stop".to_string()),
                Some(_) => None,
                None if action.is_free() && mover.mode == MoveMode::Ground => {
                    best.and_then(|(_, _, offer)| offer.prompt().map(str::to_string))
                }
                None => None,
            };
        }
    }
    if prompt.0 != player_prompt {
        prompt.0 = player_prompt;
    }
}

/// Hand the body back to the player / AI.
fn release(commands: &mut Commands, entity: Entity, mover: &mut Mover) {
    mover.move_scale = 1.0;
    mover.face = None;
    mover.root_motion = Vec3::ZERO;
    commands.entity(entity).remove::<(Using, HandTargets)>();
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn drive(
    time: Res<Time>,
    bank: Res<PoseBank>,
    feel: Res<Feel>,
    mut commands: Commands,
    mut effects: MessageWriter<UseEffect>,
    mut beats: MessageWriter<UseBeat>,
    mut characters: Query<(
        Entity,
        &Transform,
        &mut Mover,
        &Intent,
        &mut Action,
        Option<&Focus>,
        Option<&mut Using>,
        Option<&Holding>,
    )>,
    mut interactables: Query<(&Transform, &mut Interactable, &mut PropState, Option<&Children>), Without<Mover>>,
    parts: Query<(&Name, &Transform), (Without<Interactable>, Without<Mover>)>,
    globals: Query<&GlobalTransform>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, tf, mut mover, intent, mut action, focus, using, holding) in &mut characters {
        let Some(mut using) = using else {
            if intent.interact_pressed
                && action.is_free()
                && mover.mode == MoveMode::Ground
                && let Some(target) = focus.and_then(|f| f.0)
                && let Ok((itf, it, prop, _)) = interactables.get(target)
            {
                let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
                let (stand, face) = stand_and_face(itf, &it, feet);
                let phase = match offer(&it, &prop, holding.map(|h| h.tool)) {
                    Offer::Use(_) => Phase::Approach,
                    Offer::Refuse(_) => Phase::Refuse,
                    Offer::Nothing => continue,
                };
                let layer = (phase == Phase::Refuse).then(|| ActionLayer {
                    looping: true,
                    ..ActionLayer::new(REFUSE_TRACK, 0.0, Mask::Upper)
                });
                action.start(ActionKind::Interact, Owner::Interact, layer);
                mover.move_scale = 0.0;
                commands.entity(entity).insert(Using {
                    target,
                    phase,
                    t: 0.0,
                    armed: false,
                    applied: false,
                    stand,
                    face,
                    freeze: 0.0,
                    swing: Swing::default(),
                });
            }
            continue;
        };
        if action.owner != Owner::Interact {
            // A hit (or another owner) took the body: drop the use, leave their action alone.
            release(&mut commands, entity, &mut mover);
            continue;
        }
        let Ok((itf, mut it, mut prop, children)) = interactables.get_mut(using.target) else {
            action.clear();
            release(&mut commands, entity, &mut mover);
            continue;
        };

        let t_before = using.t;
        if using.freeze > 0.0 {
            using.freeze = (using.freeze - dt).max(0.0);
        } else {
            using.t += dt;
        }
        let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
        let (stand, face) = (using.stand, using.face);
        let to = (stand - feet).with_y(0.0);
        let dist = to.length();
        let stick = intent.move_dir.length();
        if stick < MOVE_RELEASED {
            using.armed = true;
        }
        let quit = intent.jump_pressed || intent.dodge_pressed || intent.interact_pressed || intent.primary_pressed;
        mover.move_scale = 0.0;

        match using.phase {
            Phase::Refuse => {
                mover.root_motion = Vec3::ZERO;
                mover.face = None;
                if let Some(layer) = &mut action.layer {
                    layer.time = using.t;
                }
                action.elapsed = using.t;
                let duration = if bank.has(REFUSE_TRACK) { bank.duration(REFUSE_TRACK) } else { REFUSE_TIME };
                let done = using.t >= duration.min(REFUSE_TIME);
                if done || quit || (using.armed && stick > MOVE_CANCEL) {
                    action.clear();
                    release(&mut commands, entity, &mut mover);
                }
            }
            Phase::Approach => {
                let heading = yaw_of(to).map_or(0.0, |y| wrap_angle(y - mover.yaw).abs());
                let pivot = 1.0 - ((heading - PIVOT_GO) / (PIVOT_STOP - PIVOT_GO)).clamp(0.0, 1.0);
                let pivot = if dist > 0.3 { pivot * pivot * (3.0 - 2.0 * pivot) } else { 1.0 };
                let speed = (dist / APPROACH_SLOW).min(1.0) * APPROACH_SPEED * pivot;
                mover.root_motion = if dist > 1e-3 { to / dist * speed } else { Vec3::ZERO };
                mover.face = Some(if dist > 0.3 { yaw_of(to).unwrap_or(face) } else { face });
                let in_place = dist < IN_PLACE && wrap_angle(face - mover.yaw).abs() < IN_PLACE_YAW;
                let timed_out = using.t > APPROACH_TIMEOUT;
                if quit || (timed_out && dist >= CLOSE_ENOUGH) {
                    action.clear();
                    release(&mut commands, entity, &mut mover);
                } else if in_place || timed_out {
                    using.phase = Phase::Use;
                    using.t = 0.0;
                }
            }
            Phase::Use => {
                mover.root_motion = (to / HOLD_TIME).clamp_length_max(HOLD_SPEED);
                mover.face = Some(face);
                let duration = if bank.has(&it.track) { bank.duration(&it.track) } else { it.duration }.max(0.2);
                action.elapsed = using.t;
                action.layer = Some(ActionLayer {
                    track: it.track.clone(),
                    time: using.t,
                    looping: it.looping,
                    mask: Mask::Full,
                    weight: 1.0,
                    style: None,
                });
                let w = hand_weight(using.t, duration, it.looping, it.reach);
                // Hands ride on the grip part (lever arm, chest lid) as it moves.
                let grip = it
                    .grip
                    .as_deref()
                    .and_then(|name| {
                        children?.iter().find_map(|c| parts.get(c).ok().filter(|(n, _)| n.as_str() == name))
                    })
                    .map_or(*itf, |(_, part)| itf.mul_transform(*part));
                let hands = it.hands.map(|h| h.map(|p| (grip.transform_point(p), w)));
                commands.entity(entity).insert(HandTargets { hands });

                if it.looping {
                    let beat = it.beat.filter(|b| b.is_finite());
                    let landed = match holding.filter(|_| it.requires.is_some()) {
                        // A tool lands when it stops; the clip beat is only the fallback.
                        Some(held) => {
                            let edge = globals.get(held.item).ok().map(blade);
                            let cycle = (using.t / duration).floor() as i64;
                            let into = using.t - cycle as f32 * duration;
                            using.swing.step(edge, dt, cycle, into, beat.map(|b| b + STRIKE_LATE))
                        }
                        None => beat.is_some_and(|b| crossed_beat(t_before, using.t, b, duration)),
                    };
                    if landed {
                        using.freeze = feel.hitstop.clamp(0.0, 0.25);
                        beats.write(UseBeat { user: entity, target: using.target });
                    }
                    if quit || (using.armed && stick > MOVE_CANCEL) || !available(&it, &prop) {
                        action.clear();
                        release(&mut commands, entity, &mut mover);
                    }
                } else {
                    if !using.applied && using.t >= it.beat.filter(|b| b.is_finite()).unwrap_or(EFFECT_AT * duration) {
                        using.applied = true;
                        apply_use(&mut it, &mut prop);
                        effects.write(UseEffect { user: entity, target: using.target });
                    }
                    if using.t >= duration {
                        action.clear();
                        release(&mut commands, entity, &mut mover);
                    }
                }
            }
        }
    }
}

/// Whether a loop's clip time passed `beat` (s into a clip of `duration`) going from `before`
/// to `after` (seconds since the use began).
pub fn crossed_beat(before: f32, after: f32, beat: f32, duration: f32) -> bool {
    let d = duration.max(1e-3);
    after > before && ((after - beat) / d).floor() > ((before - beat) / d).floor()
}

/// A finished one-shot use changes the world.
fn apply_use(it: &mut Interactable, prop: &mut PropState) {
    match it.kind.as_str() {
        "chest" => {
            prop.on = !prop.on;
            it.prompt = if prop.on { "Close chest" } else { "Open chest" }.to_string();
        }
        "lever" => prop.on = !prop.on,
        "crop" => prop.regrow = REGROW_TIME,
        _ => {}
    }
}

fn animate_props(
    time: Res<Time>,
    mut props: Query<(&Interactable, &mut PropState, &Children)>,
    mut parts: Query<(&Name, &mut Transform, &mut Visibility), Without<Interactable>>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (it, mut prop, children) in &mut props {
        let target = if prop.on { 1.0 } else { 0.0 };
        prop.amount += (target - prop.amount).clamp(-2.5 * dt, 2.5 * dt);
        prop.regrow = (prop.regrow - dt).max(0.0);
        prop.shake *= (-4.0 * dt).exp();
        let eased = prop.amount * prop.amount * (3.0 - 2.0 * prop.amount);
        let grown = 1.0 - prop.regrow / REGROW_TIME;
        for child in children.iter() {
            let Ok((name, mut tf, mut vis)) = parts.get_mut(child) else {
                continue;
            };
            match (it.kind.as_str(), name.as_str()) {
                ("chest", "Chest lid") => tf.rotation = Quat::from_rotation_x(1.9 * eased),
                ("lever", "Lever arm") => tf.rotation = Quat::from_rotation_x(-0.5 + eased),
                ("crop", "Crop") => {
                    tf.scale = Vec3::splat(grown.max(0.05));
                    vis.set_if_neq(if grown < 0.1 { Visibility::Hidden } else { Visibility::Inherited });
                }
                _ => {}
            }
        }
    }
}

/// Rings where to stand for every usable thing near the player; the focused one pulses and
/// shows the facing.
fn draw_markers(
    mut gizmos: Gizmos,
    time: Res<Time>,
    player: Query<(&Transform, Option<&Focus>, Option<&Using>, Option<&Holding>), With<Player>>,
    interactables: Query<(Entity, &Transform, &Interactable, &PropState), Without<Player>>,
) {
    let Ok((ptf, focus, using, holding)) = player.single() else {
        return;
    };
    if using.is_some_and(|u| u.phase != Phase::Approach) {
        return;
    }
    let feet = ptf.translation - Vec3::Y * HALF_HEIGHT;
    let pulse = 0.5 + 0.5 * (4.0 * time.elapsed_secs()).sin();
    let focused = using.map(|u| u.target).or(focus.and_then(|f| f.0));
    for (e, tf, it, prop) in &interactables {
        if offer(it, prop, holding.map(|h| h.tool)) == Offer::Nothing {
            continue;
        }
        let (stand, yaw) = match using.filter(|u| u.target == e) {
            Some(u) => (u.stand, u.face),
            None => stand_and_face(tf, it, feet),
        };
        let d = (stand - feet).length();
        if d > MARKER_RANGE {
            continue;
        }
        let ring = Isometry3d::new(stand + Vec3::Y * 0.03, Quat::from_rotation_x(-FRAC_PI_2));
        if focused == Some(e) {
            let color = Color::srgba(1.0, 0.82, 0.3, 0.6 + 0.4 * pulse);
            gizmos.circle(ring, 0.3 + 0.04 * pulse, color);
            let base = stand + Vec3::Y * 0.05;
            gizmos.arrow(base, base + Vec3::new(yaw.sin(), 0.0, yaw.cos()) * 0.45, color);
        } else {
            let near = 1.0 - d / MARKER_RANGE;
            gizmos.circle(ring, 0.22, Color::srgba(0.85, 0.92, 1.0, 0.2 + 0.4 * near));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_prefers_close_and_in_front() {
        let feet = Vec3::ZERO;
        let view = Vec3::Z;
        let ahead = focus_score(feet, view, Vec3::new(0.0, 0.0, 1.5), Vec3::new(0.0, 0.0, 0.8), 1.8).unwrap();
        let side = focus_score(feet, view, Vec3::new(1.5, 0.0, 0.0), Vec3::new(0.8, 0.0, 0.0), 1.8).unwrap();
        assert!(ahead < side, "{ahead} {side}");
        // Behind and not right next to it: no focus. Out of range or another floor: none.
        assert!(focus_score(feet, view, Vec3::new(0.0, 0.0, -1.5), Vec3::new(0.0, 0.0, -1.2), 1.8).is_none());
        assert!(focus_score(feet, view, Vec3::new(0.0, 0.0, 4.0), Vec3::new(0.0, 0.0, 3.2), 1.8).is_none());
        assert!(focus_score(feet, view, Vec3::new(0.0, 2.0, 1.0), Vec3::new(0.0, 2.0, 0.5), 1.8).is_none());
    }

    #[test]
    fn hands_fade_in_and_out() {
        assert_eq!(hand_weight(0.0, 2.0, false, None), 0.0);
        assert_eq!(hand_weight(1.0, 2.0, false, None), 1.0);
        assert_eq!(hand_weight(2.0, 2.0, false, None), 0.0);
        assert_eq!(hand_weight(1.0, 2.0, true, None), 1.0);
        let reach = Some((0.22, 0.3));
        assert_eq!(hand_weight(0.26, 0.833, false, reach), 1.0);
        assert_eq!(hand_weight(0.05, 0.833, false, reach), 0.0);
        assert_eq!(hand_weight(0.5, 0.833, false, reach), 0.0);
    }

    fn chest() -> Interactable {
        Interactable {
            kind: "chest".into(),
            prompt: "Open chest".into(),
            track: String::new(),
            duration: 1.0,
            looping: false,
            stand: Vec3::ZERO,
            face_yaw: 0.0,
            hands: [None, None],
            grip: None,
            range: 1.8,
            requires: None,
            beat: None,
            reach: None,
            radial: None,
        }
    }

    #[test]
    fn harvest_and_toggle() {
        let mut it = chest();
        let mut prop = PropState::default();
        apply_use(&mut it, &mut prop);
        assert!(prop.on && it.prompt == "Close chest");
        it.kind = "crop".into();
        apply_use(&mut it, &mut prop);
        assert!(!available(&it, &prop));
    }

    #[test]
    fn offers_follow_the_held_tool() {
        let tree = Interactable {
            kind: "tree".into(),
            prompt: "Chop tree".into(),
            requires: Some("axe".into()),
            ..chest()
        };
        let mut prop = PropState::default();
        assert_eq!(offer(&tree, &prop, None), Offer::Refuse("Chop tree (need axe)".into()));
        assert_eq!(offer(&tree, &prop, Some(Tool::Axe)), Offer::Use("Chop tree".into()));
        prop.regrow = 5.0;
        assert_eq!(offer(&tree, &prop, Some(Tool::Axe)), Offer::Nothing, "felled");
        let table = Interactable {
            kind: "table".into(),
            prompt: "Pick up axe".into(),
            ..chest()
        };
        let mut prop = PropState::default();
        assert_eq!(offer(&table, &prop, None), Offer::Use("Pick up axe".into()));
        assert_eq!(offer(&table, &prop, Some(Tool::Axe)), Offer::Nothing);
        prop.on = true;
        assert_eq!(offer(&table, &prop, Some(Tool::Axe)), Offer::Use("Put down axe".into()));
        assert_eq!(offer(&table, &prop, None), Offer::Nothing);
    }

    #[test]
    fn radial_stand_faces_the_root_from_the_user_side() {
        let it = Interactable {
            radial: Some(1.5),
            ..chest()
        };
        let root = Transform::from_xyz(2.0, 0.0, 3.0);
        let (stand, yaw) = stand_and_face(&root, &it, Vec3::new(10.0, 0.0, 3.0));
        assert!(stand.abs_diff_eq(Vec3::new(3.5, 0.0, 3.0), 1e-5), "{stand}");
        let facing = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        assert!(facing.abs_diff_eq(Vec3::NEG_X, 1e-5), "{facing}");
        // Standing on the root: fall back to its back side.
        let (stand, _) = stand_and_face(&root, &it, root.translation);
        assert!(stand.is_finite());
    }

    #[test]
    fn tool_strikes_when_the_blade_stops_once_per_cycle() {
        let dt = 1.0 / 60.0;
        let d = 0.967;
        let mut swing = Swing::default();
        let mut fired = Vec::new();
        let mut blade = Vec3::new(0.0, 2.0, 0.0);
        for frame in 0..180 {
            let t = frame as f32 * dt;
            let cycle = (t / d).floor() as i64;
            let into = t - cycle as f32 * d;
            // Fast downswing 0.0-0.2 s, stop (embedded) 0.2-0.4 s, slow wind-up after.
            let v = if into < 0.2 { Vec3::new(0.0, -6.0, 6.0) } else if into < 0.4 { Vec3::ZERO } else { Vec3::Y * 1.5 };
            blade += v * dt;
            if swing.step(Some(blade), dt, cycle, into, Some(0.42)) {
                fired.push((cycle, into));
            }
        }
        assert_eq!(fired.len(), 3, "{fired:?}");
        for (k, (cycle, into)) in fired.iter().enumerate() {
            assert_eq!(*cycle, k as i64);
            assert!((0.19..0.25).contains(into), "strike right after the stop: {into}");
        }
        // A blade that never moves fast still lands, at the fallback time.
        let mut swing = Swing::default();
        let hits: Vec<f32> = (0..60)
            .map(|f| f as f32 * dt)
            .filter(|&t| swing.step(Some(Vec3::X * t), dt, 0, t, Some(0.42)))
            .collect();
        assert_eq!(hits.len(), 1);
        assert!((hits[0] - 0.42).abs() < dt + 1e-4);
    }

    #[test]
    fn beats_fire_once_per_cycle() {
        let (beat, d) = (0.17, 0.967);
        let mut fired = 0;
        let mut t = 0.0;
        for _ in 0..600 {
            let next = t + 1.0 / 60.0;
            if crossed_beat(t, next, beat, d) {
                fired += 1;
            }
            t = next;
        }
        // Landings at 0.17 + k·0.967 s up to 10 s: k = 0..=10.
        assert_eq!(fired, 11, "10 s of chopping at {d} s per swing");
        assert!(!crossed_beat(0.5, 0.5, beat, d), "a frozen clip does not re-fire");
    }
}
