//! Thoral, the Iron Rider: the first boss, waiting on the dais of the north-hill ruins
//! (`ruins`). A huge man on a small horse (`horse`), round shield on his arm (`looks`), sword in
//! hand; the NPC brain fights with him, `senses` decides when he notices you.
//! - Charge (`charge`): noticed and between `CHARGE_NEAR` and `CHARGE_FAR` m away, he spurs the
//!   horse straight at you for up to `CHARGE_TIME` s (homing a little). Reaching you is a
//!   trampling blow (`Hit`, through `combat`: block or parry it, or be flung — and recover in
//!   the air). `CHARGE_COOLDOWN` s between charges.
//! - Boss bar: his name and health across the top of the screen while you are in the ruins or he
//!   is after you.
//! - Leash: drawn more than `LEASH` m from the dais, he rides back to it before fighting on.
//! - Death: a banner line and a kill deed (the guild pays a bounty on "Thoral"); he returns to
//!   the dais `RETURN` s later (`combat::Revive`).
//! - Smoke: `GB_THORAL=1` puts the player at the torii facing him.
use super::ai::Difficulty;
use super::combat::Revive;
use super::horse::spawn_horse;
use super::hud::log::{HudLog, LogKind};
use super::senses::Awareness;
use super::skills::{Sheet, xp_for_level};
use super::stats::{Attr, Stats};
use super::world::{RUINS_CENTER, ruins_level};
use super::{Action, Fighter, Hit, HitKind, Intent, Loadout, Npc, Player, PlaySet, spawn_character};
use bevy::prelude::*;
use gbui::prelude::*;

pub const NAME: &str = "Thoral";
const TITLE: &str = "THORAL, THE IRON RIDER";
const HP: f32 = 900.0;
const TEAM: u8 = 9;
/// Charge window (m), duration (s), cooldown (s), trample reach (m) and blow.
const CHARGE_NEAR: f32 = 5.0;
const CHARGE_FAR: f32 = 16.0;
const CHARGE_TIME: f32 = 1.8;
const CHARGE_COOLDOWN: f32 = 7.0;
const TRAMPLE_REACH: f32 = 1.7;
const TRAMPLE_DAMAGE: f32 = 22.0;
const TRAMPLE_KNOCKBACK: f32 = 9.0;
/// He will not be drawn further than this from the dais (m); he rides back until within
/// `HOME_NEAR` m, then takes up the fight again.
const LEASH: f32 = 28.0;
const HOME_NEAR: f32 = 4.0;
/// Seconds before he returns after a defeat; the boss bar shows within this of the ruins (m).
const RETURN: f32 = 120.0;
const BAR_NEAR: f32 = 26.0;

/// The boss (on Thoral's root).
#[derive(Component, Default)]
pub struct Boss {
    cooldown: f32,
    charging: f32,
    was_dead: bool,
    returning: bool,
}

/// Where he waits: the dais top, facing the torii.
pub fn home() -> (Vec3, f32) {
    (Vec3::new(RUINS_CENTER.x, ruins_level() + 0.3, RUINS_CENTER.y), std::f32::consts::PI)
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (spawn, spawn_bar))
        .add_systems(Update, charge.after(PlaySet::Think).before(PlaySet::Act))
        .add_systems(Update, (fallen, bar, smoke));
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (feet, yaw) = home();
    let e = spawn_character(
        &mut commands,
        NAME,
        feet,
        yaw,
        Fighter::new(TEAM, HP),
        Loadout { slots: vec!["katana".to_string()], current: 0 },
        Some(Color::srgb(0.30, 0.30, 0.34)),
    );
    let mut sheet = Sheet::default();
    sheet.grant("swordsmanship", xp_for_level(45));
    sheet.grant("defence", xp_for_level(40));
    commands.entity(e).insert((
        Npc,
        Boss::default(),
        Stats::preset(14, &[Attr::Strength, Attr::Vigor, Attr::Endurance]),
        sheet,
        Difficulty { reaction: 0.18, parry: 0.3, dodge: 0.0, guard: 0.45, aggression: 0.85, casting: 0.0 },
        Revive { after: RETURN, home: Some((feet, yaw)) },
    ));
    spawn_horse(&mut commands, &mut meshes, &mut materials, e, Color::srgb(0.32, 0.22, 0.14), Color::srgb(0.45, 0.08, 0.08));
}

