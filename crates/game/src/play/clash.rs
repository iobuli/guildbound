//! Charge clash (after Budokai 3's): two foes who both release a fully charged heavy into each
//! other within `WINDOW` s, within `RANGE` m, lock into a super-speed exchange.
//!
//! - Both get `Clashing`: `combat` drops the heavies and chains light attacks at `SPEED` × their
//!   attack speed, facing each other, untouchable by any blow (so the exchange deals nothing).
//! - For `DURATION` s each side builds a mash score: the player by tapping primary (MB1), any
//!   other fighter (an NPC, or the player on `ai::Autopilot`) at `AI_RATE` taps per second with a
//!   per-entity tempo. A bar at the top of the screen shows the tug of war while the player is in it.
//! - At the end the side with more taps lands one blow on the other: `DAMAGE` × (0.6 + margin),
//!   where margin is the winning share over an even split, with knockdown poise and knockback.
//!   A tie knocks both back. Sparks fly between them through the exchange.
//!
//! `GB_CLASH=<seconds>` starts a clash between the player and the nearest foe at that time (smoke
//! tests and recordings). Monitor log lines: `[clash] start`, `[clash] end`.
use super::ai::Autopilot;
use super::camera::CameraShake;
use super::combat::{CombatSet, Melee};
use super::fx::{Burst, BurstKind};
use super::hud::log::{HudLog, LogKind};
use super::{Fighter, Hit, HitKind, Intent, Player};
use bevy::prelude::*;

/// Seconds between the two full-charge releases, and the farthest apart (m) they may be.
pub const WINDOW: f32 = 0.6;
pub const RANGE: f32 = 5.0;
/// Length of the exchange (s), and its attack-speed multiplier.
pub const DURATION: f32 = 3.0;
pub const SPEED: f32 = 2.2;
/// Taps per second for a side not mashed by a person.
pub const AI_RATE: f32 = 7.0;
/// The winning blow at an even split; poise and knockback (m/s) it carries.
pub const DAMAGE: f32 = 30.0;
const POISE: f32 = 999.0;
const KNOCKBACK: f32 = 9.0;
/// Seconds between spark bursts, and the camera shake each adds while the player is in it.
const SPARK_EVERY: f32 = 0.18;
const SHAKE: f32 = 0.08;

/// A fighter locked in a clash with `foe`; `speed` multiplies its attack speed.
#[derive(Component, Clone, Copy, Debug)]
pub struct Clashing {
    pub foe: Entity,
    pub speed: f32,
}

/// One running clash.
#[derive(Clone, Debug)]
pub struct Clash {
    pub sides: [Entity; 2],
    pub t: f32,
    pub taps: [f32; 2],
    spark: f32,
}

impl Clash {
    pub fn new(a: Entity, b: Entity) -> Self {
        Self { sides: [a, b], t: 0.0, taps: [0.0; 2], spark: 0.0 }
    }
}

#[derive(Resource, Default)]
pub struct Clashes(pub Vec<Clash>);

/// A fighter that could clash: (entity, position, team, seconds since its full-charge release).
pub type Candidate = (Entity, Vec3, u8, f32);

/// Pairs of foes that both released a full charge within `WINDOW` s and `RANGE` m; each fighter
/// in at most one pair, nearest pairs first.
pub fn pairs(cands: &[Candidate]) -> Vec<(Entity, Entity)> {
    let ready: Vec<&Candidate> = cands.iter().filter(|c| c.3 <= WINDOW).collect();
    let mut options = Vec::new();
    for (i, a) in ready.iter().enumerate() {
        for b in &ready[i + 1..] {
            let d = a.1.distance(b.1);
            if a.2 != b.2 && d <= RANGE && (a.3 - b.3).abs() <= WINDOW {
                options.push((d, a.0, b.0));
            }
        }
    }
    options.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut used = Vec::new();
    let mut out = Vec::new();
    for (_, a, b) in options {
        if !used.contains(&a) && !used.contains(&b) {
            used.extend([a, b]);
            out.push((a, b));
        }
    }
    out
}

/// The winner's side (None on a tie) and its margin: winning share over an even split, 0..0.5.
pub fn outcome(taps: [f32; 2]) -> (Option<usize>, f32) {
    let total = taps[0] + taps[1];
    if !(total > 0.0) || (taps[0] - taps[1]).abs() < 0.5 {
        return (None, 0.0);
    }
    let w = if taps[0] > taps[1] { 0 } else { 1 };
    (Some(w), taps[w] / total - 0.5)
}

/// Damage of the winning blow for a margin (0..0.5).
pub fn blow_damage(margin: f32) -> f32 {
    DAMAGE * (0.6 + margin.clamp(0.0, 0.5) * 2.0)
}

