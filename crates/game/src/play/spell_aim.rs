//! Spell aiming (E13): hold Q (the art) or E (the spell) to aim, let go to cast.
//! - A quick tap still casts: the press goes through at once when the key is already up.
//! - Held: the cast waits (`Aiming`), and after `PREVIEW_AFTER` s `preview` draws where it will go,
//!   by the ability's `magic::Shape`: a bolt's flight arc and its landing (a frost bloom's
//!   reach), a crescent's path, a quake's ring round the caster, a lance's reach, a step's
//!   landing, the orbs' and wards' circles. The crosshair opens its ticks (`ranged::crosshair`).
//! - Let go: the press reaches `magic` (`Intent::art_pressed` / `spell_pressed`) and it casts at
//!   the crosshair, as always. Dodging or the right mouse button while aiming cancels.
//!
//! Only the player aims (brains press and cast at once). Runs in `PlaySet::Think`, between
//! `input` and `magic`: it holds the pressed flags back while aiming.
//! Ownership: `Aiming` (written here only); it rewrites only the player's two cast presses.
use super::magic::{Energy, Grimoire, Recovery, Shape, ability, affordable_charge, MASTERED_COST};
use super::hud::log::{HudLog, LogKind};
use super::{Action, ActionKind, Fighter, Mover, Owner, HALF_HEIGHT, Intent, Phys, PlaySet, Player};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

/// Seconds held before the preview shows (a tap never flashes one).
const PREVIEW_AFTER: f32 = 0.15;
/// Flight preview: time step and longest flight drawn (s).
const FLIGHT_STEP: f32 = 0.04;
const FLIGHT_MAX: f32 = 2.0;
/// The hand's height above the body's centre when nothing better is known (m).
const HAND_LIFT: f32 = 0.5;
/// How far a crescent is drawn (s of flight).
const CRESCENT_SHOW: f32 = 0.8;
/// Ring radius for a landing spot (m).
const SPOT: f32 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Art,
    Spell,
}

/// The player is holding a cast key: the cast waits for the release.
#[derive(Component, Clone, Copy, Debug)]
pub struct Aiming {
    pub which: Which,
    /// Seconds held.
    pub t: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (hold, preview).chain().in_set(PlaySet::Think));
}

fn held(intent: &Intent, which: Which) -> bool {
    match which {
        Which::Art => intent.art_held,
        Which::Spell => intent.spell_held,
    }
}

fn press(intent: &mut Intent, which: Which) {
    match which {
        Which::Art => intent.art_pressed = true,
        Which::Spell => intent.spell_pressed = true,
    }
}

