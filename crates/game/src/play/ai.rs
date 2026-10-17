//! NPC brain writing `Intent` (`Update / PlaySet::Think`), the only writer of NPC `Intent`.
//! Plain rules, no network or language model: every frame `think` reads the world (nearest
//! living enemy, its attack `Tell`, own hp / stamina) and `decide` turns that into stick, button
//! and lock-on orders, so `combat` drives NPC bodies exactly as it drives the player's:
//! - spacing: close to the moveset's `combat::reach`, back off when crowded, circle-strafe in
//!   between, switching side now and then; sprint when far;
//! - attacks: in reach, off cooldown and with stamina, a light chain of 1–3 taps (a tap is one
//!   pressed frame, released the next), or a held heavy (more likely into a raised guard);
//! - defence: an incoming swing is noticed after `Difficulty.reaction`, then guarded (sometimes
//!   raised late, inside the parry window), dodged sideways, or taken;
//! - low hp: retreat with the guard up until stamina recovers;
//! - nothing while dead or down; with no enemy in `AGGRO` range, walk home and stand.
//! `enroll` gives every new `Npc` a `Difficulty` and `combat::Revive` (back at its spawn point).
//! How hard an NPC hits and how much it takes comes from its `stats::Stats`, the same universal
//! base as the player's; `Difficulty` is only how it behaves.
use super::combat::{self, Melee, Revive, Tell};
use super::moveset::Movesets;
use super::{Fighter, HALF_HEIGHT, Intent, Loadout, MoveMode, Mover, Npc, PlaySet};
use bevy::prelude::*;

/// Enemies farther than this (m) are ignored.
const AGGRO: f32 = 14.0;
/// Sprint toward an enemy farther than this (m).
const SPRINT_FROM: f32 = 7.0;
/// Preferred distance as a share of the reach; closer than `CROWDED` of it backs off.
const SPACING: f32 = 0.85;
const CROWDED: f32 = 0.55;
/// Attack when the enemy is within reach plus this (m) (the lunge closes the rest).
const ATTACK_SLACK: f32 = 0.35;
/// React to swings whose reach plus this (m) covers the distance.
const THREAT_SLACK: f32 = 0.9;
/// Seconds between chained taps, and heavy hold time.
const TAP_GAP: f32 = 0.3;
const HEAVY_HOLD: f32 = 0.45;
/// Stamina needed to start an attack / a dodge.
const ATTACK_STAMINA: f32 = 25.0;
const DODGE_STAMINA: f32 = 22.0;
/// Guard stays up this long after the threat passes (s).
const GUARD_LINGER: f32 = 0.2;
/// Parry timing: raise the guard this long before the blow lands (s).
const PARRY_LEAD: f32 = 0.1;
/// Retreat below this hp share, until stamina is back above `RETREAT_UNTIL`.
const RETREAT_HP: f32 = 0.25;
const RETREAT_UNTIL: f32 = 80.0;
/// Strafe side flips after a random 1.5–4 s.
const STRAFE_MIN: f32 = 1.5;
const STRAFE_SPAN: f32 = 2.5;
/// Seconds before a dead NPC gets up again at home.
const REVIVE_AFTER: f32 = 6.0;
/// Home is "reached" within this distance (m).
const HOME_RADIUS: f32 = 1.0;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Npc, Brain>()
        .register_required_components::<Autopilot, Brain>()
        .add_systems(Update, (autopilot, enroll, think).chain().in_set(PlaySet::Think));
}

/// The NPC brain drives this (non-NPC) body too: `GB_FIGHT=<moveset>` puts it on the player, who
/// then wields that weapon and duels like an NPC (smoke tests and recordings).
#[derive(Component, Default)]
pub struct Autopilot;

fn autopilot(
    time: Res<Time>,
    mut done: Local<bool>,
    mut commands: Commands,
    players: Query<Entity, With<super::Player>>,
    mut equip: MessageWriter<combat::Equip>,
) {
    if *done || time.elapsed_secs() < 0.5 {
        return;
    }
    *done = true;
    let Some(set) = std::env::var("GB_FIGHT").ok().filter(|s| !s.trim().is_empty()) else { return };
    for who in &players {
        commands.entity(who).try_insert(Autopilot);
        equip.write(combat::Equip { who, set: set.trim().to_string() });
    }
}

/// How well an NPC fights. Inserted with defaults by `enroll` unless the spawner set one.
#[derive(Component, Clone, Copy, Debug)]
pub struct Difficulty {
    /// Seconds before an incoming swing is noticed.
    pub reaction: f32,
    /// Chances (0..1) of answering a noticed swing with a timed parry / a dodge / a plain guard;
    /// the rest is taken.
    pub parry: f32,
    pub dodge: f32,
    pub guard: f32,
    /// 0..1: how often an open chance to attack is taken, and how short the pause after one.
    pub aggression: f32,
}