/// Taps per second for a side nobody mashes: `AI_RATE`, ±15 % by entity so two NPCs differ.
pub fn ai_rate(entity: Entity) -> f32 {
    let h = (entity.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f32 / (1u64 << 24) as f32;
    AI_RATE * (0.85 + 0.3 * h)
}

#[derive(Component)]
struct ClashBar;
#[derive(Component)]
struct ClashFill;

pub fn plugin(app: &mut App) {
    app.init_resource::<Clashes>().add_systems(Update, (detect, smoke, tick, bar).chain().after(CombatSet));
}

fn begin(commands: &mut Commands, clashes: &mut Clashes, log: &mut MessageWriter<HudLog>, a: Entity, b: Entity, names: [&str; 2]) {
    commands.entity(a).try_insert(Clashing { foe: b, speed: SPEED });
    commands.entity(b).try_insert(Clashing { foe: a, speed: SPEED });
    clashes.0.push(Clash::new(a, b));
    info!("[clash] start {} vs {}", names[0], names[1]);
    log.write(HudLog::new(LogKind::Warn, format!("CLASH! {} vs {}: mash LMB!", names[0], names[1])));
}

#[allow(clippy::type_complexity)]
fn detect(
    mut commands: Commands,
    mut clashes: ResMut<Clashes>,
    mut log: MessageWriter<HudLog>,
    fighters: Query<(Entity, &Transform, &Fighter, &Melee, Option<&Name>), Without<Clashing>>,
) {
    let cands: Vec<Candidate> =
        fighters.iter().filter(|(_, _, f, ..)| !f.dead).map(|(e, tf, f, m, _)| (e, tf.translation, f.team, m.since_primed())).collect();
    for (a, b) in pairs(&cands) {
        let name = |e: Entity| fighters.get(e).ok().and_then(|r| r.4).map_or("?".to_string(), |n| n.to_string());
        let (na, nb) = (name(a), name(b));
        begin(&mut commands, &mut clashes, &mut log, a, b, [&na, &nb]);
    }
}

/// `GB_CLASH=<seconds>`: the player and the nearest live foe clash at that time.
#[allow(clippy::type_complexity)]
fn smoke(
    time: Res<Time>,
    mut done: Local<bool>,
    mut commands: Commands,
    mut clashes: ResMut<Clashes>,
    mut log: MessageWriter<HudLog>,
    players: Query<(Entity, &Transform, &Fighter), (With<Player>, Without<Clashing>)>,
    others: Query<(Entity, &Transform, &Fighter, Option<&Name>), (Without<Player>, Without<Clashing>)>,
) {
    if *done {
        return;
    }
    let Some(at) = std::env::var("GB_CLASH").ok().and_then(|v| v.trim().parse::<f32>().ok()) else {
        *done = true;
        return;
    };
    if time.elapsed_secs() < at {
        return;
    }
    let Ok((me, tf, f)) = players.single() else { return };
    let foe = others
        .iter()
        .filter(|(_, _, o, _)| !o.dead && o.team != f.team)
        .min_by(|a, b| a.1.translation.distance(tf.translation).total_cmp(&b.1.translation.distance(tf.translation)));
    if let Some((e, _, _, name)) = foe {
        *done = true;
        let n = name.map_or("?".to_string(), |n| n.to_string());
        begin(&mut commands, &mut clashes, &mut log, me, e, ["Player", &n]);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tick(
    time: Res<Time>,
    mut commands: Commands,
    mut clashes: ResMut<Clashes>,
    mut hits: MessageWriter<Hit>,
    mut bursts: MessageWriter<Burst>,
    mut log: MessageWriter<HudLog>,
    mut shake: ResMut<CameraShake>,
    fighters: Query<(&Transform, &Fighter, Option<&Intent>, Has<Player>, Has<Autopilot>, Option<&Name>)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    clashes.0.retain_mut(|c| {
        let (Ok(a), Ok(b)) = (fighters.get(c.sides[0]), fighters.get(c.sides[1])) else {
            for e in c.sides {
                commands.entity(e).try_remove::<Clashing>();
            }
            return false;
        };
        let rows = [a, b];
        c.t += dt;
        let mut human = false;
        for (side, (_, _, intent, player, autopilot, _)) in rows.iter().enumerate() {
            if *player && !*autopilot {
                human = true;
                if intent.is_some_and(|i| i.primary_pressed) {
                    c.taps[side] += 1.0;
                }
            } else {
                c.taps[side] += ai_rate(c.sides[side]) * dt;
            }
        }
        let (pa, pb) = (rows[0].0.translation, rows[1].0.translation);
        c.spark -= dt;
        if c.spark <= 0.0 {
            c.spark = SPARK_EVERY;
            bursts.write(Burst { kind: BurstKind::Sparks, at: (pa + pb) * 0.5 + Vec3::Y * 1.2, dir: Vec3::Y, count: 14 });
            if human || rows.iter().any(|r| r.3) {
                shake.add(SHAKE);
            }
        }
        let over = c.t >= DURATION || rows[0].1.dead || rows[1].1.dead;
        if !over {
            return true;
        }
        for e in c.sides {
            commands.entity(e).try_remove::<Clashing>();
        }
        let name = |i: usize| rows[i].5.map_or(if rows[i].3 { "Player" } else { "?" }.to_string(), |n| n.to_string());
        let (winner, margin) = outcome(c.taps);
        let blow = |from: usize, to: usize, damage: f32, poise: f32| {
            let (p, q) = (rows[from].0.translation, rows[to].0.translation);
            let dir = (q - p).with_y(0.0).normalize_or(Vec3::X);
            Hit { attacker: c.sides[from], target: c.sides[to], point: q + Vec3::Y * 1.2, direction: dir, damage, poise, knockback: KNOCKBACK, speed: 14.0, kind: HitKind::Melee }
        };
        match winner {
            Some(w) => {
                let l = 1 - w;
                hits.write(blow(w, l, blow_damage(margin), POISE));
                info!("[clash] end {} wins {:.0}:{:.0} over {}", name(w), c.taps[w], c.taps[l], name(l));
                log.write(HudLog::new(LogKind::Info, format!("{} wins the clash ({:.0} to {:.0})", name(w), c.taps[w], c.taps[l])));
            }
            None => {
                hits.write(blow(0, 1, 0.0, 0.0));
                hits.write(blow(1, 0, 0.0, 0.0));
                info!("[clash] end tie {:.0}:{:.0}", c.taps[0], c.taps[1]);
                log.write(HudLog::new(LogKind::Info, "The clash ends even."));
            }
        }
        false
    });
}

/// The tug-of-war bar while the player is in a clash: the player's share fills from the left.
fn bar(
    mut commands: Commands,
    clashes: Res<Clashes>,
    players: Query<Entity, With<Player>>,
    bars: Query<Entity, With<ClashBar>>,
    mut fill: Query<&mut Node, With<ClashFill>>,
) {
    let mine = players.iter().find_map(|p| clashes.0.iter().find(|c| c.sides.contains(&p)).map(|c| (p, c)));
    let Some((p, c)) = mine else {
        for e in &bars {
            commands.entity(e).despawn();
        }
        return;
    };
    let side = if c.sides[0] == p { 0 } else { 1 };
    let total = c.taps[0] + c.taps[1];
    let share = if total > 0.0 { c.taps[side] / total } else { 0.5 };
    if bars.is_empty() {
        commands
            .spawn((
                ClashBar,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(70),
                    left: Val::Percent(30.0),
                    width: Val::Percent(40.0),
                    height: px(18),
                    border: UiRect::all(px(2)),
                    ..default()
                },
                BorderColor::all(Color::WHITE),
                BackgroundColor(Color::srgb(0.75, 0.15, 0.12)),
            ))
            .with_child((ClashFill, Node { width: Val::Percent(50.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgb(0.2, 0.55, 0.95))));
    }
    for mut n in &mut fill {
        n.width = Val::Percent(share * 100.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(i: u32) -> Entity {
        Entity::from_raw_u32(i).unwrap()
    }

    #[test]
    fn two_foes_full_charging_together_clash() {
        let c = [(e(1), Vec3::ZERO, 0, 0.1), (e(2), Vec3::new(2.0, 0.0, 0.0), 1, 0.3)];
        assert_eq!(pairs(&c), vec![(e(1), e(2))]);
    }

    #[test]
    fn allies_far_foes_and_stale_charges_do_not() {
        let near = Vec3::new(2.0, 0.0, 0.0);
        assert!(pairs(&[(e(1), Vec3::ZERO, 0, 0.1), (e(2), near, 0, 0.1)]).is_empty(), "same team");
        assert!(pairs(&[(e(1), Vec3::ZERO, 0, 0.1), (e(2), Vec3::new(9.0, 0.0, 0.0), 1, 0.1)]).is_empty(), "too far");
        assert!(pairs(&[(e(1), Vec3::ZERO, 0, 0.1), (e(2), near, 1, f32::INFINITY)]).is_empty(), "not charged");
    }

    #[test]
    fn each_fighter_clashes_once_nearest_first() {
        let c = [(e(1), Vec3::ZERO, 0, 0.0), (e(2), Vec3::new(1.0, 0.0, 0.0), 1, 0.0), (e(3), Vec3::new(3.0, 0.0, 0.0), 1, 0.0)];
        assert_eq!(pairs(&c), vec![(e(1), e(2))]);
    }

    #[test]
    fn more_taps_win_and_a_wider_margin_hits_harder() {
        assert_eq!(outcome([21.0, 21.2]).0, None);
        let (w, m) = outcome([30.0, 10.0]);
        assert_eq!(w, Some(0));
        assert!((m - 0.25).abs() < 1e-5);
        assert!(blow_damage(0.4) > blow_damage(0.05));
        assert!((blow_damage(0.0) - 0.6 * DAMAGE).abs() < 1e-4);
    }

    #[test]
    fn npc_tempo_stays_near_the_base_rate() {
        for i in 1..50 {
            let r = ai_rate(e(i));
            assert!((AI_RATE * 0.85..=AI_RATE * 1.15).contains(&r), "{r}");
        }
    }
}
