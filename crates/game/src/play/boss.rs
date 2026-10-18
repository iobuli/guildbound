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
//! - Death: he falls from the saddle (a ragdoll; the horse stays, riderless) and his helm drops
//!   (`Trophy`: walk over it to wear it, `looks::WearHelm`, and earn "Bane of the Iron Rider").
//! - Death: a banner line and a kill deed (the guild pays a bounty on "Thoral"); he returns to
//!   the dais `RETURN` s later (`combat::Revive`).
//! - Theme: a synthesized taiko-and-flute loop (`tools/synth_boss_theme.py`) fades in while the
//!   bar is up and out after.
//! - Smoke: `GB_THORAL=1` puts the player at the torii facing him; `GB_THORAL_DOWN=1` fells him at
//!   3 s (his helm drops).
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
const TRAMPLE_KNOCKBACK: f32 = 7.0;
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

/// The boss theme (`tools/synth_boss_theme.py`): its loudness while he is engaged, and how fast it
/// fades in and out (per s).
const MUSIC: &str = "audio/synth/boss_thoral.wav";
const MUSIC_VOLUME: f32 = 0.55;
const MUSIC_FADE: f32 = 0.5;

/// Whether the boss bar (and the theme) is up this frame.
#[derive(Resource, Default)]
pub struct Engaged(pub bool);

#[derive(Component)]
struct Theme(f32);

pub fn plugin(app: &mut App) {
    app.init_resource::<Engaged>()
        .add_systems(Startup, (spawn, spawn_bar, spawn_theme))
        .add_systems(Update, charge.after(PlaySet::Think).before(PlaySet::Act))
        .add_systems(Update, (fallen, bar, theme, smoke, trophy).chain());
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

/// Which way a trample throws: mostly to the side the player stands on, a little onward.
pub fn trample_direction(charge: Vec3, toward_side: Vec3) -> Vec3 {
    (toward_side.normalize_or_zero() * 0.85 + charge.normalize_or_zero() * 0.35).normalize_or(charge)
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
            // The horse barges you aside rather than straight on (the fight stays in the arena).
            // Aside to whichever flank of the horse the player is on.
            let flank = Vec3::Y.cross(mover.forward());
            let side = if flank.dot(to) >= 0.0 { flank } else { -flank };
            let thrown = trample_direction(dir, side);
            hits.write(Hit {
                attacker: e,
                target: player,
                point: ptf.translation + Vec3::Y * 0.2,
                direction: thrown,
                damage: TRAMPLE_DAMAGE,
                poise: 60.0,
                knockback: TRAMPLE_KNOCKBACK,
                speed: 8.0,
                kind: HitKind::Melee,
            });
            info!("[boss] trample at ({:.1}, {:.1})", ptf.translation.x, ptf.translation.z);
            boss.charging = 0.0;
            boss.cooldown = CHARGE_COOLDOWN;
        } else if boss.charging <= 0.0 {
            boss.cooldown = CHARGE_COOLDOWN;
        }
    }
}

/// Thoral's trophy lying where he fell: his helm, until someone without one takes it.
#[derive(Component)]
struct Trophy {
    clock: f32,
    base: Vec3,
}

const TROPHY_REACH: f32 = 1.6;

/// The fall and the return.
#[allow(clippy::type_complexity)]
fn fallen(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut log: MessageWriter<HudLog>,
    mut shake: ResMut<super::camera::CameraShake>,
    mut bosses: Query<(Entity, &Transform, &Fighter, &mut Boss, Option<&super::horse::Ridden>)>,
    wearers: Query<(), (With<Player>, With<super::looks::WearHelm>)>,
    trophies: Query<(), With<Trophy>>,
    globals: Query<&GlobalTransform>,
) {
    for (e, tf, f, mut b, ridden) in &mut bosses {
        if f.dead && !b.was_dead {
            // He falls from the saddle; the horse stays where it stood, riderless.
            if let Some(r) = ridden {
                if let Ok(g) = globals.get(r.0) {
                    commands.entity(r.0).remove::<ChildOf>().insert(g.compute_transform());
                }
                commands.entity(e).remove::<super::select::Mounted>().insert(super::ragdoll::RagdollRequest {
                    velocity: tf.rotation * Vec3::X * 2.5 + Vec3::Y * 1.0,
                    point: tf.translation + Vec3::Y * 0.6,
                });
            }
            log.write(HudLog::new(LogKind::System, "— THORAL HAS FALLEN —"));
            shake.add(0.6);
            info!("[boss] Thoral defeated");
            // His helm falls where he did (once: not again if you already wear one).
            if wearers.is_empty() && trophies.is_empty() {
                let at = tf.translation - Vec3::Y * (super::HALF_HEIGHT - 0.45);
                let iron = materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.32, 0.35), metallic: 0.6, perceptual_roughness: 0.4, ..default() });
                let crest = materials.add(StandardMaterial { base_color: Color::srgb(0.6, 0.1, 0.08), ..default() });
                commands
                    .spawn((Name::new("Iron Rider's helm"), Trophy { clock: 0.0, base: at }, Transform::from_translation(at), Visibility::default()))
                    .with_children(|t| {
                        t.spawn((Mesh3d(meshes.add(Sphere::new(1.0))), MeshMaterial3d(iron), Transform::from_scale(Vec3::new(0.18, 0.16, 0.19))));
                        t.spawn((Mesh3d(meshes.add(Cuboid::new(0.03, 0.08, 0.3))), MeshMaterial3d(crest), Transform::from_xyz(0.0, 0.16, 0.0)));
                        t.spawn((PointLight { color: Color::srgb(1.0, 0.8, 0.4), intensity: 25_000.0, range: 4.0, shadow_maps_enabled: false, ..default() }, Transform::from_xyz(0.0, 0.3, 0.0)));
                    });
                log.write(HudLog::new(LogKind::Item, "Thoral's helm lies in the dust."));
            }
        } else if !f.dead && b.was_dead {
            log.write(HudLog::new(LogKind::Warn, "Hoofbeats in the ruins: Thoral has returned."));
            // Back in the saddle.
            if let Some(r) = ridden {
                commands.entity(r.0).insert((ChildOf(e), Transform::from_xyz(0.0, -super::HALF_HEIGHT, 0.0)));
                commands.entity(e).insert(super::select::Mounted { track: super::horse::SEAT_TRACK.to_string(), time: super::horse::SEAT_TIME });
            }
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
                top: px(34.0),
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
    mut engaged: ResMut<Engaged>,
) {
    let (Ok((tf, f, aware)), Ok(p)) = (bosses.single(), players.single()) else { return };
    let near = Vec2::new(p.translation.x, p.translation.z).distance(RUINS_CENTER) < BAR_NEAR;
    let after_you = aware.is_some_and(|a| a.alerted) && tf.translation.distance(p.translation) < 40.0;
    let show = !f.dead && (near || after_you);
    if engaged.0 != show {
        engaged.0 = show;
    }
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

fn spawn_theme(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Name::new("Boss theme"),
        Theme(0.0),
        AudioPlayer::new(assets.load(MUSIC)),
        PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(0.0)),
    ));
}

