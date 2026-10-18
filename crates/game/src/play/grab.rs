//! Command grab (G): seize the enemy in front, swing them round, throw them.
//!
//! A free, grounded fighter presses `Intent.grab_pressed`; the nearest enemy within `RANGE`
//! inside the front cone is the mark. The grab is an `Owner::Grab` / `ActionKind::Grab` action:
//! - reach (0..`CATCH`): both hands go to the mark's shoulders (`HandTargets`, with sphere grips so
//!   the fingers close on them) and the grabber closes to `HOLD_DIST`;
//! - catch at `CATCH`: if the mark is still in reach, in front and `Melee::grabbable` (no dodge
//!   i-frames, not down or dead) it is caught: its `Action` becomes `Owner::Grab` (it can't act),
//!   it plays `HELD_TRACK` and is held at arm's length; otherwise the grab whiffs and ends early;
//! - hold (`CATCH`..`THROW`): the grabber turns through `SWING` and drags the mark round with it
//!   (root motion on both); `THROW_TRACK` runs so its release lands on `THROW`;
//! - throw at `THROW`: the mark is let go and a `Hit` of kind `HitKind::Grab` (unblockable, see
//!   `combat::strike`) flings it along the swing; `combat` resolves it as usual (stagger,
//!   knockdown or ragdoll above `Feel.ragdoll_threshold`).
//! If anything takes either body (a hit on the grabber, the mark dying), both are let go.
//!
//! Ownership: the grabber's `Action` / `Mover.root_motion` / `face` / `HandTargets` during the grab
//! and the held mark's `Action` / `root_motion` / `face` while `Grabbed`.
use super::combat::Melee;
use super::select::HandTargets;
use super::{Action, ActionKind, ActionLayer, Fighter, Hit, HitKind, Intent, MoveMode, Mover, Owner, PlaySet, StaminaSpend};
use crate::motion::Mask;
use crate::motion::Rig;
use crate::motion::bank::PoseBank;
use crate::motion::hand::{Grip, Shape};
use bevy::prelude::*;

/// Reach of a grab (m, centre to centre) and the half-angle of the front cone (cos).
pub const RANGE: f32 = 1.45;
pub const CONE: f32 = 0.5;
/// Distance at which the mark is held (m).
pub const HOLD_DIST: f32 = 0.72;
/// Timeline (s): catch, throw, end; a whiff ends at `WHIFF_END`.
pub const CATCH: f32 = 0.3;
pub const THROW: f32 = 0.75;
pub const END: f32 = 1.15;
pub const WHIFF_END: f32 = 0.6;
/// The grabber's turn while swinging the mark round (rad).
pub const SWING: f32 = 1.4;
/// The throw.
pub const DAMAGE: f32 = 14.0;
pub const POISE: f32 = 60.0;
pub const KNOCKBACK: f32 = 7.5;
pub const STAMINA: f32 = 18.0;
/// Clips: the grabber's throw (release at `THROW_RELEASE` s) and the held mark's flinch.
const THROW_TRACK: &str = "ual2/OverhandThrow";
const THROW_RELEASE: f32 = 0.37;
const HELD_TRACK: &str = "ual2/Hit_Knockback";
/// Shoulder grip radius (m).
const SHOULDER: f32 = 0.06;

/// On the grabber.
#[derive(Component, Debug, Clone)]
pub struct Grabbing {
    pub mark: Entity,
    t: f32,
    caught: bool,
    thrown: bool,
    /// Facing when the grab began; the swing turns from it.
    yaw: f32,
}

/// On the held mark (read by whoever needs to know who holds it).
#[derive(Component, Debug, Clone, Copy)]
pub struct Grabbed {
    #[allow(dead_code)]
    pub by: Entity,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (start, run).chain().in_set(PlaySet::Act).before(super::combat::CombatSet));
}

