//! Balance beams (`Update`): walking a narrow `Beam` is a balancing act.
//! - Mounting: a grounded character (player or NPC) whose soles stand on a beam's top (within `CATCH` of its line)
//!   starts `Balancing`; leaving it (stepping off an end, jumping, falling) ends it. Landing on
//!   a beam from a jump kicks the sway by the landing impact.
//! - Steering (`steer`, after `PlaySet::Input`): the stick's component along the beam walks
//!   (at most `WALK_SCALE` of a walk, never a sprint); its sideways component is the balance
//!   input, not a step, and a gentle pull keeps the body over the line. NPCs (Sena's `scout`
//!   routine walks it both ways) counter on their own with softer gains (`NPC_GAIN`), so a strong
//!   drift sometimes tips them off.
//! - Sway: `tilt` (≈ radians, + toward the beam's `side`) is an inverted pendulum,
//!   tilt'' = `TIP`·tilt + `COUNTER`·input + drift − `DAMP`·tilt', where the drift is a smooth
//!   random push that grows with walking speed and sideways acceleration and shrinks with the
//!   Balance skill (`assist`). Push the stick against the lean to stay up.
//! - Past `WOBBLE` the arms windmill; past `FALL` the walker topples off the beam's side
//!   (`Mover.external`), with a log line, and cannot remount for `REMOUNT` s.
//! - Posture: `select::Posture` (written here only) leans the torso by the tilt and narrows the
//!   stance to the beam; both arms reach out sideways through `HandTargets` (only when nothing
//!   else holds the hands), tilting against the lean.
//! - Deeds: a `balance` per metre walked on a beam, a `beam` (tag "crossed") for walking one end
//!   to end without falling, a `beam` (tag "fell") for a fall.
//! - Beams: the yard beam (`hall`) and the tightrope over the pond (`spawn_tightrope`, sways
//!   `ROPE_SWAY` times harder; a fall lands in the water).
//! - Smoke: `GB_BEAM=steady|drift` walks the player along the beam it starts on (`GB_SPAWN`),
//!   countering the sway (steady) or not at all (drift). Logs `[beam] on|fell|crossed|off`.
use super::deeds::Deed;
use super::hud::log::{HudLog, LogKind};
use super::select::{HandTargets, Posture};
use super::skills::Sheet;
use super::{HALF_HEIGHT, Intent, MoveMode, Mover, Player, PlaySet};
use bevy::prelude::*;

/// A balance beam: its top line from `a` to `b` (world, the walking surface), how much harder
/// than the yard beam it sways (`sway` scales the tip-over and the drift; a rope is livelier),
/// and what it is called in the log.
#[derive(Component, Clone, Copy, Debug)]
pub struct Beam {
    pub a: Vec3,
    pub b: Vec3,
    pub sway: f32,
    pub name: &'static str,
}

impl Beam {
    fn dir(&self) -> Vec3 {
        (self.b - self.a).with_y(0.0).normalize_or(Vec3::X)
    }
    /// Horizontal right-hand side of the beam (+tilt leans this way).
    fn side(&self) -> Vec3 {
        Vec3::Y.cross(self.dir())
    }
    fn length(&self) -> f32 {
        (self.b - self.a).with_y(0.0).length()
    }
    /// Where `feet` stand along the beam: (metres from `a`, sideways offset, height above the top).
    pub fn locate(&self, feet: Vec3) -> (f32, f32, f32) {
        let rel = feet - self.a;
        let along = rel.dot(self.dir());
        let top = self.a.y + (self.b.y - self.a.y) * (along / self.length().max(1e-3)).clamp(0.0, 1.0);
        (along, rel.dot(self.side()), feet.y - top)
    }
}

