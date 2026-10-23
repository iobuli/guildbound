//! Brookmere: a walkable village, neutral gate watch, a swimmable lake and the wilderness road.
use super::combat::Revive;
use super::controller::yaw_of;
use super::occupations::Civilian;
use super::talk::{Choice, Chose, Dialogue, EndTalk, Role, TalkStarted, Talkable, Talking};
use super::world::{ground_height, world_layers};
use super::{
    Action, Fighter, Intent, Loadout, Mover, Npc, Phys, PlaySet, Player, Water, spawn_character,
};
use avian3d::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use std::f32::consts::FRAC_PI_2;

pub const CENTER: Vec2 = Vec2::new(-58.0, -48.0);
pub const LAKE: Vec2 = Vec2::new(-108.0, -57.0);
pub const LAKE_RADIUS: f32 = 12.0;
pub const LAKE_LEVEL: f32 = -0.35;

pub fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("GB_VILLAGE").as_deref() != Ok("0"))
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Terrain and placement use exactly the same village pad, road and lake bowl.
pub fn terrain_height(x: f32, z: f32, height: f32) -> f32 {
    if !enabled() {
        return height;
    }
    let p = Vec2::new(x, z);
    let q = (p - CENTER).abs() - Vec2::new(31.0, 29.0);
    let outside = q.max(Vec2::ZERO).length();
    let road_z = z.clamp(-20.0, 24.0);
    let road = p.distance(Vec2::new(CENTER.x, road_z));
    let village_weight = smooth(0.0, 9.0, outside).min(smooth(4.0, 10.0, road));
    let mut h = height * village_weight;
    let lake_d = p.distance(LAKE);
    h *= smooth(LAKE_RADIUS + 1.0, LAKE_RADIUS + 9.0, lake_d);
    h -= (1.0 - smooth(3.0, LAKE_RADIUS, lake_d)) * 2.7;
    h
}

#[derive(Component)]
pub struct Guard;

#[derive(Component)]
pub struct RoundWater(pub f32);

/// Keep random trees, loose items and tall grass away from buildings, roads and the lake.
pub fn reserved(x: f32, z: f32) -> bool {
    enabled()
        && ((Vec2::new(x, z) - CENTER)
            .abs()
            .cmple(Vec2::new(33.0, 31.0))
            .all()
            || Vec2::new(x, z).distance(LAKE) < LAKE_RADIUS + 3.0
            || ((x - CENTER.x).abs() < 4.5 && (-20.0..24.0).contains(&z)))
}