fn forward(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

/// The best mark for a grabber at `me` facing `facing`: the nearest of `candidates`
/// (entity, centre) inside `RANGE` and the front cone.
pub fn pick(me: Vec3, facing: Vec3, candidates: impl Iterator<Item = (Entity, Vec3)>) -> Option<Entity> {
    candidates
        .filter_map(|(e, p)| {
            let to = (p - me).with_y(0.0);
            let d = to.length();
            (d < RANGE && d > 1e-3 && facing.dot(to / d) > CONE).then_some((e, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e)
}

/// Where the mark is held at `t` s: in front of the grabber, turned with the swing.
pub fn hold_point(grabber: Vec3, yaw: f32, t: f32) -> Vec3 {
    let u = ((t - CATCH) / (THROW - CATCH)).clamp(0.0, 1.0);
    let u = u * u * (3.0 - 2.0 * u);
    grabber + forward(yaw + SWING * u) * HOLD_DIST
}

#[allow(clippy::type_complexity)]
fn start(
    mut commands: Commands,
    mut fighters: Query<(Entity, &Transform, &Intent, &mut Action, &mut Mover, &Fighter, &Melee), Without<Grabbing>>,
) {
    let snapshot: Vec<(Entity, Vec3, u8, bool)> = fighters
        .iter()
        .map(|(e, tf, _, _, _, f, m)| (e, tf.translation, f.team, !f.dead && m.grabbable()))
        .collect();
    for (entity, tf, intent, mut action, mut mover, fighter, _) in &mut fighters {
        if !intent.grab_pressed || !action.is_free() || mover.mode != MoveMode::Ground || fighter.dead {
            continue;
        }
        let me = tf.translation;
        let mark = pick(
            me,
            mover.forward(),
            snapshot.iter().filter(|s| s.0 != entity && s.2 != fighter.team && s.3).map(|s| (s.0, s.1)),
        );
        let yaw = mover.yaw;
        // A miss still reaches: it whiffs at the catch.
        let mark = mark.unwrap_or(entity);
        action.start(ActionKind::Grab, Owner::Grab, None);
        mover.move_scale = 0.0;
        commands.entity(entity).insert(Grabbing { mark, t: 0.0, caught: false, thrown: false, yaw });
        info!("[grab] {entity} reaches for {mark}");
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    mut grabbers: Query<(Entity, &mut Grabbing)>,
    mut bodies: Query<(&Transform, &mut Action, &mut Mover, &Fighter, &Melee, Option<&Rig>)>,
    globals: Query<&GlobalTransform>,
    mut hits: MessageWriter<Hit>,
    mut stamina: MessageWriter<StaminaSpend>,
) {
    let Some(bank) = bank else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, mut grab) in &mut grabbers {
        let before = grab.t;
        grab.t += dt;
        let t = grab.t;
        let mark = grab.mark;
        let whiff = mark == entity;

        // Both bodies, or let go.
        let Ok((gtf, gaction, ..)) = bodies.get(entity) else {
            commands.entity(entity).remove::<Grabbing>();
            continue;
        };
        let me = gtf.translation;
        let still_mine = gaction.owner == Owner::Grab;
        let mark_state = (!whiff).then(|| bodies.get(mark).ok()).flatten().map(|(tf, a, m, f, melee, rig)| {
            (tf.translation, a.owner == Owner::Grab, m.mode, f.dead, melee.grabbable(), rig.cloned())
        });
        let end = if grab.caught || !whiff { END } else { WHIFF_END };
        let lost_mark = grab.caught && mark_state.as_ref().is_none_or(|m| !m.1 || m.3 || m.2 == MoveMode::Ragdoll);
        if !still_mine || (lost_mark && !grab.thrown) || t >= end || (whiff && t >= WHIFF_END) {
            if still_mine && let Ok((_, mut a, mut m, ..)) = bodies.get_mut(entity) {
                a.clear();
                m.root_motion = Vec3::ZERO;
                m.move_scale = 1.0;
                m.face = None;
            }
            release_mark(&mut commands, &mut bodies, mark, entity, grab.thrown);
            commands.entity(entity).remove::<(Grabbing, HandTargets)>();
            continue;
        }

        // Catch.
        if !grab.caught && before < CATCH && t >= CATCH {
            let caught = mark_state.as_ref().is_some_and(|m| {
                let to = (m.0 - me).with_y(0.0);
                m.4 && !m.3 && m.2 == MoveMode::Ground && to.length() < RANGE + 0.2 && forward(grab.yaw).dot(to.normalize_or_zero()) > CONE * 0.8
            });
            if caught {
                grab.caught = true;
                if let Ok((_, mut a, mut m, ..)) = bodies.get_mut(mark) {
                    a.start(ActionKind::Grab, Owner::Grab, Some(ActionLayer::new(HELD_TRACK, 0.0, Mask::Full)));
                    m.move_scale = 0.0;
                }
                commands.entity(mark).insert(Grabbed { by: entity });
                stamina.write(StaminaSpend { who: entity, amount: STAMINA });
                info!("[grab] {entity} caught {mark}");
            } else {
                grab.mark = entity;
                info!("[grab] {entity} whiffed");
            }
        }

        // The grabber.
        let mark_pos = mark_state.as_ref().map(|m| m.0);
        if let Ok((_, mut a, mut m, ..)) = bodies.get_mut(entity) {
            a.elapsed = t;
            let throw_time = (t - (THROW - THROW_RELEASE)).max(0.0);
            a.layer = (t >= THROW - THROW_RELEASE).then(|| ActionLayer::new(THROW_TRACK, throw_time, Mask::Upper));
            let swing = ((t - CATCH) / (THROW - CATCH)).clamp(0.0, 1.0);
            let swing = swing * swing * (3.0 - 2.0 * swing);
            m.face = Some(if grab.caught { grab.yaw + SWING * swing } else { grab.yaw });
            m.root_motion = match (grab.caught, mark_pos) {
                (false, Some(p)) if t < CATCH => {
                    let to = (p - me).with_y(0.0);
                    to.normalize_or_zero() * ((to.length() - HOLD_DIST) / 0.2).clamp(-1.5, 2.5)
                }
                _ => Vec3::ZERO,
            };
        }

        // The hands: on the mark's shoulders while reaching and holding.
        let w = if t < CATCH { (t / CATCH).min(1.0) } else if grab.caught && !grab.thrown { 1.0 } else { (1.0 - (t - THROW) / 0.2).clamp(0.0, 1.0) };
        let shoulders = mark_state.as_ref().and_then(|m| m.5.as_ref()).and_then(|rig| {
            let at = |bone: usize| rig.bones.get(bone).and_then(|b| globals.get(*b).ok()).map(|g| g.translation());
            Some([at(bank.bones.upperarm[1])?, at(bank.bones.upperarm[0])?])
        });
        match shoulders {
            // The grabber's left hand takes the mark's right shoulder (they face each other).
            Some(s) if w > 0.01 && !whiff => {
                let shape = |p: Vec3| Some(Grip::new(Shape::Sphere { center: p, radius: SHOULDER }, w));
                commands.entity(entity).insert(HandTargets { hands: [Some((s[0], w)), Some((s[1], w))], grips: [shape(s[0]), shape(s[1])] });
            }
            _ => {
                commands.entity(entity).remove::<HandTargets>();
            }
        }

        // The mark, held and swung round.
        if grab.caught && !grab.thrown
            && let Ok((mtf, _, mut m, ..)) = bodies.get_mut(mark)
        {
            let want = hold_point(me, grab.yaw, t);
            m.root_motion = ((want - mtf.translation).with_y(0.0) / 0.1).clamp_length_max(6.0);
            m.face = crate::play::controller::yaw_of(me - mtf.translation);
        }

        // The throw.
        if grab.caught && !grab.thrown && t >= THROW {
            grab.thrown = true;
            let p = mark_pos.unwrap_or(me + forward(grab.yaw) * HOLD_DIST);
            // Along the swing (its tangent) and outward.
            let out = (p - me).with_y(0.0).normalize_or(forward(grab.yaw));
            let tangent = Vec3::Y.cross(out);
            let direction = (out + tangent * SWING.signum()).normalize_or(out);
            release_mark(&mut commands, &mut bodies, mark, entity, true);
            hits.write(Hit {
                attacker: entity,
                target: mark,
                point: p + Vec3::Y * 0.3,
                direction,
                damage: DAMAGE,
                poise: POISE,
                knockback: KNOCKBACK,
                speed: 9.0,
                kind: HitKind::Grab,
            });
            info!("[grab] {entity} throws {mark}");
        }
    }
}

/// Let the mark go (its action back, unless a reaction already took it).
#[allow(clippy::type_complexity)]
fn release_mark(
    commands: &mut Commands,
    bodies: &mut Query<(&Transform, &mut Action, &mut Mover, &Fighter, &Melee, Option<&Rig>)>,
    mark: Entity,
    grabber: Entity,
    _thrown: bool,
) {
    if mark == grabber {
        return;
    }
    if let Ok((_, mut a, mut m, ..)) = bodies.get_mut(mark)
        && a.owner == Owner::Grab
    {
        a.clear();
        m.root_motion = Vec3::ZERO;
        m.move_scale = 1.0;
        m.face = None;
    }
    commands.entity(mark).remove::<Grabbed>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_nearest_enemy_in_front_within_reach() {
        let me = Vec3::ZERO;
        let e = |i: u32| Entity::from_raw_u32(i).unwrap();
        let candidates = [
            (e(1), Vec3::new(0.0, 0.0, 1.3)),  // ahead, in reach
            (e(2), Vec3::new(0.0, 0.0, 0.9)),  // ahead, nearer
            (e(3), Vec3::new(0.0, 0.0, -0.5)), // behind
            (e(4), Vec3::new(0.0, 0.0, 2.0)),  // too far
            (e(5), Vec3::new(1.0, 0.0, 0.2)),  // beside
        ];
        assert_eq!(pick(me, Vec3::Z, candidates.into_iter()), Some(e(2)));
        assert_eq!(pick(me, Vec3::NEG_Z, candidates.into_iter()), Some(e(3)));
        assert_eq!(pick(me, Vec3::Z, candidates[3..4].iter().copied()), None);
    }

    #[test]
    fn the_mark_is_held_at_arms_length_and_swung_round() {
        let g = Vec3::new(1.0, 1.0, 2.0);
        let start = hold_point(g, 0.0, CATCH);
        let end = hold_point(g, 0.0, THROW);
        assert!((start.distance(g) - HOLD_DIST).abs() < 1e-5 && (end.distance(g) - HOLD_DIST).abs() < 1e-5);
        assert!((start - g).normalize().dot(Vec3::Z) > 0.999, "caught in front");
        let turned = (start - g).normalize().angle_between((end - g).normalize());
        assert!((turned - SWING).abs() < 1e-3, "swung {turned} rad");
        assert!(CATCH < THROW && THROW < END && THROW - THROW_RELEASE > CATCH * 0.5);
    }
}