/// Soles this close to the beam's line (m, sideways) and its top (m, height) stand on it.
const CATCH: f32 = 0.24;
const HEIGHT_CATCH: f32 = 0.12;
/// Fraction of a walk the stick may ask for along the beam.
const WALK_SCALE: f32 = 0.55;
/// Pull toward the line (stick units per metre off it).
const CENTRE: f32 = 2.5;
/// Pendulum: tip-over gain, the stick's counter gain, damping (all per s²).
const TIP: f32 = 4.0;
const COUNTER: f32 = 7.5;
const DAMP: f32 = 1.6;
/// Drift: base push, extra per m/s walked, per m/s² of sideways acceleration; how often (s) it
/// picks a new push, and the push a landing adds per unit of `Mover.landing`.
const DRIFT: f32 = 0.9;
const DRIFT_SPEED: f32 = 1.4;
const DRIFT_ACCEL: f32 = 0.12;
const DRIFT_HOLD: f32 = 0.8;
const LANDING_KICK: f32 = 2.5;
/// Arms windmill past `WOBBLE`; the walker falls past `FALL`.
const WOBBLE: f32 = 0.7;
const FALL: f32 = 1.0;
/// Fall push off the side (m/s) and up.
const FALL_PUSH: f32 = 2.4;
/// Lean (m/s² of acceleration the torso leans into) per unit tilt, and the narrow stance (m).
const LEAN: f32 = 16.0;
const STANCE: f32 = 0.07;
/// Arms: shoulder height above the body centre, reach to the side, weight.
const SHOULDER: f32 = 0.42;
const ARM_OUT: f32 = 0.7;
const ARM_WEIGHT: f32 = 0.85;
/// Each Balance level takes this much off the drift (at most `MAX_ASSIST`).
const ASSIST_PER_LEVEL: f32 = 0.03;
const MAX_ASSIST: f32 = 0.6;
/// NPC counter: tilt and tilt-rate gains (softer than a perfect counter).
const NPC_GAIN: f32 = 1.5;
const NPC_DAMP: f32 = 0.35;
/// Pendulum integration step (s).
const SUBSTEP: f32 = 1.0 / 120.0;
/// Walked this fraction of a beam in one go to count as crossing it.
const CROSSED: f32 = 0.85;

/// A walker on a beam (written here only).
#[derive(Component, Clone, Debug)]
pub struct Balancing {
    beam: Entity,
    pub tilt: f32,
    rate: f32,
    /// The stick's sideways balance input this frame (−1..1).
    input: f32,
    drift: f32,
    drift_clock: f32,
    clock: f32,
    /// Where on the beam the walk began, how far it has gone, and metres toward the next deed.
    start: f32,
    farthest: f32,
    walked: f32,
    crossed: bool,
    /// Whether the arms' `HandTargets` are ours to remove.
    hands: bool,
    rng: u32,
}

/// Just toppled off a beam: no remounting for `REMOUNT` s (the push needs a moment to carry the
/// body off the edge).
#[derive(Component, Clone, Copy, Debug)]
pub struct Toppled(f32);

const REMOUNT: f32 = 0.8;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, steer.after(PlaySet::Think).after(super::hall::walk).before(PlaySet::Act))
        .add_systems(Update, (mount, sway).chain().in_set(PlaySet::Act));
}

/// `GB_BEAM`: 1 = steady (counter the sway), 2 = drift (no counter).
fn smoke_mode() -> u8 {
    match std::env::var("GB_BEAM").as_deref() {
        Ok("steady") => 1,
        Ok("drift") => 2,
        _ => 0,
    }
}

/// The stick on a beam: walk along it, balance with the sideways part, stay over the line.
fn steer(
    mut mode: Local<Option<u8>>,
    beams: Query<&Beam>,
    mut walkers: Query<(&Transform, &mut Intent, &mut Balancing, Has<Player>)>,
) {
    let smoke_mode = *mode.get_or_insert_with(smoke_mode);
    for (tf, mut intent, mut bal, player) in &mut walkers {
        let smoke = if player { smoke_mode } else { 3 };
        let Ok(beam) = beams.get(bal.beam) else { continue };
        let (dir, side) = (beam.dir(), beam.side());
        let mut stick = intent.move_dir.with_y(0.0);
        if smoke == 1 || smoke == 2 {
            // Smoke walkers go straight along the beam, whichever way they were heading.
            stick = dir * if stick.dot(dir) < 0.0 { -1.0 } else { 1.0 };
        }
        let along = stick.dot(dir);
        let mut input = stick.dot(side).clamp(-1.0, 1.0);
        if smoke == 1 {
            input = (-(bal.tilt * 2.2 + bal.rate * 0.7)).clamp(-1.0, 1.0);
        } else if smoke == 2 {
            input = 0.0;
        } else if smoke == 3 {
            // A practised walker's reflexes: a softer counter than the smoke's, so a gust can win.
            input = (-(bal.tilt * NPC_GAIN + bal.rate * NPC_DAMP)).clamp(-1.0, 1.0);
        }
        bal.input = input;
        let (_, off, _) = beam.locate(tf.translation - Vec3::Y * HALF_HEIGHT);
        let walk = along.clamp(-WALK_SCALE, WALK_SCALE);
        intent.move_dir = dir * walk - side * (off * CENTRE).clamp(-0.3, 0.3);
        intent.sprint = false;
    }
}

