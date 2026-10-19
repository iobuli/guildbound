//! Debug god mode (E8, v1): F11 opens a panel to play with the systems. Every write goes through
//! the system that owns the value, never around it:
//! - target readout: hp / poise / stamina, path and rank of the lock-on target (`Intent.target`),
//!   else the nearest living foe;
//! - 1 god (self): toggles `Invulnerable` on the player (hits pass through it, like a dodge);
//! - 2 tireless (self): toggles `Tireless` (stamina stays full);
//! - 3 kill target: an unblockable `Hit` (`HitKind::Grab`) worth more than its hp (an ordinary
//!   knock unless `GB_GOD_KNOCK` says otherwise), so `combat` resolves the death itself;
//! - 4 revive target: `combat::Revive { after: 0 }`, so combat's own revive runs;
//! - 5 peaceful: flips `ai::Peaceful` (the same as F10);
//! - 6 three hours on: `sky::SetHour` (the clock's own message);
//! - 7 next weather: `weather::SetSky` with the next sky in turn (clear, overcast, rain, fog).
//!
//! The digit keys only act while this panel is open and the F12 list is closed. `GB_GOD=1` turns
//! god mode and tireless on for the player at start (smoke runs, recordings); `GB_GOD_KILL=<s>`
//! kills the nearest foe at that game time (a defeat on demand, for E7's recordings).
//! The markers act through combat's own hooks (E14), in `hold` after `CombatSet`: `Invulnerable`
//! calls `Melee::shield(SHIELD)` every frame (hits pass through like a dodge) and `Tireless` calls
//! `Melee::refresh()` and tops stamina up. "Hit lands, no damage" is a `resolve` change for
//! combat's owner (agent-2), v2.
//! Hook for the debug stats panel (E11): `GodState` is a resource another panel can read and
//! drive; this panel's own UI is just one view of it.
use super::ai::Peaceful;
use super::combat::{CombatSet, Melee, Revive};
use super::debug::DebugToggles;
use super::discipline::Mastery;
use super::sky::{Clock, SetHour};
use super::weather::{SetSky, Weather};
use super::{Fighter, Hit, HitKind, Intent, Player};
use bevy::prelude::*;
use gbui::prelude::*;

/// Combat takes no hp or poise from this fighter (hit effects still play).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Invulnerable;

/// Combat ignores this fighter's stamina spending.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Tireless;

/// What the god panel is doing; other debug panels can read or drive it.
#[derive(Resource, Clone, Debug, Default)]
pub struct GodState {
    pub open: bool,
    pub god: bool,
    pub tireless: bool,
    /// The fighter the panel is looking at (lock-on target, else the nearest foe).
    pub target: Option<Entity>,
}

/// The kill blow's knock (m/s): an ordinary-strength blow, so the death plays the way most deaths
/// do (death clip, then limp onto the ground). `GB_GOD_KNOCK=<m/s>` overrides it (e.g. 9 to throw
/// the body past the ragdoll threshold).
const KILL_KNOCK: f32 = 1.5;
/// Nearest-foe search radius when nothing is locked on (m).
const LOOK: f32 = 30.0;
/// Hours key 6 moves the clock on.
const HOURS_ON: f32 = 3.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<GodState>()
        .add_systems(Startup, (spawn_panel, smoke_start))
        .add_systems(Update, (keys, pick_target, apply_markers, smoke_kill, show).chain())
        .add_systems(Update, hold.after(CombatSet));
}

/// Seconds of untouchability `Invulnerable` renews every frame (longer than any frame).
const SHIELD: f32 = 0.2;

/// God and tireless, through combat's hooks: after combat has spent and hurt this frame.
fn hold(mut q: Query<(&mut Melee, &mut Fighter, Has<Invulnerable>, Has<Tireless>), Or<(With<Invulnerable>, With<Tireless>)>>) {
    for (mut melee, mut fighter, god, tireless) in &mut q {
        if god {
            melee.shield(SHIELD);
        }
        if tireless {
            melee.refresh();
            fighter.stamina = fighter.max_stamina;
        }
    }
}

#[derive(Component)]
struct Panel;
#[derive(Component)]
struct PanelText;

fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            Panel,
            panel(
                "stats",
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    left: px(16.0),
                    top: px(16.0),
                    width: px(320.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(12.0)),
                    row_gap: px(4.0),
                    ..default()
                },
            ),
            GlobalZIndex(61),
        ))
        .with_children(|p| {
            p.spawn(label("▌GOD MODE  [F11]", "title"));
            p.spawn((PanelText, label("", "row.text")));
        });
}

fn env_secs(name: &str) -> Option<f32> {
    std::env::var(name).ok().and_then(|s| s.trim().parse::<f32>().ok())
}

fn smoke_start(mut god: ResMut<GodState>) {
    if std::env::var("GB_GOD").is_ok_and(|v| v.trim() == "1") {
        god.god = true;
        god.tireless = true;
        info!("[god] GB_GOD=1: god mode and tireless on");
    }
}