/// Pure charge decision: start one when noticed, ready and in the window.
pub fn wants_charge(noticed: bool, cooldown: f32, distance: f32) -> bool {
    noticed && cooldown <= 0.0 && (CHARGE_NEAR..=CHARGE_FAR).contains(&distance)
}

#[allow(clippy::type_complexity)]
fn charge(
    time: Res<Time>,
    peaceful: Res<super::ai::Peaceful>,
    mut log: MessageWriter<HudLog>,
    mut hits: MessageWriter<Hit>,
    mut bosses: Query<(Entity, &Transform, &mut Boss, &mut Intent, &mut super::Mover, &Fighter, &Action, Option<&Awareness>)>,
    players: Query<(Entity, &Transform, &Fighter), With<Player>>,
) {
    let dt = time.delta_secs();
    let Ok((player, ptf, pf)) = players.single() else { return };
    for (e, tf, mut boss, mut intent, mut mover, fighter, action, awareness) in &mut bosses {
        boss.cooldown = (boss.cooldown - dt).max(0.0);
        let noticed = awareness.is_none_or(|a| a.alerted) && !peaceful.0 && !pf.dead;
        if fighter.dead || !noticed {
            boss.charging = 0.0;
            if action.is_free() {
                mover.face = None;
            }
            continue;
        }
        // The leash: too far from the dais, ride back to it first.
        let (home, _) = home();
        let from_home = (tf.translation - home).with_y(0.0).length();
        if from_home > LEASH && !boss.returning {
            boss.returning = true;
            info!("[boss] leashed at {from_home:.1} m: riding back to the dais");
        } else if from_home < HOME_NEAR && boss.returning {
            boss.returning = false;
            info!("[boss] back on the dais");
        }
        if boss.returning {
            boss.charging = 0.0;
            if action.is_free() {
                mover.face = None;
            }
            intent.move_dir = (home - tf.translation).with_y(0.0).normalize_or_zero();
            intent.sprint = false;
            intent.primary_pressed = false;
            intent.target = None;
            continue;
        }
        let to = (ptf.translation - tf.translation).with_y(0.0);
        let distance = to.length();
        // The horse turns to face you between blows (the rider cannot twist round in the saddle).
        if action.is_free() && distance > 0.5 {
            mover.face = Some(to.x.atan2(to.z));
        }
        if boss.charging <= 0.0 {
            if action.is_free() && wants_charge(noticed, boss.cooldown, distance) {
                boss.charging = CHARGE_TIME;
                log.write(HudLog::new(LogKind::Warn, "Thoral spurs his horse!").key(7300));
                info!("[boss] charge from {distance:.1} m");
            }
            continue;
        }
        boss.charging -= dt;
        let dir = to.normalize_or_zero();
        intent.move_dir = dir;
        intent.sprint = true;
        intent.walk = false;
        intent.primary_pressed = false;
        intent.target = Some(player);
        if distance < TRAMPLE_REACH {
            hits.write(Hit {
                attacker: e,
                target: player,
                point: ptf.translation + Vec3::Y * 0.2,
                direction: dir,
                damage: TRAMPLE_DAMAGE,
                poise: 60.0,
                knockback: TRAMPLE_KNOCKBACK,
                speed: 8.0,
                kind: HitKind::Melee,
            });
            info!("[boss] trample");
            boss.charging = 0.0;
            boss.cooldown = CHARGE_COOLDOWN;
        } else if boss.charging <= 0.0 {
            boss.cooldown = CHARGE_COOLDOWN;
        }
    }
}

