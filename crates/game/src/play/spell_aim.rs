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
use super::magic::{Grimoire, Shape, ability};
use super::{Action, ActionKind, ActionLayer, Fighter, Mover, Owner, HALF_HEIGHT, Intent, Phys, PlaySet, Player};
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

fn hold(time: Res<Time>, mut commands: Commands, mut players: Query<(Entity, &mut Intent, Option<&mut Aiming>, &mut Action, &mut Mover, &Fighter, &Transform), With<Player>>) {
    let dt = time.delta_secs();
    for (entity, mut intent, aiming, mut action, mut mover, fighter, tf) in &mut players {
        match aiming {
            Some(mut aim) => {
                intent.art_pressed = false;
                intent.spell_pressed = false;
                aim.t += dt;
                if fighter.dead || action.owner != Owner::Magic || action.kind != ActionKind::Channel || intent.dodge_pressed || intent.secondary_pressed {
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
                    if let Some(layer) = action.layer.as_mut() { layer.time = aim.t; }
                    mover.face = super::controller::yaw_of(intent.look - tf.translation);
                    if (aim.t / 0.15) as u32 != ((aim.t - dt) / 0.15) as u32 {
                        use super::flipbook::{Flip, Sheet};
                        commands.write_message(Flip::new(Sheet::Casting, Vec3::ZERO, 0.7 + aim.t.min(1.5) * 0.5, 0.28)
                            .follow(entity, Vec3::Y * 0.4).tinted(Color::srgb(0.2, 0.7, 1.0), 1.0).glow(0.8));
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
                commands.entity(entity).try_insert(Aiming { which, t: 0.0 });
                let mut layer = ActionLayer::new("sparking/DBZ_Ki_Charge_Loop", 0.0, crate::motion::Mask::Full);
                layer.looping = true;
                layer.style = Some(crate::motion::timing::Style { exaggeration: 1.0, hold: 0.0, ..default() });
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
    mut gizmos: Gizmos,
    players: Query<(Entity, &Transform, &Intent, &Aiming, &Grimoire), With<Player>>,
    globals: Query<&GlobalTransform>,
) {
    for (me, tf, intent, aim, grimoire) in &players {
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
        let hand = tf.translation + Vec3::Y * HAND_LIFT;
        let point = intent.target.and_then(|t| globals.get(t).ok()).map_or(intent.look, |g| g.translation() + Vec3::Y * 0.2);
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
        app.insert_resource(Time::<()>::default()).add_systems(Update, hold);
        let p = app.world_mut().spawn((Player, Intent::default())).id();
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