fn on_beam(beams: &Query<(Entity, &Beam)>, feet: Vec3) -> Option<(Entity, f32)> {
    beams.iter().find_map(|(e, b)| {
        let (along, off, up) = b.locate(feet);
        (along >= -0.05 && along <= b.length() + 0.05 && off.abs() < CATCH && up.abs() < HEIGHT_CATCH).then_some((e, along))
    })
}

/// Start and end `Balancing` as the soles meet and leave a beam.
#[allow(clippy::type_complexity)]
fn mount(
    time: Res<Time>,
    mut commands: Commands,
    beams: Query<(Entity, &Beam)>,
    mut walkers: Query<
        (Entity, &Transform, &Mover, Option<&Balancing>, Option<&HandTargets>, Option<&mut Toppled>),
        With<Intent>,
    >,
) {
    for (e, tf, mover, bal, hands, toppled) in &mut walkers {
        if let Some(mut t) = toppled {
            t.0 -= time.delta_secs();
            if t.0 <= 0.0 {
                commands.entity(e).remove::<Toppled>();
            }
            continue;
        }
        let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
        let on = (mover.mode == MoveMode::Ground && mover.grounded).then(|| on_beam(&beams, feet)).flatten();
        match (on, bal) {
            (Some((beam, along)), None) => {
                let seed = (tf.translation.x * 7919.0 + tf.translation.z * 104_729.0) as i32 as u32 | 1;
                commands.entity(e).insert((
                    Balancing {
                        beam,
                        tilt: 0.0,
                        rate: if seed & 2 == 0 { 1.0 } else { -1.0 } * mover.landing * LANDING_KICK,
                        input: 0.0,
                        drift: 0.0,
                        drift_clock: 0.0,
                        clock: 0.0,
                        start: along,
                        farthest: 0.0,
                        walked: 0.0,
                        crossed: false,
                        hands: hands.is_none(),
                        rng: seed,
                    },
                    Posture { lean: Vec3::ZERO, stance: STANCE },
                ));
                info!("[beam] on at {:.1} m", along);
            }
            (None, Some(b)) => {
                commands.entity(e).remove::<(Balancing, Posture)>();
                if b.hands {
                    commands.entity(e).remove::<HandTargets>();
                }
                info!("[beam] off after {:.1} m (tilt {:.2})", b.farthest, b.tilt);
            }
            _ => {}
        }
    }
}