#[derive(Component)]
struct Resident {
    home: Vec3,
    phase: f32,
}

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct WaterParams {
    pub color: Vec4,
    pub lake: Vec4,
    pub clock: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct VillageWater {
    #[uniform(0)]
    pub params: WaterParams,
}

impl Material for VillageWater {
    fn fragment_shader() -> ShaderRef {
        "shaders/village_water.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<VillageWater>::default())
        .add_systems(Startup, spawn.after(super::world::setup))
        .add_systems(Update, residents.in_set(PlaySet::Think))
        .add_systems(
            Update,
            guards.in_set(PlaySet::Think).after(super::ai::think),
        )
        .add_systems(Update, dialogue.after(super::guild::respond))
        .add_systems(Update, water_clock);
}

fn water_clock(time: Res<Time>, mut water: ResMut<Assets<VillageWater>>) {
    for (_, m) in water.iter_mut() {
        m.params.clock.x = time.elapsed_secs_wrapped();
    }
}

/// Shared meshes/materials keep the village inexpensive; only structural parts have colliders.
struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    cube: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    stone: Handle<StandardMaterial>,
    timber: Handle<StandardMaterial>,
    plaster: Handle<StandardMaterial>,
    roof: Handle<StandardMaterial>,
    cloth: Handle<StandardMaterial>,
}

impl Builder<'_, '_, '_> {
    fn block(
        &mut self,
        name: &str,
        at: Vec3,
        size: Vec3,
        material: Handle<StandardMaterial>,
        solid: bool,
    ) -> Entity {
        let mut entity = self.commands.spawn((
            Name::new(name.to_string()),
            Mesh3d(self.cube.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(at).with_scale(size),
        ));
        // Scale belongs to the mesh; Avian scales the unit collider by the same transform.
        if solid {
            entity.insert((
                RigidBody::Static,
                Collider::cuboid(1.0, 1.0, 1.0),
                world_layers(),
            ));
        }
        entity.id()
    }

    fn house(&mut self, x: f32, z: f32, name: &str) {
        let plaster = self.plaster.clone();
        let timber = self.timber.clone();
        let roof = self.roof.clone();
        let at = Vec3::new(x, 0.0, z);
        self.block(
            name,
            at + Vec3::Y * 0.07,
            Vec3::new(8.0, 0.14, 7.0),
            self.stone.clone(),
            true,
        );
        // Open doorway faces south; no invisible collision across the opening.
        for (offset, size) in [
            (Vec3::new(-4.0, 1.65, 0.0), Vec3::new(0.25, 3.2, 7.0)),
            (Vec3::new(4.0, 1.65, 0.0), Vec3::new(0.25, 3.2, 7.0)),
            (Vec3::new(0.0, 1.65, 3.5), Vec3::new(8.0, 3.2, 0.25)),
            (Vec3::new(-2.5, 1.65, -3.5), Vec3::new(3.0, 3.2, 0.25)),
            (Vec3::new(2.5, 1.65, -3.5), Vec3::new(3.0, 3.2, 0.25)),
            (Vec3::new(0.0, 2.9, -3.5), Vec3::new(2.0, 0.7, 0.25)),
        ] {
            self.block("House wall", at + offset, size, plaster.clone(), true);
        }
        for dx in [-4.0, 4.0] {
            for dz in [-3.5, 3.5] {
                self.block(
                    "Timber frame",
                    at + Vec3::new(dx, 1.7, dz),
                    Vec3::new(0.3, 3.4, 0.3),
                    timber.clone(),
                    false,
                );
            }
        }
        for sign in [-1.0, 1.0] {
            let e = self.block(
                "Pitched roof",
                at + Vec3::new(sign * 2.15, 4.0, 0.0),
                Vec3::new(4.9, 0.22, 8.0),
                roof.clone(),
                true,
            );
            self.commands.entity(e).insert(
                Transform::from_translation(at + Vec3::new(sign * 2.15, 4.0, 0.0))
                    .with_rotation(Quat::from_rotation_z(-sign * 0.4))
                    .with_scale(Vec3::new(4.9, 0.22, 8.0)),
            );
            self.block(
                "Window shutters",
                at + Vec3::new(sign * 2.6, 1.85, -3.65),
                Vec3::new(0.95, 0.8, 0.08),
                timber.clone(),
                false,
            );
        }
        self.block(
            "Interior table",
            at + Vec3::new(-2.1, 0.75, 1.2),
            Vec3::new(2.0, 0.16, 1.0),
            timber.clone(),
            true,
        );
        for dx in [-0.8, 0.8] {
            for dz in [-0.35, 0.35] {
                self.block(
                    "Table leg",
                    at + Vec3::new(-2.1 + dx, 0.4, 1.2 + dz),
                    Vec3::new(0.1, 0.65, 0.1),
                    timber.clone(),
                    false,
                );
            }
        }
        self.block(
            "Bed frame",
            at + Vec3::new(2.5, 0.3, 1.7),
            Vec3::new(1.4, 0.55, 2.1),
            timber,
            true,
        );
        self.block(
            "Bed cover",
            at + Vec3::new(2.5, 0.62, 1.7),
            Vec3::new(1.35, 0.12, 2.0),
            self.cloth.clone(),
            false,
        );
    }
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water: ResMut<Assets<VillageWater>>,
) {
    if !enabled() {
        return;
    }
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.43, 0.46, 0.43),
        perceptual_roughness: 0.95,
        ..default()
    });
    let timber = materials.add(StandardMaterial {
        base_color: Color::srgb(0.26, 0.15, 0.08),
        perceptual_roughness: 0.85,
        ..default()
    });
    let plaster = materials.add(StandardMaterial {
        base_color: Color::srgb(0.82, 0.73, 0.54),
        perceptual_roughness: 0.9,
        ..default()
    });
    let roof = materials.add(StandardMaterial {
        base_color: Color::srgb(0.38, 0.16, 0.11),
        perceptual_roughness: 0.85,
        ..default()
    });
    let cloth = materials.add(StandardMaterial {
        base_color: Color::srgb(0.19, 0.4, 0.46),
        perceptual_roughness: 0.9,
        ..default()
    });
    let road = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.43, 0.3),
        perceptual_roughness: 1.0,
        ..default()
    });
    let mut b = Builder {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cylinder: meshes.add(Cylinder::new(1.0, 1.0)),
        commands: &mut commands,
        stone: stone.clone(),
        timber: timber.clone(),
        plaster,
        roof,
        cloth,
    };
    for x in [-77.0, -39.0] {
        for z in [-66.0, -49.0, -32.0] {
            b.house(x, z, "Brookmere home");
        }
    }
    // Main road, cross streets, a broad square and the gate road into the wilderness.
    b.block(
        "Village main street",
        Vec3::new(-58.0, 0.018, -26.0),
        Vec3::new(7.0, 0.025, 100.0),
        road.clone(),
        false,
    );
    for z in [-72.0, -55.0, -38.0] {
        b.block(
            "Village lane",
            Vec3::new(-58.0, 0.02, z),
            Vec3::new(55.0, 0.025, 3.0),
            road.clone(),
            false,
        );
    }
    b.block(
        "Village square",
        Vec3::new(-58.0, 0.025, -48.0),
        Vec3::new(18.0, 0.025, 14.0),
        road,
        false,
    );
    // Defensive wall with a real 8m opening on the north side.
    for (at, size) in [
        (Vec3::new(-88.0, 1.6, -48.0), Vec3::new(0.8, 3.2, 56.0)),
        (Vec3::new(-28.0, 1.6, -48.0), Vec3::new(0.8, 3.2, 56.0)),
        (Vec3::new(-58.0, 1.6, -76.0), Vec3::new(60.0, 3.2, 0.8)),
        (Vec3::new(-75.0, 1.6, -20.0), Vec3::new(26.0, 3.2, 0.8)),
        (Vec3::new(-41.0, 1.6, -20.0), Vec3::new(26.0, 3.2, 0.8)),
    ] {
        b.block("Village wall", at, size, stone.clone(), true);
    }
    b.block(
        "Gate lintel",
        Vec3::new(-58.0, 4.6, -20.0),
        Vec3::new(9.5, 0.7, 1.0),
        timber.clone(),
        true,
    );
    for x in [-63.0, -53.0] {
        b.commands.spawn((
            Name::new("Gate watchtower"),
            Mesh3d(b.cylinder.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(x, 2.0, -20.0).with_scale(Vec3::new(1.5, 4.0, 1.5)),
            RigidBody::Static,
            Collider::cylinder(1.0, 1.0),
            world_layers(),
        ));
    }
    // Market stalls along the square, with produce and a cloth canopy.
    for (i, x) in [-65.0, -51.0].into_iter().enumerate() {
        for z in [-57.0, -40.0] {
            b.block(
                "Market counter",
                Vec3::new(x, 0.65, z),
                Vec3::new(3.0, 1.3, 1.1),
                timber.clone(),
                true,
            );
            b.block(
                "Market awning",
                Vec3::new(x, 2.4, z),
                Vec3::new(3.5, 0.12, 2.2),
                b.cloth.clone(),
                false,
            );
            for dx in [-1.4, 1.4] {
                b.block(
                    "Stall post",
                    Vec3::new(x + dx, 1.2, z),
                    Vec3::new(0.1, 2.4, 0.1),
                    timber.clone(),
                    false,
                );
            }
            for dx in [-0.8, 0.0, 0.8] {
                b.block(
                    "Produce crate",
                    Vec3::new(x + dx, 1.42, z),
                    Vec3::splat(0.4),
                    if i == 0 {
                        b.plaster.clone()
                    } else {
                        b.roof.clone()
                    },
                    false,
                );
            }
        }
    }
    // Well in the square and a small footbridge leading into the lake from the north shore.
    b.commands.spawn((
        Name::new("Village well"),
        Mesh3d(b.cylinder.clone()),
        MeshMaterial3d(stone.clone()),
        Transform::from_xyz(-64.0, 0.55, -48.0).with_scale(Vec3::new(1.1, 1.1, 1.1)),
        RigidBody::Static,
        Collider::cylinder(1.0, 1.0),
        world_layers(),
    ));
    for z in [-42.0, -45.0, -48.0, -51.0] {
        b.block(
            "Lake dock",
            Vec3::new(LAKE.x, 0.15, z),
            Vec3::new(2.5, 0.25, 3.0),
            timber.clone(),
            true,
        );
        for dx in [-1.0, 1.0] {
            b.block(
                "Dock piling",
                Vec3::new(LAKE.x + dx, -0.45, z),
                Vec3::new(0.18, 1.5, 0.18),
                timber.clone(),
                false,
            );
        }
    }
    // Wilderness camp marks the encounter area without obstructing the gate road.
    for x in [-67.0, -49.0] {
        b.block(
            "Goblin camp shelter",
            Vec3::new(x, ground_height(x, 18.0) + 1.9, 18.0),
            Vec3::new(4.0, 0.18, 4.0),
            b.roof.clone(),
            false,
        );
        for dx in [-1.7, 1.7] {
            b.block(
                "Camp post",
                Vec3::new(x + dx, ground_height(x + dx, 18.0) + 0.9, 18.0),
                Vec3::new(0.18, 1.8, 0.18),
                timber.clone(),
                true,
            );
        }
        b.block(
            "Camp supply crate",
            Vec3::new(x, ground_height(x, 20.0) + 0.45, 20.0),
            Vec3::splat(0.9),
            timber.clone(),
            true,
        );
    }
    // Four shared-material lanterns; unshadowed lights keep the night-time market readable.
    let lantern = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.64, 0.23),
        emissive: LinearRgba::rgb(2.0, 1.0, 0.3),
        ..default()
    });
    for (x, z) in [
        (-67.0, -55.0),
        (-49.0, -55.0),
        (-62.0, -26.0),
        (-54.0, -26.0),
    ] {
        b.block(
            "Lantern post",
            Vec3::new(x, 1.25, z),
            Vec3::new(0.12, 2.5, 0.12),
            timber.clone(),
            false,
        );
        b.block(
            "Village lantern",
            Vec3::new(x, 2.55, z),
            Vec3::new(0.25, 0.35, 0.25),
            lantern.clone(),
            false,
        );
        b.commands.spawn((
            Name::new("Lantern light"),
            PointLight {
                color: Color::srgb(1.0, 0.75, 0.42),
                intensity: 18000.0,
                range: 9.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, 2.6, z),
        ));
    }
    drop(b);
    // A cylindrical sensor matches the circular rendered shoreline.
    let half = Vec3::new(LAKE_RADIUS, 1.5, LAKE_RADIUS);
    let lake = commands
        .spawn((
            Name::new("Brookmere lake"),
            Water { half_extents: half },
            RoundWater(LAKE_RADIUS),
            Transform::from_xyz(LAKE.x, LAKE_LEVEL - half.y, LAKE.y),
            Visibility::default(),
            RigidBody::Static,
            Sensor,
            Collider::cylinder(LAKE_RADIUS, half.y * 2.0),
            CollisionLayers::new(Phys::Trigger, [Phys::Character]),
        ))
        .id();
    let material = water.add(VillageWater {
        params: WaterParams {
            color: Vec4::new(0.025, 0.23, 0.29, 0.8),
            lake: Vec4::new(LAKE.x, LAKE.y, LAKE_RADIUS, LAKE_LEVEL),
            clock: Vec4::ZERO,
        },
    });
    commands.spawn((
        Name::new("Animated lake surface"),
        Mesh3d(meshes.add(Circle::new(LAKE_RADIUS).mesh().resolution(96))),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, half.y, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        ChildOf(lake),
        NotShadowCaster,
    ));
    for (i, (name, at)) in [
        ("Ada", Vec3::new(-67.0, 0.0, -51.0)),
        ("Bram", Vec3::new(-49.0, 0.0, -45.0)),
        ("Cora", Vec3::new(-69.0, 0.0, -68.0)),
        ("Dara", Vec3::new(-46.0, 0.0, -35.0)),
        ("Eli", Vec3::new(-61.0, 0.0, -62.0)),
        ("Finn", Vec3::new(-55.0, 0.0, -30.0)),
    ]
    .into_iter()
    .enumerate()
    {
        let who = spawn_character(
            &mut commands,
            name,
            at,
            0.0,
            Fighter::new(0, 100.0),
            Loadout {
                slots: vec!["unarmed".into()],
                current: 0,
            },
            None,
        );
        super::appearance::dress_villager(&mut commands, who, name, false);
        commands.entity(who).insert((
            Civilian,
            Resident {
                home: at,
                phase: i as f32 * 1.1,
            },
            Talkable {
                name,
                role: Role::Villager,
                title: "Brookmere resident",
            },
            Revive {
                after: 8.0,
                home: Some((at, 0.0)),
            },
        ));
    }
    for (name, x) in [("Gate Guard Orin", -63.0), ("Gate Guard Vale", -53.0)] {
        let feet = Vec3::new(x, 0.0, -16.0);
        let who = spawn_character(
            &mut commands,
            name,
            feet,
            0.0,
            Fighter::new(0, 180.0),
            Loadout {
                slots: vec!["katana".into()],
                current: 0,
            },
            None,
        );
        super::appearance::dress_villager(&mut commands, who, name, true);
        commands.entity(who).insert((
            Npc,
            Guard,
            Talkable {
                name,
                role: Role::Guard,
                title: "Neutral village watch",
            },
            super::stats::Stats::preset(
                6,
                &[super::stats::Attr::Vigor, super::stats::Attr::Strength],
            ),
            super::skills::Sheet::default(),
        ));
    }
}

