//! Small effects: particle bursts (wood chips, leaves, sparks) and one-shot sounds.
//! Other systems ask for them with `Burst` and `Sfx` messages; `fx` owns every `Particle` and
//! the audio entities it spawns. Particles are plain integrated cubes (no physics): gravity,
//! drag, a bounce on the analytic terrain, then they shrink away.
use super::camera::MainCamera;
use super::world::ground_height;
use bevy::audio::Volume;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_message::<Burst>()
        .add_message::<Sfx>()
        .init_resource::<FxRng>()
        .add_systems(Startup, load)
        .add_systems(Update, (spawn_bursts, play_sounds, step_particles));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BurstKind {
    /// Wood chips flying off an axe cut.
    Chips,
    /// Leaves shaken off a canopy.
    Leaves,
    /// Hot sparks off steel meeting steel (blocks, parries).
    Sparks,
}

/// Spawn `count` particles at `at`, thrown around `dir` (world, any length; zero = all round).
#[derive(Message, Clone, Copy, Debug)]
pub struct Burst {
    pub kind: BurstKind,
    pub at: Vec3,
    pub dir: Vec3,
    pub count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfxKind {
    Chop,
    TreeFall,
    PickUp,
    PutDown,
    /// An item flying into the bag.
    Collect,
    /// A steel plate locking into place.
    Clank,
    /// A weapon or limb cutting the air (attack swing).
    Swing,
    /// A light blow landing on a body.
    Hit,
    /// A heavy blow landing on a body.
    HitHeavy,
    /// A blow stopped by a guard.
    Block,
    /// A blow turned aside by a perfect guard.
    Parry,
    /// A body hitting the ground (knockdown, death).
    BodyFall,
    /// A foot planting on grass / bare earth / sand / hard ground (stone, wood).
    StepGrass,
    StepDirt,
    StepSand,
    StepHard,
    /// Feet coming down from a jump or a fall.
    Land,
    /// Worn plate armour shifting with a step.
    Jingle,
}

/// Play a sound heard from `at` (quieter with distance from the camera).
#[derive(Message, Clone, Copy, Debug)]
pub struct Sfx {
    pub kind: SfxKind,
    pub at: Vec3,
    pub volume: f32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Particle {
    velocity: Vec3,
    spin: Vec3,
    age: f32,
    life: f32,
    size: f32,
    gravity: f32,
    drag: f32,
}

#[derive(Resource)]
struct FxAssets {
    cube: Handle<Mesh>,
    chip: [Handle<StandardMaterial>; 2],
    leaf: [Handle<StandardMaterial>; 2],
    spark: Handle<StandardMaterial>,
    chop: Vec<Handle<AudioSource>>,
    fall: Vec<Handle<AudioSource>>,
    pick: Vec<Handle<AudioSource>>,
    collect: Vec<Handle<AudioSource>>,
    clank: Vec<Handle<AudioSource>>,
    swing: Vec<Handle<AudioSource>>,
    hit: Vec<Handle<AudioSource>>,
    hit_heavy: Vec<Handle<AudioSource>>,
    block: Vec<Handle<AudioSource>>,
    parry: Vec<Handle<AudioSource>>,
    body_fall: Vec<Handle<AudioSource>>,
    step_grass: Vec<Handle<AudioSource>>,
    step_soft: Vec<Handle<AudioSource>>,
    step_hard: Vec<Handle<AudioSource>>,
    land: Vec<Handle<AudioSource>>,
    jingle: Vec<Handle<AudioSource>>,
}

/// Xorshift state for effect randomness (no gameplay depends on it).
#[derive(Resource)]
struct FxRng(u32);

impl Default for FxRng {
    fn default() -> Self {
        Self(0x9E37_79B9)
    }
}

impl FxRng {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
    fn unit(&mut self) -> Vec3 {
        let z = self.range(-1.0, 1.0);
        let a = self.range(0.0, std::f32::consts::TAU);
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(r * a.cos(), z, r * a.sin())
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            items.get(((self.next() * items.len() as f32) as usize).min(items.len() - 1))
        }
    }
}

/// Sounds are heard fully within this distance (m) and fade out by `HEAR_FAR`.
const HEAR_NEAR: f32 = 6.0;
const HEAR_FAR: f32 = 40.0;

fn load(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let mut solid = |r: f32, g: f32, b: f32| {
        mats.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: 0.9,
            ..default()
        })
    };
    let chip = [solid(0.86, 0.72, 0.50), solid(0.42, 0.29, 0.17)];
    let leaf = [solid(0.25, 0.48, 0.18), solid(0.36, 0.58, 0.22)];
    let spark = mats.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.86, 0.5),
        emissive: LinearRgba::rgb(6.0, 3.6, 1.0),
        unlit: true,
        ..default()
    });
    let sounds = |names: &[&str]| -> Vec<Handle<AudioSource>> {
        names.iter().map(|n| assets.load(format!("audio/kenney/{n}.ogg"))).collect()
    };
    commands.insert_resource(FxAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        chip,
        leaf,
        spark,
        chop: sounds(&["impactWood_heavy_000", "impactWood_heavy_001", "impactWood_heavy_002", "impactWood_heavy_003", "impactWood_heavy_004"]),
        fall: sounds(&["impactPlank_medium_000", "impactPlank_medium_001", "impactPlank_medium_002"]),
        pick: sounds(&["impactWood_medium_000", "impactWood_medium_001", "impactWood_medium_002"]),
        collect: sounds(&["handleSmallLeather", "handleSmallLeather2"]),
        clank: sounds(&["impactMetal_light_000", "impactMetal_light_001", "impactMetal_light_002", "impactPlate_light_000", "impactPlate_light_001"]),
        swing: sounds(&["cloth1", "cloth2", "cloth3", "cloth4"]),
        hit: sounds(&["impactPunch_medium_000", "impactPunch_medium_001", "impactPunch_medium_002", "impactPunch_medium_003", "impactPunch_medium_004"]),
        hit_heavy: sounds(&["impactPunch_heavy_000", "impactPunch_heavy_001", "impactPunch_heavy_002", "impactPunch_heavy_003", "impactPunch_heavy_004"]),
        block: sounds(&["impactPlate_light_000", "impactPlate_light_001", "impactPlate_light_002"]),
        parry: sounds(&["impactMetal_light_000", "impactMetal_light_001", "impactMetal_light_002"]),
        body_fall: sounds(&["impactSoft_heavy_000", "impactSoft_heavy_001", "impactSoft_heavy_002", "impactSoft_heavy_003", "impactSoft_heavy_004"]),
        step_grass: sounds(&["footstep_grass_000", "footstep_grass_001", "footstep_grass_002", "footstep_grass_003", "footstep_grass_004"]),
        step_soft: sounds(&["impactSoft_medium_000", "impactSoft_medium_001", "impactSoft_medium_002"]),
        step_hard: vec![assets.load("audio/combat/step.wav")],
        land: vec![assets.load("audio/combat/land.wav")],
        jingle: sounds(&["beltHandle1", "beltHandle2", "metalClick"]),
    });
}

