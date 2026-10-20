//! Arts and spells (`Update / PlaySet::Act`, after combat). Two pools, as in the setting this
//! takes its inspiration from:
//! - **Battlewill** (aura of the body) fuels **arts** on Q: no chant, little or no wind-up,
//!   physical techniques that look like magic. It builds up in a fight (landing melee blows,
//!   taking them) and trickles back at rest.
//! - **Magicules** fuel **spells** on E: a short cast (the chant), then the effect. They
//!   regenerate after a pause. MB5 binds the next known spell to E.
//!
//! Abilities are a table (`ABILITIES`): pool, element, cost, cast clip window and
//! release point (from the clips' measured hand-speed peaks), damage / poise / knockback and a
//! `Shape`: a projectile (`Bolt`), a thrown aura blade (`Crescent`, passes through up to three
//! foes), a ground slam (`Quake`), a chained lightning strike (`Lance`), orbiting motes that
//! launch and home (`Orbs`, Lissajous orbits), or a dash (`Step`). Known abilities are the ones the
//! fighter has discovered (`skills::Sheet::abilities`; `skills` discovers them from deeds).
//!
//! Mastery: every blow an ability lands (for Aura Ward: every blow it turns) adds `MASTERY_PER_HIT`; at full mastery the world
//! acknowledges it as **Mastered**: it costs `MASTERED_COST` of the price, casts `MASTERED_SPEED` times
//! faster, hits `MASTERED_POWER` times harder and its effects grow, but mastering it permanently
//! takes `MASTERED_DEBT` magicules from the fighter's maximum.
//!
//! Damage: arts scale by `stats::Derived::might`, spells by `spell`; blows go out as `Hit`s
//! (`HitKind::Spell`, resolved by `combat` like everything else). Casting owns the `Action`
//! (`Owner::Magic`, `ActionKind::Cast`); a hit reaction (`Owner::Hit`) interrupts it.
use super::combat::{CombatSet, segment_distance};
use super::camera::CameraShake;
use super::hud::log::{HudLog, LogKind};
use super::monitor::Monitor;
use super::deeds::Deed;
use super::skills::Sheet;
use super::stats::Derived;
use super::tools::Holding;
use super::vfx::{self, Element, Expire, Flash, Vfx};
use super::world::TrainingDummy;
use super::{
    Action, ActionKind, ActionLayer, Fighter, HALF_HEIGHT, Hit, HitKind, Intent, MoveMode, Mover, Owner, PlaySet,
    Player,
};
use crate::motion::Mask;
use crate::motion::bank::PoseBank;
use crate::motion::Rig;
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;

/// Mastery per landed blow (20 blows to mastered), and what mastery changes.
pub const MASTERY_PER_HIT: f32 = 0.05;
pub const MASTERED_COST: f32 = 0.65;
pub const MASTERED_SPEED: f32 = 1.35;
pub const MASTERED_POWER: f32 = 1.2;
pub const MASTERED_FLAIR: f32 = 1.35;
pub const MASTERED_DEBT: f32 = 8.0;
/// Over-mana: the most a pool can hold above its maximum (× max), and how fast it bleeds (/s).
pub const OVER_CAP: f32 = 1.5;
pub const OVER_DECAY: f32 = 4.0;
/// Pools: Battlewill per landed melee blow / per blow taken, at rest (/s); magicules (/s) after
/// `MP_DELAY` s without casting.
const BW_ON_HIT: f32 = 7.0;
const BW_ON_HURT: f32 = 4.0;
const BW_REST: f32 = 2.0;
const MP_REGEN: f32 = 8.0;
const MP_DELAY: f32 = 1.2;
/// Body capsule of fighters and dummies for spell hits (as `combat`).
const BODY_LOW: f32 = 0.25;
const BODY_HIGH: f32 = 1.55;
const BODY_RADIUS: f32 = 0.26;
const DUMMY_HALF: f32 = 0.6;
const DUMMY_RADIUS: f32 = 0.28;
/// Stick share while casting a spell (arts root the body).
const CAST_MOVE: f32 = 0.35;
/// Orbs circle this long before launching (s), and turn toward their target at this rate (rad/s).
const ORBIT_TIME: f32 = 0.7;
const HOMING: f32 = 5.0;
/// Lance: half-angle of the aim cone (cos), chain range (m).
const LANCE_CONE: f32 = 0.93;
/// A player's lance only jumps to a foe within ~8 degrees of the crosshair.
const SKILL_LANCE_CONE: f32 = 0.99;
const CHAIN_RANGE: f32 = 5.5;
/// Dash length (s).
const STEP_TIME: f32 = 0.12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    Battlewill,
    Magicules,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Bolt { speed: f32, gravity: f32, radius: f32, bloom: f32 },
    Crescent { speed: f32, radius: f32 },
    Quake { radius: f32 },
    Lance { range: f32, chains: u32 },
    Orbs { count: u32, speed: f32 },
    Step { distance: f32 },
    /// A shell of aura on the caster: for `duration` s, `share` of every blow's damage and poise
    /// is turned aside (`Ward`, read by `combat`).
    Ward { duration: f32, share: f32 },
    /// Hush the caster's aura for `duration` s (`senses::Veiled`): enemies' sense rays stop
    /// bending toward them. Attacking or casting anything else ends it.
    Veil { duration: f32 },
}

#[derive(Clone, Copy, Debug)]
pub struct Ability {
    pub id: &'static str,
    pub name: &'static str,
    pub pool: Pool,
    pub element: Element,
    pub cost: f32,
    pub track: &'static str,
    /// Clip window (s), playback rate and release point (share of the window).
    pub start: f32,
    pub end: f32,
    pub speed: f32,
    pub release: f32,
    pub damage: f32,
    pub poise: f32,
    pub knockback: f32,
    pub shape: Shape,
    pub about: &'static str,
}

impl Ability {
    pub fn is_art(&self) -> bool {
        self.pool == Pool::Battlewill
    }
}

