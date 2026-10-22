//! Special hand-to-hand moves, scripted like `grab`:
//!
//! - **Vault slam** (`Vault`): grab (G) while sprinting at a foe up to `VAULT_RANGE` m ahead. The fighter leaps
//!   over the foe (`LEAP_TRACK`, an upward kick of `VAULT_LIFT` m/s and root motion carrying it to a spot
//!   `BEHIND` m past the foe over `VAULT_AIR` s), seizes the foe's head at the top of the arc (hands on the head
//!   bone; the foe is held, `grab::Grabbed`), and on landing slams it down: an unblockable `HitKind::Grab` of
//!   `SLAM_DAMAGE` with knockdown poise, a shockwave and a camera kick.
//! - **Launcher → flash step → dive** (`Aerial`): a heavy that lands with full charge (`Melee::since_primed`)
//!   launches the foe (`LAUNCH` m/s up). After `FLASH_DELAY` s the attacker flash-steps above it (teleport to
//!   `ABOVE` m over the foe, aura bursts where it left and where it arrives, a white impact frame), then dives
//!   (`DIVE` m/s down, root motion onto the foe, `DIVE_TRACK`): on contact a `SPIKE_DAMAGE` knockdown blow
//!   with a shockwave, slow-motion beat and camera kick.
//! - `GB_MOVE=vault@<s>` / `GB_MOVE=launch@<s>` starts one from the player on the nearest foe at that time
//!   (smoke runs and recordings).
//!
//! Ownership: the mover's `Action` (`Owner::Grab`), `root_motion`, `face` and `external` during the move, and
//! the held foe's `Action` while vaulted. Monitor log lines: `[moves] vault`, `[moves] slam`, `[moves]
//! launch`, `[moves] flash`, `[moves] spike`.
use super::camera::CameraShake;
use super::clash::ImpactFrame;
use super::combat::{Hitstop, Melee};
use super::controller::{Teleport, yaw_of};
use super::grab::Grabbed;
use super::select::HandTargets;
use super::vfx::{self, Element, Vfx};
use super::{Action, ActionKind, ActionLayer, Fighter, HALF_HEIGHT, Hit, HitKind, Intent, MoveMode, Mover, Owner, PlaySet, Player};
use crate::motion::bank::PoseBank;
use crate::motion::{Mask, Rig};
use bevy::prelude::*;

pub const VAULT_RANGE: f32 = 4.0;
const VAULT_CONE: f32 = 0.6;
pub const VAULT_AIR: f32 = 0.55;
const VAULT_LIFT: f32 = 7.0;
const BEHIND: f32 = 1.0;
/// Fraction of the leap at which the head is seized.
const SEIZE_AT: f32 = 0.4;
const VAULT_END: f32 = 0.95;
pub const SLAM_DAMAGE: f32 = 24.0;
const SLAM_POISE: f32 = 999.0;
const SLAM_KNOCKBACK: f32 = 3.0;
const LEAP_TRACK: &str = "ual2/NinjaJump_Start";
const HELD_TRACK: &str = "ual2/Hit_Knockback";

pub const LAUNCH: f32 = 9.0;
pub const FLASH_DELAY: f32 = 0.32;
const ABOVE: f32 = 1.7;
const DIVE: f32 = 16.0;
const DIVE_TIMEOUT: f32 = 0.7;
pub const SPIKE_DAMAGE: f32 = 22.0;
const SPIKE_POISE: f32 = 999.0;
/// The dive pose of the generated keyframe sheet: tuck and turn (0.34 s in), head-first dive, fist driving down at 0.66 s.
const DIVE_TRACK: &str = "kev/aerial_dive_attack";
const DIVE_FROM: f32 = 0.34;

#[derive(Component, Clone, Debug)]
pub struct Vault {
    pub mark: Entity,
    t: f32,
    from: Vec3,
    to: Vec3,
    seized: bool,
    slammed: bool,
}

#[derive(Component, Clone, Debug)]
pub struct Aerial {
    pub target: Entity,
    t: f32,
    flashed: bool,
    struck: bool,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (smoke, start_vault, run_vault, launch, run_aerial).chain().in_set(PlaySet::Act).before(super::combat::CombatSet));
}

/// Where a vault lands: `BEHIND` m past the mark along the line from `me`.
pub fn landing(me: Vec3, mark: Vec3) -> Vec3 {
    let dir = (mark - me).with_y(0.0).normalize_or(Vec3::Z);
    mark.with_y(me.y) + dir * BEHIND
}

