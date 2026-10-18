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
//! - ranged sets (`archer`): keep `SHOOT_NEAR`..`SHOOT_FAR` from the enemy (back-pedal, close in,
//!   strafe), aim (secondary), draw to full and release (bow) or press (crossbow, knives), a
//!   pause between shots;
//! - magic (`cast`): the bound spell (E) from range when magicules allow, the bound art (Q) up
//!   close when Battlewill allows, each a `Difficulty.casting` chance per look, a few seconds
//!   apart; never mid-swing or while guarding;
//! - nothing while dead or down; with no enemy in `AGGRO` range, walk home and stand.
//! `enroll` gives every new `Npc` a `Difficulty` and `combat::Revive` (back at its spawn point).
//! How hard an NPC hits and how much it takes comes from its `stats::Stats`, the same universal
//! base as the player's; `Difficulty` is only how it behaves.
use super::combat::{self, Melee, Revive, Tell};
use super::occupations::Civilian;
use super::magic::{Energy, Grimoire, MASTERED_COST, ability};
use super::moveset::{Kind, Movesets};
use super::hud::log::{HudLog, LogKind};
use super::{Fighter, HALF_HEIGHT, Intent, Loadout, MoveMode, Mover, Npc, Player, PlaySet};
use bevy::prelude::*;

/// Casting: first chance after spotting an enemy, re-look interval, pause after a cast (s), and
/// the spell range (m) past the weapon's reach.
const CAST_FIRST: f32 = 2.0;
const CAST_LOOK: f32 = 0.5;
const CAST_PAUSE: [f32; 2] = [2.5, 5.0];
const SPELL_FROM: f32 = 1.5;
const SPELL_TO: f32 = 12.0;
/// Arts are used within the reach plus this (m).
const ART_REACH: f32 = 2.5;

/// Ranged sets: preferred distance band (m), first shot and pause between shots (s, shortened by
/// aggression), extra draw beyond the bow's full draw (share, random).
const SHOOT_NEAR: f32 = 5.0;
const SHOOT_FAR: f32 = 11.0;
const SHOT_FIRST: f32 = 1.0;
const SHOT_PAUSE: [f32; 2] = [0.5, 1.8];
const OVERDRAW: f32 = 0.25;

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
/// A foe that keeps its guard up this long (s) gets grabbed: the brain closes in to
/// `GRAB_CLOSE` short of `grab::RANGE` and grabs, with this chance per look (scaled by aggression).
const GRAB_AFTER: f32 = 1.2;
const GRAB_CLOSE: f32 = 0.35;
const GRAB_CHANCE: f32 = 0.7;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Npc, Brain>()
        .register_required_components::<Autopilot, Brain>()
        .insert_resource(Peaceful(std::env::var("GB_PEACEFUL").is_ok_and(|v| v.trim() == "1")))
        .add_systems(Update, (peaceful, autopilot, enroll, think).chain().in_set(PlaySet::Think).after(super::senses::SensesSet));
}

/// Peaceful mode (F10, or `GB_PEACEFUL=1`): no NPC picks the player as a foe, so nothing attacks
/// (otherwise an NPC with `senses::Awareness` only picks the player once alerted)
/// you (NPCs still fight each other, and a sparring bout you agreed to, the rank exam, still runs).
#[derive(Resource, Default)]
pub struct Peaceful(pub bool);

fn peaceful(keys: Res<ButtonInput<KeyCode>>, mut on: ResMut<Peaceful>, mut log: MessageWriter<HudLog>) {
    if keys.just_pressed(KeyCode::F10) {
        on.0 = !on.0;
        let text = if on.0 { "Peaceful: enemies leave you alone  [F10]" } else { "Hostile: enemies may attack you  [F10]" };
        log.write(HudLog::new(LogKind::System, text).key(7102));
        info!("[ai] peaceful={}", on.0);
    }
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
    /// 0..1: chance of casting when a cast is possible (checked every `CAST_LOOK` s).
    pub casting: f32,
}