pub static ABILITIES: [Ability; 10] = [
    // --- Arts (Battlewill, Q).
    Ability {
        id: "crescent_edge",
        name: "Crescent Edge",
        pool: Pool::Battlewill,
        element: Element::Aura,
        cost: 25.0,
        track: "ual2/Sword_Regular_A",
        start: 0.0,
        end: 0.43,
        speed: 1.1,
        release: 0.54,
        damage: 22.0,
        poise: 25.0,
        knockback: 3.0,
        shape: Shape::Crescent { speed: 16.0, radius: 0.75 },
        about: "Hurl a blade of aura from the swing. Cuts through up to three foes.",
    },
    Ability {
        id: "flash_step",
        name: "Flash Step",
        pool: Pool::Battlewill,
        element: Element::Aura,
        cost: 18.0,
        track: "sekiro/Run-Forwards",
        start: 0.0,
        end: 0.3,
        speed: 1.6,
        release: 0.0,
        damage: 0.0,
        poise: 0.0,
        knockback: 0.0,
        shape: Shape::Step { distance: 6.0 },
        about: "Aura bursts from the legs: cross six metres in a blink.",
    },
    Ability {
        id: "aura_bullet",
        name: "Aura Bullet",
        pool: Pool::Battlewill,
        element: Element::Aura,
        cost: 14.0,
        track: "ual1/Spell_Simple_Shoot",
        start: 0.0,
        end: 0.45,
        speed: 1.2,
        release: 0.17,
        damage: 12.0,
        poise: 10.0,
        knockback: 1.5,
        shape: Shape::Bolt { speed: 28.0, gravity: 0.0, radius: 0.14, bloom: 0.0 },
        about: "Flick a dense bead of aura. Almost no wind-up.",
    },
    Ability {
        id: "earthbreaker",
        name: "Earthbreaker",
        pool: Pool::Battlewill,
        element: Element::Aura,
        cost: 40.0,
        track: "sekiro/Zombie Smash",
        start: 0.9,
        end: 2.0,
        speed: 1.25,
        release: 0.57,
        damage: 26.0,
        poise: 50.0,
        knockback: 6.0,
        shape: Shape::Quake { radius: 3.2 },
        about: "Drive aura into the ground: a shockwave throws everyone near.",
    },
    Ability {
        id: "aura_ward",
        name: "Aura Ward",
        pool: Pool::Battlewill,
        element: Element::Aura,
        cost: 30.0,
        track: "ual1/Spell_Simple_Enter",
        start: 0.0,
        end: 0.53,
        speed: 1.2,
        release: 0.7,
        damage: 0.0,
        poise: 0.0,
        knockback: 0.0,
        shape: Shape::Ward { duration: 5.0, share: 0.6 },
        about: "Wrap the body in hardened aura: for five seconds, blows lose most of their bite.",
    },
    Ability {
        id: "veil",
        name: "Veil",
        pool: Pool::Battlewill,
        element: Element::Aura,
        cost: 20.0,
        track: "ual1/Spell_Simple_Enter",
        start: 0.0,
        end: 0.4,
        speed: 1.4,
        release: 0.6,
        damage: 0.0,
        poise: 0.0,
        knockback: 0.0,
        shape: Shape::Veil { duration: 12.0 },
        about: "Draw the aura in until it barely stirs: for twelve seconds, enemies cannot feel you. Attacking ends it.",
    },
    // --- Spells (magicules, E).
    Ability {
        id: "cinder_bolt",
        name: "Cinder Bolt",
        pool: Pool::Magicules,
        element: Element::Fire,
        cost: 18.0,
        track: "sekiro/Magic Action Small Projectile",
        start: 0.3,
        end: 1.3,
        speed: 1.4,
        release: 0.52,
        damage: 24.0,
        poise: 20.0,
        knockback: 3.0,
        shape: Shape::Bolt { speed: 20.0, gravity: 2.0, radius: 0.2, bloom: 0.0 },
        about: "A seething ball of flame on a chaotic flow. Arcs slightly.",
    },
    Ability {
        id: "frost_bloom",
        name: "Frost Bloom",
        pool: Pool::Magicules,
        element: Element::Frost,
        cost: 26.0,
        track: "sekiro/Magic Action Big Projectile",
        start: 0.5,
        end: 1.6,
        speed: 1.3,
        release: 0.59,
        damage: 18.0,
        poise: 30.0,
        knockback: 2.0,
        shape: Shape::Bolt { speed: 14.0, gravity: 0.0, radius: 0.25, bloom: 2.2 },
        about: "A seed of ice that flowers into a crystal rosette, striking all around.",
    },
    Ability {
        id: "storm_lance",
        name: "Storm Lance",
        pool: Pool::Magicules,
        element: Element::Lightning,
        cost: 30.0,
        track: "sekiro/Magic Action Energy Beam",
        start: 0.7,
        end: 1.5,
        speed: 1.2,
        release: 0.34,
        damage: 28.0,
        poise: 25.0,
        knockback: 3.5,
        shape: Shape::Lance { range: 18.0, chains: 2 },
        about: "Lightning to the foe you face, leaping on to two more.",
    },
    Ability {
        id: "orbit_motes",
        name: "Orbit Motes",
        pool: Pool::Magicules,
        element: Element::Arcane,
        cost: 34.0,
        track: "ual1/Spell_Simple_Enter",
        start: 0.0,
        end: 0.53,
        speed: 1.0,
        release: 0.53,
        damage: 12.0,
        poise: 12.0,
        knockback: 1.5,
        shape: Shape::Orbs { count: 3, speed: 12.0 },
        about: "Three motes trace Lissajous orbits around you, then hunt your target.",
    },
];

pub fn ability(id: &str) -> Option<&'static Ability> {
    ABILITIES.iter().find(|a| a.id == id)
}

