//! Goblin patrols plus procedural wolves and boars, all using the capsule controller and Hit resolver.
use super::controller::yaw_of;
use super::talk::Talking;
use super::village::{self, Guard};
use super::world::ground_height;
use super::{
    Action, Fighter, HALF_HEIGHT, Hit, HitKind, Intent, Loadout, Mover, Npc, Phys, PlaySet, Player,
    spawn_character,
};
use crate::motion::ModelRequest;
use avian3d::prelude::*;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Component)]
pub struct Goblin;

#[derive(Component)]
pub struct Creature {
    home: Vec3,
    phase: f32,
    bite: f32,
    cooldown: f32,
    boar: bool,
}

#[derive(Component)]
struct Body {
    owner: Entity,
}
#[derive(Component)]
struct Leg {
    owner: Entity,
    offset: Vec3,
    phase: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn.after(super::world::setup))
        .add_systems(Update, think.in_set(PlaySet::Think))
        .add_systems(Update, animate.after(PlaySet::Act));
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !village::enabled() {
        return;
    }
    for (i, (x, z, weapon)) in [
        (-65.0, 7.0, "katana"),
        (-52.0, 10.0, "unarmed"),
        (-61.0, 21.0, "axe"),
        (-48.0, 22.0, "bow"),
    ]
    .into_iter()
    .enumerate()
    {
        let feet = Vec3::new(x, ground_height(x, z), z);
        let who = spawn_character(
            &mut commands,
            &format!("Goblin Raider {}", i + 1),
            feet,
            PI,
            Fighter::new(2, 65.0),
            Loadout {
                slots: vec![weapon.into()],
                current: 0,
            },
            Some(Color::srgb(0.27, 0.51, 0.15)),
        );
        let mut sheet = super::skills::Sheet::default();
        sheet.grant(
            if weapon == "bow" {
                "archery"
            } else {
                "swordsmanship"
            },
            super::skills::xp_for_level(5),
        );
        commands.entity(who).insert((
            Npc,
            Goblin,
            sheet,
            super::stats::Stats::preset(
                2,
                &[super::stats::Attr::Dexterity, super::stats::Attr::Vigor],
            ),
        ));
    }
    let sphere = meshes.add(Sphere::new(1.0).mesh().uv(12, 8));
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let cone = meshes.add(Cone {
        radius: 1.0,
        height: 1.0,
    });
    let wolf = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.29, 0.32),
        perceptual_roughness: 0.95,
        ..default()
    });
    let boar = materials.add(StandardMaterial {
        base_color: Color::srgb(0.31, 0.19, 0.12),
        perceptual_roughness: 0.95,
        ..default()
    });
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.055, 0.05),
        ..default()
    });
    let ivory = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.82, 0.65),
        ..default()
    });
    let eyes = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.55, 0.08),
        emissive: LinearRgba::rgb(0.3, 0.12, 0.01),
        ..default()
    });
    for (i, (x, z, is_boar)) in [
        (-96.0, -16.0, false),
        (-102.0, -21.0, false),
        (-114.0, -25.0, true),
        (-117.0, -37.0, true),
    ]
    .into_iter()
    .enumerate()
    {
        let feet = Vec3::new(x, ground_height(x, z), z);
        let who = spawn_character(
            &mut commands,
            if is_boar { "Wild Boar" } else { "Grey Wolf" },
            feet,
            0.0,
            Fighter::new(2, if is_boar { 90.0 } else { 55.0 }),
            Loadout {
                slots: vec!["unarmed".into()],
                current: 0,
            },
            None,
        );
        // Animal visuals have their own gait; no humanoid mesh or appearance is requested.
        commands.entity(who).remove::<ModelRequest>().insert((
            Npc,
            Creature {
                home: feet,
                phase: i as f32,
                bite: 0.0,
                cooldown: 1.0,
                boar: is_boar,
            },
            super::skills::Sheet::default(),
        ));
        let visual = commands
            .spawn((
                Name::new("Creature visual"),
                Body { owner: who },
                Transform::from_xyz(0.0, -HALF_HEIGHT, 0.0),
                Visibility::default(),
                ChildOf(who),
            ))
            .id();
        let coat = if is_boar { boar.clone() } else { wolf.clone() };
        let mut part = |name: &str,
                        mesh: Handle<Mesh>,
                        mat: Handle<StandardMaterial>,
                        at: Vec3,
                        size: Vec3| {
            commands
                .spawn((
                    Name::new(name.to_string()),
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_translation(at).with_scale(size),
                    ChildOf(visual),
                ))
                .id()
        };
        part(
            "Animal torso",
            sphere.clone(),
            coat.clone(),
            Vec3::new(0.0, 0.92, 0.0),
            Vec3::new(if is_boar { 0.43 } else { 0.33 }, 0.4, 0.72),
        );
        part(
            "Animal head",
            sphere.clone(),
            coat.clone(),
            Vec3::new(0.0, 1.14, 0.62),
            Vec3::new(0.28, 0.3, 0.38),
        );
        part(
            "Animal muzzle",
            cube.clone(),
            dark.clone(),
            Vec3::new(0.0, 1.06, 0.96),
            Vec3::new(0.25, 0.18, 0.3),
        );
        part(
            "Animal tail",
            cube.clone(),
            coat.clone(),
            Vec3::new(0.0, 0.95, -0.85),
            Vec3::new(0.11, 0.13, 0.45),
        );
        for s in [-1.0, 1.0] {
            part(
                "Pointed ear",
                cone.clone(),
                coat.clone(),
                Vec3::new(s * 0.18, 1.5, 0.55),
                Vec3::new(0.1, 0.3, 0.12),
            );
            part(
                "Animal eye",
                sphere.clone(),
                eyes.clone(),
                Vec3::new(s * 0.23, 1.23, 0.82),
                Vec3::splat(0.045),
            );
            if is_boar {
                part(
                    "Boar tusk",
                    cone.clone(),
                    ivory.clone(),
                    Vec3::new(s * 0.2, 1.0, 1.03),
                    Vec3::new(0.06, 0.3, 0.06),
                );
            }
        }
        for (k, (x, z)) in [(-0.25, 0.48), (0.25, 0.48), (-0.25, -0.45), (0.25, -0.45)]
            .into_iter()
            .enumerate()
        {
            let offset = Vec3::new(x, 0.67, z);
            let leg = commands
                .spawn((
                    Name::new("Animal leg pivot"),
                    Leg {
                        owner: who,
                        offset,
                        phase: if k == 0 || k == 3 { 0.0 } else { PI },
                    },
                    Transform::from_translation(offset),
                    Visibility::default(),
                    ChildOf(visual),
                ))
                .id();
            commands.spawn((
                Name::new("Animal leg"),
                Mesh3d(cube.clone()),
                MeshMaterial3d(coat.clone()),
                Transform::from_xyz(0.0, -0.29, 0.0).with_scale(Vec3::new(0.15, 0.6, 0.16)),
                ChildOf(leg),
            ));
            commands.spawn((
                Name::new("Animal paw"),
                Mesh3d(cube.clone()),
                MeshMaterial3d(dark.clone()),
                Transform::from_xyz(0.0, -0.58, 0.04).with_scale(Vec3::new(0.2, 0.14, 0.26)),
                ChildOf(leg),
            ));
        }
    }
}

