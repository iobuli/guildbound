//! Ranged weapons (`Update / PlaySet::Act`, after combat): the movesets of kind `ranged`
//! (`assets/anim/movesets/{bow,crossbow,knives}.json`, wielded from the chest like any weapon).
//! - Secondary held aims: the set's `aim` clip on the upper body (held at its `hold` time), the
//!   body turned to the camera's look point (or the lock-on target), half speed, a crosshair.
//! - Primary fires. With `aim.draw_time > 0` (bow) holding primary draws (an `aim.draw` clip on
//!   the aim track is scrubbed from its start to `hold` with the pull): release shoots, the
//!   arrow's speed and damage growing with the draw (`DRAW_MIN`..1). Without (crossbow, knives) a
//!   press shoots at once. `Moveset.cooldown` spaces shots; a `reload` clip plays through it.
//! - Projectiles (`ProjectileDef`: speed, gravity, radius, damage, poise, knockback, count and
//!   spread for fans of knives) fly from the drawing hand toward the aim point, sweep against body
//!   capsules and dummies like spells do, and emit `Hit` (`HitKind::Projectile`, damage scaled by
//!   the shooter's `stats::Derived::might`). Arrows and bolts that hit the world stick there for
//!   `STUCK_TIME` s; knives clatter and vanish.
//! Each volley is a `shoot` deed and each fighter struck a `shoot_hit` (tagged with the
//! projectile kind: arrow, bolt, knife).
//! Shooting owns the `Action` (`Owner::Ranged`, `ActionKind::Aim` / `Shoot` / `Reload`); hit
//! reactions interrupt it.
use super::combat::{CombatSet, Weapon, WeaponSlot, segment_distance};
use super::deeds::Deed;
use super::moveset::{Kind, Moveset, Movesets, ProjectileDef};
use super::stats::Derived;
use super::tools::Holding;
use super::world::TrainingDummy;
use super::{
    Action, ActionKind, ActionLayer, Fighter, HALF_HEIGHT, Hit, HitKind, Intent, Loadout, MoveMode, Mover, Owner, Phys,
    PlaySet, Player,
};
use super::monitor::Monitor;
use crate::motion::bank::PoseBank;
use crate::motion::{Mask, Rig};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Shortest draw that still shoots (share of a full draw), and its speed / damage share.
const DRAW_MIN: f32 = 0.25;
/// Stick share while aiming.
const AIM_MOVE: f32 = 0.5;
/// Projectiles stuck in the world stay this long (s).
const STUCK_TIME: f32 = 8.0;
/// Body capsules (as `combat`).
const BODY_LOW: f32 = 0.25;
const BODY_HIGH: f32 = 1.55;
const BODY_RADIUS: f32 = 0.26;
const DUMMY_HALF: f32 = 0.6;
const DUMMY_RADIUS: f32 = 0.28;
/// Shots leave from the chest's height in front of the body (m).
const MUZZLE_UP: f32 = 0.45;
const MUZZLE_FORWARD: f32 = 0.45;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_crosshair)
        .add_systems(Update, demo.after(PlaySet::Input).before(PlaySet::Act))
        .add_systems(Update, (shoot, hold_bow, fly, crosshair).chain().in_set(PlaySet::Act).after(CombatSet));
}

/// Smoke-test driver: `GB_SHOOT=bow|crossbow|knives` equips that weapon, aims at the nearest
/// training dummy from 2 s on and shoots about once a second (the bow at a full draw).
fn demo(
    time: Res<Time>,
    mut equip: MessageWriter<super::combat::Equip>,
    mut sent: Local<bool>,
    mut players: Query<(Entity, &Transform, &mut Intent), With<Player>>,
    dummies: Query<&Transform, (With<TrainingDummy>, Without<Player>)>,
) {
    let Ok(set) = std::env::var("GB_SHOOT") else { return };
    let Ok((who, tf, mut intent)) = players.single_mut() else { return };
    if !*sent {
        *sent = true;
        equip.write(super::combat::Equip { who, set });
    }
    let t = time.elapsed_secs();
    if t < 2.0 {
        return;
    }
    if let Some(d) = dummies.iter().min_by(|a, b| {
        a.translation.distance(tf.translation).total_cmp(&b.translation.distance(tf.translation))
    }) {
        intent.look = d.translation;
    }
    intent.secondary_held = true;
    let phase = (t - 2.0).rem_euclid(1.0);
    intent.primary_held = phase < 0.8;
    intent.primary_pressed = phase < time.delta_secs();
}