#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    debug: Option<Res<DebugToggles>>,
    mut god: ResMut<GodState>,
    mut peaceful: Option<ResMut<Peaceful>>,
    mut commands: Commands,
    player: Query<(Entity, &Transform), With<Player>>,
    fighters: Query<(&Transform, &Fighter)>,
    mut hits: MessageWriter<Hit>,
    clock: Option<Res<Clock>>,
    weather: Option<Res<Weather>>,
    mut hours: MessageWriter<SetHour>,
    mut skies: MessageWriter<SetSky>,
) {
    let Some(keys) = keys else { return };
    if keys.just_pressed(KeyCode::F11) {
        god.open = !god.open;
    }
    if !god.open || debug.is_some_and(|d| d.list_open) {
        return;
    }
    if keys.just_pressed(KeyCode::Digit1) {
        god.god = !god.god;
        info!("[god] god mode {}", if god.god { "on" } else { "off" });
    }
    if keys.just_pressed(KeyCode::Digit2) {
        god.tireless = !god.tireless;
        info!("[god] tireless {}", if god.tireless { "on" } else { "off" });
    }
    let Ok((me, me_tf)) = player.single() else { return };
    if keys.just_pressed(KeyCode::Digit3)
        && let Some(t) = god.target
    {
        kill(&mut hits, me, me_tf.translation, t, &fighters);
    }
    if keys.just_pressed(KeyCode::Digit4)
        && let Some(t) = god.target
    {
        commands.entity(t).try_insert(Revive { after: 0.0, home: None });
        info!("[god] revive target");
    }
    if keys.just_pressed(KeyCode::Digit5)
        && let Some(p) = peaceful.as_deref_mut()
    {
        p.0 = !p.0;
        info!("[god] peaceful {}", if p.0 { "on" } else { "off" });
    }
    if keys.just_pressed(KeyCode::Digit6)
        && let Some(c) = clock.as_deref()
    {
        hours.write(SetHour((c.hour + HOURS_ON).rem_euclid(24.0)));
        info!("[god] +{HOURS_ON} h");
    }
    if keys.just_pressed(KeyCode::Digit7)
        && let Some(w) = weather.as_deref()
    {
        let next = Weather::cycled(w.sky);
        skies.write(SetSky(next));
        info!("[god] weather {}", next.word());
    }
}

/// Send the killing blow through combat (an unblockable grab throw worth more than its hp).
fn kill(hits: &mut MessageWriter<Hit>, me: Entity, from: Vec3, target: Entity, fighters: &Query<(&Transform, &Fighter)>) {
    let Ok((tf, f)) = fighters.get(target) else { return };
    if f.dead {
        return;
    }
    let to = tf.translation;
    let mut dir = Vec3::new(to.x - from.x, 0.0, to.z - from.z).normalize_or(Vec3::NEG_Z);
    if !dir.is_finite() {
        dir = Vec3::NEG_Z;
    }
    hits.write(Hit {
        attacker: me,
        target,
        point: to + Vec3::Y * 0.2,
        direction: dir,
        damage: f.max_hp.max(f.hp) * 10.0 + 1000.0,
        poise: 1000.0,
        knockback: env_secs("GB_GOD_KNOCK").filter(|k| k.is_finite() && *k >= 0.0).unwrap_or(KILL_KNOCK),
        speed: 0.0,
        kind: HitKind::Grab,
    });
    info!("[god] kill target");
}

/// The panel's target: the lock-on target, else the nearest living foe (not the player).
fn pick_target(
    mut god: ResMut<GodState>,
    player: Query<(Entity, &Transform, &Intent, &Fighter), With<Player>>,
    fighters: Query<(Entity, &Transform, &Fighter), Without<Player>>,
) {
    let Ok((_, me, intent, mine)) = player.single() else {
        god.target = None;
        return;
    };
    let locked = intent.target.filter(|t| fighters.contains(*t));
    let nearest = || {
        fighters
            .iter()
            .filter(|(_, t, f)| !f.dead && f.team != mine.team && t.translation.distance(me.translation) < LOOK)
            .min_by(|a, b| a.1.translation.distance(me.translation).total_cmp(&b.1.translation.distance(me.translation)))
            .map(|(e, ..)| e)
    };
    let pick = locked.or_else(nearest);
    if god.target != pick {
        god.target = pick;
    }
}

/// Keep the player's markers in step with the panel.
fn apply_markers(god: Res<GodState>, mut commands: Commands, player: Query<(Entity, Has<Invulnerable>, Has<Tireless>), With<Player>>) {
    let Ok((me, inv, tired)) = player.single() else { return };
    match (god.god, inv) {
        (true, false) => {
            commands.entity(me).try_insert(Invulnerable);
        }
        (false, true) => {
            commands.entity(me).try_remove::<Invulnerable>();
        }
        _ => {}
    }
    match (god.tireless, tired) {
        (true, false) => {
            commands.entity(me).try_insert(Tireless);
        }
        (false, true) => {
            commands.entity(me).try_remove::<Tireless>();
        }
        _ => {}
    }
}

