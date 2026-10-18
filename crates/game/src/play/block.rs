//! Blocking with the weapon itself: while a fighter guards, its weapon hand puts the blade in the
//! path of the blow that is coming.
//!
//! For every guarding fighter (`Melee::guarding`) holding a drawn weapon, `place` finds the most
//! pressing enemy swing in reach (`Melee::tell`, the soonest to land within `THREAT_RANGE`) and
//! the attacker's blade (its `WeaponSlot`). The interception point lies `GUARD_AHEAD` in front of
//! the defender's chest toward the attacker, at the height of the attacker's blade tip (clamped to
//! the body) and shifted toward the side the swing comes from. The weapon hand
//! (`HandTargets`) moves so that the defender's blade, at `CROSS_AT` of its length, sits on that
//! point; its weight rises as the blow nears (`READY` s before the active frames) and falls back
//! to the guard clip's pose afterwards.
//!
//! Ownership: the weapon hand's `HandTargets` while guarding, only when no other owner holds one
//! (`sheath` reaching, `interact`, `traverse`): `BlockHands.owns` remembers whose it is.
use super::combat::{BLADE_FROM, Melee, Weapon, WeaponSlot};
use super::moveset::Movesets;
use super::select::HandTargets;
use super::sheath::Sheath;
use super::{Fighter, Loadout, PlaySet};
use bevy::prelude::*;

/// Enemies farther than this (m, centre to centre) are no threat.
pub const THREAT_RANGE: f32 = 3.2;
/// The hand starts moving this long (s) before the blow's active frames.
pub const READY: f32 = 0.45;
/// Interception: distance ahead of the chest (m), height range above the centre (m), largest
/// sideways shift (m), and the share of the defender's blade that crosses there.
pub const GUARD_AHEAD: f32 = 0.42;
pub const HEIGHT: (f32, f32) = (-0.15, 0.75);
pub const SIDE_MAX: f32 = 0.35;
pub const CROSS_AT: f32 = 0.4;
/// Chest height above the capsule centre (m).
const CHEST: f32 = 0.4;
/// Weight easing rate (1/s).
const EASE: f32 = 10.0;

/// Per fighter: current hand weight and whether `block` inserted the `HandTargets`.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct BlockHands {
    w: f32,
    owns: bool,
    last: Vec3,
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Fighter, BlockHands>()
        .add_systems(Update, place.in_set(PlaySet::Act).after(super::sheath::SheathSet));
}

/// Where the defender's blade should cross the blow: `me` / `foe` are capsule centres, `tip` the
/// attacker's blade tip (world).
pub fn intercept(me: Vec3, foe: Vec3, tip: Vec3) -> Vec3 {
    let to = (foe - me).with_y(0.0);
    let ahead = to.normalize_or(Vec3::Z);
    let side = Vec3::Y.cross(ahead);
    let chest = me + Vec3::Y * CHEST;
    let height = (tip.y - me.y).clamp(HEIGHT.0, HEIGHT.1);
    let shift = (tip - me).dot(side).clamp(-SIDE_MAX, SIDE_MAX);
    (chest + ahead * GUARD_AHEAD + side * shift).with_y(me.y + height)
}

