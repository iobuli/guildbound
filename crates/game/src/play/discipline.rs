//! Discipline: what a fighter has mastered and how far, and what that does in a fight.
//! - `Mastery { path, rank }` on every fighter. The player's path is the class picked
//!   (`class::ClassChoice`) and its rank the guild rank (`contracts::Board`, F..S = 0..6); an NPC's
//!   path is Monk for the order (`looks::is_monk` names), else follows its weapon (blades → Blade,
//!   bow → Hunter, magic → Arcanist, bare hands → none) and is recomputed when the weapon changes;
//!   its rank follows its level (`stats::Stats`, every `LEVELS_PER_RANK` levels).
//! - Monk: fists hit harder than any weapon in his hands (`weapon_scale`), his guard is a
//!   defensive state that turns aside more and costs less (`guard_bonus`), and a guard raised
//!   inside the enemy's blow (the parry window, wider for a monk) is a **perfect defence**.
//!   Against a blade, the held guard is a deflect: two fingers laid on the blade's path
//!   (`DeflectReach`, `Form::TwoFinger`), damage reduced. The perfect one depends on who attacks
//!   (`perfect_outcome`):
//!   - bare hands → **Counter**: the riposte (the blow's damage × `Counter.damage` × the monk's
//!     fists) lands and the attacker is `Broken` for the counter's `stun` s;
//!   - a weapon, attacker within `SEIZE_GAP` ranks → **Disarm**: the weapon flies from his hand
//!     (`Disarm`, a dropped blade he takes back after `DISARM_RECOVER` s); no damage, he fights on
//!     bare-handed;
//!   - a weapon, attacker `SEIZE_GAP`+ ranks below → **Seize**: the monk grips the blade and
//!     strikes; the attacker staggers weaponless and is `Broken` for `BROKEN` s. No ragdoll.
//!   A monk striking a `Broken` foe lands a **finisher** (`combat`: ×3 damage, at least 40% of
//!   its max hp, the body thrown).
//! - Every path at max rank (S) gets its own counter, the same rule shown differently
//!   (`Flare`): Monk a golden palm burst, Blade a white iai flash, Arcanist a frost bloom that
//!   holds longer, Hunter a green snare that leaves the attacker open longer.
//! - Rank shows: from rank D a wind shell (the `WindAura` shader) wraps the fighter, brighter
//!   and wider each rank, coloured by path.
//! - Rank is felt: humans who notice a fighter `FLEE_GAP` or more ranks above them run instead
//!   of fighting (`ai`); demons (`Species::Demon`) try their luck whatever the gap.
//! - Monk hands (`hands`, through `select::HandForm` into the hand rig's `Form`s): fists while
//!   fighting bare-handed, a cupped lead palm and a rear fist on guard, soft cupped hands at ease.
//!   His guard also catches what flies at him (`combat`): arrows are flicked aside between two
//!   fingers, spells cupped and drunk as mana (`combat::ABSORB_SHARE` of the spell's damage, the pool
//!   may run over to `magic::OVER_CAP` × its maximum); a countered blade is caught
//!   between two fingers (`HandFlash`).
//! Log lines: `[discipline] <who> counters|disarms|seizes the blade of <whom> (<flare>, …)`,
//! `[discipline] <who> loses the <weapon> to <whom>`, `[discipline] finisher on <whom>`, `[discipline] <who> flees`.
use super::fx::{Burst, BurstKind, Sfx, SfxKind};
use avian3d::prelude::{AngularVelocity, Collider, CollisionLayers, Friction, LinearVelocity, RigidBody};
use super::select::HandForm;
use crate::motion::hand::Form;
use super::wind_aura::{WindAura, WindAuraParams};
use super::{Fighter, Player};
use bevy::prelude::*;

/// Ranks F..S.
pub const MAX_RANK: u8 = 6;
/// An NPC gains a rank every this many levels.
pub const LEVELS_PER_RANK: u32 = 3;
/// Seconds a countered attacker stays stunned.
pub const STUN: f32 = 1.5;
/// Ranks above a human that make it run.
pub const FLEE_GAP: u8 = 2;
/// The aura shows from this rank (D).
pub const AURA_FROM: u8 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Path {
    #[default]
    None,
    Monk,
    Blade,
    Arcanist,
    Hunter,
}