/// The fall and the return.
fn fallen(mut log: MessageWriter<HudLog>, mut shake: ResMut<super::camera::CameraShake>, mut bosses: Query<(&Fighter, &mut Boss)>) {
    for (f, mut b) in &mut bosses {
        if f.dead && !b.was_dead {
            log.write(HudLog::new(LogKind::System, "— THORAL HAS FALLEN —"));
            shake.add(0.6);
            info!("[boss] Thoral defeated");
        } else if !f.dead && b.was_dead {
            log.write(HudLog::new(LogKind::Warn, "Hoofbeats in the ruins: Thoral has returned."));
        }
        b.was_dead = f.dead;
    }
}

#[derive(Component)]
struct Bar;
#[derive(Component)]
struct BarFill;

fn spawn_bar(mut commands: Commands) {
    commands
        .spawn((
            Bar,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                top: px(18.0),
                left: percent(25.0),
                width: percent(50.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(4.0),
                ..default()
            },
            GlobalZIndex(50),
        ))
        .with_children(|p| {
            p.spawn(label(TITLE, "title"));
            p.spawn((
                Node { width: percent(100.0), height: px(10.0), ..default() },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.8)),
            ))
            .with_children(|b| {
                b.spawn((BarFill, Node { width: percent(100.0), height: percent(100.0), ..default() }, BackgroundColor(Color::srgb(0.75, 0.12, 0.1))));
            });
        });
}

#[allow(clippy::type_complexity)]
fn bar(
    bosses: Query<(&Transform, &Fighter, Option<&Awareness>), With<Boss>>,
    players: Query<&Transform, With<Player>>,
    mut root: Query<&mut Node, (With<Bar>, Without<BarFill>)>,
    mut fill: Query<&mut Node, (With<BarFill>, Without<Bar>)>,
) {
    let (Ok((tf, f, aware)), Ok(p)) = (bosses.single(), players.single()) else { return };
    let near = Vec2::new(p.translation.x, p.translation.z).distance(RUINS_CENTER) < BAR_NEAR;
    let after_you = aware.is_some_and(|a| a.alerted) && tf.translation.distance(p.translation) < 40.0;
    let show = !f.dead && (near || after_you);
    for mut n in &mut root {
        let want = if show { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
    if show {
        for mut n in &mut fill {
            n.width = percent(100.0 * (f.hp / f.max_hp.max(1.0)).clamp(0.0, 1.0));
        }
    }
}

/// `GB_THORAL=1`: the player stands at the torii, facing the dais.
fn smoke(time: Res<Time>, mut done: Local<bool>, mut commands: Commands, players: Query<Entity, With<Player>>) {
    // After the world has placed the player at the spawn (it does so once it is up).
    if *done || time.elapsed_secs() < 2.5 {
        return;
    }
    *done = true;
    if !std::env::var("GB_THORAL").is_ok_and(|v| v.trim() == "1") {
        return;
    }
    let at = Vec2::new(RUINS_CENTER.x, RUINS_CENTER.y - 8.5);
    for p in &players {
        commands.entity(p).insert(super::controller::Teleport {
            feet: Vec3::new(at.x, super::world::ground_height(at.x, at.y), at.y),
            yaw: 0.0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn he_charges_only_when_ready_noticed_and_in_range() {
        assert!(wants_charge(true, 0.0, 10.0));
        assert!(!wants_charge(false, 0.0, 10.0), "not before he notices you");
        assert!(!wants_charge(true, 2.0, 10.0), "cooling down");
        assert!(!wants_charge(true, 0.0, 2.0), "too close: he swings instead");
        assert!(!wants_charge(true, 0.0, 30.0), "too far");
    }

    #[test]
    fn he_waits_on_the_dais() {
        let (feet, _) = home();
        assert!((feet.y - (ruins_level() + 0.3)).abs() < 1e-4);
    }
}