impl Default for Difficulty {
    fn default() -> Self {
        Self {
            reaction: 0.28,
            parry: 0.25,
            dodge: 0.2,
            guard: 0.35,
            aggression: 0.6,
        }
    }
}

/// The brain's memory (required by `Npc`).
#[derive(Component, Clone, Debug)]
pub struct Brain {
    rng: u32,
    /// Strafe side (+1 left of the enemy line, −1 right) and seconds until it may flip.
    side: f32,
    flip_in: f32,
    /// Seconds until the next attack may start.
    cooldown: f32,
    /// Light taps still to chain, seconds until the next one, and heavy hold left (s).
    taps: u8,
    tap_in: f32,
    holding: f32,
    /// The enemy swing being answered: seconds since noticed (by its `Tell.elapsed`).
    swing: Option<f32>,
    /// That swing's `Tell.elapsed` last frame (a smaller one is a new swing).
    last_elapsed: f32,
    answered: bool,
    defence: Defence,
    /// Seconds the guard stays up after the threat passes.
    linger: f32,
    retreating: bool,
    /// Spawn feet and yaw.
    home: Option<(Vec3, f32)>,
}

impl Default for Brain {
    fn default() -> Self {
        Self {
            rng: 0x9E37_79B9,
            side: 1.0,
            flip_in: STRAFE_MIN,
            cooldown: 1.0,
            taps: 0,
            tap_in: 0.0,
            holding: 0.0,
            swing: None,
            last_elapsed: 0.0,
            answered: false,
            defence: Defence::None,
            linger: 0.0,
            retreating: false,
            home: None,
        }
    }
}

impl Brain {
    /// xorshift32 → 0..1.
    fn roll(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Defence {
    None,
    Guard,
    /// Raise the guard once the blow is this close (s).
    Parry,
}

/// What `decide` sees.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub me: Vec3,
    pub hp: f32,
    pub stamina: f32,
    pub exhausted: bool,
    pub attacking: bool,
    /// Reach of my blows (m).
    pub reach: f32,
    pub enemy: Option<Foe>,
    /// Spawn feet, if known.
    pub home: Option<Vec3>,
}

#[derive(Clone, Copy, Debug)]
pub struct Foe {
    pub pos: Vec3,
    pub tell: Option<Tell>,
    pub guarding: bool,
}

/// What `decide` asks for this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Orders {
    pub move_dir: Vec3,
    pub sprint: bool,
    pub press: bool,
    pub hold: bool,
    pub release: bool,
    pub guard: bool,
    pub dodge: bool,
}