impl Path {
    /// The path an NPC walks: monks of the order (`looks::is_monk`: Brother, Sister, Elder,
    /// Novice) are Monks whatever they hold; anyone else follows the weapon. Bare hands alone don't
    /// make a monk (a brawler or a disarmed duelist has no path).
    pub fn of_npc(name: &str, set: &str, kind: Option<super::moveset::Kind>) -> Path {
        use super::moveset::Kind;
        if super::looks::is_monk(name) {
            return Path::Monk;
        }
        match kind {
            Some(Kind::Unarmed) => Path::None,
            Some(Kind::Ranged) => Path::Hunter,
            Some(Kind::Magic) => Path::Arcanist,
            _ if ["katana", "longsword", "shortsword", "knight_sword", "claymore", "greatsword", "dagger"].contains(&set) => Path::Blade,
            _ => Path::None,
        }
    }

    pub fn colour(self) -> Vec3 {
        match self {
            Path::Monk => Vec3::new(1.0, 0.78, 0.3),
            Path::Blade => Vec3::new(0.92, 0.95, 1.0),
            Path::Arcanist => Vec3::new(0.45, 0.75, 1.0),
            Path::Hunter => Vec3::new(0.5, 1.0, 0.55),
            Path::None => Vec3::new(0.85, 0.85, 0.8),
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mastery {
    pub path: Path,
    pub rank: u8,
}

/// Human or demon: demons don't run from rank.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Species {
    #[default]
    Human,
    Demon,
}

/// Broken by a perfect defence: staggering, open; a monk's next blow on it is a finisher.
#[derive(Component, Clone, Copy, Debug)]
pub struct Broken {
    pub left: f32,
}

/// What a perfect defence does to the attacker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Bare-handed attacker: riposte, briefly Broken.
    Counter,
    /// Weapon, attacker close in rank: knocked from the hand.
    Disarm,
    /// Weapon, attacker far below: blade seized, attacker struck and Broken, weaponless.
    Seize,
}

/// The weapon leaves this fighter's hand (`combat` on a disarm or seize; applied by `disarm`).
#[derive(Component, Clone, Debug)]
pub struct Disarm {
    pub set: String,
    /// Flat direction the weapon flies.
    pub toward: Vec3,
    /// Who took it.
    pub by: Entity,
}

/// The deflecting hand reaches for the point the blade crossed (`DeflectReach`, `HandTargets`).
#[derive(Component, Clone, Copy, Debug)]
pub struct DeflectReach {
    pub point: Vec3,
    pub left: f32,
}

/// Ranks below the defender at which a perfect defence seizes the blade instead of disarming.
pub const SEIZE_GAP: u8 = 2;
/// Seconds a seized attacker stays Broken.
pub const BROKEN: f32 = 3.5;
/// Seconds before a disarmed fighter has its weapon back.
pub const DISARM_RECOVER: f32 = 8.0;

/// What a perfect defence by `defender` does to `attacker`.
pub fn perfect_outcome(defender: Mastery, attacker: Mastery, attacker_unarmed: bool) -> Outcome {
    if attacker_unarmed {
        Outcome::Counter
    } else if defender.rank >= attacker.rank.saturating_add(SEIZE_GAP) {
        Outcome::Seize
    } else {
        Outcome::Disarm
    }
}

/// How a counter shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flare {
    Palm,
    Iai,
    Frost,
    Snare,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Counter {
    /// Riposte damage as a share of the blow that was countered.
    pub damage: f32,
    /// Seconds the attacker is stunned.
    pub stun: f32,
    pub flare: Flare,
}

/// A counter happened (`combat` → here for the flare).
#[derive(Message, Clone, Copy, Debug)]
pub struct Countered {
    pub by: Entity,
    pub at: Vec3,
    pub flare: Flare,
    pub path: Path,
}

/// Damage multiplier for a fighter of `m` striking with fists (`unarmed`) or a weapon.
pub fn weapon_scale(m: Mastery, unarmed: bool) -> f32 {
    match (m.path, unarmed) {
        (Path::Monk, true) => 1.35 + 0.05 * m.rank as f32,
        (Path::Monk, false) => 0.75,
        _ => 1.0,
    }
}

/// Fidelity by rank: the chance a light or heavy blow is drawn from the mocap strike library
/// instead of the moveset's own (`combat::start_swing`). Novices repeat their few moves; masters
/// rarely throw the same blow twice; a monk lives in the library.
pub fn variety(m: Mastery) -> f32 {
    let base = super::repertoire::VARIANT_CHANCE + 0.07 * m.rank as f32;
    let monk = if m.path == Path::Monk { 0.25 } else { 0.0 };
    (base + monk).min(0.9)
}

/// Guard bonus: (extra share absorbed, extra parry window s, stamina cost multiplier).
pub fn guard_bonus(m: Mastery) -> (f32, f32, f32) {
    match m.path {
        Path::Monk => (0.25, 0.08 + 0.02 * m.rank as f32, 0.6),
        _ if m.rank >= MAX_RANK => (0.05, 0.04, 0.9),
        _ => (0.0, 0.0, 1.0),
    }
}

/// The counter a parry becomes for `m`: always for a monk (stronger each rank), at max rank
/// for every other path.
pub fn counter(m: Mastery) -> Option<Counter> {
    let r = m.rank as f32;
    match m.path {
        Path::Monk => Some(Counter { damage: 0.8 + 0.1 * r, stun: STUN, flare: Flare::Palm }),
        _ if m.rank < MAX_RANK => None,
        Path::Blade => Some(Counter { damage: 1.6, stun: STUN, flare: Flare::Iai }),
        Path::Arcanist => Some(Counter { damage: 0.8, stun: STUN + 0.75, flare: Flare::Frost }),
        Path::Hunter => Some(Counter { damage: 1.0, stun: STUN + 0.5, flare: Flare::Snare }),
        Path::None => Some(Counter { damage: 1.0, stun: STUN, flare: Flare::Iai }),
    }
}

/// Does a human of rank `own` run from someone of rank `other`?
pub fn flees(species: Species, own: u8, other: u8) -> bool {
    species == Species::Human && other >= own.saturating_add(FLEE_GAP)
}

/// Rank of an NPC at `level`.
pub fn rank_of_level(level: u32) -> u8 {
    ((level.saturating_sub(1)) / LEVELS_PER_RANK).min(MAX_RANK as u32) as u8
}

/// A hand shape held for a moment (a catch, a counter): overrides the fighting hands.
#[derive(Component, Clone, Copy, Debug)]
pub struct HandFlash {
    pub form: Form,
    pub left: f32,
}

/// Mana drunk from caught spells this frame (applied by `absorb` into `magic::Energy`). Spells
/// caught in the same frame add up (`Absorb::add`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Absorb(pub f32);

impl Absorb {
    /// Queue `amount` more mana for `who`, adding to anything caught earlier this frame.
    pub fn add(commands: &mut Commands, who: Entity, amount: f32) {
        commands.entity(who).entry::<Absorb>().or_default().and_modify(move |mut a| a.0 += amount);
    }
}

pub fn plugin(app: &mut App) {
    app.add_message::<Countered>()
        .add_systems(Update, (assign, player_rank, tick_broken, flare, flare_fade, rank_aura, absorb))
        .add_systems(Update, (disarm, recover_weapons, deflect_reach, finisher_prompt))
        .add_systems(Update, hands.after(super::PlaySet::Act));
}

/// What a fighter's free hands should be: `[left, right]`.
pub fn fighting_hands(path: Path, unarmed: bool, guarding: bool, engaged: bool, flash: Option<Form>) -> [Option<(Form, f32)>; 2] {
    if let Some(form) = flash {
        return match form {
            // The catch is the right hand's; the left stays a guard fist.
            Form::TwoFinger => [Some((Form::Fist, 0.9)), Some((Form::TwoFinger, 1.0))],
            f => [Some((f, 1.0)), Some((f, 1.0))],
        };
    }
    if !unarmed {
        return [None, None];
    }
    match (path, guarding, engaged) {
        // The monk's guard: lead palm open and cupped to turn the blow, rear fist ready.
        (Path::Monk, true, _) => [Some((Form::Cupped, 0.9)), Some((Form::Fist, 0.9))],
        (_, true, _) | (_, _, true) => [Some((Form::Fist, 0.95)), Some((Form::Fist, 0.95))],
        // At ease a monk's hands stay soft.
        (Path::Monk, false, false) => [Some((Form::Cupped, 0.5)), Some((Form::Cupped, 0.5))],
        _ => [None, None],
    }
}

/// Shape the free hands: fists for anyone fighting bare-handed, the monk's cupped guard, and a
/// moment's catch shape (`HandFlash`).
#[allow(clippy::type_complexity)]
fn hands(
    time: Res<Time>,
    mut commands: Commands,
    movesets: Option<Res<super::moveset::Movesets>>,
    mut fighters: Query<(
        Entity,
        &Mastery,
        &Fighter,
        &super::Loadout,
        &super::combat::Melee,
        Option<&super::Intent>,
        Option<&mut HandFlash>,
        Option<&mut HandForm>,
    )>,
) {
    let dt = time.delta_secs();
    for (e, m, fighter, loadout, melee, intent, flash, form) in &mut fighters {
        let mut shown = None;
        if let Some(mut f) = flash {
            f.left -= dt;
            if f.left <= 0.0 {
                commands.entity(e).remove::<HandFlash>();
            } else {
                shown = Some(f.form);
            }
        }
        let want = if fighter.dead {
            [None, None]
        } else {
            let unarmed = movesets
                .as_deref()
                .and_then(|s| s.get(loadout.equipped()))
                .is_some_and(|s| s.kind == super::moveset::Kind::Unarmed);
            let engaged = melee.current_attack().is_some() || intent.is_some_and(|i| i.target.is_some());
            fighting_hands(m.path, unarmed, melee.guarding(), engaged, shown)
        };
        match form {
            Some(mut f) if f.0 != want => f.0 = want,
            Some(_) => {}
            None if want != [None, None] => {
                commands.entity(e).try_insert(HandForm(want));
            }
            None => {}
        }
    }
}

/// Pour caught spells into the pool, past its top (over-mana, `magic::OVER_CAP`).
fn absorb(mut commands: Commands, mut q: Query<(Entity, &Absorb, Option<&mut super::magic::Energy>)>, mut log: MessageWriter<super::hud::log::HudLog>, players: Query<(), With<Player>>) {
    for (e, a, energy) in &mut q {
        commands.entity(e).remove::<Absorb>();
        let Some(mut energy) = energy else { continue };
        let cap = energy.max_magicules * super::magic::OVER_CAP;
        energy.magicules = (energy.magicules + a.0).min(cap.max(energy.magicules));
        if players.contains(e) {
            log.write(super::hud::log::HudLog::new(super::hud::log::LogKind::System, format!("You drink the spell: {:.0} / {:.0} mana", energy.magicules, energy.max_magicules)).key(7600));
        }
    }
}

/// Give each NPC fighter its mastery (path from its name or weapon, rank from its level), and
/// recompute the path whenever its weapon changes (armory, pickup, disarm). A rank already set
/// (at spawn) is kept. The player's comes from `player_rank`.
#[allow(clippy::type_complexity)]
fn assign(
    mut commands: Commands,
    movesets: Option<Res<super::moveset::Movesets>>,
    mut fighters: Query<
        (Entity, &super::Loadout, Option<&super::stats::Stats>, Option<&Name>, Option<&mut Mastery>),
        (With<Fighter>, Without<Player>, Or<(Without<Mastery>, Changed<super::Loadout>)>),
    >,
    players: Query<Entity, (With<Player>, Without<Mastery>)>,
) {
    for e in &players {
        commands.entity(e).try_insert(Mastery::default());
    }
    for (e, loadout, stats, name, mastery) in &mut fighters {
        let set = loadout.equipped();
        let kind = movesets.as_deref().and_then(|m| m.get(set)).map(|s| s.kind);
        let path = Path::of_npc(name.map_or("", |n| n.as_str()), set, kind);
        match mastery {
            Some(mut m) => {
                if m.path != path {
                    m.path = path;
                }
            }
            None => {
                let rank = stats.map_or(0, |s| rank_of_level(s.level));
                commands.entity(e).try_insert(Mastery { path, rank });
            }
        }
    }
}

/// The player's path is the class chosen and the rank the guild's card.
fn player_rank(
    choice: Option<Res<super::class::ClassChoice>>,
    board: Option<Res<super::contracts::Board>>,
    mut players: Query<&mut Mastery, With<Player>>,
) {
    let Ok(mut m) = players.single_mut() else { return };
    let path = choice.and_then(|c| c.picked).map_or(Path::None, |c| c.path());
    // `GB_RANK=<0..6>` overrides the card for smoke runs.
    let forced = std::env::var("GB_RANK").ok().and_then(|s| s.trim().parse::<u8>().ok());
    let rank = forced.unwrap_or_else(|| board.map_or(0, |b| b.rank() as u8)).min(MAX_RANK);
    let want = Mastery { path, rank };
    if *m != want {
        *m = want;
    }
}

fn tick_broken(time: Res<Time>, mut commands: Commands, mut broken: Query<(Entity, &mut Broken)>) {
    for (e, mut s) in &mut broken {
        s.left -= time.delta_secs();
        if s.left <= 0.0 {
            commands.entity(e).remove::<Broken>();
        }
    }
}

/// A weapon on the ground after a disarm; its owner takes it back after `left` s.
#[derive(Component)]
struct DroppedWeapon {
    owner: Entity,
    set: String,
    left: f32,
}

/// Take the weapon out of the hand (the fighter fights on bare-handed) and throw a blade on the
/// ground where it flew.
fn disarm(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fighters: Query<(Entity, &Disarm, &mut super::Loadout, &Transform, Option<&Name>)>,
    names: Query<&Name>,
) {
    for (e, d, mut loadout, tf, name) in &mut fighters {
        commands.entity(e).remove::<Disarm>();
        if d.set.is_empty() || d.set == "unarmed" || loadout.equipped() != d.set {
            continue;
        }
        let index = loadout.slots.iter().position(|s| s == "unarmed").unwrap_or_else(|| {
            loadout.slots.push("unarmed".into());
            loadout.slots.len() - 1
        });
        loadout.current = index;
        let steel = materials.add(StandardMaterial { base_color: Color::srgb(0.75, 0.77, 0.8), metallic: 0.9, perceptual_roughness: 0.3, ..default() });
        let at = tf.translation + Vec3::Y * 0.5 + d.toward * 0.4;
        let spin = Quat::from_rotation_z(1.2) * Quat::from_rotation_y(d.toward.x.atan2(d.toward.z));
        commands.spawn((
            Name::new(format!("Dropped {}", d.set)),
            DroppedWeapon { owner: e, set: d.set.clone(), left: DISARM_RECOVER },
            Mesh3d(meshes.add(Cuboid::new(0.035, 0.9, 0.01))),
            MeshMaterial3d(steel),
            Transform::from_translation(at).with_rotation(spin),
            RigidBody::Dynamic,
            Collider::cuboid(0.035, 0.9, 0.02),
            CollisionLayers::new(super::Phys::Debris, [super::Phys::World, super::Phys::Debris]),
            LinearVelocity(d.toward * 3.0 + Vec3::Y * 3.5),
            AngularVelocity(Vec3::new(4.0, 0.0, 9.0)),
            Friction::new(0.9),
        ));
        let by = names.get(d.by).map_or("someone", |n| n.as_str());
        info!("[discipline] {} loses the {} to {by}", name.map_or("?", |n| n.as_str()), d.set);
    }
}

/// After `DISARM_RECOVER` s the disarmed fighter has its weapon back (equipped once it is free).
fn recover_weapons(
    time: Res<Time>,
    mut commands: Commands,
    mut dropped: Query<(Entity, &mut DroppedWeapon)>,
    mut equip: MessageWriter<super::combat::Equip>,
    alive: Query<&Fighter>,
) {
    for (e, mut d) in &mut dropped {
        d.left -= time.delta_secs();
        if d.left > 0.0 {
            continue;
        }
        if alive.get(d.owner).is_ok_and(|f| !f.dead) {
            equip.write(super::combat::Equip { who: d.owner, set: d.set.clone() });
        }
        commands.entity(e).despawn();
    }
}

/// Marks `HandTargets` this module put on (so it only removes its own).
#[derive(Component)]
struct ReachOwned;

/// The deflecting (right) hand reaches out to where the blade crossed, rising and easing back
/// over the reach's life, unless something else already drives the hands.
fn deflect_reach(
    time: Res<Time>,
    mut commands: Commands,
    mut reaching: Query<(Entity, &mut DeflectReach, Option<&super::select::HandTargets>, Has<ReachOwned>)>,
) {
    for (e, mut r, targets, owned) in &mut reaching {
        r.left -= time.delta_secs();
        if r.left <= 0.0 {
            commands.entity(e).remove::<DeflectReach>();
            if owned {
                commands.entity(e).remove::<(super::select::HandTargets, ReachOwned)>();
            }
            continue;
        }
        if targets.is_some() && !owned {
            continue;
        }
        // Up fast, back slow.
        let w = (r.left / 0.35).clamp(0.0, 1.0) * 0.85;
        commands.entity(e).try_insert((
            super::select::HandTargets { hands: [None, Some((r.point, w))], grips: [None, None] },
            ReachOwned,
        ));
    }
}

/// A monk player sees when a foe is Broken and open to a finisher.
fn finisher_prompt(
    players: Query<(&Transform, &Mastery), With<Player>>,
    broken: Query<(&Transform, Option<&Name>), Added<Broken>>,
    mut log: MessageWriter<super::hud::log::HudLog>,
) {
    let Ok((me, m)) = players.single() else { return };
    if m.path != Path::Monk {
        return;
    }
    for (tf, name) in &broken {
        if tf.translation.distance(me.translation) < 4.0 {
            let who = name.map_or("Your foe", |n| n.as_str());
            log.write(super::hud::log::HudLog::new(super::hud::log::LogKind::System, format!("{who} is broken: strike to finish")).key(7610));
        }
    }
}

#[derive(Component)]
struct FlareShell {
    colour: Vec3,
    age: f32,
    life: f32,
    material: Handle<WindAura>,
}

/// The counter's flash: a burst shell in the path's colour, plus sparks and a sound.
fn flare(
    mut commands: Commands,
    mut events: MessageReader<Countered>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindAura>>,
    mut bursts: MessageWriter<Burst>,
    mut sfx: MessageWriter<Sfx>,
) {
    for c in events.read() {
        let (colour, life, count) = match c.flare {
            Flare::Palm => (c.path.colour(), 0.45, 40),
            Flare::Iai => (Vec3::new(1.0, 1.0, 1.0), 0.3, 60),
            Flare::Frost => (Vec3::new(0.5, 0.85, 1.0), 0.8, 30),
            Flare::Snare => (Vec3::new(0.45, 1.0, 0.5), 0.6, 30),
        };
        let material = materials.add(WindAura {
            params: WindAuraParams {
                color: (colour * 1.6).extend(1.0),
                wind: Vec4::new(0.0, 1.0, 6.0, 2.2),
                clock: Vec4::new(0.0, 4.0, 0.0, 0.0),
            },
        });
        commands.spawn((
            Name::new("Counter flare"),
            FlareShell { colour: colour * 1.6, age: 0.0, life, material: material.clone() },
            Mesh3d(meshes.add(Sphere::new(1.0))),
            MeshMaterial3d(material),
            Transform::from_translation(c.at).with_scale(Vec3::splat(0.3)),
            bevy::light::NotShadowCaster,
        ));
        bursts.write(Burst { kind: BurstKind::Sparks, at: c.at, dir: Vec3::ZERO, count });
        sfx.write(Sfx { kind: SfxKind::Parry, at: c.at, volume: 1.0 });
    }
}

fn flare_fade(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<WindAura>>,
    mut shells: Query<(Entity, &mut FlareShell, &mut Transform)>,
) {
    for (e, mut f, mut tf) in &mut shells {
        f.age += time.delta_secs();
        let u = (f.age / f.life).clamp(0.0, 1.0);
        tf.scale = Vec3::splat(0.3 + 1.6 * (1.0 - (1.0 - u) * (1.0 - u)));
        if let Some(mut m) = materials.get_mut(&f.material) {
            // Additive: fading the colour fades the shell.
            m.params.color = (f.colour * (1.0 - u)).extend(1.0);
        }
        if u >= 1.0 {
            commands.entity(e).despawn();
        }
    }
}

/// The rank aura shell on a fighter (child), and the rank it was made for.
#[derive(Component)]
struct RankAura {
    shell: Entity,
    mastery: Mastery,
}

/// Wrap ranked fighters in their aura; rebuild it when the rank or path changes.
#[allow(clippy::type_complexity)]
fn rank_aura(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindAura>>,
    fighters: Query<(Entity, &Mastery, &Fighter, Option<&RankAura>), Changed<Mastery>>,
    mut dead: Query<(Entity, &Fighter, &RankAura, Has<Visibility>), Changed<Fighter>>,
    mut shells: Query<&mut Visibility>,
) {
    for (e, m, _, aura) in &fighters {
        if let Some(a) = aura {
            if a.mastery == *m {
                continue;
            }
            commands.entity(a.shell).try_despawn();
            commands.entity(e).remove::<RankAura>();
        }
        if m.rank < AURA_FROM {
            continue;
        }
        let k = (m.rank - AURA_FROM + 1) as f32 / (MAX_RANK - AURA_FROM + 1) as f32;
        let material = materials.add(WindAura {
            params: WindAuraParams {
                // Dim at D, bright at S.
                color: (m.path.colour() * (0.25 + 0.75 * k)).extend(1.0),
                wind: Vec4::new(0.0, 1.0, 1.5 + 2.5 * k, 1.2 + k),
                clock: Vec4::new(0.0, 1.5 + 2.0 * k, 0.0, 0.0),
            },
        });
        let shell = commands
            .spawn((
                Name::new("Rank aura"),
                Mesh3d(meshes.add(Capsule3d::new(0.42 + 0.12 * k, 1.1))),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, 0.0, 0.0),
                ChildOf(e),
                bevy::light::NotShadowCaster,
            ))
            .id();
        commands.entity(e).try_insert(RankAura { shell, mastery: *m });
    }
    // The dead don't glow.
    for (_, fighter, aura, _) in &mut dead {
        if let Ok(mut v) = shells.get_mut(aura.shell) {
            let want = if fighter.dead { Visibility::Hidden } else { Visibility::Inherited };
            if *v != want {
                *v = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(path: Path, rank: u8) -> Mastery {
        Mastery { path, rank }
    }

    #[test]
    fn monk_fists_beat_his_weapons() {
        assert!(weapon_scale(m(Path::Monk, 0), true) > 1.0);
        assert!(weapon_scale(m(Path::Monk, 0), false) < 1.0);
        assert!(weapon_scale(m(Path::Monk, 6), true) > weapon_scale(m(Path::Monk, 0), true));
        assert_eq!(weapon_scale(m(Path::Blade, 6), true), 1.0);
    }

    #[test]
    fn counters_are_monk_or_max_rank() {
        assert!(counter(m(Path::Monk, 0)).is_some());
        for p in [Path::Blade, Path::Arcanist, Path::Hunter] {
            assert!(counter(m(p, MAX_RANK - 1)).is_none(), "{p:?}");
            let c = counter(m(p, MAX_RANK)).expect("max rank counters");
            assert!(c.stun >= STUN);
        }
        // Same rule, shown differently.
        let flares: Vec<_> = [Path::Monk, Path::Blade, Path::Arcanist, Path::Hunter]
            .map(|p| counter(m(p, MAX_RANK)).unwrap().flare)
            .to_vec();
        assert_eq!(flares, vec![Flare::Palm, Flare::Iai, Flare::Frost, Flare::Snare]);
    }

    #[test]
    fn variety_rises_with_rank_and_monks_most() {
        assert!(variety(m(Path::Blade, 6)) > variety(m(Path::Blade, 0)));
        assert!(variety(m(Path::Monk, 0)) > variety(m(Path::Blade, 0)));
        assert!(variety(m(Path::Monk, 6)) <= 0.9);
    }

    #[test]
    fn monk_guard_is_a_defensive_state() {
        let (absorb, window, cost) = guard_bonus(m(Path::Monk, 0));
        assert!(absorb > 0.0 && window > 0.0 && cost < 1.0);
        assert_eq!(guard_bonus(m(Path::Hunter, 2)), (0.0, 0.0, 1.0));
    }

    #[test]
    fn hands_follow_the_fight() {
        assert_eq!(fighting_hands(Path::Blade, false, true, true, None), [None, None]);
        assert_eq!(fighting_hands(Path::None, true, false, true, None), [Some((Form::Fist, 0.95)); 2]);
        assert_eq!(fighting_hands(Path::Monk, true, true, true, None)[0].map(|f| f.0), Some(Form::Cupped));
        // A catch shows even with a weapon in hand (it is the off hand's moment).
        assert_eq!(fighting_hands(Path::Monk, false, true, true, Some(Form::TwoFinger))[1].map(|f| f.0), Some(Form::TwoFinger));
    }

    #[test]
    fn only_the_order_are_monks() {
        use super::super::moveset::Kind;
        assert_eq!(Path::of_npc("Brother Ishin", "unarmed", Some(Kind::Unarmed)), Path::Monk);
        assert_eq!(Path::of_npc("Elder Tadao", "katana", Some(Kind::Melee)), Path::Monk);
        // A bare-handed brawler or a disarmed duelist is no monk.
        assert_eq!(Path::of_npc("Opponent", "unarmed", Some(Kind::Unarmed)), Path::None);
        assert_eq!(Path::of_npc("Opponent", "katana", Some(Kind::Melee)), Path::Blade);
        assert_eq!(Path::of_npc("Hunter", "bow", Some(Kind::Ranged)), Path::Hunter);
    }

    #[test]
    fn perfect_defence_disarms_close_ranks_and_seizes_far_lower() {
        let monk = m(Path::Monk, 3);
        assert_eq!(perfect_outcome(monk, m(Path::Blade, 3), true), Outcome::Counter);
        assert_eq!(perfect_outcome(monk, m(Path::Blade, 3), false), Outcome::Disarm);
        assert_eq!(perfect_outcome(monk, m(Path::Blade, 2), false), Outcome::Disarm);
        assert_eq!(perfect_outcome(monk, m(Path::Blade, 1), false), Outcome::Seize);
        assert_eq!(perfect_outcome(monk, m(Path::None, 0), false), Outcome::Seize);
        // A stronger attacker is only ever disarmed.
        assert_eq!(perfect_outcome(m(Path::Monk, 0), m(Path::Blade, 6), false), Outcome::Disarm);
    }

    #[test]
    fn humans_run_from_rank_and_demons_dont() {
        assert!(flees(Species::Human, 0, 2));
        assert!(!flees(Species::Human, 1, 2));
        assert!(!flees(Species::Demon, 0, 6));
        assert_eq!(rank_of_level(1), 0);
        assert_eq!(rank_of_level(4), 1);
        assert_eq!(rank_of_level(99), MAX_RANK);
    }
}