/// Of the `discovered` abilities (`skills::Sheet::abilities`), the arts or the spells, in table
/// order.
pub fn known(discovered: &[&'static str], arts: bool) -> impl Iterator<Item = &'static Ability> {
    ABILITIES.iter().filter(move |a| discovered.contains(&a.id) && a.is_art() == arts)
}

/// The two pools (written by `magic`; `discipline` adds over-mana from absorbed spells, up to
/// `OVER_CAP` × the maximum, which bleeds away at `OVER_DECAY` /s).
#[derive(Component, Clone, Debug)]
pub struct Energy {
    pub battlewill: f32,
    pub max_battlewill: f32,
    pub magicules: f32,
    pub max_magicules: f32,
    /// Seconds since the last spell.
    pub rested: f32,
}

impl Default for Energy {
    fn default() -> Self {
        Self {
            battlewill: 40.0,
            max_battlewill: 100.0,
            magicules: 100.0,
            max_magicules: 100.0,
            rested: MP_DELAY,
        }
    }
}

/// Bindings and mastery (written by `magic` only; the stats panel binds through `Bind`).
#[derive(Component, Clone, Debug)]
pub struct Grimoire {
    pub art: &'static str,
    pub spell: &'static str,
    pub mastery: HashMap<&'static str, f32>,
    /// Magicules given up to master abilities.
    pub debt: f32,
}

impl Default for Grimoire {
    fn default() -> Self {
        Self {
            art: "crescent_edge",
            spell: "cinder_bolt",
            mastery: HashMap::new(),
            debt: 0.0,
        }
    }
}

impl Grimoire {
    /// Put back saved bindings and mastery (`savegame`); unknown ability ids are skipped.
    pub fn restore(&mut self, art: &str, spell: &str, mastery: &HashMap<String, f32>, debt: f32) {
        if let Some(a) = ability(art) {
            self.art = a.id;
        }
        if let Some(a) = ability(spell) {
            self.spell = a.id;
        }
        for (id, m) in mastery {
            if let Some(a) = ability(id) {
                self.mastery.insert(a.id, m.clamp(0.0, 1.0));
            }
        }
        if debt.is_finite() {
            self.debt = debt.max(0.0);
        }
    }
}

impl Grimoire {
    pub fn mastery(&self, id: &str) -> f32 {
        self.mastery.get(id).copied().unwrap_or(0.0)
    }
    pub fn is_mastered(&self, id: &str) -> bool {
        self.mastery(id) >= 1.0
    }
}

/// An active Aura Ward (written by `magic`; `combat` reads `share`).
#[derive(Component, Clone, Copy, Debug)]
pub struct Ward {
    pub left: f32,
    pub share: f32,
    shell: Entity,
}

/// The visible shell of a ward, a child of its owner.
#[derive(Component)]
pub(crate) struct WardShell {
    pub(crate) owner: Entity,
    pub(crate) age: f32,
}

/// Bind an ability to its slot (Q for arts, E for spells).
#[derive(Message, Clone, Copy, Debug)]
pub struct Bind {
    pub who: Entity,
    pub id: &'static str,
}

/// A cast in progress (with the `Action` it owns). Read from outside through `progress`.
#[derive(Component, Clone, Debug)]
pub struct Casting {
    ability: &'static Ability,
    t: f32,
    rate: f32,
    released: bool,
    mastered: bool,
    power: f32,
    aim: Vec3,
    /// What the caster aimed at (no target): the release re-aims at it from the hand.
    aim_point: Vec3,
    target: Option<Entity>,
    motes: Option<Entity>,
    /// Flash Step blinked (teleported) instead of dashing.
    blinked: bool,
    /// The player's cast: a skill shot, aimed where the crosshair points at the release, never led
    /// or homed onto a target.
    skill: bool,
}

impl Casting {
    /// The ability being cast and how far through its clip window (0..1), for the HUD.
    pub fn progress(&self) -> (&'static str, f32) {
        let span = (self.ability.end - self.ability.start).max(0.05);
        (self.ability.id, (self.t / span).clamp(0.0, 1.0))
    }
}

/// A flying spell or art.
#[derive(Component, Clone, Debug)]
pub struct Projectile {
    owner: Entity,
    team: u8,
    ability: &'static Ability,
    power: f32,
    flair: f32,
    velocity: Vec3,
    life: f32,
    hits_left: u32,
    struck: Vec<Entity>,
    /// Orbiting its owner (orbs) until this many seconds have passed: (age, phase).
    orbit: Option<(f32, f32)>,
    homing: Option<Entity>,
    /// Where a skill-shot orb launches after its orbit (the crosshair point at the release).
    aim: Option<Vec3>,
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Fighter, Energy>()
        .register_required_components::<Fighter, Grimoire>()
        .add_message::<Bind>()
        .add_systems(Update, demo.after(PlaySet::Input).before(PlaySet::Act))
        .add_systems(
            Update,
            (pools, bind, start_cast, blink, drive_cast, superhero_land, fly, ward, ward_mastery)
                .chain()
                .in_set(PlaySet::Act)
                .after(CombatSet),
        );
}

/// Pool sizes from stats, regeneration, Battlewill from blows.
fn pools(
    time: Res<Time>,
    mut hits: MessageReader<Hit>,
    mut rests: MessageReader<super::guild::Rested>,
    mut q: Query<(&mut Energy, &Grimoire, Option<&Derived>, &Fighter)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for r in rests.read() {
        if let Ok((mut e, ..)) = q.get_mut(r.who) {
            e.battlewill = e.max_battlewill;
            e.magicules = e.max_magicules;
        }
    }
    for hit in hits.read() {
        if hit.kind == HitKind::Melee
            && let Ok((mut e, ..)) = q.get_mut(hit.attacker)
        {
            e.battlewill = (e.battlewill + BW_ON_HIT).min(e.max_battlewill);
        }
        if let Ok((mut e, ..)) = q.get_mut(hit.target) {
            e.battlewill = (e.battlewill + BW_ON_HURT).min(e.max_battlewill);
        }
    }
    for (mut e, g, d, f) in &mut q {
        if let Some(d) = d {
            e.max_battlewill = d.max_battlewill;
            e.max_magicules = (d.max_mana - g.debt).max(20.0);
        }
        if f.dead {
            continue;
        }
        e.rested += dt;
        let mp_rate = MP_REGEN * d.map_or(1.0, |d| d.mana_regen);
        if e.magicules > e.max_magicules {
            // Over-mana (spells a monk caught, `discipline`) bleeds back down to the pool's top.
            e.magicules = (e.magicules - OVER_DECAY * dt).max(e.max_magicules);
        } else {
            if e.rested > MP_DELAY {
                e.magicules += mp_rate * dt;
            }
            e.magicules = e.magicules.min(e.max_magicules);
        }
        e.battlewill += BW_REST * dt;
        e.battlewill = e.battlewill.min(e.max_battlewill);
    }
}

/// Binding from the stats panel; MB5 cycles the bound spell.
fn bind(
    mut binds: MessageReader<Bind>,
    mut q: Query<(Entity, &mut Grimoire, &Sheet, &Intent)>,
    mut log: MessageWriter<HudLog>,
    players: Query<(), With<Player>>,
) {
    for b in binds.read() {
        let Ok((_, mut g, s, _)) = q.get_mut(b.who) else { continue };
        let Some(a) = ability(b.id).filter(|a| s.has_ability(a.id)) else { continue };
        if a.is_art() {
            g.art = a.id;
        } else {
            g.spell = a.id;
        }
    }
    for (e, mut g, s, intent) in &mut q {
        if !intent.next_spell {
            continue;
        }
        let spells: Vec<_> = known(&s.abilities, false).collect();
        if spells.is_empty() {
            continue;
        }
        let i = spells.iter().position(|a| a.id == g.spell).map_or(0, |i| (i + 1) % spells.len());
        g.spell = spells[i].id;
        if players.contains(e) {
            log.write(HudLog::new(LogKind::System, format!("Spell: {}", spells[i].name)).key(7001));
        }
    }
}

/// World position of a fighter's right hand (falls back to the chest).
fn hand(rig: Option<&Rig>, bank: &PoseBank, globals: &Query<&GlobalTransform>, tf: &Transform) -> Vec3 {
    rig.and_then(|r| r.bones.get(bank.bones.hand[1]).copied())
        .filter(|e| *e != Entity::PLACEHOLDER)
        .and_then(|e| globals.get(e).ok())
        .map(|g| g.translation())
        .filter(|p| p.is_finite())
        .unwrap_or(tf.translation + Vec3::Y * 0.5 + tf.forward() * 0.3)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn start_cast(
    mut commands: Commands,
    bank: Option<Res<PoseBank>>,
    vfx: Option<Res<Vfx>>,
    mut log: MessageWriter<HudLog>,
    mut q: Query<
        (
            Entity,
            &Transform,
            &Intent,
            &mut Action,
            &mut Mover,
            &mut Energy,
            &Grimoire,
            &Sheet,
            &Fighter,
            Option<&Derived>,
            Option<&Rig>,
            Has<Holding>,
            Has<Player>,
        ),
        Without<Casting>,
    >,
    globals: Query<&GlobalTransform>,
) {
    let (Some(bank), Some(vfx)) = (bank, vfx) else { return };
    for (entity, tf, intent, mut action, mut mover, mut energy, g, sheet, fighter, derived, rig, holding, player) in &mut q {
        let id = if intent.art_pressed {
            g.art
        } else if intent.spell_pressed {
            g.spell
        } else {
            continue;
        };
        let Some(a) = ability(id).filter(|a| sheet.has_ability(a.id)) else { continue };
        if fighter.dead || holding || !action.is_free() || !matches!(mover.mode, MoveMode::Ground | MoveMode::Air) {
            continue;
        }
        let mastered = g.is_mastered(a.id);
        let cost = a.cost * if mastered { MASTERED_COST } else { 1.0 };
        let pool = match a.pool {
            Pool::Battlewill => &mut energy.battlewill,
            Pool::Magicules => &mut energy.magicules,
        };
        if *pool < cost {
            if player {
                let what = if a.is_art() { "Not enough Battlewill" } else { "Not enough magicules" };
                log.write(HudLog::new(LogKind::Warn, what).key(7002));
            }
            continue;
        }
        *pool -= cost;
        if !a.is_art() {
            energy.rested = 0.0;
        }
        let d = derived.copied();
        let scale = if a.is_art() { d.map_or(1.0, |d| d.might) } else { d.map_or(1.0, |d| d.spell) };
        let cast_speed = if a.is_art() { 1.0 } else { d.map_or(1.0, |d| d.cast_speed) };
        let rate = a.speed * cast_speed * if mastered { MASTERED_SPEED } else { 1.0 };
        let origin = hand(rig, &bank, &globals, tf);
        // The player aims by hand (the crosshair's point); NPCs aim at their target.
        let target = if player { None } else { intent.target };
        let aim_point = target
            .and_then(|t| globals.get(t).ok())
            .map(|g| g.translation() + Vec3::Y * 0.2)
            .unwrap_or(intent.look);
        let aim = (aim_point - origin).normalize_or(tf.forward().as_vec3());
        let mask = if a.is_art() { Mask::Full } else { Mask::Upper };
        action.start(ActionKind::Cast, Owner::Magic, Some(ActionLayer::new(a.track, a.start, mask)));
        let yaw = aim.x.atan2(aim.z);
        mover.face = Some(yaw);
        mover.move_scale = if a.is_art() { 0.0 } else { CAST_MOVE };
        // Spells gather motes at the hand while the chant runs.
        let motes = (!a.is_art() && a.release > 0.0).then(|| {
            commands
                .spawn((
                    Name::new("Cast motes"),
                    vfx.cast(a.element),
                    Transform::from_translation(origin),
                ))
                .id()
        });
        commands.entity(entity).try_insert(Casting {
            ability: a,
            t: 0.0,
            rate,
            released: false,
            mastered,
            power: scale * if mastered { MASTERED_POWER } else { 1.0 },
            aim,
            aim_point,
            target,
            motes,
            blinked: false,
            skill: player,
        });
        if a.id != "veil" {
            commands.entity(entity).try_remove::<super::senses::Veiled>();
        }
    }
}

/// Everything a release needs from the world.
struct World<'a> {
    targets: Vec<Target>,
    vfx: &'a Vfx,
}

#[derive(Clone, Debug)]
struct Target {
    entity: Entity,
    /// Per-limb hurtboxes (`hurtbox`); empty for dummies.
    parts: Vec<super::hurtbox::Part>,
    center: Vec3,
    axis: [Vec3; 2],
    radius: f32,
    team: Option<u8>,
    alive: bool,
}

fn gather<F: bevy::ecs::query::QueryFilter>(
    fighters: &Query<(Entity, &Transform, &Fighter, Option<&super::hurtbox::Hurtboxes>), Without<Projectile>>,
    dummies: &Query<(Entity, &Transform), F>,
) -> Vec<Target> {
    let mut v: Vec<Target> = fighters
        .iter()
        .map(|(e, tf, f, boxes)| {
            let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
            Target {
                entity: e,
                parts: boxes.map(|b| b.0.clone()).unwrap_or_default(),
                center: tf.translation,
                axis: [feet + Vec3::Y * BODY_LOW, feet + Vec3::Y * BODY_HIGH],
                radius: BODY_RADIUS,
                team: Some(f.team),
                alive: !f.dead,
            }
        })
        .collect();
    v.extend(dummies.iter().map(|(e, tf)| Target {
        entity: e,
        parts: Vec::new(),
        center: tf.translation,
        axis: [tf.translation - Vec3::Y * DUMMY_HALF, tf.translation + Vec3::Y * DUMMY_HALF],
        radius: DUMMY_RADIUS,
        team: None,
        alive: true,
    }));
    v
}

fn foe(t: &Target, me: Entity, team: u8) -> bool {
    t.entity != me && t.alive && t.team != Some(team)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn drive_cast(
    time: Res<Time>,
    mut commands: Commands,
    bank: Option<Res<PoseBank>>,
    vfx: Option<Res<Vfx>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shake: ResMut<CameraShake>,
    mut monitor: ResMut<Monitor>,
    mut hits: MessageWriter<Hit>,
    mut deeds: MessageWriter<Deed>,
    mut casters: Query<(
        Entity,
        &Transform,
        &mut Action,
        &mut Mover,
        &mut Casting,
        &Fighter,
        &mut Grimoire,
        Option<&Rig>,
        Has<Player>,
        Option<&Intent>,
    )>,
    fighters: Query<(Entity, &Transform, &Fighter, Option<&super::hurtbox::Hurtboxes>), Without<Projectile>>,
    dummies: Query<(Entity, &Transform), With<TrainingDummy>>,
    globals: Query<&GlobalTransform>,
    mut motes: Query<&mut Transform, (Without<Fighter>, Without<Projectile>, Without<TrainingDummy>)>,
    vels: Res<super::aim::Velocities>,
) {
    let (Some(bank), Some(vfx)) = (bank, vfx) else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let world = World {
        targets: gather(&fighters, &dummies),
        vfx: &vfx,
    };
    for (entity, tf, mut action, mut mover, mut c, fighter, mut grimoire, rig, player, intent) in &mut casters {
        // Interrupted (a hit reaction took the body) or dead: drop the cast.
        if action.owner != Owner::Magic || action.kind != ActionKind::Cast || fighter.dead {
            if let Some(m) = c.motes {
                commands.entity(m).try_despawn();
            }
            commands.entity(entity).try_remove::<Casting>();
            continue;
        }
        let a = c.ability;
        let span = (a.end - a.start).max(0.05);
        c.t += dt * c.rate;
        let u = c.t / span;
        if let Some(layer) = action.layer.as_mut() {
            layer.time = a.start + c.t.min(span);
        }
        action.elapsed += dt;
        let origin = hand(rig, &bank, &globals, tf);
        if let Some(m) = c.motes
            && let Ok(mut mt) = motes.get_mut(m)
        {
            mt.translation = origin;
        }
        if let Shape::Step { distance } = a.shape {
            // The dash: aura blows the body along the aim; afterimage sparks trail it.
            let dir = Vec3::new(c.aim.x, 0.0, c.aim.z).normalize_or(tf.forward().as_vec3());
            let elapsed = action.elapsed;
            if c.blinked {
                // Already there (`blink`): just let the burst finish.
                if elapsed > STEP_TIME {
                    finish(&mut commands, entity, &mut action, &mut mover, &c);
                }
            } else if elapsed <= STEP_TIME {
                mover.root_motion = dir * (distance / STEP_TIME);
                vfx::burst(&mut commands, world.vfx.burst(Element::Aura), tf.translation, 0.5);
            } else {
                finish(&mut commands, entity, &mut action, &mut mover, &c);
                // Plant the stop: a quick landing on the spot.
                if mover.mode == MoveMode::Ground {
                    commands.entity(entity).try_insert(super::ragdoll::Acrobatics::land());
                }
            }
            continue;
        }
        mover.root_motion = Vec3::ZERO;
        if !c.released && u >= a.release {
            c.released = true;
            if let Some(m) = c.motes.take() {
                commands.entity(m).try_despawn();
            }
            // A skill shot goes where the crosshair points now, not where it pointed at the start.
            if c.skill
                && let Some(i) = intent
                && i.look.is_finite()
            {
                c.aim_point = i.look;
            }
            // Re-aim at the release from the hand: lead a target, lift for the drop.
            let (speed, gravity) = match a.shape {
                Shape::Bolt { speed, gravity, .. } => (speed, gravity),
                Shape::Crescent { speed, .. } => (speed, 0.0),
                _ => (0.0, 0.0),
            };
            if speed > 0.0 {
                let locked = c.target.and_then(|t| globals.get(t).ok().map(|g| (t, g.translation() + Vec3::Y * 0.2)));
                let mut dir = match locked {
                    Some((t, p)) => super::aim::lead(origin, p, vels.of(t), speed, gravity).0,
                    None => super::aim::ballistic(origin, c.aim_point, speed, gravity),
                };
                if matches!(a.shape, Shape::Crescent { .. }) {
                    // The crescent skims along the ground: keep its pitch, turn its heading.
                    dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or(c.aim.with_y(0.0)) * c.aim.with_y(0.0).length() + Vec3::Y * c.aim.y;
                }
                if dir.is_finite() && dir.length_squared() > 0.5 {
                    c.aim = dir.normalize();
                }
            }
            let team = fighter.team;
            let flair = if c.mastered { MASTERED_FLAIR } else { 1.0 };
            let landed = release(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut hits,
                &world,
                entity,
                team,
                tf,
                origin,
                &c,
                flair,
            );
            monitor.write(&format!("{:.2} - cast_{} - {landed} Ground Cast 0.00", time.elapsed_secs(), a.id));
            if player && matches!(a.shape, Shape::Quake { .. } | Shape::Lance { .. }) {
                shake.add(if matches!(a.shape, Shape::Quake { .. }) { 0.5 } else { 0.25 });
            }
            gain(&mut grimoire, a.id, landed);
            deeds.write(Deed::new(entity, "cast", a.id, 1.0));
            if landed > 0 {
                deeds.write(Deed::new(entity, "cast_hit", a.id, landed as f32));
            }
        }
        if u >= 1.0 {
            finish(&mut commands, entity, &mut action, &mut mover, &c);
        }
    }
}

fn finish(commands: &mut Commands, entity: Entity, action: &mut Action, mover: &mut Mover, c: &Casting) {
    if let Some(m) = c.motes {
        commands.entity(m).try_despawn();
    }
    action.clear();
    mover.move_scale = 1.0;
    mover.face = None;
    mover.root_motion = Vec3::ZERO;
    commands.entity(entity).try_remove::<Casting>();
}

/// Flash Step blink: farthest reach (m), how far from the target it lands (m), how far down an
/// air blink looks for ground (m), and the least drop that counts as one (m).
const BLINK_RANGE: f32 = 16.0;
const BLINK_GAP: f32 = 1.4;
const DROP_PROBE: f32 = 40.0;
const DROP_MIN: f32 = 0.5;

/// Where a blink to `target` (body centre, facing `target_fwd`) lands: behind it if that is clear,
/// else on the near side, else beside it. `clear(a, b)`: nothing between; `floor(p)`: the ground
/// under `p`. Returns the feet and the yaw that faces the target.
pub fn blink_spot(
    from: Vec3,
    target: Vec3,
    target_fwd: Vec3,
    mut clear: impl FnMut(Vec3, Vec3) -> bool,
    mut floor: impl FnMut(Vec3) -> Option<Vec3>,
) -> Option<(Vec3, f32)> {
    let fwd = target_fwd.with_y(0.0).normalize_or(Vec3::Z);
    let near = (from - target).with_y(0.0).normalize_or(-fwd);
    let quarter = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    for side in [-fwd, near, quarter * near, quarter.inverse() * near] {
        let spot = target + side * BLINK_GAP;
        if !(clear(from, spot) && clear(target, spot)) {
            continue;
        }
        let Some(feet) = floor(spot).filter(|f| (f.y - (target.y - super::HALF_HEIGHT)).abs() < 1.5) else { continue };
        let face = target - spot;
        return Some((feet, face.x.atan2(face.z)));
    }
    None
}

/// A blinked caster that has to land hard (blinked down from the air).
#[derive(Component)]
struct Superhero;

/// Flash Step as a blink: with a lock-on target in reach, teleport beside or behind it facing it;
/// in the air without one, drop straight to the ground ahead (a superhero landing follows).
/// Otherwise the step stays the dash `drive_cast` plays.
#[allow(clippy::type_complexity)]
fn blink(
    mut commands: Commands,
    vfx: Option<Res<Vfx>>,
    spatial: SpatialQuery,
    mut shake: ResMut<CameraShake>,
    mut casters: Query<(Entity, &Transform, &Mover, &mut Casting, Has<Player>)>,
    globals: Query<&GlobalTransform>,
    fighters: Query<&Fighter>,
) {
    let Some(vfx) = vfx else { return };
    let filter = SpatialQueryFilter::from_mask(super::Phys::World);
    let clear = |a: Vec3, b: Vec3| {
        let d = b - a;
        let len = d.length();
        Dir3::new(d).map_or(true, |dir| spatial.cast_ray(a, dir, len, true, &filter).is_none())
    };
    let floor = |p: Vec3| {
        let from = p + Vec3::Y * 1.5;
        spatial.cast_ray(from, Dir3::NEG_Y, DROP_PROBE, true, &filter).map(|h| from - Vec3::Y * h.distance)
    };
    for (e, tf, mover, mut c, player) in &mut casters {
        if c.blinked || !matches!(c.ability.shape, Shape::Step { .. }) {
            continue;
        }
        let from = tf.translation;
        let target = c
            .target
            .filter(|t| fighters.get(*t).map_or(true, |f| !f.dead))
            .and_then(|t| globals.get(t).ok())
            .map(|g| (g.translation(), g.rotation() * Vec3::Z))
            .filter(|(p, _)| p.distance(from) <= BLINK_RANGE);
        let (dest, hero) = match target {
            Some((at, fwd)) => (blink_spot(from, at, fwd, clear, floor), false),
            None if mover.mode == MoveMode::Air => {
                let ahead = Vec3::new(c.aim.x, 0.0, c.aim.z).normalize_or_zero() * 2.0;
                let down = floor(from + ahead).filter(|f| from.y - super::HALF_HEIGHT - f.y > DROP_MIN);
                (down.map(|f| (f, mover.yaw)), true)
            }
            None => (None, false),
        };
        let Some((feet, yaw)) = dest else { continue };
        c.blinked = true;
        commands.entity(e).try_insert(super::controller::Teleport { feet, yaw });
        vfx::burst(&mut commands, vfx.burst(Element::Aura), from, 0.7);
        vfx::burst(&mut commands, vfx.burst(Element::Aura), feet + Vec3::Y * super::HALF_HEIGHT, 0.9);
        if hero {
            commands.entity(e).try_insert(Superhero);
        }
        if player {
            shake.add(0.15);
        }
        info!("[magic] blink {} ({:.1}, {:.1}, {:.1})", if hero { "down to" } else { "to target at" }, feet.x, feet.y, feet.z);
    }
}

/// After an air blink: touch down in the deep hard-land crouch with a burst and a thud.
#[allow(clippy::type_complexity)]
fn superhero_land(
    mut commands: Commands,
    vfx: Option<Res<Vfx>>,
    mut shake: ResMut<CameraShake>,
    mut deeds: MessageWriter<Deed>,
    mut bodies: Query<
        (Entity, &Transform, &Mover, &mut Action, Option<&mut super::parkour::Landing>, Has<Player>),
        (With<Superhero>, Without<Casting>),
    >,
) {
    let Some(vfx) = vfx else { return };
    for (e, tf, mover, mut action, landing, player) in &mut bodies {
        if mover.mode != MoveMode::Ground {
            continue;
        }
        commands.entity(e).try_remove::<Superhero>();
        if let Some(mut l) = landing
            && action.is_free()
        {
            super::parkour::superhero(&mut action, &mut l);
        }
        let feet = tf.translation - Vec3::Y * super::HALF_HEIGHT;
        vfx::burst(&mut commands, vfx.burst(Element::Aura), feet + Vec3::Y * 0.1, 1.6);
        if player {
            shake.add(0.5);
        }
        deeds.write(Deed::new(e, "land", "Hard", 1.0));
        info!("[magic] superhero landing at ({:.1}, {:.1}, {:.1})", feet.x, feet.y, feet.z);
    }
}

/// Add mastery for `n` landed blows.
fn gain(g: &mut Grimoire, id: &'static str, n: u32) {
    if n == 0 {
        return;
    }
    let before = g.mastery(id);
    let after = (before + MASTERY_PER_HIT * n as f32).min(1.0);
    g.mastery.insert(id, after);
    if before < 1.0 && after >= 1.0 {
        g.debt += MASTERED_DEBT;
    }
}

fn spell_hit(owner: Entity, a: &Ability, power: f32, target: Entity, point: Vec3, dir: Vec3, speed: f32) -> Hit {
    Hit {
        attacker: owner,
        target,
        point,
        direction: Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::Z),
        damage: a.damage * power,
        poise: a.poise * power,
        knockback: a.knockback,
        speed,
        kind: HitKind::Spell,
    }
}

/// The ability's effect at its release point; returns the blows landed right away.
#[allow(clippy::too_many_arguments)]
fn release(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    hits: &mut MessageWriter<Hit>,
    world: &World,
    owner: Entity,
    team: u8,
    tf: &Transform,
    origin: Vec3,
    c: &Casting,
    flair: f32,
) -> u32 {
    let a = c.ability;
    let mut landed = 0;
    match a.shape {
        Shape::Bolt { speed, .. } | Shape::Crescent { speed, .. } => {
            let crescent = matches!(a.shape, Shape::Crescent { .. });
            let from = if crescent { tf.translation + Vec3::Y * 0.2 + c.aim * 0.6 } else { origin };
            let mut e = commands.spawn((
                Name::new(a.name),
                Projectile {
                    owner,
                    team,
                    ability: a,
                    power: c.power,
                    flair,
                    velocity: c.aim * speed,
                    life: 2.5,
                    hits_left: if crescent { 3 } else { 1 },
                    struck: Vec::new(),
                    orbit: None,
                    homing: None,
                    aim: None,
                },
                Transform::from_translation(from).with_rotation(Quat::from_rotation_arc(Vec3::Z, c.aim)),
                Visibility::default(),
            ));
            e.with_child((world.vfx.trail(a.element), Transform::default()));
            if crescent {
                let color = a.element.core() * 1.4;
                e.with_child((
                    Mesh3d(meshes.add(vfx::crescent(0.95 * flair, 2.3, 0.32 * flair))),
                    MeshMaterial3d(vfx::glow(materials, color)),
                    Transform::from_translation(-Vec3::Z * 0.6),
                    bevy::light::NotShadowCaster,
                ));
            } else {
                let color = a.element.core();
                e.with_child((
                    Mesh3d(meshes.add(Sphere::new(0.12 * flair))),
                    MeshMaterial3d(vfx::glow(materials, color * 1.5)),
                    Transform::default(),
                    bevy::light::NotShadowCaster,
                ));
                e.with_child((
                    PointLight {
                        color: Color::LinearRgba(color.with_alpha(1.0) * (1.0 / color.red.max(color.green).max(color.blue))),
                        intensity: 40_000.0,
                        range: 6.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::default(),
                ));
            }
        }
        Shape::Quake { radius } => {
            let feet = tf.translation - Vec3::Y * (HALF_HEIGHT - 0.05);
            let r = radius * flair;
            for t in world.targets.iter().filter(|t| foe(t, owner, team)) {
                let away = Vec3::new(t.center.x - feet.x, 0.0, t.center.z - feet.z);
                if away.length() <= r + t.radius {
                    hits.write(spell_hit(owner, a, c.power, t.entity, t.center, away, 12.0));
                    landed += 1;
                }
            }
            let color = a.element.core() * 1.3;
            commands.spawn((
                Name::new("Shock ring"),
                Mesh3d(meshes.add(vfx::ring(0.75, 1.0))),
                MeshMaterial3d(vfx::glow(materials, color)),
                Transform::from_translation(feet).with_scale(Vec3::splat(0.6)),
                Flash {
                    age: 0.0,
                    life: 0.45,
                    grow: 7.0 * flair,
                    color,
                },
                bevy::light::NotShadowCaster,
            ));
            commands.spawn((
                world.vfx.shock(),
                Transform::from_translation(feet).with_scale(Vec3::splat(flair)),
                Expire { left: 1.6 },
            ));
            vfx::impact(commands, meshes, materials, world.vfx, Element::Aura, feet + Vec3::Y * 0.3, flair * 1.5);
        }
        Shape::Lance { range, chains } => {
            // Strike the foe nearest the aim within the cone, then leap to the next ones.
            let mut from = origin;
            let mut dir = c.aim;
            let mut struck: Vec<Entity> = Vec::new();
            let locked = c.target.and_then(|t| world.targets.iter().find(|x| x.entity == t && x.alive));
            let mut next = locked.cloned().or_else(|| {
                world
                    .targets
                    .iter()
                    .filter(|t| foe(t, owner, team))
                    .filter(|t| {
                        let to = t.center - from;
                        to.length() < range && to.normalize_or_zero().dot(dir) > if c.skill { SKILL_LANCE_CONE } else { LANCE_CONE }
                    })
                    .min_by(|x, y| (x.center - from).length().total_cmp(&(y.center - from).length()))
                    .cloned()
            });
            let mut seed = owner.to_bits() as u32 ^ (c.t.to_bits());
            for jump in 0..=chains {
                let Some(t) = next.take() else {
                    if jump == 0 {
                        // Nothing in reach: the bolt strikes the ground ahead.
                        let end = from + dir * range * 0.6 - Vec3::Y * 1.0;
                        vfx::spawn_bolt(commands, meshes, materials, from, end, seed);
                        vfx::impact(commands, meshes, materials, world.vfx, Element::Lightning, end, flair);
                    }
                    break;
                };
                let point = t.center + Vec3::Y * 0.3;
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                vfx::spawn_bolt(commands, meshes, materials, from, point, seed);
                vfx::impact(commands, meshes, materials, world.vfx, Element::Lightning, point, flair);
                dir = (point - from).normalize_or(dir);
                let falloff = 0.7f32.powi(jump as i32);
                hits.write(spell_hit(owner, a, c.power * falloff, t.entity, point, dir, 18.0));
                landed += 1;
                struck.push(t.entity);
                from = point;
                next = world
                    .targets
                    .iter()
                    .filter(|x| foe(x, owner, team) && !struck.contains(&x.entity))
                    .filter(|x| x.center.distance(from) < CHAIN_RANGE)
                    .min_by(|x, y| x.center.distance(from).total_cmp(&y.center.distance(from)))
                    .cloned();
            }
        }
        Shape::Orbs { count, .. } => {
            for i in 0..count {
                let phase = i as f32 / count as f32 * std::f32::consts::TAU;
                let mut e = commands.spawn((
                    Name::new("Orbit mote"),
                    Projectile {
                        owner,
                        team,
                        ability: a,
                        power: c.power,
                        flair,
                        velocity: Vec3::ZERO,
                        life: ORBIT_TIME + 3.0,
                        hits_left: 1,
                        struck: Vec::new(),
                        orbit: Some((0.0, phase)),
                        homing: c.target,
                        aim: c.skill.then_some(c.aim_point),
                    },
                    Transform::from_translation(origin),
                    Visibility::default(),
                ));
                e.with_child((world.vfx.trail(a.element), Transform::default()));
                e.with_child((
                    Mesh3d(meshes.add(Sphere::new(0.09 * flair))),
                    MeshMaterial3d(vfx::glow(materials, a.element.core() * 1.6)),
                    Transform::default(),
                    bevy::light::NotShadowCaster,
                ));
            }
        }
        Shape::Step { .. } => {}
        Shape::Veil { duration } => {
            commands.entity(owner).try_insert(super::senses::Veiled(duration * flair));
            vfx::burst(commands, world.vfx.burst(Element::Arcane), tf.translation, 0.6);
        }
        Shape::Ward { duration, share } => {
            // A faint tint and three bright bands spinning on tilted axes (a gyroscope of aura).
            let tint = vfx::glow(materials, a.element.core() * 0.07);
            let band = vfx::glow(materials, a.element.core() * 0.9);
            let ring = meshes.add(vfx::ring(WARD_RADIUS - 0.025, WARD_RADIUS));
            let shell = commands
                .spawn((
                    Name::new("Ward shell"),
                    WardShell { owner, age: 0.0 },
                    Mesh3d(meshes.add(Sphere::new(WARD_RADIUS))),
                    MeshMaterial3d(tint),
                    Transform::from_xyz(0.0, WARD_LIFT, 0.0),
                    ChildOf(owner),
                    bevy::light::NotShadowCaster,
                ))
                .with_children(|sh| {
                    for (i, tilt) in WARD_TILTS.into_iter().enumerate() {
                        sh.spawn((
                            WardBand {
                                axis: Quat::from_euler(EulerRot::XYZ, tilt[0].to_radians(), 0.0, tilt[1].to_radians()),
                                speed: WARD_SPIN * if i % 2 == 0 { 1.0 } else { -1.3 },
                            },
                            Mesh3d(ring.clone()),
                            MeshMaterial3d(band.clone()),
                            Transform::default(),
                            bevy::light::NotShadowCaster,
                        ));
                    }
                })
                .id();
            commands.entity(owner).try_insert(Ward {
                left: duration * flair,
                share: (share * if flair > 1.0 { 1.15 } else { 1.0 }).min(0.85),
                shell,
            });
            vfx::impact(commands, meshes, materials, world.vfx, Element::Aura, tf.translation + Vec3::Y * WARD_LIFT, 1.4 * flair);
        }
    }
    landed
}

/// Ward shell: radius (m) and height of its centre above the body's centre; band tilts (deg about
/// x, z) and spin (rad/s).
const WARD_RADIUS: f32 = 0.95;
const WARD_LIFT: f32 = 0.1;
const WARD_TILTS: [[f32; 2]; 3] = [[0.0, 0.0], [62.0, 20.0], [-58.0, -35.0]];
const WARD_SPIN: f32 = 2.4;

/// One spinning band of a ward shell.
#[derive(Component)]
struct WardBand {
    axis: Quat,
    speed: f32,
}

/// A blow turned by the ward (`ward` deed from `combat`) masters it as a landed blow does an
/// attack.
fn ward_mastery(mut deeds: MessageReader<Deed>, mut grimoires: Query<&mut Grimoire>) {
    for d in deeds.read().filter(|d| d.kind == "ward") {
        if let Ok(mut g) = grimoires.get_mut(d.who) {
            gain(&mut g, "aura_ward", 1);
        }
    }
}

/// Wards run down; shells breathe, shrink away at the end, and go with their ward.
fn ward(
    time: Res<Time>,
    mut commands: Commands,
    mut wards: Query<(Entity, &mut Ward, &Fighter)>,
    mut shells: Query<(Entity, &mut WardShell, &mut Transform), Without<WardBand>>,
    mut bands: Query<(&WardBand, &mut Transform), Without<WardShell>>,
) {
    for (b, mut tf) in &mut bands {
        tf.rotation = b.axis * Quat::from_rotation_y(b.speed * time.elapsed_secs());
    }
    let dt = time.delta_secs();
    for (e, mut w, f) in &mut wards {
        w.left -= dt;
        if w.left <= 0.0 || f.dead {
            commands.entity(e).try_remove::<Ward>();
            commands.entity(w.shell).try_despawn();
        }
    }
    for (e, mut shell, mut tf) in &mut shells {
        shell.age += dt;
        let Ok((_, w, _)) = wards.get(shell.owner) else {
            commands.entity(e).try_despawn();
            continue;
        };
        if w.shell != e {
            commands.entity(e).try_despawn();
            continue;
        }
        let grow = (shell.age / 0.15).min(1.0);
        let fade = (w.left / 0.4).clamp(0.0, 1.0);
        let breathe = 1.0 + 0.04 * (shell.age * 7.0).sin();
        tf.scale = Vec3::splat((grow * fade * breathe).max(0.01));
    }
}

/// Lissajous orbit offset around the owner's chest at `t` s for a mote starting at `phase`:
/// x = R sin(3t + φ), z = R sin(2t + φ), y = h sin(5t + φ) — a figure that never repeats on the
/// short time it lives, so the three motes weave around each other.
pub fn lissajous(t: f32, phase: f32) -> Vec3 {
    let r = 0.9;
    let w = 4.0;
    Vec3::new(r * (3.0 * w * t + phase).sin(), 0.3 * (5.0 * w * t + phase).sin(), r * (2.0 * w * t + phase).sin())
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn fly(
    time: Res<Time>,
    mut commands: Commands,
    vfx: Option<Res<Vfx>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    spatial: SpatialQuery,
    mut hits: MessageWriter<Hit>,
    mut projectiles: Query<(Entity, &mut Projectile, &mut Transform)>,
    fighters: Query<(Entity, &Transform, &Fighter, Option<&super::hurtbox::Hurtboxes>), Without<Projectile>>,
    dummies: Query<(Entity, &Transform), (With<TrainingDummy>, Without<Projectile>)>,
    mut grimoires: Query<&mut Grimoire>,
    mut monitor: ResMut<Monitor>,
    mut deeds: MessageWriter<Deed>,
) {
    let Some(vfx) = vfx else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let targets = gather(&fighters, &dummies);
    let filter = SpatialQueryFilter::from_mask(super::Phys::World);
    for (entity, mut p, mut tf) in &mut projectiles {
        p.life -= dt;
        let prev = tf.translation;
        let a = p.ability;
        let speed = match a.shape {
            Shape::Bolt { speed, .. } | Shape::Crescent { speed, .. } | Shape::Orbs { speed, .. } => speed,
            _ => 10.0,
        };
        // Orbit, then launch toward the target (or forward).
        if let Some((age, phase)) = p.orbit {
            let owner = targets.iter().find(|t| t.entity == p.owner).map(|t| t.center + Vec3::Y * 0.3);
            let age = age + dt;
            if age < ORBIT_TIME
                && let Some(c) = owner
            {
                tf.translation = c + lissajous(age, phase);
                p.orbit = Some((age, phase));
                continue;
            }
            p.orbit = None;
            let aim = p
                .homing
                .and_then(|h| targets.iter().find(|t| t.entity == h))
                .map(|t| t.center)
                .or(p.aim)
                .or_else(|| {
                    targets
                        .iter()
                        .filter(|t| foe(t, p.owner, p.team))
                        .min_by(|x, y| x.center.distance(prev).total_cmp(&y.center.distance(prev)))
                        .map(|t| t.center)
                })
                .unwrap_or(prev + Vec3::Y);
            p.velocity = (aim - prev).normalize_or(Vec3::Y) * speed;
        }
        // Homing turn.
        if let Some(h) = p.homing
            && let Some(t) = targets.iter().find(|t| t.entity == h && t.alive)
        {
            let want = (t.center - prev).normalize_or(p.velocity.normalize_or(Vec3::Z));
            let have = p.velocity.normalize_or(want);
            let turned = have.slerp(want, (HOMING * dt).min(1.0));
            p.velocity = turned * speed;
        }
        if let Shape::Bolt { gravity, .. } = a.shape {
            p.velocity.y -= gravity * dt;
        }
        let next = prev + p.velocity * dt;
        tf.translation = next;
        if p.velocity.length_squared() > 1e-6 {
            tf.rotation = Quat::from_rotation_arc(Vec3::Z, p.velocity.normalize());
        }
        let radius = match a.shape {
            Shape::Bolt { radius, .. } | Shape::Crescent { radius, .. } => radius,
            _ => 0.15,
        } * p.flair;
        // Bodies.
        let mut landed = 0;
        let mut burst_at = None;
        let (owner, team) = (p.owner, p.team);
        for t in targets.iter().filter(|t| foe(t, owner, team)) {
            if p.hits_left == 0 || p.struck.contains(&t.entity) {
                continue;
            }
            let (d, s, _) = segment_distance(prev, next, t.axis[0], t.axis[1]);
            if d <= radius + t.radius {
                let mut point = prev.lerp(next, s);
                // The limb it actually crosses, or it flies past between them.
                let mut scale = 1.0;
                if !t.parts.is_empty() {
                    let Some((_, at, region)) = super::hurtbox::refine([prev, prev], [next, next], radius, &t.parts) else { continue };
                    point = at;
                    scale = region.damage();
                }
                let mut hit = spell_hit(p.owner, a, p.power, t.entity, point, p.velocity, p.velocity.length());
                hit.damage *= scale;
                hits.write(hit);
                p.struck.push(t.entity);
                p.hits_left -= 1;
                landed += 1;
                burst_at = Some(point);
            }
        }
        // The world.
        let travel = next - prev;
        if burst_at.is_none()
            && let Ok(dir) = Dir3::new(travel)
            && let Some(hit) = spatial.cast_ray(prev, dir, travel.length(), true, &filter)
        {
            burst_at = Some(prev + dir * hit.distance);
            p.hits_left = 0;
        }
        if let Some(at) = burst_at {
            monitor.write(&format!(
                "{:.2} - spell_impact_{} - {landed} ({:.1} {:.1} {:.1}) Cast {:.2}",
                time.elapsed_secs(),
                a.id,
                at.x,
                at.y,
                at.z,
                p.velocity.length()
            ));
            vfx::impact(&mut commands, &mut meshes, &mut materials, &vfx, a.element, at, p.flair);
            // Frost blooms: the rosette strikes everyone around the impact.
            if let Shape::Bolt { bloom, .. } = a.shape
                && bloom > 0.0
            {
                let struck = p.struck.clone();
                for t in targets.iter().filter(|t| foe(t, owner, team) && !struck.contains(&t.entity)) {
                    if t.center.distance(at) <= bloom * p.flair + t.radius {
                        let dir = t.center - at;
                        hits.write(spell_hit(p.owner, a, p.power * 0.7, t.entity, t.center, dir, 10.0));
                        landed += 1;
                    }
                }
                p.hits_left = 0;
            }
        }
        if landed > 0 {
            deeds.write(Deed::new(p.owner, "cast_hit", a.id, landed as f32));
            if let Ok(mut g) = grimoires.get_mut(p.owner) {
                gain(&mut g, a.id, landed);
            }
        }
        if p.hits_left == 0 || p.life <= 0.0 {
            // Let the trail's particles finish: detach and fade the head.
            commands.entity(entity).try_despawn();
        }
    }
}

/// Smoke-test driver: `GB_CAST=cinder_bolt,storm_lance,...` makes the player (with those abilities
/// discovered) lock the nearest training dummy and cast those abilities in turn, one every `GB_CAST_EVERY` s (default
/// 1.6) from 2 s on, with full pools and no NPCs about. For headless screenshots of the effects.
fn demo(
    time: Res<Time>,
    mut commands: Commands,
    opponents: Query<Entity, With<super::Npc>>,
    mut state: Local<Option<(Vec<&'static Ability>, usize, f32)>>,
    mut players: Query<(&Transform, &mut Intent, &mut Grimoire, &mut Energy, &mut Sheet), With<Player>>,
    dummies: Query<(Entity, &Transform), (With<TrainingDummy>, Without<Player>)>,
) {
    let st = state.get_or_insert_with(|| {
        let list = std::env::var("GB_CAST")
            .unwrap_or_default()
            .split(',')
            .filter_map(|id| ability(id.trim()))
            .collect();
        let every = std::env::var("GB_CAST_EVERY").ok().and_then(|s| s.parse().ok()).unwrap_or(1.6);
        (list, 0, every)
    });
    if st.0.is_empty() {
        return;
    }
    // A clear stage: NPCs (who would interrupt the casts) are kept far off.
    for npc in &opponents {
        commands.entity(npc).try_insert(super::controller::Teleport {
            feet: Vec3::new(40.0, 0.0, 40.0),
            yaw: 0.0,
        });
    }
    if time.elapsed_secs() < 2.0 {
        return;
    }
    let Ok((tf, mut intent, mut g, mut e, mut sheet)) = players.single_mut() else { return };
    // GB_AIR_BLINK=1: jump just before each cast and cast with no target (an air blink).
    let air = std::env::var("GB_AIR_BLINK").is_ok_and(|v| v.trim() == "1");
    for a in &st.0 {
        if !sheet.has_ability(a.id) {
            sheet.abilities.push(a.id);
        }
    }
    e.battlewill = e.max_battlewill;
    e.magicules = e.max_magicules;
    let due = ((time.elapsed_secs() - 2.0) / st.2) as usize;
    if air {
        // Jump so the cast comes at the top of the jump.
        let lead = 2.0 + st.1 as f32 * st.2 - time.elapsed_secs();
        if lead <= 0.3 && lead > 0.3 - time.delta_secs() {
            intent.jump_pressed = true;
        }
        // Held, so the jump is not cut short.
        intent.jump_held = lead > 0.0 && lead <= 0.3;
    }
    if due < st.1 {
        return;
    }
    let a = st.0[st.1 % st.0.len()];
    st.1 = due + 1;
    let target = dummies
        .iter()
        .min_by(|x, y| x.1.translation.distance(tf.translation).total_cmp(&y.1.translation.distance(tf.translation)));
    if let Some((e, t)) = target.filter(|_| !air) {
        intent.target = Some(e);
        intent.look = t.translation;
    }
    if air {
        intent.target = None;
    }
    if a.is_art() {
        g.art = a.id;
        intent.art_pressed = true;
    } else {
        g.spell = a.id;
        intent.spell_pressed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_consistent() {
        for a in &ABILITIES {
            assert!(a.cost > 0.0, "{}", a.id);
            assert!(a.end > a.start && (0.0..=1.0).contains(&a.release), "{}", a.id);
            assert!(ability(a.id).is_some_and(|b| std::ptr::eq(a, b)));
            assert_eq!(a.is_art(), a.element == Element::Aura, "{}: arts are Battlewill", a.id);
        }
        let g = Grimoire::default();
        let start = super::super::skills::STARTING;
        assert!(ability(g.art).is_some_and(|a| a.is_art() && start.contains(&a.id)));
        assert!(ability(g.spell).is_some_and(|a| !a.is_art() && start.contains(&a.id)));
    }

    #[test]
    fn every_cast_clip_is_in_the_bank() {
        let (bank, _) = PoseBank::build_with(std::path::Path::new("__no_poses__.json")).expect("bank");
        for a in &ABILITIES {
            assert!(bank.has(a.track), "{}: {}", a.id, a.track);
            assert!(a.end <= bank.duration(a.track) + 1e-3, "{}: window past the clip", a.id);
        }
    }

    #[test]
    fn mastery_happens_once_and_costs_magicules() {
        let mut g = Grimoire::default();
        for _ in 0..19 {
            gain(&mut g, "cinder_bolt", 1);
        }
        assert!(!g.is_mastered("cinder_bolt"));
        gain(&mut g, "cinder_bolt", 1);
        assert!(g.is_mastered("cinder_bolt"));
        assert_eq!(g.debt, MASTERED_DEBT);
        gain(&mut g, "cinder_bolt", 5);
        assert_eq!(g.debt, MASTERED_DEBT, "only once");
    }

    #[test]
    fn ward_is_an_art_that_never_strikes() {
        let w = ability("aura_ward").expect("ward");
        assert!(w.is_art());
        assert!(matches!(w.shape, Shape::Ward { share, .. } if share > 0.0 && share < 1.0));
        assert_eq!(w.damage, 0.0);
    }

    #[test]
    fn known_follows_discovery() {
        let start = super::super::skills::STARTING;
        assert_eq!(known(&start, true).map(|a| a.id).collect::<Vec<_>>(), ["crescent_edge"]);
        assert_eq!(known(&start, false).map(|a| a.id).collect::<Vec<_>>(), ["cinder_bolt"]);
        let all: Vec<&'static str> = ABILITIES.iter().map(|a| a.id).collect();
        assert_eq!(known(&all, true).count() + known(&all, false).count(), ABILITIES.len());
        assert_eq!(known(&[], true).count(), 0);
    }

    #[test]
    fn lissajous_stays_around_the_chest() {
        for i in 0..100 {
            let o = lissajous(i as f32 * 0.01, 1.0);
            assert!(o.length() < 1.4 && o.y.abs() <= 0.3);
        }
    }
}