/// A shooter's state (added on demand).
#[derive(Component, Clone, Debug, Default)]
pub struct Shooting {
    /// Seconds primary has been drawn (bow), seconds until the next shot may go.
    draw: f32,
    drawing: bool,
    cooldown: f32,
}

/// A flying arrow, bolt or knife.
#[derive(Component, Clone, Debug)]
pub struct Shot {
    owner: Entity,
    team: u8,
    def: ProjectileDef,
    power: f32,
    velocity: Vec3,
    life: f32,
    /// Stuck in the world: seconds left.
    stuck: Option<f32>,
}

fn ranged_set<'a>(sets: &'a Movesets, loadout: &Loadout) -> Option<&'a Moveset> {
    sets.get(loadout.equipped()).filter(|s| s.kind == Kind::Ranged)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn shoot(
    time: Res<Time>,
    mut commands: Commands,
    sets: Res<Movesets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut deeds: MessageWriter<Deed>,
    bank: Option<Res<PoseBank>>,
    mut monitor: ResMut<Monitor>,
    rigs: Query<&Rig>,
    mut q: Query<(
        Entity,
        &Transform,
        &Intent,
        &mut Action,
        &mut Mover,
        &Loadout,
        &Fighter,
        Option<&Derived>,
        Option<&mut Shooting>,
        Has<Holding>,
    )>,
    globals: Query<&GlobalTransform>,
    vels: Res<super::aim::Velocities>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, tf, intent, mut action, mut mover, loadout, fighter, derived, shooting, holding) in &mut q {
        let Some(set) = ranged_set(&sets, loadout) else { continue };
        let (Some(aim), Some(def)) = (set.aim.as_ref(), set.projectile.as_ref()) else { continue };
        let Some(mut st) = shooting else {
            commands.entity(entity).try_insert(Shooting::default());
            continue;
        };
        st.cooldown = (st.cooldown - dt).max(0.0);
        let ours = action.owner == Owner::Ranged;
        let can = !fighter.dead && !holding && matches!(mover.mode, MoveMode::Ground | MoveMode::Air);
        if !can || (!action.is_free() && !ours) {
            st.drawing = false;
            continue;
        }
        let aiming = intent.secondary_held;
        let locked = intent.target.and_then(|t| globals.get(t).ok().map(|g| (t, g.translation() + Vec3::Y * 0.3)));
        let look = locked.map_or(intent.look, |(_, p)| p);
        // From the chest toward the aim (not the body's facing: an archer stands side-on).
        let ahead = (look - tf.translation).with_y(0.0).normalize_or(tf.forward().as_vec3());
        let muzzle = tf.translation + Vec3::Y * MUZZLE_UP + ahead * MUZZLE_FORWARD;
        let dir = (look - muzzle).normalize_or(tf.forward().as_vec3());
        let yaw = dir.x.atan2(dir.z);

        // Reload (crossbow) / recovery plays out its cooldown.
        if ours && action.kind == ActionKind::Reload {
            if st.cooldown <= 0.0 {
                action.clear();
                mover.move_scale = 1.0;
            } else if let Some(layer) = action.layer.as_mut() {
                layer.time += dt;
            }
            continue;
        }

        let draw_time = aim.draw_time.max(0.0);
        let want_fire = if draw_time > 0.0 {
            if intent.primary_held && st.cooldown <= 0.0 {
                st.drawing = true;
                st.draw += dt;
                false
            } else if st.drawing && !intent.primary_held {
                st.drawing = false;
                true
            } else {
                false
            }
        } else {
            intent.primary_pressed && st.cooldown <= 0.0
        };

        if aiming || st.drawing || want_fire {
            if !ours || action.kind != ActionKind::Aim {
                action.start(ActionKind::Aim, Owner::Ranged, Some(ActionLayer::new(aim.clip.track.clone(), aim.hold, Mask::Upper)));
            }
            mover.face = Some(yaw + aim.body_yaw_deg.to_radians());
            mover.move_scale = AIM_MOVE;
        } else if ours {
            action.clear();
            mover.face = None;
            mover.move_scale = 1.0;
            st.draw = 0.0;
            continue;
        }

        // A draw clip on the aim track scrubs from its start (arrow nocked) to `hold` (full draw)
        // as the pull grows.
        if let Some(draw) = aim.draw.as_ref().filter(|d| d.track == aim.clip.track)
            && action.kind == ActionKind::Aim
            && let Some(layer) = action.layer.as_mut()
        {
            let pull = if draw_time > 0.0 { (st.draw / draw_time).clamp(0.0, 1.0) } else { 0.0 };
            layer.time = draw.start + (aim.hold - draw.start) * pull;
        }

        if want_fire {
            let pull = if draw_time > 0.0 { (st.draw / draw_time).clamp(0.0, 1.0) } else { 1.0 };
            st.draw = 0.0;
            if pull < DRAW_MIN {
                continue;
            }
            let might = derived.map_or(1.0, |d| d.might);
            let power = (DRAW_MIN + (1.0 - DRAW_MIN) * pull) * might;
            let speed_share = 0.55 + 0.45 * pull;
            // Lead a locked target and lift the shot for its drop, so it lands where aimed.
            let speed = def.speed * speed_share;
            let dir = match locked {
                Some((t, p)) => super::aim::lead(muzzle, p, vels.of(t), speed, def.gravity).0,
                None => super::aim::ballistic(muzzle, look, speed, def.gravity),
            };
            let count = def.count.max(1);
            for k in 0..count {
                let spread = if count > 1 {
                    (k as f32 / (count - 1) as f32 - 0.5) * def.spread.to_radians()
                } else {
                    0.0
                };
                let d = Quat::from_rotation_y(spread) * dir;
                spawn_shot(&mut commands, &mut meshes, &mut materials, entity, fighter.team, def, power, muzzle, d * speed);
            }
            deeds.write(Deed::new(entity, "shoot", def.kind.clone(), 1.0));
            // Monitor: how far the drawn arrow line in the pose (string hand → bow hand) is
            // off the aim (deg), for the bow's side-on stance (`bow_align` lines).
            if aim.draw.is_some()
                && let (Some(bank), Ok(rig)) = (bank.as_deref(), rigs.get(entity))
            {
                let at = |side: usize| rig.bones.get(bank.bones.hand[side]).and_then(|e| globals.get(*e).ok()).map(|g| g.translation());
                if let (Some(bow), Some(string)) = (at(0), at(1)) {
                    let line = (bow - string).normalize_or_zero();
                    let off = line.angle_between(dir).to_degrees();
                    let yaw_off = (line.x.atan2(line.z) - dir.x.atan2(dir.z)).to_degrees();
                    let yaw_off = (yaw_off + 540.0).rem_euclid(360.0) - 180.0;
                    let pitch = |v: Vec3| v.y.atan2(Vec2::new(v.x, v.z).length()).to_degrees();
                    monitor.write(&format!(
                        "{:.2} bow_align - {off:.1} Ground yaw={yaw_off:.1} pitch={:.1}/{:.1} pull={pull:.2}",
                        time.elapsed_secs(),
                        pitch(line),
                        pitch(dir)
                    ));
                }
            }
            st.cooldown = set.cooldown.max(0.05);
            if let Some(reload) = &aim.reload {
                action.start(ActionKind::Reload, Owner::Ranged, Some(ActionLayer::new(reload.track.clone(), reload.start, Mask::Upper)));
            } else if let Some(release) = &aim.release {
                action.start(ActionKind::Shoot, Owner::Ranged, Some(ActionLayer::new(release.track.clone(), release.start, Mask::Upper)));
                action.kind = ActionKind::Reload;
            }
        }
    }
}