fn next(rng: &mut u32) -> f32 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    (*rng as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// The sway: integrate the pendulum, pose the body, and topple or finish.
#[allow(clippy::type_complexity)]
fn sway(
    time: Res<Time>,
    mut commands: Commands,
    beams: Query<&Beam>,
    mut walkers: Query<(
        Entity,
        &Transform,
        &mut Mover,
        &mut Balancing,
        &mut Posture,
        Option<&mut HandTargets>,
        Option<&Sheet>,
        Option<&Name>,
        Has<Player>,
    )>,
    mut deeds: MessageWriter<Deed>,
    mut log: MessageWriter<HudLog>,
) {
    let dt = time.delta_secs().min(0.25);
    if dt <= 0.0 {
        return;
    }
    for (e, tf, mut mover, mut bal, mut posture, hands, sheet, name, player) in &mut walkers {
        let who = name.map_or("?", |n| n.as_str());
        let Ok(beam) = beams.get(bal.beam) else { continue };
        let (dir, side) = (beam.dir(), beam.side());
        bal.clock += dt;
        bal.drift_clock -= dt;
        if bal.drift_clock <= 0.0 {
            bal.drift_clock = DRIFT_HOLD * (0.6 + 0.4 * next(&mut bal.rng).abs());
            bal.drift = next(&mut bal.rng);
        }
        let speed = mover.velocity.with_y(0.0).length();
        let assist = sheet.map_or(0.0, |s| (s.level("balance") as f32 * ASSIST_PER_LEVEL).min(MAX_ASSIST));
        let push = (DRIFT + DRIFT_SPEED * speed) * bal.drift + DRIFT_ACCEL * mover.accel.dot(side);
        // Fixed substeps keep the pendulum the same at any frame rate.
        let steps = (dt / SUBSTEP).ceil().max(1.0);
        let h = dt / steps;
        for _ in 0..steps as u32 {
            let accel = beam.sway * (TIP * bal.tilt + push * (1.0 - assist)) + COUNTER * bal.input - DAMP * bal.rate;
            bal.rate += accel * h;
            bal.tilt += bal.rate * h;
        }

        // Progress along the beam.
        let (along, _, _) = beam.locate(tf.translation - Vec3::Y * HALF_HEIGHT);
        let gone = (along - bal.start).abs();
        if gone > bal.farthest {
            bal.walked += gone - bal.farthest;
            bal.farthest = gone;
        }
        while bal.walked >= 1.0 {
            bal.walked -= 1.0;
            deeds.write(Deed::new(e, "balance", "", 1.0));
        }
        if !bal.crossed && bal.farthest >= beam.length() * CROSSED {
            bal.crossed = true;
            deeds.write(Deed::new(e, "beam", "crossed", 1.0));
            if player {
                log.write(HudLog::new(LogKind::Info, format!("Crossed the {}.", beam.name)));
            }
            info!("[beam] {who} crossed tilt={:.2}", bal.tilt);
        }

        // Topple.
        if bal.tilt.abs() > FALL {
            let out = side * bal.tilt.signum();
            mover.external += out * FALL_PUSH + Vec3::Y * 0.6;
            deeds.write(Deed::new(e, "beam", "fell", 1.0));
            if player {
                log.write(HudLog::new(LogKind::Warn, "You lose your footing!"));
            }
            info!("[beam] {who} fell tilt={:.2} after {:.1} m", bal.tilt, bal.farthest);
            commands.entity(e).remove::<(Balancing, Posture)>().insert(Toppled(REMOUNT));
            if bal.hands {
                commands.entity(e).remove::<HandTargets>();
            }
            continue;
        }

        // Pose: lean with the tilt, arms out and tilted against it, windmilling when close.
        posture.lean = side * bal.tilt * LEAN;
        posture.stance = STANCE;
        if !bal.hands {
            continue;
        }
        let wobble = ((bal.tilt.abs() - WOBBLE) / (FALL - WOBBLE)).clamp(0.0, 1.0);
        let shoulder = tf.translation + Vec3::Y * SHOULDER + dir * 0.08;
        let angle = -bal.tilt * 0.45 + wobble * (bal.clock * 13.0).sin() * 0.6;
        let reach = |s: f32| shoulder + side * (s * ARM_OUT * angle.cos()) + Vec3::Y * (s * ARM_OUT * angle.sin());
        // +X of the body is its left; the left hand goes to whichever side is its left.
        let left_is_plus = side.dot(Vec3::new(mover.yaw.cos(), 0.0, -mover.yaw.sin())) > 0.0;
        let (l, r) = if left_is_plus { (reach(1.0), reach(-1.0)) } else { (reach(-1.0), reach(1.0)) };
        let targets = [Some((l, ARM_WEIGHT)), Some((r, ARM_WEIGHT))];
        match hands {
            Some(mut h) => h.hands = targets,
            None => {
                commands.entity(e).insert(HandTargets { hands: targets });
            }
        }
    }
}

/// The tightrope over the pond: a rope `ROPE_UP` above the higher bank end, from west to east
/// across `POND_CENTER`, a platform with steps at each end. A fall lands in the water.
const ROPE_HALF: f32 = 7.5;
const ROPE_UP: f32 = 1.2;
const ROPE_SWAY: f32 = 1.5;

pub(crate) fn spawn_tightrope(kit: &mut super::world::Kit) {
    use super::world::{POND_CENTER, ground_height};
    use avian3d::prelude::Collider;
    let (wood, rope) = (kit.pal.dark_wood.clone(), kit.pal.straw.clone());
    let ends = [-1.0f32, 1.0].map(|s| Vec2::new(POND_CENTER.x + s * ROPE_HALF, POND_CENTER.y));
    let top = ends.iter().map(|p| ground_height(p.x, p.y)).fold(f32::MIN, f32::max) + ROPE_UP;
    let (a, b) = (Vec3::new(ends[0].x, top, ends[0].y), Vec3::new(ends[1].x, top, ends[1].y));
    let len = b.x - a.x;
    kit.solid(
        "Tightrope",
        Cylinder::new(0.03, len),
        &rope,
        Transform::from_translation((a + b) * 0.5 - Vec3::Y * 0.03).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        Some(Collider::cuboid(0.1, len, 0.1)),
    );
    for (end, out) in [(a, -1.0f32), (b, 1.0)] {
        // Platform (its top level with the rope) and steps down the outer side.
        let ground = ground_height(end.x + out * 0.8, end.z);
        let h = top - ground;
        let centre = end + Vec3::X * out * 0.8;
        kit.solid("Rope platform", Cuboid::new(1.6, h, 1.6), &wood, Transform::from_translation(centre.with_y(ground + h * 0.5)), Some(Collider::cuboid(1.6, h, 1.6)));
        let steps = (h / 0.3).ceil() as usize;
        for i in 1..steps {
            let sh = h * i as f32 / steps as f32;
            let x = centre.x + out * (0.8 + (steps - i) as f32 * 0.45 - 0.225);
            let g = ground_height(x, end.z);
            let size = Vec3::new(0.45, (ground + sh - g).max(0.1) + 0.3, 1.2);
            kit.solid("Rope step", Cuboid::from_size(size), &wood, Transform::from_xyz(x, ground + sh - size.y * 0.5, end.z), Some(Collider::cuboid(size.x, size.y, size.z)));
        }
        for dz in [-0.6, 0.6] {
            kit.solid("Rope post", Cylinder::new(0.06, 1.0), &wood, Transform::from_translation(end + Vec3::new(0.0, 0.5, dz)), None);
        }
    }
    kit.commands.spawn((Name::new("Tightrope line"), Beam { a, b, sway: ROPE_SWAY, name: "tightrope" }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beam() -> Beam {
        Beam { a: Vec3::new(0.0, 0.6, 0.0), b: Vec3::new(8.0, 0.6, 0.0), sway: 1.0, name: "beam" }
    }

    #[test]
    fn locate_measures_along_side_and_height() {
        let (along, off, up) = beam().locate(Vec3::new(3.0, 0.65, 0.1));
        assert!((along - 3.0).abs() < 1e-4);
        assert!((off.abs() - 0.1).abs() < 1e-4);
        assert!((up - 0.05).abs() < 1e-4);
    }

    /// Unattended, the pendulum tips over; the smoke counter holds it under the wobble line.
    #[test]
    fn the_sway_falls_alone_and_holds_with_a_counter() {
        let run = |counter: bool| {
            let (mut tilt, mut rate, mut worst) = (0.05f32, 0.0f32, 0.0f32);
            for _ in 0..600 {
                let input = if counter { (-(tilt * 2.2 + rate * 0.7)).clamp(-1.0, 1.0) } else { 0.0 };
                let accel = TIP * tilt + COUNTER * input + DRIFT * 0.8 - DAMP * rate;
                rate += accel / 60.0;
                tilt += rate / 60.0;
                worst = worst.max(tilt.abs());
                if tilt.abs() > FALL {
                    return (true, worst);
                }
            }
            (false, worst)
        };
        assert!(run(false).0);
        let (fell, worst) = run(true);
        assert!(!fell && worst < WOBBLE, "worst {worst}");
    }
}