#[allow(clippy::type_complexity)]
fn hold(time: Res<Time>, mut commands: Commands, mut log: MessageWriter<HudLog>, mut players: Query<(Entity, &mut Intent, Option<&mut Aiming>, &mut Action, &mut Mover, &Fighter, &Transform, &Energy, &Grimoire, &Recovery, Option<&crate::motion::Rig>), With<Player>>, bank: Option<Res<crate::motion::bank::PoseBank>>) {
    let dt = time.delta_secs();
    for (entity, mut intent, aiming, mut action, mut mover, fighter, tf, energy, grimoire, recovery, rig) in &mut players {
        match aiming {
            Some(mut aim) => {
                intent.art_pressed = false;
                intent.spell_pressed = false;
                aim.t += dt;
                if fighter.dead || action.owner != Owner::Magic || action.kind != ActionKind::Channel || intent.dodge_pressed || intent.secondary_pressed || intent.jump_pressed {
                    commands.entity(entity).try_remove::<Aiming>();
                    if action.owner == Owner::Magic && action.kind == ActionKind::Channel {
                        action.clear(); mover.move_scale = 1.0; mover.face = None;
                    }
                } else if !held(&intent, aim.which) {
                    commands.entity(entity).try_remove::<Aiming>();
                    intent.cast_charge = (aim.t / 1.5).clamp(0.0, 1.0);
                    action.clear(); mover.move_scale = 1.0; mover.face = None;
                    press(&mut intent, aim.which);
                } else {
                    let id = if aim.which == Which::Art { grimoire.art } else { grimoire.spell };
                    if let Some(a) = ability(id) { action.layer = Some(super::magic_fx::channel(a, aim.t)); }
                    mover.face = super::controller::yaw_of(intent.look - tf.translation);
                    if (aim.t / 0.15) as u32 != ((aim.t - dt) / 0.15) as u32 {
                        use super::flipbook::{Flip, Sheet};
                        let id = if aim.which == Which::Art { grimoire.art } else { grimoire.spell };
                        if let Some(a) = ability(id) {
                            let charge = affordable_charge(a, energy, grimoire.is_mastered(id), aim.t / 1.5);
                            let tint = super::magic_fx::charge_color(a, charge);
                            let palm = |side| rig.zip(bank.as_ref()).and_then(|(r, b)|
                                r.bones.get(b.bones.hand[side])).copied().filter(|e| *e != Entity::PLACEHOLDER);
                            let focus = Flip::new(Sheet::Magic8, Vec3::ZERO, 0.18 + charge * 0.14, 0.2)
                                .tinted(tint, 1.0).glow(0.45);
                            let focus = match (palm(0), palm(1)) {
                                (Some(left), Some(right)) if super::magic_fx::two_handed(a) => focus.between(left, right),
                                (_, Some(right)) => focus.follow(right, Vec3::ZERO),
                                _ => focus.follow(entity, tf.rotation * Vec3::new(0.0, 0.5, 0.35)),
                            };
                            commands.write_message(focus);
                            commands.write_message(Flip::new(Sheet::Ward, Vec3::ZERO, 1.5 + charge * 0.5, 0.25)
                                .flat().follow(entity, Vec3::Y * (-HALF_HEIGHT + 0.04)).tinted(tint, 1.0).glow(0.3));
                            if (aim.t / 0.5) as u32 != ((aim.t - dt) / 0.5) as u32 {
                                log.write(HudLog::new(LogKind::Info, format!("{}: {:.0}% | release to cast | guard/dodge/jump cancels", a.name, charge * 100.0)).key(7003));
                            }
                        }
                    }
                }
            }
            None => {
                if fighter.dead || !action.is_free() { continue; }
                let which = if intent.art_pressed && intent.art_held {
                    Which::Art
                } else if intent.spell_pressed && intent.spell_held {
                    Which::Spell
                } else {
                    continue;
                };
                intent.art_pressed = false;
                intent.spell_pressed = false;
                let id = if which == Which::Art { grimoire.art } else { grimoire.spell };
                let Some(a) = ability(id) else { continue };
                let available = if a.is_art() { energy.battlewill } else { energy.magicules };
                let cost = a.cost * if grimoire.is_mastered(id) { MASTERED_COST } else { 1.0 };
                if available < cost || recovery.remaining(a.pool) > 0.0 {
                    log.write(HudLog::new(LogKind::Warn, format!("{}: need {:.0} energy and a ready slot", a.name, cost)).key(7002));
                    continue;
                }
                commands.entity(entity).try_insert(Aiming { which, t: 0.0 });
                let layer = super::magic_fx::channel(a, 0.0);
                action.start(ActionKind::Channel, Owner::Magic, Some(layer));
                mover.move_scale = 0.15;
            }
        }
    }
}

/// A circle lying on the ground at `at`.
fn ground_ring(gizmos: &mut Gizmos, at: Vec3, radius: f32, color: Color) {
    gizmos.circle(Isometry3d::new(at + Vec3::Y * 0.04, Quat::from_rotation_x(FRAC_PI_2)), radius, color);
}

/// The ground under `p` (or `p` itself when nothing is below within reach).
fn ground(spatial: &SpatialQuery, p: Vec3) -> Vec3 {
    let filter = SpatialQueryFilter::from_mask(Phys::World);
    let from = p + Vec3::Y * 0.5;
    spatial.cast_ray(from, Dir3::NEG_Y, 30.0, true, &filter).map_or(p, |h| from - Vec3::Y * h.distance)
}