/// Cant of a held bow's stave (rad): its top tips away from the string hand, as archers hold it.
const BOW_CANT: f32 = 0.18;

/// Bows on a draw clip (side-on archery): while the bow is up, its stave stands upright in the
/// bow hand, canted a little, whatever the hand bone's frame (the weapon's grip in the moveset
/// suits other poses). `combat::arm` puts the grip back when the action ends.
#[allow(clippy::type_complexity)]
fn hold_bow(
    sets: Res<Movesets>,
    archers: Query<(&Transform, &Intent, &Action, &Loadout, &WeaponSlot)>,
    mut weapons: Query<(&mut Transform, &ChildOf), (With<Weapon>, Without<Intent>)>,
    globals: Query<&GlobalTransform>,
) {
    for (tf, intent, action, loadout, slot) in &archers {
        let Some(set) = ranged_set(&sets, loadout) else { continue };
        let Some(aim) = set.aim.as_ref() else { continue };
        // A primitive stave runs from the weapon origin up its length: grip it in the middle.
        let half = set
            .weapon
            .as_ref()
            .and_then(|w| w.primitive.as_ref().map(|p| p.size[1] * 0.5 * w.scale))
            .unwrap_or(0.0);
        if aim.draw.is_none() || action.owner != Owner::Ranged {
            continue;
        }
        let Ok((mut wtf, parent)) = weapons.get_mut(slot.weapon) else { continue };
        let Ok(hand) = globals.get(parent.parent()) else { continue };
        let ahead = (intent.look - tf.translation).with_y(0.0).normalize_or(Vec3::Z);
        let stave = Quat::from_axis_angle(ahead, -BOW_CANT) * Vec3::Y;
        let (hand_scale, hand_rot, _) = hand.to_scale_rotation_translation();
        let s = hand_scale.x.abs().max(1e-3);
        wtf.rotation = hand_rot.inverse() * Quat::from_rotation_arc(Vec3::Y, stave);
        wtf.translation = hand_rot.inverse() * (-stave * half) / s;
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_shot(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    owner: Entity,
    team: u8,
    def: &ProjectileDef,
    power: f32,
    from: Vec3,
    velocity: Vec3,
) {
    let [r, g, b] = def.color.unwrap_or([0.55, 0.42, 0.28]);
    let body = materials.add(StandardMaterial {
        base_color: Color::srgb(r, g, b),
        perceptual_roughness: 0.6,
        ..default()
    });
    let steel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.72, 0.76),
        metallic: 0.9,
        perceptual_roughness: 0.3,
        ..default()
    });
    let (length, head) = match def.kind.as_str() {
        "bolt" => (0.42, 0.05),
        "knife" => (0.22, 0.1),
        _ => (0.72, 0.06),
    };
    commands
        .spawn((
            Name::new(format!("Shot {}", def.kind)),
            Shot {
                owner,
                team,
                def: def.clone(),
                power,
                velocity,
                life: def.lifetime.max(0.5),
                stuck: None,
            },
            Transform::from_translation(from).with_rotation(Quat::from_rotation_arc(Vec3::Z, velocity.normalize_or(Vec3::Z))),
            Visibility::default(),
        ))
        .with_children(|s| {
            // Shaft along −Z (behind the head), head at the origin pointing +Z.
            s.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.008, length))),
                MeshMaterial3d(body),
                Transform::from_xyz(0.0, 0.0, -length * 0.5).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
            s.spawn((
                Mesh3d(meshes.add(Cone {
                    radius: 0.02,
                    height: head,
                })),
                MeshMaterial3d(steel),
                Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn fly(
    time: Res<Time>,
    mut commands: Commands,
    spatial: SpatialQuery,
    mut hits: MessageWriter<Hit>,
    mut deeds: MessageWriter<Deed>,
    mut shots: Query<(Entity, &mut Shot, &mut Transform)>,
    fighters: Query<(Entity, &Transform, &Fighter), Without<Shot>>,
    dummies: Query<(Entity, &Transform), (With<TrainingDummy>, Without<Shot>)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let filter = SpatialQueryFilter::from_mask(Phys::World);
    for (entity, mut s, mut tf) in &mut shots {
        if let Some(left) = s.stuck.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                commands.entity(entity).try_despawn();
            }
            continue;
        }
        s.life -= dt;
        let prev = tf.translation;
        s.velocity.y -= s.def.gravity * dt;
        let next = prev + s.velocity * dt;
        // Bodies first (fighters of other teams, dummies).
        let mut struck = None;
        for (e, t, f) in &fighters {
            if e == s.owner || f.dead || f.team == s.team {
                continue;
            }
            let feet = t.translation - Vec3::Y * HALF_HEIGHT;
            let (d, u, _) = segment_distance(prev, next, feet + Vec3::Y * BODY_LOW, feet + Vec3::Y * BODY_HIGH);
            if d <= s.def.radius + BODY_RADIUS {
                struck = Some((e, prev.lerp(next, u)));
                break;
            }
        }
        if struck.is_none() {
            for (e, t) in &dummies {
                let (d, u, _) = segment_distance(prev, next, t.translation - Vec3::Y * DUMMY_HALF, t.translation + Vec3::Y * DUMMY_HALF);
                if d <= s.def.radius + DUMMY_RADIUS {
                    struck = Some((e, prev.lerp(next, u)));
                    break;
                }
            }
        }
        if let Some((target, point)) = struck {
            let dir = Vec3::new(s.velocity.x, 0.0, s.velocity.z).normalize_or(Vec3::Z);
            hits.write(Hit {
                attacker: s.owner,
                target,
                point,
                direction: dir,
                damage: s.def.damage * s.power,
                poise: s.def.poise * s.power,
                knockback: s.def.knockback,
                speed: s.velocity.length(),
                kind: HitKind::Projectile,
            });
            deeds.write(Deed::new(s.owner, "shoot_hit", s.def.kind.clone(), 1.0));
            commands.entity(entity).try_despawn();
            continue;
        }
        let travel = next - prev;
        if let Ok(dir) = Dir3::new(travel)
            && let Some(hit) = spatial.cast_ray(prev, dir, travel.length(), true, &filter)
        {
            // Arrows and bolts bury their head; knives clatter away.
            tf.translation = prev + dir * (hit.distance + 0.04);
            if s.def.kind == "knife" {
                commands.entity(entity).try_despawn();
            } else {
                s.stuck = Some(STUCK_TIME);
            }
            continue;
        }
        tf.translation = next;
        if s.velocity.length_squared() > 1e-4 {
            tf.rotation = Quat::from_rotation_arc(Vec3::Z, s.velocity.normalize());
        }
        if s.life <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

#[derive(Component)]
struct Crosshair;

fn spawn_crosshair(mut commands: Commands) {
    commands
        .spawn((
            Crosshair,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .with_child((gbui::prelude::label("·  +  ·", "title"), Pickable::IGNORE));
}

/// The crosshair shows while the player aims a ranged weapon.
fn crosshair(players: Query<&Action, With<Player>>, mut q: Query<&mut Visibility, With<Crosshair>>) {
    let aiming = players.iter().any(|a| a.owner == Owner::Ranged);
    for mut v in &mut q {
        v.set_if_neq(if aiming { Visibility::Inherited } else { Visibility::Hidden });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranged_movesets_parse_and_aim() {
        let sets = Movesets::load(&super::super::moveset::movesets_dir());
        for name in ["bow", "crossbow", "knives"] {
            let s = sets.get(name).unwrap_or_else(|| panic!("{name} missing"));
            assert_eq!(s.kind, Kind::Ranged);
            let p = s.projectile.as_ref().expect("projectile");
            assert!(p.speed > 0.0 && p.damage > 0.0 && p.radius > 0.0);
            assert!(s.aim.is_some() && s.cooldown > 0.0, "{name}");
        }
        assert!(sets.get("bow").unwrap().aim.as_ref().unwrap().draw_time > 0.0, "the bow draws");
    }
}
