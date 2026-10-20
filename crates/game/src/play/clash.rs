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
//! Visual flair (anime): glowing speed streaks burst outward from between the two through the
//! exchange, and an impact frame marks the finishing blow and any knockdown-weight hit (`IMPACT_POISE`): a
//! white screen flash for `FLASH_WHITE` real seconds, then a dark beat (`FLASH_DARK`), timed in real
//! time so the hitstop's slowed clock does not stretch it.
//!
//! Clash effects (`clash_fx`): at the lock-in a shockwave, an aura burst on both fighters, a slow-motion beat
//! (`combat::Hitstop`) and a camera kick, and a glowing ring on the ground that follows the pair and pulses;
//! through the exchange fire and lightning bursts alternate between them and lightning arcs from chest to
//! chest; at the finish a fire impact, a wider shockwave and a second slow-motion beat over the impact frame.
//!
//! `GB_CLASH=<seconds>` starts a clash between the player and the nearest foe at that time (smoke
//! tests and recordings). Monitor log lines: `[clash] start`, `[clash] end`.
use super::ai::Autopilot;
use super::camera::CameraShake;
use super::combat::{CombatSet, Hitstop, Melee};
use super::vfx::{self, Element, Vfx};
use std::collections::HashMap;
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
/// Speed streaks: seconds between bursts, streaks per burst, speed (m/s), life (s).
const STREAK_EVERY: f32 = 0.05;
const STREAKS: usize = 3;
const STREAK_SPEED: f32 = 22.0;
const STREAK_LIFE: f32 = 0.22;
/// Impact frame: real seconds of white, then of dark; hits with at least this poise get one.
pub const FLASH_WHITE: f32 = 0.05;
pub const FLASH_DARK: f32 = 0.07;
pub const IMPACT_POISE: f32 = 60.0;
/// Clash effects: seconds between element bursts and between lightning arcs; slow-motion beats (s).
const BURST_EVERY: f32 = 0.34;
const ARC_EVERY: f32 = 0.45;
const SLOWMO_START: f32 = 0.3;
const SLOWMO_END: f32 = 0.35;

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

/// Real seconds into the current impact frame (None: none showing).
#[derive(Resource, Default)]
pub struct ImpactFrame(pub Option<f32>);

impl ImpactFrame {
    pub fn trigger(&mut self) {
        self.0 = Some(0.0);
    }
}

/// Overlay colour `t` real seconds into an impact frame (None when it has ended).
pub fn flash_colour(t: f32) -> Option<Color> {
    if !(t >= 0.0) {
        return None;
    }
    if t < FLASH_WHITE {
        Some(Color::srgba(1.0, 1.0, 1.0, 0.85))
    } else if t < FLASH_WHITE + FLASH_DARK {
        Some(Color::srgba(0.0, 0.0, 0.0, 0.55))
    } else {
        None
    }
}

#[derive(Component)]
struct FlashOverlay;

#[derive(Component)]
struct Streak {
    velocity: Vec3,
    life: f32,
}

#[derive(Component)]
struct ClashBar;
#[derive(Component)]
struct ClashFill;