/// Fade the theme in while he is engaged, out after.
fn theme(time: Res<Time>, engaged: Res<Engaged>, mut theme: Query<(&mut Theme, &mut AudioSink)>) {
    let target = if engaged.0 { MUSIC_VOLUME } else { 0.0 };
    for (mut t, mut sink) in &mut theme {
        let step = MUSIC_VOLUME * MUSIC_FADE * time.delta_secs();
        t.0 = if t.0 < target { (t.0 + step).min(target) } else { (t.0 - step).max(target) };
        sink.set_volume(bevy::audio::Volume::Linear(t.0));
    }
}

/// The helm bobs and turns; walk over it to take it (`looks::WearHelm`, a `trophy` deed: the
/// "Bane of the Iron Rider" title).
fn trophy(
    time: Res<Time>,
    mut commands: Commands,
    mut deeds: MessageWriter<super::deeds::Deed>,
    mut log: MessageWriter<HudLog>,
    mut trophies: Query<(Entity, &mut Trophy, &mut Transform)>,
    players: Query<(Entity, &Transform), (With<Player>, Without<Trophy>)>,
) {
    let Ok((player, ptf)) = players.single() else { return };
    for (e, mut t, mut tf) in &mut trophies {
        t.clock += time.delta_secs();
        tf.translation = t.base + Vec3::Y * (0.08 * (t.clock * 2.0).sin());
        tf.rotation = Quat::from_rotation_y(t.clock * 0.8);
        if tf.translation.with_y(0.0).distance(ptf.translation.with_y(0.0)) < TROPHY_REACH {
            commands.entity(e).try_despawn();
            commands.entity(player).try_insert(super::looks::WearHelm);
            deeds.write(super::deeds::Deed::new(player, "trophy", "thoral_helm", 1.0));
            log.write(HudLog::new(LogKind::Item, "You take the Iron Rider's helm."));
            info!("[boss] trophy taken");
        }
    }
}

/// `GB_THORAL=1`: the player stands at the torii, facing the dais.
fn smoke(
    time: Res<Time>,
    mut done: Local<bool>,
    mut commands: Commands,
    players: Query<Entity, With<Player>>,
    bosses: Query<(Entity, &Transform), With<Boss>>,
    mut hits: MessageWriter<Hit>,
) {
    // `GB_THORAL_DOWN=1`: a blow nobody survives at 3 s (through `combat`, like any other).
    let t = time.elapsed_secs();
    if t >= 3.0 && t - time.delta_secs() < 3.0 && std::env::var("GB_THORAL_DOWN").is_ok_and(|v| v.trim() == "1") {
        if let (Ok(p), Ok((b, btf))) = (players.single(), bosses.single()) {
            hits.write(Hit {
                attacker: p,
                target: b,
                point: btf.translation,
                direction: Vec3::Z,
                damage: 1.0e5,
                poise: 1.0e3,
                knockback: 0.0,
                speed: 10.0,
                kind: HitKind::Melee,
            });
        }
    }
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

    #[test]
    fn a_trample_throws_mostly_aside() {
        let d = trample_direction(Vec3::Z, Vec3::X);
        assert!(d.x > 0.8 && d.z > 0.2 && d.z < 0.5);
    }
}