fn residents(
    time: Res<Time>,
    talking: Res<Talking>,
    players: Query<(Entity, &Transform), With<Player>>,
    mut villagers: Query<
        (
            Entity,
            &Transform,
            &Resident,
            &Fighter,
            &Action,
            &mut Intent,
        ),
        Without<Player>,
    >,
) {
    for (entity, tf, resident, fighter, action, mut intent) in &mut villagers {
        *intent = Intent::default();
        if fighter.dead || !action.is_free() {
            continue;
        }
        if talking.npc == Some(entity) {
            if let Ok((who, player)) = players.single() {
                intent.target = Some(who);
                intent.look = player.translation;
            }
            continue;
        }
        // Small routes stay in the lanes. Walking shares the normal collision controller.
        let t = time.elapsed_secs() * 0.08 + resident.phase;
        let goal = resident.home + Vec3::new(t.sin() * 1.2, 0.0, (t * 0.7).cos() * 2.0);
        let to = (goal - tf.translation).with_y(0.0);
        if to.length() > 0.4 {
            intent.move_dir = to.normalize_or_zero() * 0.28;
            intent.walk = true;
        }
        intent.look = goal + Vec3::Y;
    }
}

fn guards(
    talking: Res<Talking>,
    players: Query<(Entity, &Transform), With<Player>>,
    mut guards: Query<(Entity, &Transform, &Guard, &mut Intent, &mut Mover), Without<Player>>,
) {
    for (entity, tf, _, mut intent, mut mover) in &mut guards {
        if talking.npc == Some(entity) {
            *intent = Intent::default();
            if let Ok((who, player)) = players.single() {
                intent.target = Some(who);
                intent.look = player.translation;
                mover.face = yaw_of(player.translation - tf.translation);
            }
        }
    }
}

