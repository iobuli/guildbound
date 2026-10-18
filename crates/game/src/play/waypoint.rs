//! Finding your way, and a place to come back to.
//! - Compass: a strip across the top of the screen with the cardinal points sliding as the camera
//!   turns, and a marker toward the current goal (`goal`): the ruins while Thoral stands, the guild
//!   desk while a Thoral bounty waits to be paid, else none. The marker shows the goal's name and
//!   distance; past the strip's edge it pins to the edge with an arrow.
//! - Shrines: small stone shrines (courtyard, below the ruins' torii). Within `SHRINE_REACH` m, F
//!   rests there: whole again (`guild::Rested`), and death brings you back to that shrine
//!   (`combat::Revive.home`, which this module sets on the player).
use super::boss::Boss;
use super::camera::CameraRig;
use super::combat::Revive;
use super::guild::Rested;
use super::hud::log::{HudLog, LogKind};
use super::world::{Kit, RUINS_CENTER, ground_height};
use super::{Fighter, Intent, Player, PlaySet};
use bevy::prelude::*;
use gbui::prelude::*;
use std::f32::consts::{PI, TAU};

/// Compass strip width (px) and the arc of view it spans (rad).
const STRIP: f32 = 520.0;
const ARC: f32 = 1.9;
/// Resting reach (m).
const SHRINE_REACH: f32 = 1.8;

/// A shrine you can rest at.
#[derive(Component, Clone)]
pub struct Shrine {
    pub name: &'static str,
    pub feet: Vec3,
    pub yaw: f32,
}

/// Where the compass points: a place and its name.
#[derive(Clone, Debug, PartialEq)]
pub struct Goal {
    pub name: &'static str,
    pub at: Vec3,
}

/// The current goal from what the world knows: Thoral alive → the ruins; Thoral beaten and his
/// bounty unpaid → the guild desk; else nothing.
pub fn goal(thoral_alive: bool, bounty_waiting: bool) -> Option<Goal> {
    if thoral_alive {
        Some(Goal { name: "Ruins — Thoral", at: Vec3::new(RUINS_CENTER.x, 0.0, RUINS_CENTER.y) })
    } else if bounty_waiting {
        Some(Goal { name: "Guild desk — bounty", at: super::hall::DESK_STAND })
    } else {
        None
    }
}

/// Where along the strip (−1 left edge .. 1 right edge) a world direction shows for a camera at
/// `yaw`; `None` when it is outside the arc. Things to the camera's left show left.
pub fn strip_x(yaw: f32, toward: Vec3) -> (f32, bool) {
    let a = toward.x.atan2(toward.z);
    let mut rel = a - yaw;
    while rel > PI {
        rel -= TAU;
    }
    while rel < -PI {
        rel += TAU;
    }
    // +yaw turns the view toward +x; +x is the camera's left (yaw 0 looks along +z).
    let x = -rel / (ARC * 0.5);
    (x.clamp(-1.0, 1.0), x.abs() <= 1.0)
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_compass)
        .add_systems(Update, rest.after(PlaySet::Input).before(PlaySet::Act))
        .add_systems(Update, (compass, prompt));
}

/// Spawn the shrines (called from `world::setup`).
pub(crate) fn spawn_shrines(kit: &mut Kit) {
    let (stone, cloth, flame) = (kit.pal.stone.clone(), kit.pal.cloth.clone(), kit.pal.flame.clone());
    for (name, x, z, yaw) in [("Courtyard shrine", -3.5f32, -14.0f32, 0.0f32), ("Torii shrine", 3.2, 22.4, PI)] {
        let feet = Vec3::new(x, ground_height(x, z), z);
        kit.solid("Shrine base", Cuboid::new(0.9, 0.25, 0.7), &stone, Transform::from_translation(feet + Vec3::Y * 0.12), Some(avian3d::prelude::Collider::cuboid(0.9, 0.25, 0.7)));
        kit.solid("Shrine body", Cuboid::new(0.6, 0.6, 0.45), &stone, Transform::from_translation(feet + Vec3::Y * 0.55), None);
        kit.solid("Shrine roof", Cuboid::new(0.95, 0.1, 0.75), &cloth, Transform::from_translation(feet + Vec3::Y * 0.92).with_rotation(Quat::from_rotation_y(yaw)), None);
        let f = kit.solid("Shrine candle", Sphere::new(0.06), &flame, Transform::from_translation(feet + Vec3::Y * 0.92 + Vec3::Y * 0.12), None);
        kit.commands.entity(f).insert(PointLight { color: Color::srgb(1.0, 0.7, 0.35), intensity: 18_000.0, range: 5.0, shadow_maps_enabled: false, ..default() });
        // Where you stand to rest, and come back to: in front of it.
        let front = feet + Quat::from_rotation_y(yaw) * Vec3::Z * 1.0;
        kit.commands.spawn((Name::new(name), Shrine { name, feet: front.with_y(ground_height(front.x, front.z)), yaw: yaw + PI }, Transform::from_translation(feet)));
    }
}