#[allow(clippy::type_complexity)]
fn preview(
    spatial: SpatialQuery,
    bank: Option<Res<crate::motion::bank::PoseBank>>,
    globals: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
    players: Query<(Entity, &Transform, &Intent, &Aiming, &Grimoire, Option<&crate::motion::Rig>), With<Player>>,
) {
    for (me, tf, intent, aim, grimoire, rig) in &players {
        if aim.t < PREVIEW_AFTER {
            continue;
        }
        let id = match aim.which {
            Which::Art => grimoire.art,
            Which::Spell => grimoire.spell,
        };
        let Some(a) = ability(id) else { continue };
        let core = a.element.core();
        let peak = core.red.max(core.green).max(core.blue).max(1e-3);
        let color = Color::LinearRgba(LinearRgba::new(core.red / peak, core.green / peak, core.blue / peak, 0.9));
        let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
        let hand = bank.as_ref().map_or(tf.translation + Vec3::Y * HAND_LIFT,
            |b| super::magic::cast_origin(a, rig, b, &globals, tf));
        // Player casts are skill shots; the preview must use the same obstruction-aware
        // crosshair point as release, even when melee lock-on is active.
        let point = intent.look;
        let flat = (point - feet).with_y(0.0).normalize_or(tf.forward().as_vec3());
        match a.shape {
            Shape::Bolt { speed, gravity, bloom, .. } => {
                let dir = super::aim::ballistic(hand, point, speed, gravity);
                let filter = SpatialQueryFilter::from_mask([Phys::World, Phys::Character]).with_excluded_entities([me]);
                let mut points = vec![hand];
                let mut at = hand;
                let mut t = 0.0;
                while t < FLIGHT_MAX {
                    t += FLIGHT_STEP;
                    let next = hand + dir * speed * t - Vec3::Y * 0.5 * gravity * t * t;
                    let step = next - at;
                    if let Ok(d) = Dir3::new(step)
                        && let Some(hit) = spatial.cast_ray(at, d, step.length(), true, &filter)
                    {
                        at += d * hit.distance;
                        points.push(at);
                        break;
                    }
                    at = next;
                    points.push(at);
                }
                gizmos.linestrip(points, color);
                ground_ring(&mut gizmos, ground(&spatial, at), if bloom > 0.0 { bloom } else { SPOT }, color);
            }
            Shape::Crescent { speed, radius } => {
                let end = feet + flat * speed * CRESCENT_SHOW;
                gizmos.line(feet + Vec3::Y * 0.3, end + Vec3::Y * 0.3, color);
                ground_ring(&mut gizmos, ground(&spatial, end), radius, color);
            }
            Shape::Quake { radius } => {
                ground_ring(&mut gizmos, feet, radius, color);
                ground_ring(&mut gizmos, feet, radius * 0.5, color);
            }
            Shape::Lance { range, .. } => {
                let to = point - hand;
                let end = hand + to.clamp_length_max(range);
                gizmos.line(hand, end, color);
                ground_ring(&mut gizmos, ground(&spatial, end), SPOT * 1.5, color);
            }
            Shape::Step { distance } => {
                let end = ground(&spatial, feet + flat * distance);
                gizmos.line(feet + Vec3::Y * 0.1, end + Vec3::Y * 0.1, color);
                ground_ring(&mut gizmos, end, SPOT, color);
            }
            Shape::Orbs { .. } => {
                gizmos.circle(Isometry3d::new(tf.translation, Quat::from_rotation_x(FRAC_PI_2)), 1.2, color);
            }
            Shape::Ward { .. } | Shape::Veil { .. } => {
                // A shell round the caster: three rings at the hips, chest and head.
                for (lift, r) in [(-0.4, 0.75), (0.2, 0.85), (0.8, 0.6)] {
                    gizmos.circle(Isometry3d::new(tf.translation + Vec3::Y * lift, Quat::from_rotation_x(FRAC_PI_2)), r, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tap_casts_at_once_and_a_hold_waits_for_the_release() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_message::<HudLog>()
            .add_message::<super::super::flipbook::Flip>()
            .add_systems(Update, hold);
        let p = app.world_mut().spawn((Player, Intent::default(), Action::default(), Mover::default(),
            Fighter::new(0, 100.0), Transform::default(), Energy::default(), Grimoire::default(), Recovery::default())).id();
        // Tap: pressed with the key already up.
        app.world_mut().get_mut::<Intent>(p).unwrap().spell_pressed = true;
        app.update();
        assert!(app.world().get::<Intent>(p).unwrap().spell_pressed, "a tap goes straight through");
        assert!(app.world().get::<Aiming>(p).is_none());
        // Hold: pressed and held.
        {
            let mut i = app.world_mut().get_mut::<Intent>(p).unwrap();
            i.spell_pressed = true;
            i.spell_held = true;
        }
        app.update();
        assert!(!app.world().get::<Intent>(p).unwrap().spell_pressed, "held: the cast waits");
        assert_eq!(app.world().get::<Aiming>(p).map(|a| a.which), Some(Which::Spell));
        // Release.
        app.world_mut().get_mut::<Intent>(p).unwrap().spell_held = false;
        app.update();
        assert!(app.world().get::<Intent>(p).unwrap().spell_pressed, "let go: it casts");
        assert!(app.world().get::<Aiming>(p).is_none());
    }
}