/// The best vault mark: the nearest foe within `VAULT_RANGE` inside the front cone.
pub fn vault_mark(me: Vec3, facing: Vec3, candidates: impl Iterator<Item = (Entity, Vec3)>) -> Option<Entity> {
    candidates
        .filter_map(|(e, p)| {
            let to = (p - me).with_y(0.0);
            let d = to.length();
            (d < VAULT_RANGE && d > 0.6 && facing.dot(to / d) > VAULT_CONE).then_some((e, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e)
}

fn begin_vault(commands: &mut Commands, me: Entity, mark: Entity, from: Vec3, to_pos: Vec3, action: &mut Action, mover: &mut Mover) {
    action.start(ActionKind::Grab, Owner::Grab, Some(ActionLayer::new(LEAP_TRACK, 0.0, Mask::Full)));
    mover.move_scale = 0.0;
    mover.external.y = VAULT_LIFT - mover.velocity.y;
    commands.entity(me).insert(Vault { mark, t: 0.0, from, to: landing(from, to_pos), seized: false, slammed: false });
    info!("[moves] vault {me} over {mark}");
}

#[allow(clippy::type_complexity)]
fn start_vault(mut commands: Commands, mut fighters: Query<(Entity, &Transform, &Intent, &mut Action, &mut Mover, &Fighter, &Melee), (Without<Vault>, Without<Aerial>)>) {
    let snapshot: Vec<(Entity, Vec3, u8, bool)> = fighters.iter().map(|(e, tf, _, _, _, f, m)| (e, tf.translation, f.team, !f.dead && m.grabbable())).collect();
    for (e, tf, intent, mut action, mut mover, f, _) in &mut fighters {
        // A sprinting grab vaults; a standing one stays the command grab (`grab`).
        if !intent.grab_pressed || !mover.sprinting || !action.is_free() || mover.mode != MoveMode::Ground || f.dead {
            continue;
        }
        let me = tf.translation;
        let Some(mark) = vault_mark(me, mover.forward(), snapshot.iter().filter(|s| s.0 != e && s.2 != f.team && s.3).map(|s| (s.0, s.1))) else { continue };
        let at = snapshot.iter().find(|s| s.0 == mark).map_or(me, |s| s.1);
        begin_vault(&mut commands, e, mark, me, at, &mut action, &mut mover);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn run_vault(
    time: Res<Time>,
    mut commands: Commands,
    bank: Option<Res<PoseBank>>,
    vfx: Option<Res<Vfx>>,
    mut shake: ResMut<CameraShake>,
    mut hits: MessageWriter<Hit>,
    mut vaults: Query<(Entity, &mut Vault)>,
    mut bodies: Query<(&Transform, &mut Action, &mut Mover, &Fighter, Option<&Rig>), Without<Vault>>,
    mut me_q: Query<(&Transform, &mut Action, &mut Mover), With<Vault>>,
    globals: Query<&GlobalTransform>,
) {
    let Some(bank) = bank else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (e, mut v) in &mut vaults {
        v.t += dt;
        let Ok((tf, mut action, mut mover)) = me_q.get_mut(e) else { continue };
        let mine = action.owner == Owner::Grab;
        let u = (v.t / VAULT_AIR).min(1.0);
        // The leap: carried over the mark to the landing spot.
        if mine && u < 1.0 {
            mover.root_motion = (v.to - v.from).with_y(0.0) / VAULT_AIR;
            mover.face = yaw_of(v.to - tf.translation).or(mover.face);
            action.elapsed = v.t;
        } else {
            mover.root_motion = Vec3::ZERO;
        }
        let mark = bodies.get_mut(v.mark).ok();
        // Seize the head at the top of the arc.
        if !v.seized && u >= SEIZE_AT {
            v.seized = true;
            if let Some((_, mut a, mut m, f, _)) = mark
                && !f.dead
            {
                a.start(ActionKind::Grab, Owner::Grab, Some(ActionLayer::new(HELD_TRACK, 0.0, Mask::Full)));
                m.move_scale = 0.0;
                commands.entity(v.mark).insert(Grabbed { by: e });
            }
        }
        // Hands on the head while holding.
        let head = bodies.get(v.mark).ok().and_then(|(_, _, _, _, rig)| rig).and_then(|rig| rig.bones.get(bank.bones.head)).and_then(|b| globals.get(*b).ok()).map(|g| g.translation());
        match head {
            Some(h) if v.seized && !v.slammed => {
                commands.entity(e).insert(HandTargets { hands: [Some((h, 1.0)), Some((h, 1.0))], grips: [None, None] });
            }
            _ => {
                commands.entity(e).remove::<HandTargets>();
            }
        }
        // Landing: slam the mark down.
        if !v.slammed && u >= 1.0 && mover.mode == MoveMode::Ground {
            v.slammed = true;
            let mark_pos = bodies.get(v.mark).map(|b| b.0.translation).unwrap_or(v.to);
            release(&mut commands, &mut bodies, v.mark);
            hits.write(Hit {
                attacker: e,
                target: v.mark,
                point: mark_pos + Vec3::Y * 1.3,
                direction: (Vec3::NEG_Y * 0.8 + (mark_pos - tf.translation).with_y(0.0).normalize_or(Vec3::Z) * 0.6).normalize(),
                damage: SLAM_DAMAGE,
                poise: SLAM_POISE,
                knockback: SLAM_KNOCKBACK,
                speed: 12.0,
                kind: HitKind::Grab,
            });
            if let Some(vfx) = &vfx {
                vfx::burst(&mut commands, vfx.shock(), mark_pos - Vec3::Y * HALF_HEIGHT, 2.2);
            }
            commands.write_message(super::fracture::Shatter { at: mark_pos - Vec3::Y * HALF_HEIGHT, radius: 3.5, force: 60.0 });
            shake.add(0.5);
            info!("[moves] slam {e} slams {}", v.mark);
        }
        if v.t >= VAULT_END || !mine {
            if mine {
                action.clear();
                mover.move_scale = 1.0;
                mover.face = None;
                mover.root_motion = Vec3::ZERO;
            }
            if !v.slammed {
                release(&mut commands, &mut bodies, v.mark);
            }
            commands.entity(e).remove::<(Vault, HandTargets)>();
        }
    }
}

#[allow(clippy::type_complexity)]
fn release(commands: &mut Commands, bodies: &mut Query<(&Transform, &mut Action, &mut Mover, &Fighter, Option<&Rig>), Without<Vault>>, mark: Entity) {
    if let Ok((_, mut a, mut m, ..)) = bodies.get_mut(mark)
        && a.owner == Owner::Grab
    {
        a.clear();
        m.move_scale = 1.0;
        m.face = None;
        m.root_motion = Vec3::ZERO;
    }
    commands.entity(mark).remove::<Grabbed>();
}

/// A full-charge heavy that lands launches its target, and the attacker follows up.
fn launch(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    attackers: Query<&Melee, (Without<Aerial>, Without<Vault>)>,
    mut movers: Query<&mut Mover>,
) {
    for h in hits.read() {
        if h.kind != HitKind::Melee || h.damage <= 0.0 || h.attacker == h.target {
            continue;
        }
        let Ok(m) = attackers.get(h.attacker) else { continue };
        if m.since_primed() > 0.6 {
            continue;
        }
        if let Ok(mut tm) = movers.get_mut(h.target) {
            tm.external.y = LAUNCH - tm.velocity.y.min(0.0);
        }
        commands.entity(h.attacker).try_insert(Aerial { target: h.target, t: 0.0, flashed: false, struck: false });
        info!("[moves] launch {} launches {}", h.attacker, h.target);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn run_aerial(
    time: Res<Time>,
    mut commands: Commands,
    vfx: Option<Res<Vfx>>,
    mut shake: ResMut<CameraShake>,
    mut hitstop: ResMut<Hitstop>,
    mut frame: ResMut<ImpactFrame>,
    mut hits: MessageWriter<Hit>,
    mut aerials: Query<(Entity, &mut Aerial, &Transform, &mut Action, &mut Mover)>,
    targets: Query<(&Transform, &Fighter), Without<Aerial>>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (e, mut a, tf, mut action, mut mover) in &mut aerials {
        a.t += dt;
        let Ok((ttf, tf_f)) = targets.get(a.target) else {
            commands.entity(e).remove::<Aerial>();
            continue;
        };
        let foe = ttf.translation;
        // Flash step: appear above the launched foe.
        if !a.flashed && a.t >= FLASH_DELAY {
            a.flashed = true;
            let from = tf.translation;
            let feet = foe - Vec3::Y * HALF_HEIGHT + Vec3::Y * ABOVE;
            let yaw = yaw_of(foe - from).unwrap_or(mover.yaw);
            commands.entity(e).insert(Teleport { feet, yaw });
            action.start(ActionKind::Attack, Owner::Grab, Some(ActionLayer::new(DIVE_TRACK, DIVE_FROM, Mask::Full)));
            mover.move_scale = 0.0;
            if let Some(vfx) = &vfx {
                vfx::burst(&mut commands, vfx.burst(Element::Aura), from, 1.3);
                vfx::burst(&mut commands, vfx.burst(Element::Lightning), feet + Vec3::Y * HALF_HEIGHT, 1.3);
            }
            frame.trigger();
            info!("[moves] flash {e} above {}", a.target);
        }
        // The dive onto the foe.
        if a.flashed && !a.struck {
            mover.external.y = -DIVE - mover.velocity.y.max(-DIVE);
            mover.root_motion = ((foe - tf.translation).with_y(0.0) / 0.15).clamp_length_max(10.0);
            action.elapsed = a.t;
            if let Some(layer) = action.layer.as_mut() {
                layer.time = DIVE_FROM + (a.t - FLASH_DELAY);
            }
            let close = tf.translation.distance(foe) < 1.3 || (tf.translation.y < foe.y + 0.4 && (tf.translation - foe).with_y(0.0).length() < 1.5);
            if close && !tf_f.dead {
                a.struck = true;
                hits.write(Hit {
                    attacker: e,
                    target: a.target,
                    point: foe + Vec3::Y * 0.6,
                    direction: (Vec3::NEG_Y * 0.9 + (foe - tf.translation).with_y(0.0).normalize_or(Vec3::Z) * 0.3).normalize(),
                    damage: SPIKE_DAMAGE,
                    poise: SPIKE_POISE,
                    knockback: 4.0,
                    speed: 16.0,
                    kind: HitKind::Melee,
                });
                if let Some(vfx) = &vfx {
                    vfx::burst(&mut commands, vfx.shock(), foe - Vec3::Y * HALF_HEIGHT, 2.6);
                    vfx::burst(&mut commands, vfx.burst(Element::Fire), foe, 1.2);
                }
                commands.write_message(super::fracture::Shatter { at: foe - Vec3::Y * HALF_HEIGHT, radius: 4.0, force: 70.0 });
                hitstop.trigger(0.3);
                shake.add(0.6);
                info!("[moves] spike {e} spikes {}", a.target);
            }
        }
        if a.struck || a.t >= FLASH_DELAY + DIVE_TIMEOUT {
            if action.owner == Owner::Grab {
                action.clear();
            }
            mover.move_scale = 1.0;
            mover.root_motion = Vec3::ZERO;
            mover.face = None;
            commands.entity(e).remove::<Aerial>();
        }
    }
}

/// `GB_MOVE=vault@<s>` / `GB_MOVE=launch@<s>`: the player does that move on the nearest foe at that time.
#[allow(clippy::type_complexity)]
fn smoke(
    time: Res<Time>,
    mut done: Local<bool>,
    mut commands: Commands,
    mut players: Query<(Entity, &Transform, &mut Action, &mut Mover, &Fighter), (With<Player>, Without<Vault>, Without<Aerial>)>,
    mut others: Query<(Entity, &Transform, &Fighter, &mut Mover), Without<Player>>,
) {
    if *done {
        return;
    }
    let Some((kind, at)) = std::env::var("GB_MOVE").ok().and_then(|v| {
        let (k, t) = v.split_once('@')?;
        Some((k.trim().to_string(), t.trim().parse::<f32>().ok()?))
    }) else {
        *done = true;
        return;
    };
    if time.elapsed_secs() < at {
        return;
    }
    let Ok((me, tf, mut action, mut mover, f)) = players.single_mut() else { return };
    let foe = others
        .iter()
        .filter(|(_, _, o, _)| !o.dead && o.team != f.team)
        .min_by(|a, b| a.1.translation.distance(tf.translation).total_cmp(&b.1.translation.distance(tf.translation)))
        .map(|(e, t, ..)| (e, t.translation));
    let Some((foe, pos)) = foe else { return };
    *done = true;
    match kind.as_str() {
        "vault" => begin_vault(&mut commands, me, foe, tf.translation, pos, &mut action, &mut mover),
        "launch" => {
            if let Ok((_, _, _, mut fm)) = others.get_mut(foe) {
                fm.external.y = LAUNCH;
            }
            commands.entity(me).insert(Aerial { target: foe, t: 0.0, flashed: false, struck: false });
            info!("[moves] launch {me} launches {foe} (smoke)");
        }
        _ => warn!("[moves] GB_MOVE: unknown move '{kind}'"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(i: u32) -> Entity {
        Entity::from_raw_u32(i).unwrap()
    }

    #[test]
    fn a_vault_lands_behind_the_mark() {
        let me = Vec3::new(0.0, 1.0, 0.0);
        let mark = Vec3::new(0.0, 1.0, 3.0);
        let to = landing(me, mark);
        assert!((to - Vec3::new(0.0, 1.0, 3.0 + BEHIND)).length() < 1e-5, "{to}");
    }

    #[test]
    fn a_vault_picks_the_nearest_foe_ahead_in_range() {
        let me = Vec3::ZERO;
        let fwd = Vec3::Z;
        let c = [(e(1), Vec3::new(0.0, 0.0, 3.5)), (e(2), Vec3::new(0.0, 0.0, 2.0)), (e(3), Vec3::new(0.0, 0.0, -2.0)), (e(4), Vec3::new(0.0, 0.0, 6.0))];
        assert_eq!(vault_mark(me, fwd, c.into_iter()), Some(e(2)));
        assert_eq!(vault_mark(me, fwd, [(e(3), Vec3::new(0.0, 0.0, -2.0))].into_iter()), None, "behind");
        assert_eq!(vault_mark(me, fwd, [(e(4), Vec3::new(0.0, 0.0, 6.0))].into_iter()), None, "too far");
    }
}