impl Default for Difficulty {
    fn default() -> Self {
        Self {
            reaction: 0.28,
            parry: 0.25,
            dodge: 0.2,
            guard: 0.35,
            aggression: 0.6,
            casting: 0.35,
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
    /// Seconds until the next cast may be considered.
    cast_in: f32,
    /// Ranged: draw time left (s) while drawing, and seconds until the next shot.
    drawing: f32,
    shot_in: f32,
    /// Seconds the foe has kept its guard up; closing in to grab it.
    guarded: f32,
    grabbing: bool,
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
            cast_in: CAST_FIRST,
            drawing: 0.0,
            shot_in: SHOT_FIRST,
            guarded: 0.0,
            grabbing: false,
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
    pub grab: bool,
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

    // A turtling foe gets grabbed: close in, then seize it (a guard can't stop a throw).
    b.guarded = if foe.guarding && foe.tell.is_none() { b.guarded + dt } else { 0.0 };
    if b.guarded > GRAB_AFTER && !b.grabbing && !v.attacking && b.cooldown <= 0.0 && v.stamina > ATTACK_STAMINA {
        b.cooldown = 0.4;
        b.grabbing = b.roll() < GRAB_CHANCE * d.aggression.clamp(0.05, 1.0);
    }
    if b.grabbing {
        if !foe.guarding || v.attacking {
            b.grabbing = false;
        } else if dist > super::grab::RANGE - GRAB_CLOSE {
            o.move_dir = ahead;
            return o;
        } else {
            b.grabbing = false;
            b.guarded = 0.0;
            b.cooldown = 1.2;
            o.grab = true;
            return o;
        }
    }

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

/// What an archer sees: itself, the enemy, and its weapon's draw time (0 = press to shoot).
#[derive(Clone, Copy, Debug)]
pub struct ArcherView {
    pub me: Vec3,
    pub foe: Vec3,
    pub draw_time: f32,
}

/// One frame of thought for a ranged set: spacing, aim (`guard` = secondary held), draw / shoot.
pub fn archer(v: &ArcherView, d: &Difficulty, b: &mut Brain, dt: f32) -> Orders {
    let dt = if dt.is_finite() { dt.clamp(0.0, 0.1) } else { 0.0 };
    b.shot_in -= dt;
    b.flip_in -= dt;
    let mut o = Orders::default();
    let to = flat(v.foe - v.me);
    let dist = to.length();
    let toward = to.normalize_or(Vec3::Z);
    let side = Vec3::Y.cross(toward) * b.side;
    if b.flip_in <= 0.0 {
        b.side = if b.roll() < 0.5 { -1.0 } else { 1.0 };
        b.flip_in = STRAFE_MIN + 2.0 * b.roll();
    }
    o.move_dir = if dist < SHOOT_NEAR {
        -toward * 0.9
    } else if dist > SHOOT_FAR {
        toward * 0.8
    } else {
        side * 0.35
    };
    o.sprint = dist > SHOOT_FAR + 4.0;
    o.guard = dist <= SHOOT_FAR + 3.0;
    let pause = || SHOT_PAUSE[0] + (SHOT_PAUSE[1] - SHOT_PAUSE[0]) * (1.0 - d.aggression.clamp(0.0, 1.0));
    if v.draw_time > 0.0 {
        if b.drawing > 0.0 {
            b.drawing -= dt;
            o.hold = b.drawing > 0.0;
            if !o.hold {
                b.shot_in = pause() * (0.7 + 0.6 * b.roll());
            }
        } else if b.shot_in <= 0.0 && o.guard {
            b.drawing = v.draw_time * (1.0 + OVERDRAW * b.roll());
            o.hold = true;
        }
    } else if b.shot_in <= 0.0 && o.guard {
        o.press = true;
        b.shot_in = pause() * (0.7 + 0.6 * b.roll());
    }
    o
}

/// What casting sees: the distance to the enemy, the weapon reach, and for the bound art and
/// spell whether its pool can pay. `busy`: attacking, guarding or exhausted.
#[derive(Clone, Copy, Debug)]
pub struct CastView {
    pub dist: f32,
    pub reach: f32,
    pub art_ready: bool,
    pub spell_ready: bool,
    pub busy: bool,
}

/// Cast this frame? (art, spell). Spells from range, arts up close.
pub fn cast(v: &CastView, d: &Difficulty, b: &mut Brain, dt: f32) -> (bool, bool) {
    b.cast_in -= if dt.is_finite() { dt.clamp(0.0, 0.1) } else { 0.0 };
    if b.cast_in > 0.0 || v.busy {
        return (false, false);
    }
    b.cast_in = CAST_LOOK;
    let spell = v.spell_ready && (v.reach + SPELL_FROM..=SPELL_TO).contains(&v.dist);
    let art = v.art_ready && v.dist <= v.reach + ART_REACH;
    if !(spell || art) || b.roll() >= d.casting.clamp(0.0, 1.0) {
        return (false, false);
    }
    b.cast_in = CAST_PAUSE[0] + (CAST_PAUSE[1] - CAST_PAUSE[0]) * b.roll();
    if spell { (false, true) } else { (true, false) }
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
    civilians: Query<(), With<Civilian>>,
    peaceful: Res<Peaceful>,
    choosing: Option<Res<super::class::ClassChoice>>,
    players: Query<(), With<Player>>,
    mut npcs: Query<
        (
            Entity,
            &mut Intent,
            &mut Brain,
            Option<&Difficulty>,
            Option<&Energy>,
            Option<&Grimoire>,
            Has<combat::Sparring>,
            Option<&super::senses::Awareness>,
        ),
        Or<(With<Npc>, With<Autopilot>)>,
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut intent, mut brain, difficulty, energy, grimoire, sparring, awareness) in &mut npcs {
        // The player is a foe only once noticed (`senses`), never in peaceful mode, always in a bout.
        let noticed = sparring || awareness.is_none_or(|a| a.alerted);
        // Nobody attacks while the player is still choosing a class.
        let spare_player = !sparring && (peaceful.0 || !noticed || choosing.as_deref().is_some_and(|c| c.open));
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
            .filter(|(e, _, f, ..)| *e != entity && !f.dead && f.team != fighter.team && !civilians.contains(*e))
            .filter(|(e, ..)| !(spare_player && players.contains(*e)))
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
        let difficulty = difficulty.copied().unwrap_or_default();
        let set = movesets.get(loadout.equipped());
        let ranged = set.filter(|s| s.kind == Kind::Ranged).map(|s| s.aim.as_ref().map_or(0.0, |a| a.draw_time.max(0.0)));
        let o = match (ranged, foe) {
            (Some(draw_time), Some((_, pos, _))) => archer(&ArcherView { me, foe: pos, draw_time }, &difficulty, &mut brain, dt),
            _ => decide(&view, &difficulty, &mut brain, dt),
        };
        let (art, spell) = match (foe, energy, grimoire) {
            (Some((_, pos, _)), Some(e), Some(g)) => {
                let can_pay = |id: &str| {
                    ability(id).is_some_and(|a| {
                        let cost = a.cost * if g.is_mastered(a.id) { MASTERED_COST } else { 1.0 };
                        let pool = if a.is_art() { e.battlewill } else { e.magicules };
                        pool >= cost
                    })
                };
                let cv = CastView {
                    dist: flat(pos - me).length(),
                    reach,
                    art_ready: can_pay(g.art),
                    spell_ready: can_pay(g.spell),
                    busy: melee.attacking() || melee.guarding() || melee.exhausted() || o.press || o.hold || o.guard,
                };
                cast(&cv, &difficulty, &mut brain, dt)
            }
            _ => (false, false),
        };
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
            art_pressed: art,
            spell_pressed: spell,
            grab_pressed: o.grab,
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
    fn archer_keeps_range_draws_and_releases() {
        let d = Difficulty::default();
        let me = Vec3::ZERO;
        let mut b = Brain::default();
        let close = archer(&ArcherView { me, foe: Vec3::Z * 2.0, draw_time: 0.8 }, &d, &mut b, 1.0 / 60.0);
        assert!(close.move_dir.z < -0.5, "backs away");
        let far = archer(&ArcherView { me, foe: Vec3::Z * 20.0, draw_time: 0.8 }, &d, &mut b, 1.0 / 60.0);
        assert!(far.move_dir.z > 0.5 && !far.guard, "closes in, not aiming");
        // In the band: aims, then holds a draw at least the full draw time, then lets go.
        let v = ArcherView { me, foe: Vec3::Z * 8.0, draw_time: 0.8 };
        let mut b = Brain::default();
        let mut held = 0;
        let mut released = false;
        for _ in 0..300 {
            let o = archer(&v, &d, &mut b, 1.0 / 60.0);
            assert!(o.guard);
            if o.hold {
                held += 1;
            } else if held > 0 {
                released = true;
                break;
            }
        }
        assert!(released && held as f32 / 60.0 >= 0.78, "held {held} frames");
    }

    #[test]
    fn crossbows_press() {
        let d = Difficulty::default();
        let mut b = Brain::default();
        let v = ArcherView { me: Vec3::ZERO, foe: Vec3::Z * 8.0, draw_time: 0.0 };
        let presses = (0..300).filter(|_| archer(&v, &d, &mut b, 1.0 / 60.0).press).count();
        assert!((1..=6).contains(&presses), "{presses}");
    }

    fn cast_view(dist: f32) -> CastView {
        CastView {
            dist,
            reach: 1.5,
            art_ready: true,
            spell_ready: true,
            busy: false,
        }
    }

    #[test]
    fn casts_spells_from_range_and_arts_up_close() {
        let d = Difficulty {
            casting: 1.0,
            ..default()
        };
        let first = |v: CastView| {
            let mut b = Brain::default();
            (0..400).map(|_| cast(&v, &d, &mut b, 1.0 / 60.0)).find(|c| c.0 || c.1)
        };
        assert_eq!(first(cast_view(8.0)), Some((false, true)));
        assert_eq!(first(cast_view(1.2)), Some((true, false)));
        assert_eq!(first(cast_view(20.0)), None, "out of spell range");
        assert_eq!(first(CastView { busy: true, ..cast_view(8.0) }), None);
        assert_eq!(first(CastView { spell_ready: false, ..cast_view(8.0) }), None, "no magicules");
    }

    #[test]
    fn casts_are_spaced() {
        let d = Difficulty {
            casting: 1.0,
            ..default()
        };
        let mut b = Brain::default();
        let casts = (0..600).filter(|_| {
            let c = cast(&cast_view(8.0), &d, &mut b, 1.0 / 60.0);
            c.0 || c.1
        });
        // 10 s: the first after CAST_FIRST, then one per CAST_PAUSE at most.
        assert!((1..=4).contains(&casts.count()));
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
    fn closes_in_and_grabs_a_foe_that_keeps_its_guard_up() {
        let d = Difficulty { aggression: 1.0, ..default() };
        let mut b = Brain { cooldown: 0.0, ..default() };
        let mut v = view(1.6);
        v.enemy.as_mut().unwrap().guarding = true;
        let (mut closed, mut grabbed) = (false, false);
        for _ in 0..600 {
            let o = decide(&v, &d, &mut b, 1.0 / 60.0);
            // Walk the foe closer whenever the brain steps in.
            if o.move_dir.z > 0.9 {
                closed = true;
                let foe = v.enemy.as_mut().unwrap();
                foe.pos.z = (foe.pos.z - 0.02).max(0.5);
            }
            if o.grab {
                grabbed = true;
                assert!(v.enemy.unwrap().pos.z <= super::super::grab::RANGE - GRAB_CLOSE + 1e-3);
                break;
            }
        }
        assert!(closed && grabbed, "closed {closed} grabbed {grabbed}");
        // An open foe is never grabbed.
        let mut b = Brain { cooldown: 0.0, ..default() };
        let open = view(1.0);
        assert!((0..300).all(|_| !decide(&open, &d, &mut b, 1.0 / 60.0).grab));
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
