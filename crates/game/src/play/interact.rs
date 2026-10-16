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
//! (`PropState`): chest lid and lever toggle, crops and the table item come back after a while,
//! the tree canopy shakes with every chop.
use super::controller::{wrap_angle, yaw_of};
use super::select::HandTargets;
use super::{
    Action, ActionKind, ActionLayer, HALF_HEIGHT, InteractPrompt, Intent, Interactable, MoveMode, Mover, Owner,
    PlaySet, Player,
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

pub fn plugin(app: &mut App) {
    app.register_required_components::<Interactable, PropState>()
        .add_systems(Update, (focus, drive).chain().in_set(PlaySet::Act))
        .add_systems(Update, (animate_props.before(drive), draw_markers));
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
    /// Completed loop cycles; the one-shot effect has been applied.
    cycles: u32,
    applied: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Approach,
    Use,
}

/// What an interactable remembers between uses.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PropState {
    /// Toggle (chest open, lever thrown).
    pub on: bool,
    /// Seconds until a harvested / taken thing is back (0 = present).
    pub regrow: f32,
    /// `on` eased 0..1 for the animation, and the tree's shake energy.
    amount: f32,
    shake: f32,
}

/// Whether the thing can be used now (harvested crops and the taken item must grow back).
pub fn available(it: &Interactable, prop: &PropState) -> bool {
    !(matches!(it.kind.as_str(), "crop" | "table") && prop.regrow > 0.0)
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

/// Hand IK weight over a use: in after the reach starts, out before the clip ends.
fn hand_weight(t: f32, duration: f32, looping: bool) -> f32 {
    let smooth = |a: f32, b: f32, x: f32| {
        let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
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
    characters: Query<(Entity, &Transform, &Mover, &Intent, &Action, Option<&Focus>, Option<&Using>, Has<Player>)>,
    interactables: Query<(Entity, &Transform, &Interactable, &PropState), Without<Mover>>,
) {
    let busy: Vec<Entity> = characters.iter().filter_map(|c| c.6.map(|u| u.target)).collect();
    let mut player_prompt = None;
    for (entity, tf, mover, intent, action, focus, using, player) in &characters {
        let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
        let look = (intent.look - feet).with_y(0.0);
        let view = if player && look.is_finite() && look.length_squared() > 1e-4 { look } else { mover.forward() };
        let best = interactables
            .iter()
            .filter(|(e, _, it, prop)| available(it, prop) && (!busy.contains(e) || using.is_some_and(|u| u.target == *e)))
            .filter_map(|(e, itf, it, _)| {
                let stand = itf.translation + itf.rotation * it.stand;
                focus_score(feet, view, itf.translation, stand, it.range).map(|s| (e, s, it))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let wanted = best.map(|(e, ..)| e);
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
                None if action.is_free() && mover.mode == MoveMode::Ground => best.map(|(_, _, it)| it.prompt.clone()),
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

#[allow(clippy::type_complexity)]
fn drive(
    time: Res<Time>,
    bank: Res<PoseBank>,
    mut commands: Commands,
    mut characters: Query<(Entity, &Transform, &mut Mover, &Intent, &mut Action, Option<&Focus>, Option<&mut Using>)>,
    mut interactables: Query<(&Transform, &mut Interactable, &mut PropState, Option<&Children>), Without<Mover>>,
    parts: Query<(&Name, &Transform), (Without<Interactable>, Without<Mover>)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, tf, mut mover, intent, mut action, focus, using) in &mut characters {
        let Some(mut using) = using else {
            if intent.interact_pressed
                && action.is_free()
                && mover.mode == MoveMode::Ground
                && let Some(target) = focus.and_then(|f| f.0)
                && interactables.contains(target)
            {
                action.start(ActionKind::Interact, Owner::Interact, None);
                mover.move_scale = 0.0;
                commands.entity(entity).insert(Using {
                    target,
                    phase: Phase::Approach,
                    t: 0.0,
                    armed: false,
                    cycles: 0,
                    applied: false,
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

        using.t += dt;
        let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
        let stand = itf.translation + itf.rotation * it.stand;
        let to = (stand - feet).with_y(0.0);
        let dist = to.length();
        let face = face_yaw(itf, &it);
        let stick = intent.move_dir.length();
        if stick < MOVE_RELEASED {
            using.armed = true;
        }
        let quit = intent.jump_pressed || intent.dodge_pressed || intent.interact_pressed || intent.primary_pressed;
        mover.move_scale = 0.0;

        match using.phase {
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
                let w = hand_weight(using.t, duration, it.looping);
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
                    let cycles = (using.t / duration) as u32;
                    if cycles > using.cycles {
                        using.cycles = cycles;
                        prop.shake = 1.0;
                    }
                    if quit || (using.armed && stick > MOVE_CANCEL) {
                        action.clear();
                        release(&mut commands, entity, &mut mover);
                    }
                } else {
                    if !using.applied && using.t >= EFFECT_AT * duration {
                        using.applied = true;
                        apply_use(&mut it, &mut prop);
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

/// A finished one-shot use changes the world.
fn apply_use(it: &mut Interactable, prop: &mut PropState) {
    match it.kind.as_str() {
        "chest" => {
            prop.on = !prop.on;
            it.prompt = if prop.on { "Close chest" } else { "Open chest" }.to_string();
        }
        "lever" => prop.on = !prop.on,
        "crop" | "table" => prop.regrow = REGROW_TIME,
        _ => {}
    }
}

fn animate_props(
    time: Res<Time>,
    mut props: Query<(&Interactable, &mut PropState, &Children)>,
    mut parts: Query<(&Name, &mut Transform, &mut Visibility), Without<Interactable>>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let now = time.elapsed_secs();
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
                ("tree", "Tree canopy") => {
                    let s = prop.shake;
                    tf.rotation = Quat::from_rotation_z(0.06 * s * (17.0 * now).sin())
                        * Quat::from_rotation_x(0.04 * s * (13.0 * now).cos());
                }
                ("crop", "Crop") => {
                    tf.scale = Vec3::splat(grown.max(0.05));
                    vis.set_if_neq(if grown < 0.1 { Visibility::Hidden } else { Visibility::Inherited });
                }
                ("table", "Table item") => {
                    vis.set_if_neq(if prop.regrow > 0.0 { Visibility::Hidden } else { Visibility::Inherited });
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
    player: Query<(&Transform, Option<&Focus>, Option<&Using>), With<Player>>,
    interactables: Query<(Entity, &Transform, &Interactable, &PropState), Without<Player>>,
) {
    let Ok((ptf, focus, using)) = player.single() else {
        return;
    };
    if using.is_some_and(|u| u.phase == Phase::Use) {
        return;
    }
    let feet = ptf.translation - Vec3::Y * HALF_HEIGHT;
    let pulse = 0.5 + 0.5 * (4.0 * time.elapsed_secs()).sin();
    let focused = using.map(|u| u.target).or(focus.and_then(|f| f.0));
    for (e, tf, it, prop) in &interactables {
        if !available(it, prop) {
            continue;
        }
        let stand = tf.translation + tf.rotation * it.stand;
        let d = (stand - feet).length();
        if d > MARKER_RANGE {
            continue;
        }
        let ring = Isometry3d::new(stand + Vec3::Y * 0.03, Quat::from_rotation_x(-FRAC_PI_2));
        if focused == Some(e) {
            let color = Color::srgba(1.0, 0.82, 0.3, 0.6 + 0.4 * pulse);
            gizmos.circle(ring, 0.3 + 0.04 * pulse, color);
            let yaw = face_yaw(tf, it);
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
        assert_eq!(hand_weight(0.0, 2.0, false), 0.0);
        assert_eq!(hand_weight(1.0, 2.0, false), 1.0);
        assert_eq!(hand_weight(2.0, 2.0, false), 0.0);
        assert_eq!(hand_weight(1.0, 2.0, true), 1.0);
    }

    #[test]
    fn harvest_and_toggle() {
        let mut it = Interactable {
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
        };
        let mut prop = PropState::default();
        apply_use(&mut it, &mut prop);
        assert!(prop.on && it.prompt == "Close chest");
        it.kind = "crop".into();
        apply_use(&mut it, &mut prop);
        assert!(!available(&it, &prop));
    }
}