/// `GB_GOD_KILL=<s>`: at that game time, kill the nearest foe (a defeat on demand), then log the
/// victim's state every 0.5 s for 3 s (`[god] watch`), so a smoke run can check the death from the
/// log alone.
#[allow(clippy::type_complexity)]
fn smoke_kill(
    time: Res<Time>,
    mut at: Local<Option<Option<f32>>>,
    mut watch: Local<Option<(Entity, f32)>>,
    god: Res<GodState>,
    player: Query<(Entity, &Transform), With<Player>>,
    fighters: Query<(&Transform, &Fighter)>,
    states: Query<(&Fighter, &Transform, &super::Mover, Option<&Name>)>,
    mut hits: MessageWriter<Hit>,
) {
    let Some(when) = *at.get_or_insert_with(|| env_secs("GB_GOD_KILL")) else { return };
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    if let Some((who, from)) = *watch {
        let step = ((t - from) / 0.5).floor();
        if step > ((t - dt - from) / 0.5).floor() {
            match states.get(who) {
                Ok((f, tf, m, n)) => info!(
                    "[god] watch +{:.1}s {}: hp {:.0} dead {} mode {:?} at ({:.1}, {:.2}, {:.1})",
                    t - from,
                    n.map_or("?", |n| n.as_str()),
                    f.hp,
                    f.dead,
                    m.mode,
                    tf.translation.x,
                    tf.translation.y,
                    tf.translation.z
                ),
                Err(_) => info!("[god] watch +{:.1}s: target gone (despawned)", t - from),
            }
        }
        if t - from > 3.0 {
            *watch = None;
        }
    }
    if !(t >= when && t - dt < when) {
        return;
    }
    let (Ok((me, me_tf)), Some(target)) = (player.single(), god.target) else {
        info!("[god] GB_GOD_KILL: no target in reach");
        return;
    };
    if let Ok((f, tf, _, n)) = states.get(target) {
        info!("[god] GB_GOD_KILL: {} hp {:.0} at ({:.1}, {:.1})", n.map_or("?", |n| n.as_str()), f.hp, tf.translation.x, tf.translation.z);
    }
    kill(&mut hits, me, me_tf.translation, target, &fighters);
    *watch = Some((target, t));
}

/// The panel text: the toggles and the target's numbers.
pub fn panel_text(god: &GodState, target: Option<(&str, &Fighter, Option<&Mastery>)>, peaceful: bool, hour: &str, sky: &str) -> String {
    let on = |b: bool| if b { "■" } else { "□" };
    let mut s = format!(
        " 1  {} god (hits pass through)\n 2  {} tireless\n 3  kill target\n 4  revive target\n 5  {} peaceful\n 6  +{HOURS_ON} h  ({hour})\n 7  weather: {sky}\n",
        on(god.god),
        on(god.tireless),
        on(peaceful),
    );
    match target {
        Some((name, f, m)) => {
            s += &format!(
                "\ntarget  {name}{}\n  hp {:.0}/{:.0}  poise {:.0}/{:.0}  stamina {:.0}/{:.0}",
                if f.dead { "  (dead)" } else { "" },
                f.hp,
                f.max_hp,
                f.poise,
                f.max_poise,
                f.stamina,
                f.max_stamina
            );
            if let Some(m) = m {
                s += &format!("\n  path {:?}  rank {}", m.path, m.rank);
            }
        }
        None => s += "\ntarget  none (lock on with Tab)",
    }
    s
}

#[allow(clippy::too_many_arguments)]
fn show(
    god: Res<GodState>,
    peaceful: Option<Res<Peaceful>>,
    clock: Option<Res<Clock>>,
    weather: Option<Res<Weather>>,
    fighters: Query<(&Fighter, Option<&Name>, Option<&Mastery>)>,
    mut root: Query<&mut Node, With<Panel>>,
    mut text: Query<&mut Text, With<PanelText>>,
) {
    for mut node in &mut root {
        let want = if god.open { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    if !god.open {
        return;
    }
    let target = god.target.and_then(|t| fighters.get(t).ok()).map(|(f, n, m)| (n.map_or("?", |n| n.as_str()), f, m));
    let hour = clock.map_or(String::from("--:--"), |c| c.label());
    let sky = weather.map_or("?", |w| w.sky.word());
    let s = panel_text(&god, target, peaceful.is_some_and(|p| p.0), &hour, sky);
    for mut tx in &mut text {
        if tx.0 != s {
            tx.0 = s.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_text_lists_the_toggles_and_the_target() {
        let god = GodState { open: true, god: true, ..default() };
        let mut f = Fighter::new(1, 80.0);
        f.hp = 30.0;
        let s = panel_text(&god, Some(("Goblin", &f, None)), true, "09:00", "rain");
        assert!(s.contains("■ god"));
        assert!(s.contains("□ tireless"));
        assert!(s.contains("■ peaceful"));
        assert!(s.contains("target  Goblin"));
        assert!(s.contains("hp 30/80"));
        assert!(s.contains("(09:00)") && s.contains("weather: rain"));
        let none = panel_text(&god, None, false, "09:00", "clear");
        assert!(none.contains("none (lock on"));
    }
}