fn think(
    time: Res<Time>,
    talking: Res<Talking>,
    peaceful: Res<super::ai::Peaceful>,
    spatial: SpatialQuery,
    targets: Query<(Entity, &Transform, &Fighter), Or<(With<Player>, With<Guard>)>>,
    players: Query<(), With<Player>>,
    mut animals: Query<(
        Entity,
        &Transform,
        &Fighter,
        &Action,
        &mut Creature,
        &mut Intent,
        &mut Mover,
    )>,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, tf, fighter, action, mut creature, mut intent, mut mover) in &mut animals {
        *intent = Intent::default();
        mover.face = None;
        creature.phase += mover.velocity.with_y(0.0).length() * dt * 3.0;
        creature.cooldown = (creature.cooldown - dt).max(0.0);
        if fighter.dead || !action.is_free() {
            creature.bite = 0.0;
            continue;
        }
        let target = targets
            .iter()
            .filter(|(e, p, f)| {
                !f.dead
                    && f.team != fighter.team
                    && !(players.contains(*e) && (talking.active() || peaceful.0))
                    && p.translation.distance(creature.home) < 25.0
            })
            .map(|(e, p, _)| (e, p.translation, tf.translation.distance(p.translation)))
            .filter(|(_, _, d)| *d < 13.0)
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((target, at, distance)) = target {
            intent.target = Some(target);
            intent.look = at + Vec3::Y * 0.2;
            let to = (at - tf.translation).with_y(0.0).normalize_or(Vec3::Z);
            mover.face = yaw_of(to);
            if creature.bite > 0.0 {
                let before = creature.bite;
                creature.bite = (creature.bite - dt).max(0.0);
                if before > 0.18 && creature.bite <= 0.18 && distance < 2.0 {
                    let filter = SpatialQueryFilter::from_mask(Phys::World);
                    let clear = Dir3::new(at - tf.translation).ok().is_some_and(|dir| {
                        spatial
                            .cast_ray(tf.translation, dir, distance, true, &filter)
                            .is_none()
                    });
                    if clear {
                        hits.write(Hit {
                            attacker: entity,
                            target,
                            point: at,
                            direction: to,
                            damage: if creature.boar { 13.0 } else { 9.0 },
                            poise: 12.0,
                            knockback: 1.3,
                            speed: 2.0,
                            kind: HitKind::Melee,
                        });
                    }
                }
            } else if distance < 1.65 && creature.cooldown <= 0.0 {
                creature.bite = 0.55;
                creature.cooldown = if creature.boar { 2.0 } else { 1.5 };
            } else if distance > 1.35 {
                intent.move_dir = to * if distance > 5.0 { 0.8 } else { 0.55 };
            }
        } else {
            creature.bite = 0.0;
            let goal = creature.home
                + Vec3::new(
                    (time.elapsed_secs() * 0.12 + creature.home.x).sin() * 2.0,
                    0.0,
                    0.0,
                );
            let to = (goal - tf.translation).with_y(0.0);
            if to.length() > 0.8 {
                intent.move_dir = to.normalize_or_zero() * 0.3;
                intent.walk = true;
            }
            intent.look = goal + Vec3::Y;
        }
    }
}