#[allow(clippy::type_complexity)]
fn place(
    time: Res<Time>,
    mut commands: Commands,
    movesets: Res<Movesets>,
    mut fighters: Query<(Entity, &Transform, &Fighter, &Melee, &Loadout, Option<&WeaponSlot>, &Sheath, &mut BlockHands, Option<&HandTargets>)>,
    globals: Query<&GlobalTransform>,
    held: Query<&ChildOf, With<Weapon>>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    // Every swing in progress: (attacker, team, centre, blade tip, seconds until it lands).
    let swings: Vec<(Entity, u8, Vec3, Vec3, f32)> = fighters
        .iter()
        .filter(|(_, _, f, ..)| !f.dead)
        .filter_map(|(e, tf, f, melee, _, slot, ..)| {
            let tell = melee.tell()?;
            let slot = slot?;
            let g = globals.get(slot.weapon).ok()?;
            let tip = g.transform_point(Vec3::Y * (BLADE_FROM + slot.length));
            Some((e, f.team, tf.translation, tip, tell.until_active))
        })
        .collect();

    for (entity, tf, fighter, melee, loadout, slot, sheath, mut hands, targets) in &mut fighters {
        let me = tf.translation;
        let def = movesets.get(loadout.equipped()).and_then(|m| m.weapon.as_ref());
        let threat = (melee.guarding() && !fighter.dead && sheath.in_hand() && slot.is_some() && def.is_some())
            .then(|| {
                swings
                    .iter()
                    .filter(|s| s.0 != entity && s.1 != fighter.team && (s.2 - me).with_y(0.0).length() < THREAT_RANGE)
                    .min_by(|a, b| a.4.total_cmp(&b.4))
            })
            .flatten();
        let goal = threat.map_or(0.0, |s| if s.4.is_finite() { (1.0 - s.4 / READY).clamp(0.0, 1.0).max(if s.4 <= 0.0 { 1.0 } else { 0.0 }) } else { 0.0 });
        hands.w += (goal - hands.w) * (EASE * dt).min(1.0);

        let busy = targets.is_some() && !hands.owns;
        if hands.w < 0.01 || busy {
            if hands.owns {
                commands.entity(entity).remove::<HandTargets>();
                hands.owns = false;
            }
            continue;
        }
        let (Some(slot), Some(def)) = (slot, def) else { continue };
        let Ok(weapon) = globals.get(slot.weapon) else { continue };
        let Some(hand) = held.get(slot.weapon).ok().and_then(|c| globals.get(c.parent()).ok()) else { continue };
        if let Some(s) = threat {
            // Move the hand by however far the crossing point is from where it should be.
            let cross = weapon.transform_point(Vec3::Y * (BLADE_FROM + slot.length * CROSS_AT));
            hands.last = hand.translation() + (intercept(me, s.2, s.3) - cross);
        }
        let side = usize::from(def.hand != "l");
        let mut out = [None, None];
        out[side] = Some((hands.last, hands.w));
        commands.entity(entity).insert(HandTargets { hands: out, ..default() });
        hands.owns = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blade_meets_the_blow_between_the_fighters_on_its_side() {
        let me = Vec3::new(0.0, 1.0, 0.0);
        let foe = Vec3::new(0.0, 1.0, 2.0);
        // A swing coming in high from the defender's left (+X when facing +Z).
        let p = intercept(me, foe, Vec3::new(0.6, 2.2, 1.2));
        assert!((p.z - GUARD_AHEAD).abs() < 1e-4, "ahead toward the foe: {p}");
        assert!((p.x - SIDE_MAX).abs() < 1e-4, "on the blow's side, clamped: {p}");
        assert!((p.y - (me.y + HEIGHT.1)).abs() < 1e-4, "no higher than the head: {p}");
        // A low sweep from the right.
        let q = intercept(me, foe, Vec3::new(-0.2, 0.6, 1.0));
        assert!(q.x < 0.0 && q.y < me.y, "low and to the right: {q}");
    }

    #[test]
    fn a_block_step_walks_exactly_its_length() {
        use super::super::combat::{BLOCK_STEP_MAX, BLOCK_STEP_TIME, step_length, step_speed};
        let length = step_length(3.0);
        assert!(length > 0.3 && length <= BLOCK_STEP_MAX);
        assert_eq!(step_length(1000.0), BLOCK_STEP_MAX);
        let n = 600;
        let dt = BLOCK_STEP_TIME / n as f32;
        let walked: f32 = (0..n).map(|i| step_speed((i as f32 + 0.5) * dt, length) * dt).sum();
        assert!((walked - length).abs() < 1e-3, "walked {walked} of {length}");
        assert_eq!(step_speed(BLOCK_STEP_TIME, length), 0.0);
    }
}