/// One frame of thought.
pub fn decide(v: &View, d: &Difficulty, b: &mut Brain, dt: f32) -> Orders {
    let dt = if dt.is_finite() { dt.clamp(0.0, 0.1) } else { 0.0 };
    b.cooldown -= dt;
    b.tap_in -= dt;
    b.flip_in -= dt;
    let mut o = Orders::default();

    let Some(foe) = v.enemy else {
        *b = Brain { rng: b.rng, home: b.home, ..default() };
        if let Some(home) = v.home {
            let back = flat(home - v.me);
            if back.length() > HOME_RADIUS {
                o.move_dir = back.normalize() * 0.6;
            }
        }
        return o;
    };
    let to = flat(foe.pos - v.me);
    let dist = to.length();
    let ahead = to.normalize_or(Vec3::Z);
    let left = Vec3::Y.cross(ahead);

    // Finish a held heavy / chained taps already under way.
    if b.holding > 0.0 {
        b.holding -= dt;
        if b.holding <= 0.0 {
            o.release = true;
        } else {
            o.hold = true;
        }
        return o;
    }
    if b.taps > 0 && b.tap_in <= 0.0 && v.attacking {
        b.taps -= 1;
        b.tap_in = TAP_GAP;
        o.press = true;
        o.hold = true;
        return o;
    }
    if !v.attacking && b.tap_in <= -TAP_GAP {
        b.taps = 0;
    }

    // Defence against the foe's swing.
    let threat = foe.tell.filter(|t| dist < t.reach + THREAT_SLACK);
    match threat {
        Some(t) => {
            let new = b.swing.is_none() || t.elapsed + 1e-3 < b.last_elapsed;
            b.last_elapsed = t.elapsed;
            if new {
                b.swing = Some(0.0);
                b.answered = false;
                b.defence = Defence::None;
            }
            let seen = b.swing.map_or(0.0, |s| s + dt);
            b.swing = Some(seen);
            if !b.answered && seen >= d.reaction && !v.attacking {
                b.answered = true;
                let r = b.roll();
                if r < d.parry {
                    b.defence = Defence::Parry;
                } else if r < d.parry + d.dodge && v.stamina > DODGE_STAMINA && !v.exhausted {
                    o.dodge = true;
                    let side = if b.roll() < 0.5 { 1.0 } else { -1.0 };
                    o.move_dir = (left * side - ahead * 0.3).normalize();
                    return o;
                } else if r < d.parry + d.dodge + d.guard {
                    b.defence = Defence::Guard;
                }
            }
            let raise = match b.defence {
                Defence::Guard => true,
                Defence::Parry => t.until_active <= PARRY_LEAD,
                Defence::None => false,
            };
            if raise {
                o.guard = true;
                b.linger = GUARD_LINGER;
            }
        }
        None => {
            b.swing = None;
            b.defence = Defence::None;
            if b.linger > 0.0 {
                b.linger -= dt;
                o.guard = true;
            }
        }
    }

    // Retreat while hurt and winded.
    if v.hp < RETREAT_HP && v.stamina < RETREAT_UNTIL {
        b.retreating = true;
    } else if v.stamina >= RETREAT_UNTIL || v.hp >= RETREAT_HP {
        b.retreating = false;
    }
    if b.retreating {
        o.move_dir = (-ahead + left * b.side * 0.4).normalize();
        o.guard = true;
        return o;
    }

    // Spacing and strafing.
    if b.flip_in <= 0.0 {
        b.flip_in = STRAFE_MIN + STRAFE_SPAN * b.roll();
        if b.roll() < 0.5 {
            b.side = -b.side;
        }
    }
    let want = v.reach * SPACING;
    o.move_dir = if dist > want + 0.5 {
        o.sprint = dist > SPRINT_FROM && !v.exhausted;
        ahead
    } else if dist < v.reach * CROWDED {
        (-ahead + left * b.side * 0.3).normalize() * 0.7
    } else {
        let radial = ((dist - want) * 1.5).clamp(-0.5, 0.5);
        (left * b.side * 0.55 + ahead * radial).clamp_length_max(1.0)
    };

    // Attack when open.
    let in_reach = dist <= v.reach + ATTACK_SLACK;
    if in_reach && !o.guard && !v.attacking && b.cooldown <= 0.0 && v.stamina > ATTACK_STAMINA && !v.exhausted {
        let aggression = d.aggression.clamp(0.05, 1.0);
        b.cooldown = 0.5 + (1.0 - aggression) * 1.5 + b.roll() * 0.6;
        if b.roll() < aggression {
            let heavy = if foe.guarding { 0.6 } else { 0.2 };
            if b.roll() < heavy {
                b.holding = HEAVY_HOLD;
                o.press = true;
                o.hold = true;
            } else {
                b.taps = (b.roll() * 3.0) as u8;
                b.tap_in = TAP_GAP;
                o.press = true;
                o.hold = true;
            }
        }
    }
    o
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// New NPCs: difficulty, revive at home, a per-NPC random seed.
fn enroll(
    mut commands: Commands,
    mut npcs: Query<(Entity, &Transform, &Mover, &mut Brain, Option<&Difficulty>), Added<Npc>>,
) {
    for (entity, tf, mover, mut brain, difficulty) in &mut npcs {
        let difficulty = difficulty.copied().unwrap_or_default();
        let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
        brain.home = Some((feet, mover.yaw));
        brain.rng ^= entity.to_bits() as u32 | 1;
        commands.entity(entity).try_insert((
            difficulty,
            Revive {
                after: REVIVE_AFTER,
                home: Some((feet, mover.yaw)),
            },
        ));
    }
}

/// Read the world for every NPC and write its `Intent`.
#[allow(clippy::type_complexity)]
fn think(
    time: Res<Time>,
    movesets: Res<Movesets>,
    fighters: Query<(Entity, &Transform, &Fighter, &Melee, &Mover, &Loadout)>,
    mut npcs: Query<(Entity, &mut Intent, &mut Brain, Option<&Difficulty>), Or<(With<Npc>, With<Autopilot>)>>,
) {
    let dt = time.delta_secs();
    for (entity, mut intent, mut brain, difficulty) in &mut npcs {
        let Ok((_, tf, fighter, melee, mover, loadout)) = fighters.get(entity) else {
            continue;
        };
        let me = tf.translation;
        let own_body = matches!(mover.mode, MoveMode::Ground | MoveMode::Air);
        if fighter.dead || melee.down() || !own_body {
            *intent = Intent {
                look: intent.look,
                ..default()
            };
            continue;
        }
        let foe = fighters
            .iter()
            .filter(|(e, _, f, ..)| *e != entity && !f.dead && f.team != fighter.team)
            .map(|(e, t, _, m, ..)| (e, t.translation, m))
            .filter(|(_, p, _)| flat(*p - me).length() < AGGRO)
            .min_by(|a, b| flat(a.1 - me).length().total_cmp(&flat(b.1 - me).length()));
        let reach = movesets.get(loadout.equipped()).map_or(1.0, combat::reach);
        let view = View {
            me,
            hp: fighter.hp / fighter.max_hp.max(1.0),
            stamina: fighter.stamina,
            exhausted: melee.exhausted(),
            attacking: melee.attacking(),
            reach,
            enemy: foe.map(|(_, pos, m)| Foe {
                pos,
                tell: m.tell(),
                guarding: m.guarding(),
            }),
            home: brain.home.map(|h| h.0 + Vec3::Y * HALF_HEIGHT),
        };
        let o = decide(&view, &difficulty.copied().unwrap_or_default(), &mut brain, dt);
        let look = foe.map_or_else(
            || {
                let yaw = brain.home.map_or(mover.yaw, |h| h.1);
                me + Vec3::new(yaw.sin(), 0.0, yaw.cos()) * 5.0
            },
            |f| f.1,
        );
        *intent = Intent {
            move_dir: if o.move_dir.is_finite() { o.move_dir.clamp_length_max(1.0) } else { Vec3::ZERO },
            look,
            sprint: o.sprint,
            dodge_pressed: o.dodge,
            primary_pressed: o.press,
            primary_held: o.hold,
            primary_released: o.release,
            secondary_held: o.guard,
            secondary_pressed: o.guard && !intent.secondary_held,
            target: foe.map(|f| f.0),
            ..default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(dist: f32) -> View {
        View {
            me: Vec3::ZERO,
            hp: 1.0,
            stamina: 100.0,
            exhausted: false,
            attacking: false,
            reach: 1.5,
            enemy: Some(Foe {
                pos: Vec3::Z * dist,
                tell: None,
                guarding: false,
            }),
            home: None,
        }
    }

    fn tell(elapsed: f32, until_active: f32) -> Tell {
        Tell {
            elapsed,
            until_active,
            active: false,
            heavy: false,
            reach: 1.5,
        }
    }

    #[test]
    fn closes_distance_when_far() {
        let mut b = Brain::default();
        let o = decide(&view(10.0), &Difficulty::default(), &mut b, 1.0 / 60.0);
        assert!(o.move_dir.z > 0.9 && o.sprint);
        assert!(!o.press);
    }

    #[test]
    fn attacks_in_reach_when_aggressive() {
        let d = Difficulty {
            aggression: 1.0,
            ..default()
        };
        let mut b = Brain {
            cooldown: 0.0,
            ..default()
        };
        let mut pressed = false;
        for _ in 0..10 {
            pressed |= decide(&view(1.4), &d, &mut b, 1.0 / 60.0).press;
        }
        assert!(pressed);
    }

    #[test]
    fn guards_a_noticed_swing() {
        let d = Difficulty {
            reaction: 0.1,
            parry: 0.0,
            dodge: 0.0,
            guard: 1.0,
            ..default()
        };
        let mut b = Brain::default();
        let mut v = view(1.6);
        let mut guarded = false;
        for i in 0..20 {
            v.enemy.as_mut().unwrap().tell = Some(tell(i as f32 / 60.0, 0.4));
            guarded |= decide(&v, &d, &mut b, 1.0 / 60.0).guard;
        }
        assert!(guarded);
    }

    #[test]
    fn parry_waits_for_the_blow() {
        let d = Difficulty {
            reaction: 0.0,
            parry: 1.0,
            ..default()
        };
        let mut b = Brain::default();
        let mut v = view(1.6);
        v.enemy.as_mut().unwrap().tell = Some(tell(0.0, 0.5));
        assert!(!decide(&v, &d, &mut b, 1.0 / 60.0).guard);
        v.enemy.as_mut().unwrap().tell = Some(tell(0.45, 0.05));
        assert!(decide(&v, &d, &mut b, 1.0 / 60.0).guard);
    }

    #[test]
    fn retreats_when_hurt_and_winded() {
        let mut b = Brain::default();
        let mut v = view(2.0);
        v.hp = 0.1;
        v.stamina = 20.0;
        let o = decide(&v, &Difficulty::default(), &mut b, 1.0 / 60.0);
        assert!(o.move_dir.z < 0.0 && o.guard);
    }

    #[test]
    fn walks_home_without_an_enemy() {
        let mut b = Brain::default();
        let mut v = view(1.0);
        v.enemy = None;
        v.home = Some(Vec3::X * 5.0);
        let o = decide(&v, &Difficulty::default(), &mut b, 1.0 / 60.0);
        assert!(o.move_dir.x > 0.5 && !o.press);
    }
}