fn animate(
    time: Res<Time>,
    animals: Query<(&Creature, &Mover, &Fighter)>,
    mut visuals: Query<
        (&mut Transform, Option<&Body>, Option<&Leg>),
        (Without<Creature>, Or<(With<Body>, With<Leg>)>),
    >,
) {
    for (mut tf, body, leg) in &mut visuals {
        if let Some(body) = body {
            let Ok((creature, mover, fighter)) = animals.get(body.owner) else {
                continue;
            };
            let target = if fighter.dead {
                Quat::from_rotation_z(-FRAC_PI_2)
            } else {
                Quat::from_rotation_x(-creature.bite.sin() * 0.18)
            };
            tf.rotation = tf
                .rotation
                .slerp(target, 1.0 - (-10.0 * time.delta_secs()).exp());
            tf.translation.y = -HALF_HEIGHT
                + if fighter.dead {
                    0.25
                } else {
                    creature.phase.sin().abs() * 0.035 * mover.velocity.length().min(1.0)
                };
        } else if let Some(leg) = leg {
            let Ok((creature, mover, fighter)) = animals.get(leg.owner) else {
                continue;
            };
            let swing = if fighter.dead {
                0.0
            } else {
                (creature.phase + leg.phase).sin() * (mover.velocity.length() * 0.12).min(0.65)
            };
            tf.translation = leg.offset;
            tf.rotation = Quat::from_rotation_x(swing);
        }
    }
}