fn choices() -> Vec<Choice> {
    [
        ("Where am I?", "village_where"),
        ("What is outside the gate?", "village_wilds"),
        ("Show me the lake", "village_lake"),
        ("Leave", "village_leave"),
    ]
    .into_iter()
    .map(|(label, id)| Choice {
        label: label.into(),
        id,
    })
    .collect()
}

fn dialogue(
    mut started: MessageReader<TalkStarted>,
    mut chosen: MessageReader<Chose>,
    mut dialogue: ResMut<Dialogue>,
    mut end: MessageWriter<EndTalk>,
    npcs: Query<&Talkable>,
) {
    for event in started
        .read()
        .filter(|e| matches!(e.role, Role::Villager | Role::Guard))
    {
        let name = npcs.get(event.npc).map_or("Friend", |n| n.name);
        *dialogue = Dialogue {
            line: if event.role == Role::Guard {
                format!(
                    "{name}, village watch. You may pass freely. We protect Brookmere; goblins gather up the northern road."
                )
            } else {
                format!(
                    "Welcome to Brookmere. I'm {name}. Rest in the village, or visit the market before heading out."
                )
            },
            options: choices(),
        };
    }
    for pick in chosen
        .read()
        .filter(|e| matches!(e.role, Role::Villager | Role::Guard))
    {
        dialogue.line = match pick.id {
            "village_where" => "Brookmere's square. The homes are open, market stalls line the streets, and Rook teaches sword and ki nearby. F talks; Z changes style. B swaps your aiming shoulder.".into(),
            "village_wilds" => "Go north through the open gate. Our guards are neutral to travellers and defend against monsters. Goblin raiders camp farther north; wolves and boars roam west. Keep clear if you need to recover.".into(),
            "village_lake" => "The lake is west of the village. Exit through the north gate and follow the outer wall west to the dock. Deep water supports swimming; the shallows let you wade back out.".into(),
            "village_leave" => { end.write_default(); String::new() },
            _ => continue,
        };
    }
}