pub fn plugin(app: &mut App) {
    app.init_resource::<Clashes>()
        .init_resource::<ImpactFrame>()
        .add_systems(Update, (detect, smoke, tick, bar, streaks, clash_fx, impacts, flash).chain().after(CombatSet));
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
    others: Query<(Entity, &Transform, &Fighter, Option<&Name>, &Melee), (Without<Player>, Without<Clashing>)>,
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
        // Never a fighter that is down (knocked down, getting up or dead).
        .filter(|(_, _, o, _, m)| !o.dead && o.team != f.team && !m.down())
        .min_by(|a, b| a.1.translation.distance(tf.translation).total_cmp(&b.1.translation.distance(tf.translation)));
    if let Some((e, _, _, name, _)) = foe {
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
    fn an_impact_frame_is_white_then_dark_then_gone() {
        let rgba = |t| flash_colour(t).map(|c| c.to_srgba());
        assert!(rgba(0.0).is_some_and(|c| c.red > 0.9 && c.alpha > 0.5));
        assert!(rgba(FLASH_WHITE + 0.01).is_some_and(|c| c.red < 0.1));
        assert!(rgba(FLASH_WHITE + FLASH_DARK + 0.01).is_none());
        assert!(rgba(f32::NAN).is_none());
    }

    #[test]
    fn npc_tempo_stays_near_the_base_rate() {
        for i in 1..50 {
            let r = ai_rate(e(i));
            assert!((AI_RATE * 0.85..=AI_RATE * 1.15).contains(&r), "{r}");
        }
    }
}


/// Speed streaks out of every running clash, and their flight.
#[allow(clippy::too_many_arguments)]
fn streaks(
    time: Res<Time>,
    mut commands: Commands,
    clashes: Res<Clashes>,
    places: Query<&Transform, Without<Streak>>,
    mut flying: Query<(Entity, &mut Transform, &mut Streak)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut kit: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut clock: Local<f32>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (e, mut tf, mut s) in &mut flying {
        s.life -= dt;
        if s.life <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        tf.translation += s.velocity * dt;
        tf.scale = Vec3::new(1.0, 1.0, (s.life / STREAK_LIFE).max(0.05));
    }
    if clashes.0.is_empty() {
        return;
    }
    *clock -= dt;
    if *clock > 0.0 {
        return;
    }
    *clock = STREAK_EVERY;
    let (mesh, mat) = kit
        .get_or_insert_with(|| (meshes.add(Cuboid::new(0.025, 0.025, 1.4)), super::vfx::glow(&mut materials, LinearRgba::rgb(3.0, 2.6, 1.6))))
        .clone();
    let seed = (time.elapsed_secs() * 1000.0) as u32;
    for (n, c) in clashes.0.iter().enumerate() {
        let (Ok(a), Ok(b)) = (places.get(c.sides[0]), places.get(c.sides[1])) else { continue };
        let mid = (a.translation + b.translation) * 0.5 + Vec3::Y * 1.1;
        for k in 0..STREAKS {
            let h = |i: u32| ((seed ^ (n as u32 * 7919) ^ (k as u32 * 104_729) ^ i).wrapping_mul(2_654_435_761) >> 8) as f32 / (1u32 << 24) as f32;
            let dir = Vec3::new(h(1) - 0.5, (h(2) - 0.5) * 0.6, h(3) - 0.5).normalize_or(Vec3::X);
            commands.spawn((
                Name::new("Clash streak"),
                Streak { velocity: dir * STREAK_SPEED, life: STREAK_LIFE },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(mid + dir * 0.4).looking_to(dir, Vec3::Y),
                bevy::light::NotShadowCaster,
            ));
        }
    }
}

/// Knockdown-weight hits (and the clash's finishing blow) get an impact frame.
fn impacts(mut hits: MessageReader<Hit>, mut frame: ResMut<ImpactFrame>) {
    if hits.read().any(|h| h.poise >= IMPACT_POISE && h.damage > 0.0) {
        frame.trigger();
    }
}

/// The impact-frame overlay, in real time.
fn flash(real: Res<Time<Real>>, mut commands: Commands, mut frame: ResMut<ImpactFrame>, mut overlay: Query<(Entity, &mut BackgroundColor), With<FlashOverlay>>) {
    let Some(t) = frame.0 else { return };
    let colour = flash_colour(t);
    frame.0 = colour.map(|_| t + real.delta_secs().clamp(0.0, 0.05));
    match (colour, overlay.single_mut()) {
        (Some(c), Ok((_, mut bg))) => bg.0 = c,
        (Some(c), Err(_)) => {
            commands.spawn((
                FlashOverlay,
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
                BackgroundColor(c),
                GlobalZIndex(50),
            ));
        }
        (None, Ok((e, _))) => commands.entity(e).despawn(),
        (None, Err(_)) => {}
    }
}


/// The ring under a clash, and the clash it marks.
#[derive(Component)]
struct ClashRing(Entity, Entity);

/// Per running clash: its last midpoint and the effect clocks (burst, arc), plus a counter.
#[derive(Default)]
struct FxState {
    mid: Vec3,
    burst: f32,
    arc: f32,
    n: u32,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn clash_fx(
    time: Res<Time>,
    mut commands: Commands,
    clashes: Res<Clashes>,
    vfx: Option<Res<Vfx>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut hitstop: ResMut<Hitstop>,
    mut shake: ResMut<CameraShake>,
    places: Query<&Transform, (Without<ClashRing>, Without<Streak>)>,
    mut rings: Query<(Entity, &ClashRing, &mut Transform)>,
    mut state: Local<HashMap<(Entity, Entity), FxState>>,
) {
    let Some(vfx) = vfx else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let mut live = Vec::new();
    for c in &clashes.0 {
        let key = (c.sides[0], c.sides[1]);
        live.push(key);
        let (Ok(a), Ok(b)) = (places.get(c.sides[0]), places.get(c.sides[1])) else { continue };
        let (pa, pb) = (a.translation, b.translation);
        let mid = (pa + pb) * 0.5;
        let fresh = !state.contains_key(&key);
        let st = state.entry(key).or_default();
        st.mid = mid;
        if fresh {
            // Lock-in: shockwave, aura on both, a slow-motion beat, the ring.
            vfx::burst(&mut commands, vfx.shock(), mid, 2.5);
            vfx::burst(&mut commands, vfx.burst(Element::Aura), pa + Vec3::Y, 1.5);
            vfx::burst(&mut commands, vfx.burst(Element::Aura), pb + Vec3::Y, 1.5);
            hitstop.trigger(SLOWMO_START);
            shake.add(0.5);
            commands.spawn((
                Name::new("Clash ring"),
                ClashRing(key.0, key.1),
                Mesh3d(meshes.add(vfx::ring(1.7, 2.0))),
                MeshMaterial3d(vfx::glow(&mut materials, Element::Aura.core() * 0.8)),
                Transform::from_translation(mid - Vec3::Y * 0.85),
                bevy::light::NotShadowCaster,
            ));
        }
        st.burst -= dt;
        if st.burst <= 0.0 {
            st.burst = BURST_EVERY;
            st.n += 1;
            // Alternate sides and elements: fire off one fist, lightning off the other.
            let (from, to, e) = if st.n % 2 == 0 { (pa, pb, Element::Fire) } else { (pb, pa, Element::Lightning) };
            let at = from.lerp(to, 0.4) + Vec3::Y * 1.2;
            vfx::burst(&mut commands, vfx.burst(e), at, 0.45);
        }
        st.arc -= dt;
        if st.arc <= 0.0 {
            st.arc = ARC_EVERY;
            vfx::spawn_bolt(&mut commands, &mut meshes, &mut materials, pa + Vec3::Y * 1.2, pb + Vec3::Y * 1.2, st.n.wrapping_mul(2_654_435_761));
        }
    }
    // The ring follows its pair and pulses; a finished clash loses its ring and gets its finale.
    let t = time.elapsed_secs();
    for (e, ring, mut tf) in &mut rings {
        match state.get(&(ring.0, ring.1)) {
            Some(st) if live.contains(&(ring.0, ring.1)) => {
                tf.translation = st.mid - Vec3::Y * 0.85;
                tf.rotation = Quat::from_rotation_y(t * 2.5);
                tf.scale = Vec3::splat(1.0 + 0.12 * (t * 14.0).sin());
            }
            _ => commands.entity(e).despawn(),
        }
    }
    let ended: Vec<(Entity, Entity)> = state.keys().filter(|k| !live.contains(k)).copied().collect();
    for k in ended {
        if let Some(st) = state.remove(&k) {
            vfx::impact(&mut commands, &mut meshes, &mut materials, &vfx, Element::Fire, st.mid + Vec3::Y, 2.2);
            vfx::burst(&mut commands, vfx.shock(), st.mid, 3.5);
            hitstop.trigger(SLOWMO_END);
            shake.add(0.7);
        }
    }
}