/// F by a shrine: rest, and make it the place you come back to.
fn rest(
    time: Res<Time>,
    mut players: Query<(Entity, &Transform, &mut Intent, &Fighter, Option<&mut Revive>), With<Player>>,
    shrines: Query<(&Transform, &Shrine)>,
    mut rested: MessageWriter<Rested>,
    mut log: MessageWriter<HudLog>,
) {
    let Ok((e, tf, mut intent, fighter, revive)) = players.single_mut() else { return };
    // Smoke: `GB_REST=1` presses F at 3 s.
    let t = time.elapsed_secs();
    if t >= 3.0 && t - time.delta_secs() < 3.0 && std::env::var("GB_REST").is_ok_and(|v| v.trim() == "1") {
        intent.interact_pressed = true;
    }
    if !intent.interact_pressed || fighter.dead {
        return;
    }
    let Some((_, shrine)) = shrines.iter().find(|(s, _)| s.translation.distance(tf.translation) < SHRINE_REACH + 0.9) else { return };
    intent.interact_pressed = false;
    rested.write(Rested { who: e });
    if let Some(mut r) = revive {
        r.home = Some((shrine.feet, shrine.yaw));
    }
    log.write(HudLog::new(LogKind::Item, format!("You rest at the {}. Whole again; you will return here.", shrine.name.to_lowercase())).key(7400));
    info!("[waypoint] rested at {}", shrine.name);
}

/// "[F] Rest" while by a shrine.
fn prompt(players: Query<&Transform, With<Player>>, shrines: Query<(&Transform, &Shrine)>, mut log: MessageWriter<HudLog>, mut near: Local<bool>) {
    let Ok(tf) = players.single() else { return };
    let now = shrines.iter().any(|(s, _)| s.translation.distance(tf.translation) < SHRINE_REACH + 0.9);
    if now && !*near {
        log.write(HudLog::new(LogKind::System, "[F] Rest at the shrine").key(7401));
    }
    *near = now;
}

#[derive(Component)]
struct Strip;
#[derive(Component)]
struct Cardinal(f32);
#[derive(Component)]
struct Marker;

fn spawn_compass(mut commands: Commands) {
    commands
        .spawn((
            Strip,
            Node {
                position_type: PositionType::Absolute,
                top: px(4.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(px(-STRIP * 0.5)),
                width: px(STRIP),
                height: px(24.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.45)),
            GlobalZIndex(45),
        ))
        .with_children(|p| {
            for (name, yaw) in [("N", 0.0f32), ("E", -PI / 2.0), ("S", PI), ("W", PI / 2.0)] {
                // Compass yaw convention: +z is north (yaw 0), +x is west (yaw π/2) seen from the
                // camera's convention that +yaw turns toward +x.
                p.spawn((Cardinal(yaw), Node { position_type: PositionType::Absolute, top: px(3.0), ..default() }, label(name, "title")));
            }
            p.spawn((Marker, Node { position_type: PositionType::Absolute, top: px(3.0), display: Display::None, ..default() }, label("", "row.text")));
        });
}

#[allow(clippy::type_complexity)]
fn compass(
    rig: Option<Res<CameraRig>>,
    bosses: Query<&Fighter, With<Boss>>,
    guild: Option<Res<super::guild::Guild>>,
    players: Query<&Transform, With<Player>>,
    mut cardinals: Query<(&Cardinal, &mut Node), Without<Marker>>,
    mut marker: Query<(&mut Node, &mut Text), (With<Marker>, Without<Cardinal>)>,
) {
    let (Some(rig), Ok(me)) = (rig, players.single()) else { return };
    let yaw = rig.yaw;
    for (c, mut node) in &mut cardinals {
        let (x, shown) = strip_x(yaw, Vec3::new(c.0.sin(), 0.0, c.0.cos()));
        node.left = px((x + 1.0) * 0.5 * STRIP - 6.0);
        node.display = if shown { Display::Flex } else { Display::None };
    }
    let alive = bosses.iter().any(|f| !f.dead);
    let waiting = guild.as_deref().is_some_and(|g| g.bounties.get(super::boss::NAME).is_some_and(|n| *n > 0));
    let target = goal(alive, waiting);
    for (mut node, mut text) in &mut marker {
        let Some(g) = &target else {
            node.display = Display::None;
            continue;
        };
        let to = (g.at - me.translation).with_y(0.0);
        let (x, inside) = strip_x(yaw, to);
        let arrow = if inside { "◆" } else if x < 0.0 { "◀" } else { "▶" };
        text.0 = format!("{arrow} {}  {:.0} m", g.name, to.length());
        node.display = Display::Flex;
        node.left = px(((x + 1.0) * 0.5 * STRIP - 10.0).clamp(0.0, STRIP - 180.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strip_places_directions_like_the_view() {
        // Looking north (+z): north is centred, west (+x) is to the left, behind is off the strip.
        let (x, shown) = strip_x(0.0, Vec3::Z);
        assert!(shown && x.abs() < 1e-4);
        let (x, shown) = strip_x(0.0, Vec3::new(1.0, 0.0, 2.0));
        assert!(shown && x < 0.0, "+x shows left: {x}");
        let (_, shown) = strip_x(0.0, -Vec3::Z);
        assert!(!shown);
        // Looking east (yaw −π/2, toward −x): east is centred.
        let (x, _) = strip_x(-PI / 2.0, Vec3::NEG_X);
        assert!(x.abs() < 1e-4);
    }

    #[test]
    fn the_goal_follows_the_boss_and_the_bounty() {
        assert_eq!(goal(true, false).map(|g| g.name), Some("Ruins — Thoral"));
        assert_eq!(goal(false, true).map(|g| g.name), Some("Guild desk — bounty"));
        assert_eq!(goal(false, false), None);
    }
}