fn spawn_bursts(mut commands: Commands, fx: Option<Res<FxAssets>>, mut rng: ResMut<FxRng>, mut bursts: MessageReader<Burst>) {
    let Some(fx) = fx else {
        bursts.clear();
        return;
    };
    for burst in bursts.read() {
        if !burst.at.is_finite() {
            continue;
        }
        let dir = burst.dir.normalize_or_zero();
        for _ in 0..burst.count.min(200) {
            let (velocity, size, life, gravity, drag, material) = match burst.kind {
                BurstKind::Chips => {
                    let throw = (dir * 0.8 + Vec3::Y * 0.6 + rng.unit() * 0.55).normalize_or(Vec3::Y);
                    let mat = rng.pick(&fx.chip).cloned();
                    (throw * rng.range(2.0, 4.5), rng.range(0.025, 0.055), rng.range(0.9, 1.5), 12.0, 0.6, mat)
                }
                BurstKind::Sparks => {
                    let throw = (dir * 0.6 + rng.unit() * 0.8).normalize_or(Vec3::Y);
                    let mat = Some(fx.spark.clone());
                    (throw * rng.range(3.0, 7.0), rng.range(0.012, 0.024), rng.range(0.15, 0.35), 6.0, 3.0, mat)
                }
                BurstKind::Leaves => {
                    let throw = (dir * 0.3 + rng.unit()).normalize_or(Vec3::Y);
                    let mat = rng.pick(&fx.leaf).cloned();
                    (throw * rng.range(0.8, 3.0), rng.range(0.06, 0.12), rng.range(1.8, 3.2), 2.5, 2.0, mat)
                }
            };
            let Some(material) = material else { continue };
            let spin = rng.unit() * rng.range(4.0, 14.0);
            let offset = rng.unit() * 0.08;
            commands.spawn((
                Name::new("Particle"),
                Particle {
                    velocity,
                    spin,
                    age: 0.0,
                    life,
                    size,
                    gravity,
                    drag,
                },
                Mesh3d(fx.cube.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(burst.at + offset)
                    .with_rotation(Quat::from_scaled_axis(rng.unit() * 3.0))
                    .with_scale(Vec3::new(size, size * 0.45, size)),
                bevy::light::NotShadowCaster,
            ));
        }
    }
}

fn step_particles(mut commands: Commands, time: Res<Time>, mut particles: Query<(Entity, &mut Particle, &mut Transform)>) {
    let dt = time.delta_secs().clamp(0.0, 0.05);
    for (e, mut p, mut tf) in &mut particles {
        p.age += dt;
        if p.age >= p.life || !tf.translation.is_finite() {
            commands.entity(e).try_despawn();
            continue;
        }
        let drag = (-p.drag * dt).exp();
        p.velocity = (p.velocity - Vec3::Y * p.gravity * dt) * drag;
        tf.translation += p.velocity * dt;
        let floor = ground_height(tf.translation.x, tf.translation.z) + p.size * 0.25;
        if tf.translation.y < floor {
            tf.translation.y = floor;
            if p.velocity.y < 0.0 {
                p.velocity.y *= -0.25;
                p.velocity.x *= 0.5;
                p.velocity.z *= 0.5;
                p.spin *= 0.4;
            }
        }
        tf.rotation = (Quat::from_scaled_axis(p.spin * dt) * tf.rotation).normalize();
        let fade = ((p.life - p.age) / (0.3 * p.life)).clamp(0.0, 1.0);
        tf.scale = Vec3::new(p.size, p.size * 0.45, p.size) * fade;
    }
}

fn play_sounds(
    mut commands: Commands,
    fx: Option<Res<FxAssets>>,
    mut rng: ResMut<FxRng>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut sounds: MessageReader<Sfx>,
) {
    let Some(fx) = fx else {
        sounds.clear();
        return;
    };
    let ear = camera.single().ok().map(|c| c.translation());
    for sfx in sounds.read() {
        let distance = ear.map_or(0.0, |e| e.distance(sfx.at));
        let fade = 1.0 - ((distance - HEAR_NEAR) / (HEAR_FAR - HEAR_NEAR)).clamp(0.0, 1.0);
        let volume = (sfx.volume * fade).clamp(0.0, 2.0);
        if !(volume > 0.01) {
            continue;
        }
        let (bank, speed) = match sfx.kind {
            SfxKind::Chop => (&fx.chop, rng.range(0.92, 1.08)),
            SfxKind::TreeFall => (&fx.fall, rng.range(0.55, 0.7)),
            SfxKind::PickUp => (&fx.pick, rng.range(1.1, 1.25)),
            SfxKind::PutDown => (&fx.pick, rng.range(0.9, 1.0)),
            SfxKind::Collect => (&fx.collect, rng.range(1.15, 1.35)),
            SfxKind::Clank => (&fx.clank, rng.range(0.85, 1.05)),
            SfxKind::Swing => (&fx.swing, rng.range(1.5, 1.9)),
            SfxKind::Hit => (&fx.hit, rng.range(0.95, 1.1)),
            SfxKind::HitHeavy => (&fx.hit_heavy, rng.range(0.85, 1.0)),
            SfxKind::Block => (&fx.block, rng.range(0.9, 1.05)),
            SfxKind::Parry => (&fx.parry, rng.range(1.25, 1.45)),
            SfxKind::BodyFall => (&fx.body_fall, rng.range(0.7, 0.85)),
            SfxKind::StepGrass => (&fx.step_grass, rng.range(0.9, 1.12)),
            SfxKind::StepDirt => (&fx.step_soft, rng.range(1.25, 1.5)),
            SfxKind::StepSand => (&fx.step_soft, rng.range(1.6, 1.9)),
            SfxKind::StepHard => (&fx.step_hard, rng.range(0.9, 1.15)),
            SfxKind::Land => (&fx.land, rng.range(0.9, 1.05)),
            SfxKind::Jingle => (&fx.jingle, rng.range(1.2, 1.5)),
        };
        let Some(handle) = rng.pick(bank).cloned() else { continue };
        commands.spawn((
            Name::new("Sfx"),
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)).with_speed(speed),
        ));
    }
}
