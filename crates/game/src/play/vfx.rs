//! Spell and art visuals: GPU particles (`bevy_hanabi`) plus a few procedural meshes, one look per
//! `Element`. `magic` asks for them through `Vfx` (a resource of effect assets) and the spawn
//! helpers below; this module owns nothing gameplay reads.
//!
//! The particle motion is built from small algorithms evaluated per particle on the GPU:
//! - Fire rides an ABC (Arnold–Beltrami–Childress) flow, the divergence-free chaotic field
//!   v = (A sin z + C cos y, B sin x + A cos z, C sin y + B cos x), sampled at the particle's
//!   position and scrolled by time, plus buoyancy, so flames curl and tumble instead of drifting.
//! - Frost bursts place shards by Vogel's phyllotaxis model (the sunflower): particle n of N sits
//!   at angle n·φ (golden angle 2.39996 rad) and radius c·√n, then flies out along that ray, so
//!   an impact flowers into a crystal rosette.
//! - Arcane particles swirl on a tangential field about the emitter (a vortex) with a slow
//!   radial breath, and the orbs themselves (in `magic`) trace Lissajous figures.
//! - Aura (Battlewill) sparks rise with upward drag; the shockwave debris arcs under gravity.
//!
//! Shared meshes for cores, crescents and rings; lightning geometry is built only at spawn:
//! - `lightning`: fractal midpoint displacement (each halving offsets the midpoint by a shrinking
//!   random amount perpendicular to the bolt) with stochastic branching, drawn as crossed ribbons
//!   so it reads from every angle, with brightness flicker instead of repeated mesh uploads;
//! - `crescent`: a tapered moon-shaped blade of aura for the thrown slash;
//! - `ring`: the expanding shockwave annulus.
//! All of them are additive, unlit, HDR-bright and fade out (`Flash`). The camera has `Bloom`,
//! so anything brighter than 1 glows.
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_hanabi::prelude::*;

/// Lightning brightness changes this often (s), and lasts this long (s).
const FLICKER: f32 = 0.08;
pub const BOLT_LIFE: f32 = 0.28;
/// Golden angle (rad).
const GOLDEN: f32 = 2.399_963;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Element {
    /// Battlewill: the body's aura.
    Aura,
    Fire,
    Frost,
    Lightning,
    Arcane,
}

impl Element {
    pub const ALL: [Element; 5] = [Element::Aura, Element::Fire, Element::Frost, Element::Lightning, Element::Arcane];
    /// HDR core colour (bright enough to bloom).
    pub fn core(self) -> LinearRgba {
        match self {
            Element::Aura => LinearRgba::rgb(2.2, 4.2, 4.0),
            Element::Fire => LinearRgba::rgb(6.0, 2.2, 0.5),
            Element::Frost => LinearRgba::rgb(2.4, 3.6, 6.0),
            Element::Lightning => LinearRgba::rgb(4.5, 4.8, 9.0),
            Element::Arcane => LinearRgba::rgb(4.2, 1.4, 6.0),
        }
    }
    pub fn charged_core(self, charge: f32) -> LinearRgba {
        let base = self.core();
        let peak = base.red.max(base.green).max(base.blue).max(1.0);
        let rgb = Vec3::new(base.red, base.green, base.blue) / peak;
        let hot = match self { Element::Fire => Vec3::new(1.0, 0.94, 0.55), _ => Vec3::new(0.95, 0.96, 1.0) };
        let c = rgb.lerp(hot, charge.clamp(0.0, 1.0).powf(1.4) * 0.85) * (1.8 + charge * 0.8);
        LinearRgba::rgb(c.x, c.y, c.z)
    }
    /// Colour particles fade toward.
    fn tail(self) -> Vec4 {
        match self {
            Element::Aura => Vec4::new(0.2, 0.9, 0.8, 0.0),
            Element::Fire => Vec4::new(0.25, 0.08, 0.05, 0.0),
            Element::Frost => Vec4::new(0.7, 0.9, 1.0, 0.0),
            Element::Lightning => Vec4::new(0.4, 0.5, 1.5, 0.0),
            Element::Arcane => Vec4::new(0.6, 0.2, 1.2, 0.0),
        }
    }
    fn index(self) -> usize {
        self as usize
    }
}

/// The effect library: one asset per element and use, and the procedural sprite each element's
/// particles sample (`sprite_alpha`).
#[derive(Resource, Clone)]
pub struct Vfx {
    trail: [Handle<EffectAsset>; 5],
    burst: [Handle<EffectAsset>; 5],
    cast: [Handle<EffectAsset>; 5],
    rise: [Handle<EffectAsset>; 5],
    shock: Handle<EffectAsset>,
    sprite: [Handle<Image>; 5],
    pub core_mesh: Handle<Mesh>,
    pub ball_mesh: Handle<Mesh>,
    pub ring_mesh: Handle<Mesh>,
    pub crescent_mesh: Handle<Mesh>,
    pub stone_material: Handle<StandardMaterial>,
    cores: [[Handle<StandardMaterial>; 3]; 5],
}

/// Decorative effects yield first under load; projectiles and combat remain authoritative.
#[derive(Resource)]
pub struct VfxLimits {
    pub emitters: usize,
    pub sprites: usize,
    pub flashes: usize,
}

impl Default for VfxLimits {
    fn default() -> Self {
        match std::env::var("GB_VFX_QUALITY").as_deref() {
            Ok("low") => Self { emitters: 16, sprites: 24, flashes: 24 },
            Ok("high") => Self { emitters: 48, sprites: 72, flashes: 64 },
            _ => Self { emitters: 32, sprites: 48, flashes: 40 },
        }
    }
}

#[derive(Resource, Default)]
pub struct VfxLoad {
    pub emitters: usize,
    pub peak_emitters: usize,
    pub dropped_emitters: usize,
    pub sprites: usize,
    pub peak_sprites: usize,
    pub dropped_sprites: usize,
}

#[derive(Component)]
struct AdmittedEmitter;

impl Vfx {
    pub fn core_material(&self, e: Element) -> Handle<StandardMaterial> {
        self.cores[e.index()][0].clone()
    }
    pub fn charged_material(&self, e: Element, charge: f32) -> Handle<StandardMaterial> {
        self.cores[e.index()][if charge > 0.85 { 2 } else if charge > 0.35 { 1 } else { 0 }].clone()
    }
    pub fn rising(&self, e: Element) -> (ParticleEffect, EffectMaterial) {
        self.bundle(&self.rise[e.index()], e)
    }
    fn bundle(&self, effect: &Handle<EffectAsset>, e: Element) -> (ParticleEffect, EffectMaterial) {
        (
            ParticleEffect::new(effect.clone()),
            EffectMaterial {
                images: vec![self.sprite[e.index()].clone()],
            },
        )
    }
    /// A trail that follows its parent and leaves particles in the world.
    pub fn trail(&self, e: Element) -> (ParticleEffect, EffectMaterial) {
        self.bundle(&self.trail[e.index()], e)
    }
    /// A one-shot burst.
    pub fn burst(&self, e: Element) -> (ParticleEffect, EffectMaterial) {
        self.bundle(&self.burst[e.index()], e)
    }
    /// Motes gathering at a casting hand.
    pub fn cast(&self, e: Element) -> (ParticleEffect, EffectMaterial) {
        self.bundle(&self.cast[e.index()], e)
    }
    /// Shockwave debris.
    pub fn shock(&self) -> (ParticleEffect, EffectMaterial) {
        self.bundle(&self.shock, Element::Aura)
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins(HanabiPlugin)
        .init_resource::<VfxLimits>()
        .init_resource::<VfxLoad>()
        .add_systems(Startup, build)
        .add_systems(PostStartup, test_rig)
        .add_systems(Update, (flash, expire))
        .add_systems(PostUpdate, limit_effects
            .after(bevy::transform::TransformSystems::Propagate)
            .before(bevy_hanabi::EffectSystems::CompileEffects)
            .before(bevy_hanabi::EffectSystems::TickSpawners))
        .add_systems(Last, (skip_empty_uploads, report_load));
}

fn build(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut effects: ResMut<Assets<EffectAsset>>,
    mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let trail = Element::ALL.map(|e| effects.add(trail_effect(e)));
    let burst = Element::ALL.map(|e| effects.add(burst_effect(e)));
    let cast = Element::ALL.map(|e| effects.add(cast_effect(e)));
    let rise = Element::ALL.map(|e| effects.add(rise_effect(e)));
    let shock = effects.add(shock_debris());
    let sprite = Element::ALL.map(|e| images.add(sprite_image(e)));
    let cores = Element::ALL.map(|e| std::array::from_fn(|i| glow(&mut materials, e.charged_core(i as f32 * 0.5))));
    commands.insert_resource(Vfx {
        trail,
        burst,
        cast,
        rise,
        shock,
        sprite,
        core_mesh: meshes.add(Sphere::new(1.0).mesh().ico(1).expect("valid icosphere")),
        ball_mesh: meshes.add(Sphere::new(1.0).mesh().uv(16, 10)),
        ring_mesh: meshes.add(ring(0.94, 1.0)),
        crescent_mesh: meshes.add(crescent(0.95, 2.3, 0.25)),
        stone_material: materials.add(StandardMaterial { base_color: Color::srgb(0.36, 0.4, 0.43),
            perceptual_roughness: 0.95, ..default() }),
        cores,
    });
}

/// Run before Hanabi compiles/extracts instances, so rejected decoration allocates no GPU buffer.
fn limit_effects(
    mut commands: Commands, limits: Res<VfxLimits>, mut load: ResMut<VfxLoad>,
    admitted: Query<Entity, (With<ParticleEffect>, With<AdmittedEmitter>)>,
    fresh: Query<(Entity, &GlobalTransform), (With<ParticleEffect>, Without<AdmittedEmitter>)>,
    player: Query<&GlobalTransform, With<super::Player>>,
    flashes: Query<(Entity, &Flash)>,
    ghosts: Query<(), With<super::afterimage::Afterimage>>,
) {
    let eye = player.iter().next().map(|t| t.translation());
    let mut count = admitted.iter().count();
    let mut candidates: Vec<_> = fresh.iter().map(|(e, tf)|
        (e, eye.map_or(0.0, |p| p.distance_squared(tf.translation())))).collect();
    candidates.sort_unstable_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.to_bits().cmp(&b.0.to_bits())));
    for (e, distance) in candidates {
        if count >= limits.emitters || distance > 80.0 * 80.0 {
            commands.entity(e).remove::<(ParticleEffect, EffectMaterial, bevy_hanabi::CompiledParticleEffect)>();
            load.dropped_emitters += 1;
        } else {
            commands.entity(e).insert(AdmittedEmitter);
            count += 1;
        }
    }
    load.emitters = count;
    load.peak_emitters = load.peak_emitters.max(count);
    let mut accents: Vec<_> = flashes.iter().map(|(e, f)| (e, f.age)).collect();
    accents.sort_unstable_by(|a, b| a.1.total_cmp(&b.1));
    for (e, _) in accents.into_iter().skip(limits.flashes.saturating_sub(ghosts.iter().count())) {
        commands.entity(e).try_despawn();
    }
}

fn report_load(time: Res<Time<Real>>, load: Res<VfxLoad>, limits: Res<VfxLimits>,
    meshes: Res<Assets<Mesh>>, materials: Res<Assets<StandardMaterial>>, mut last: Local<f32>) {
    if time.elapsed_secs() - *last < 5.0 { return; }
    *last = time.elapsed_secs();
    info!("[vfx-budget] emitters={}/{} peak={} dropped={} sprites={}/{} peak={} dropped={} meshes={} materials={}",
        load.emitters, limits.emitters, load.peak_emitters, load.dropped_emitters,
        load.sprites, limits.sprites, load.peak_sprites, load.dropped_sprites, meshes.len(), materials.len());
}

/// Bevy's mesh allocator cannot allocate zero-byte vertex/index streams. Keep empty
/// generated or imported meshes on the CPU, where they can still be filled later.
fn skip_empty_uploads(mut events: MessageReader<AssetEvent<Mesh>>, mut meshes: ResMut<Assets<Mesh>>,
    mut deferred: Local<std::collections::HashMap<AssetId<Mesh>, RenderAssetUsages>>) {
    for event in events.read() {
        let id = match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => *id,
            AssetEvent::Removed { id } => { deferred.remove(id); continue; }
            _ => continue,
        };
        if meshes.get(id).is_some_and(|m| m.count_vertices() == 0
            && m.asset_usage.contains(RenderAssetUsages::RENDER_WORLD)) {
            if let Some(mut mesh) = meshes.get_mut(id) {
                deferred.insert(id, mesh.asset_usage);
                mesh.asset_usage = RenderAssetUsages::MAIN_WORLD;
            }
            info!("[render-budget] skipped empty mesh upload {id:?}");
        } else if meshes.get(id).is_some_and(|m| m.count_vertices() > 0)
            && let Some(usage) = deferred.remove(&id)
            && let Some(mut mesh) = meshes.get_mut(id)
        {
            mesh.asset_usage = usage;
        }
    }
}

/// `GB_VFX_TEST=1`: permanent effects in front of the spawn camera (a glow sphere, a fire and a
/// frost emitter, a lightning bolt) to check the rendering path headlessly.
fn test_rig(
    mut commands: Commands,
    vfx: Option<Res<Vfx>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if std::env::var_os("GB_VFX_TEST").is_none() {
        return;
    }
    let Some(vfx) = vfx else { return };
    let at = |x: f32| Vec3::new(x, 1.4, -9.5);
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.3))),
        MeshMaterial3d(glow(&mut materials, Element::Arcane.core())),
        Transform::from_translation(at(-1.5)),
    ));
    commands.spawn((vfx.trail(Element::Fire), Transform::from_translation(at(0.0))));
    commands.spawn((vfx.trail(Element::Frost), Transform::from_translation(at(1.5))));
    let path = lightning_path(at(-2.5), at(2.5) + Vec3::Y, 6, 0.22, 0.35, 3);
    commands.spawn((
        Mesh3d(meshes.add(ribbons(&path, 0.07))),
        MeshMaterial3d(glow(&mut materials, Element::Lightning.core())),
        Transform::default(),
    ));
}

// ---------------------------------------------------------------------------------------------
// Particle effects.

/// Finish an effect's expressions with one sprite slot (slot 0, `EffectMaterial`) multiplied into
/// the particle colour (the sprites are white with their shape in alpha): shaped motes instead of
/// hard quads.
fn finish(w: ExprWriter) -> (Module, ParticleTextureModifier) {
    let slot = w.lit(0u32).expr();
    let mut module = w.finish();
    module.add_texture_slot("sprite");
    let sprite = ParticleTextureModifier {
        texture_slot: slot,
        sample_mapping: ImageSampleMapping::Modulate,
    };
    (module, sprite)
}


fn gradient(e: Element, peak: f32) -> bevy_hanabi::Gradient<Vec4> {
    let c = e.core();
    let mut g = bevy_hanabi::Gradient::new();
    g.add_key(0.0, Vec4::new(c.red * peak, c.green * peak, c.blue * peak, 1.0));
    g.add_key(0.35, Vec4::new(c.red, c.green, c.blue, 0.9));
    g.add_key(1.0, e.tail());
    g
}

fn sizes(start: f32, end: f32) -> bevy_hanabi::Gradient<Vec3> {
    let mut g = bevy_hanabi::Gradient::new();
    g.add_key(0.0, Vec3::splat(start));
    g.add_key(1.0, Vec3::splat(end));
    g
}

/// Per-element acceleration field, evaluated per particle on the GPU.
fn field(w: &ExprWriter, e: Element) -> ExprHandle {
    let p = w.attr(Attribute::POSITION);
    let t = w.time();
    match e {
        Element::Fire => {
            // ABC flow at spatial frequency 3/m, scrolled by time, plus buoyancy.
            let (a, b, c) = (1.0, 0.8, 0.6);
            let k = w.lit(3.0);
            let x = p.clone().x() * k.clone() + t.clone();
            let y = p.clone().y() * k.clone() + t.clone() * w.lit(0.7);
            let z = p.z() * k + t * w.lit(1.3);
            let vx = z.clone().sin() * w.lit(a) + y.clone().cos() * w.lit(c);
            let vy = x.clone().sin() * w.lit(b) + z.cos() * w.lit(a) + w.lit(2.5);
            let vz = y.sin() * w.lit(c) + x.cos() * w.lit(b);
            (vx.vec3(vy, vz) * w.lit(6.0)).expr()
        }
        Element::Arcane => {
            // Vortex about the vertical through the particle's spawn area: tangent = up × p,
            // plus a breathing radial term.
            let flat = p.clone() * w.lit(Vec3::new(1.0, 0.0, 1.0));
            let tangent = w.lit(Vec3::Y).cross(flat.clone());
            let breath = (t * w.lit(5.0)).sin() * w.lit(2.0);
            (tangent * w.lit(9.0) + flat * breath).expr()
        }
        Element::Aura => w.lit(Vec3::Y * 1.6).expr(),
        Element::Frost => w.lit(Vec3::Y * -1.2).expr(),
        Element::Lightning => w.lit(Vec3::ZERO).expr(),
    }
}

/// A trail following a projectile (world-space particles left behind).
fn trail_effect(e: Element) -> EffectAsset {
    let w = ExprWriter::new();
    let init_pos = SetPositionSphereModifier {
        center: w.lit(Vec3::ZERO).expr(),
        radius: w.lit(0.09).expr(),
        dimension: ShapeDimension::Volume,
    };
    let init_vel = SetVelocitySphereModifier {
        center: w.lit(Vec3::ZERO).expr(),
        speed: w.lit(0.2).uniform(w.lit(0.9)).expr(),
    };
    let init_age = SetAttributeModifier::new(Attribute::AGE, w.lit(0.0).expr());
    let init_life = SetAttributeModifier::new(Attribute::LIFETIME, w.lit(0.25).uniform(w.lit(0.55)).expr());
    let accel = AccelModifier::new(field(&w, e));
    let drag = LinearDragModifier::new(w.lit(2.0).expr());
    let size = match e {
        Element::Fire => sizes(0.09, 0.015),
        Element::Frost => sizes(0.14, 0.05),
        Element::Lightning => sizes(0.09, 0.0),
        _ => sizes(0.2, 0.0),
    };
    let (module, sprite) = finish(w);
    EffectAsset::new(128, SpawnerSettings::rate(90.0.into()), module)
        .with_name("trail")
        .render(sprite)
        .with_simulation_space(SimulationSpace::Global)
        .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_life)
        .update(accel)
        .update(drag)
        .render(ColorOverLifetimeModifier::new(gradient(e, 1.6)))
        .render(SizeOverLifetimeModifier {
            gradient: size,
            screen_space_size: false,
        })
}

/// An impact burst (once, on spawn).
fn burst_effect(e: Element) -> EffectAsset {
    let w = ExprWriter::new();
    let count = match e {
        Element::Fire => 120.0,
        Element::Frost => 140.0,
        _ => 88.0,
    };
    let life = match e {
        Element::Lightning => (0.15, 0.4),
        Element::Frost => (0.6, 1.2),
        Element::Fire => (0.5, 1.1),
        _ => (0.4, 0.9),
    };
    let drag = match e {
        Element::Lightning => 6.0,
        Element::Frost => 3.5,
        _ => 2.5,
    };
    let init_age = SetAttributeModifier::new(Attribute::AGE, w.lit(0.0).expr());
    let init_life = SetAttributeModifier::new(Attribute::LIFETIME, w.lit(life.0).uniform(w.lit(life.1)).expr());
    let drag = LinearDragModifier::new(w.lit(drag).expr());
    let size = SizeOverLifetimeModifier {
        gradient: match e {
            Element::Frost => sizes(0.12, 0.04),
            Element::Lightning => sizes(0.09, 0.0),
            Element::Fire => sizes(0.24, 0.04),
            _ => sizes(0.16, 0.0),
        },
        screen_space_size: false,
    };
    let (init_pos, init_vel): (SetAttributeModifier, SetAttributeModifier) = if e == Element::Frost {
        // Vogel phyllotaxis: n = floor(rand · N), angle n·φ, radius c·√n, flying out along it.
        let n = (w.rand(ScalarType::Float) * w.lit(count)).floor();
        let angle = n.clone() * w.lit(GOLDEN);
        let r = n.sqrt() * w.lit(0.035);
        let dir = angle.clone().cos().vec3(w.lit(0.15), angle.sin());
        let pos = dir.clone() * r.clone();
        let vel = dir * (w.lit(2.0) + r * w.lit(6.0));
        (
            SetAttributeModifier::new(Attribute::POSITION, pos.expr()),
            SetAttributeModifier::new(Attribute::VELOCITY, vel.expr()),
        )
    } else {
        // A small ball flying out radially, each particle at its own speed.
        let speed = match e {
            Element::Lightning => (6.0, 14.0),
            Element::Arcane => (1.5, 5.0),
            _ => (2.0, 7.0),
        };
        let dir = (w.rand(VectorType::VEC3F) * w.lit(2.0) - w.lit(1.0)).normalized();
        let pos = dir.clone() * w.lit(0.15) * w.rand(ScalarType::Float);
        let vel = dir * w.lit(speed.0).uniform(w.lit(speed.1));
        (
            SetAttributeModifier::new(Attribute::POSITION, pos.expr()),
            SetAttributeModifier::new(Attribute::VELOCITY, vel.expr()),
        )
    };
    let accel = AccelModifier::new(field(&w, e));
    let (module, sprite) = finish(w);
    let mut asset = EffectAsset::new(256, SpawnerSettings::once(count.into()), module)
        .with_name("burst")
        .render(sprite)
        .with_simulation_space(SimulationSpace::Global)
        .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_life)
        .update(accel)
        .update(drag)
        .render(ColorOverLifetimeModifier::new(gradient(e, 1.1)))
        .render(size);
    if matches!(e, Element::Frost | Element::Lightning) {
        asset = asset.render(OrientModifier::new(OrientMode::AlongVelocity));
    }
    asset
}

/// Gathering motes at the hand while a cast winds up: particles start on a sphere and fall
/// inward (velocity toward the centre).
fn cast_effect(e: Element) -> EffectAsset {
    let w = ExprWriter::new();
    let init_pos = SetPositionSphereModifier {
        center: w.lit(Vec3::ZERO).expr(),
        radius: w.lit(0.45).expr(),
        dimension: ShapeDimension::Surface,
    };
    let p = w.attr(Attribute::POSITION);
    let init_vel = SetAttributeModifier::new(Attribute::VELOCITY, (p * w.lit(-2.2)).expr());
    let init_age = SetAttributeModifier::new(Attribute::AGE, w.lit(0.0).expr());
    let init_life = SetAttributeModifier::new(Attribute::LIFETIME, w.lit(0.2).uniform(w.lit(0.35)).expr());
    let (module, sprite) = finish(w);
    EffectAsset::new(64, SpawnerSettings::rate(90.0.into()), module)
        .with_name("cast")
        .render(sprite)
        .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_life)
        .render(ColorOverLifetimeModifier::new(gradient(e, 1.2)))
        .render(SizeOverLifetimeModifier {
            gradient: sizes(0.05, 0.14),
            screen_space_size: false,
        })
}

/// Small rising motes around a charging caster. One bounded emitter replaces stacked aura planes.
fn rise_effect(e: Element) -> EffectAsset {
    let w = ExprWriter::new();
    let pos = SetPositionCircleModifier { center: w.lit(Vec3::new(0.0, -0.7, 0.0)).expr(),
        axis: w.lit(Vec3::Y).expr(), radius: w.lit(0.62).expr(), dimension: ShapeDimension::Surface };
    let vel = SetAttributeModifier::new(Attribute::VELOCITY, w.lit(Vec3::Y * 1.6).expr());
    let age = SetAttributeModifier::new(Attribute::AGE, w.lit(0.0).expr());
    let life = SetAttributeModifier::new(Attribute::LIFETIME, w.lit(0.6).uniform(w.lit(1.05)).expr());
    let c = e.charged_core(0.35);
    let mut colors = bevy_hanabi::Gradient::new();
    colors.add_key(0.0, Vec4::new(c.red, c.green, c.blue, 0.0));
    colors.add_key(0.2, Vec4::new(c.red, c.green, c.blue, 0.7));
    colors.add_key(1.0, Vec4::new(c.red, c.green, c.blue, 0.0));
    let (module, sprite) = finish(w);
    EffectAsset::new(64, SpawnerSettings::rate(42.0.into()), module).with_name("charge rise")
        .with_alpha_mode(bevy_hanabi::AlphaMode::Add).render(sprite)
        .init(pos).init(vel).init(age).init(life)
        .render(ColorOverLifetimeModifier::new(colors))
        .render(SizeOverLifetimeModifier { gradient: sizes(0.035, 0.055), screen_space_size: false })
}

/// Shockwave debris: chunks thrown up and out in a ring, falling back under gravity.
fn shock_debris() -> EffectAsset {
    let w = ExprWriter::new();
    let init_pos = SetPositionCircleModifier {
        center: w.lit(Vec3::ZERO).expr(),
        axis: w.lit(Vec3::Y).expr(),
        radius: w.lit(0.6).expr(),
        dimension: ShapeDimension::Volume,
    };
    let p = w.attr(Attribute::POSITION);
    let up = w.lit(3.0).uniform(w.lit(6.5));
    let out = w.lit(2.0).uniform(w.lit(5.0));
    let vel = p.normalized() * out + w.lit(Vec3::Y) * up;
    let init_vel = SetAttributeModifier::new(Attribute::VELOCITY, vel.expr());
    let init_age = SetAttributeModifier::new(Attribute::AGE, w.lit(0.0).expr());
    let init_life = SetAttributeModifier::new(Attribute::LIFETIME, w.lit(0.6).uniform(w.lit(1.1)).expr());
    let gravity = AccelModifier::new(w.lit(Vec3::Y * -14.0).expr());
    let mut g = bevy_hanabi::Gradient::new();
    g.add_key(0.0, Vec4::new(2.5, 4.5, 4.2, 1.0));
    g.add_key(0.4, Vec4::new(0.55, 0.5, 0.45, 1.0));
    g.add_key(1.0, Vec4::new(0.3, 0.27, 0.24, 0.0));
    let (module, sprite) = finish(w);
    EffectAsset::new(96, SpawnerSettings::once(64.0.into()), module)
        .with_name("shock")
        .render(sprite)
        .with_simulation_space(SimulationSpace::Global)
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_life)
        .update(gravity)
        .render(ColorOverLifetimeModifier::new(g))
        .render(SizeOverLifetimeModifier {
            gradient: sizes(0.14, 0.08),
            screen_space_size: false,
        })
}

// ---------------------------------------------------------------------------------------------
// Procedural sprites.

/// Sprite edge (texels).
const SPRITE: usize = 64;

fn smoothstep(a: f32, b: f32, value: f32) -> f32 {
    let t = ((value - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Alpha of an element's particle sprite at (x, y) in -1..1 (0 outside the shape):
/// - Aura: a Gaussian glow.
/// - Fire: a flame tongue: a blob stretched upward whose edge is eaten by value noise, so every
///   mote has a ragged, flickering outline.
/// - Frost: a six-fold snowflake: the angle is folded into one 30° sector (mirror symmetry of the
///   hexagonal crystal), where a thin arm along the sector edge carries two side branches at 60°.
/// - Lightning: a four-point spark (two crossed exponential streaks) around a hot core.
/// - Arcane: a soft ring with a faint core.
pub fn sprite_alpha(e: Element, x: f32, y: f32) -> f32 {
    let r = (x * x + y * y).sqrt();
    let a = match e {
        Element::Aura => (-r * r * 4.0).exp(),
        Element::Fire => {
            let ny = y + 0.25;
            let stretch = Vec2::new(x * 1.5, if ny > 0.0 { ny * 0.8 } else { ny * 1.6 }).length();
            let n = vnoise(x * 4.0 + 7.0, y * 4.0) * 0.35 + vnoise(x * 9.0, y * 9.0 + 3.0) * 0.15;
            ((1.0 - stretch - n + 0.25) * 3.0).clamp(0.0, 1.0) * (-r * r).exp()
        }
        Element::Frost => {
            // Binbun3D StatusFXFree ice_particle (CC0), baked once into the particle sprite.
            let circle = (1.0 - r).max(0.0);
            let branches = ((r * std::f32::consts::PI * 5.0).sin() * 0.5 + 0.5).powi(10) * circle;
            let streaks = (y.atan2(x) * 6.0).cos() + circle.powi(4) * 4.0 + branches * 3.0;
            smoothstep(0.85, 1.2, streaks)
        }
        Element::Lightning => {
            let streak = |u: f32, v: f32| (-u.abs() * 3.0).exp() * (-v.abs() * 40.0).exp();
            (streak(x, y) + streak(y, x) + (-r * r * 25.0).exp()).min(1.0)
        }
        Element::Arcane => {
            // StatusFXFree streaks.gdshader overlay/radial mask, with a faint ring under it.
            let angle = x.atan2(y);
            let a = (angle * 5.0 + 0.4).sin();
            let b = (angle * 11.0 - 0.4).sin();
            let overlay = if a < 0.5 { 2.0 * a * b } else { 1.0 - 2.0 * (1.0 - a) * (1.0 - b) };
            let mask = (1.0 - r).max(0.0);
            let mut value = overlay - (1.0 - r) * 0.4;
            value *= 1.0 - (r - value * 0.5);
            value = (value * mask).min(mask);
            let hole = ((r - 0.1) / 0.3).clamp(0.0, 1.0).powi(4);
            smoothstep(0.1, 1.0, value) * hole + 0.35 * (-((r - 0.62) / 0.12).powi(2)).exp()
        },
    };
    // Fade to nothing at the sprite edge so no quad corner ever shows.
    (a * (1.0 - r).clamp(0.0, 1.0).sqrt()).clamp(0.0, 1.0)
}

/// Smooth value noise on an integer lattice (for sprite edges).
fn vnoise(x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let h = |i: f32, j: f32| {
        let mut n = (i as i32).wrapping_mul(374_761_393) ^ (j as i32).wrapping_mul(668_265_263);
        n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
        ((n ^ (n >> 16)) & 0xffff) as f32 / 65535.0
    };
    let top = h(x0, y0) + (h(x0 + 1.0, y0) - h(x0, y0)) * sx;
    let bottom = h(x0, y0 + 1.0) + (h(x0 + 1.0, y0 + 1.0) - h(x0, y0 + 1.0)) * sx;
    top + (bottom - top) * sy
}

fn sprite_image(e: Element) -> Image {
    let n = SPRITE;
    let mut data = Vec::with_capacity(n * n * 4);
    for j in 0..n {
        for i in 0..n {
            let x = (i as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let y = 1.0 - (j as f32 + 0.5) / n as f32 * 2.0;
            let a = (sprite_alpha(e, x, y) * 255.0 + 0.5) as u8;
            data.extend([255, 255, 255, a]);
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: n as u32,
            height: n as u32,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Despawned once `left` reaches 0 (bursts, finished trails).
#[derive(Component)]
pub struct Expire {
    pub left: f32,
}

/// A one-shot particle burst at `at`.
pub fn burst(commands: &mut Commands, effect: (ParticleEffect, EffectMaterial), at: Vec3, scale: f32) {
    commands.spawn((
        Name::new("Burst"),
        effect,
        Transform::from_translation(at).with_scale(Vec3::splat(scale.max(0.1))),
        Expire { left: 1.3 },
    ));
}

/// An impact: shaped particles and a compact shared-mesh core. Bloom supplies the glow;
/// transient point lights on every mote/impact added clustered-light work without useful detail.
pub fn impact(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    vfx: &Vfx,
    e: Element,
    at: Vec3,
    scale: f32,
) {
    burst(commands, vfx.burst(e), at, scale);
    // A scorch / frost / crack on the ground below, if it is near (`marks`).
    commands.spawn((super::marks::MarkRequest { element: e, size: scale }, Transform::from_translation(at)));
    let color = e.core();
    commands.spawn((
        Name::new("Impact glow"),
        Mesh3d(vfx.core_mesh.clone()),
        MeshMaterial3d(glow(materials, color)),
        Transform::from_translation(at).with_scale(Vec3::splat(0.12 * scale)),
        Flash {
            age: 0.0,
            life: 0.16,
            grow: 5.0,
            color: color * 1.2,
        },
        bevy::light::NotShadowCaster,
    ));
}

fn expire(time: Res<Time>, mut commands: Commands, mut q: Query<(Entity, &mut Expire)>) {
    let dt = time.delta_secs();
    for (e, mut x) in &mut q {
        x.left -= dt;
        if x.left <= 0.0 {
            commands.entity(e).try_despawn();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Procedural meshes.

/// A fading additive mesh: `life` s, growing by `grow` (scale per second).
#[derive(Component)]
pub struct Flash {
    pub age: f32,
    pub life: f32,
    pub grow: f32,
    pub color: LinearRgba,
}

#[derive(Component)]
pub struct RingPulse {
    pub radius: f32,
}

pub fn ring_pulse(commands: &mut Commands, materials: &mut Assets<StandardMaterial>, vfx: &Vfx,
    element: Element, at: Vec3, radius: f32) {
    let color = element.core() * 0.45;
    commands.spawn((
        Name::new("Spell ripple"), Mesh3d(vfx.ring_mesh.clone()),
        MeshMaterial3d(glow(materials, color)),
        Transform::from_translation(at).with_scale(Vec3::splat(radius * 0.15)),
        RingPulse { radius },
        Flash { age: 0.0, life: 0.38, grow: 0.0, color },
        bevy::light::NotShadowCaster,
    ));
}

/// Unlit additive HDR material.
pub fn glow(materials: &mut Assets<StandardMaterial>, color: LinearRgba) -> Handle<StandardMaterial> {
    // Unlit materials output their base colour (emissive is ignored), so the HDR colour goes there.
    materials.add(StandardMaterial {
        base_color: Color::LinearRgba(color),
        unlit: true,
        alpha_mode: bevy::prelude::AlphaMode::Add,
        cull_mode: None,
        double_sided: true,
        ..default()
    })
}

fn flash(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(Entity, &mut Flash, &mut Transform, &MeshMaterial3d<StandardMaterial>, Option<&Bolt>, Option<&RingPulse>)>,
) {
    let dt = time.delta_secs();
    for (e, mut f, mut tf, mat, bolt, pulse) in &mut q {
        f.age += dt;
        if f.age >= f.life {
            commands.entity(e).try_despawn();
            continue;
        }
        let u = f.age / f.life;
        if let Some(pulse) = pulse {
            tf.scale = Vec3::splat(pulse.radius * (0.15 + 0.85 * (1.0 - (1.0 - u).powi(2))));
        } else {
            tf.scale *= 1.0 + f.grow * dt;
        }
        if let Some(mut m) = materials.get_mut(&mat.0) {
            let flicker = bolt.map_or(1.0, |b| 0.72 + 0.28 * ((f.age / FLICKER).floor() + b.phase).sin().abs());
            let k = (1.0 - u) * (1.0 - u) * flicker;
            m.base_color = Color::LinearRgba(f.color * k);
        }
    }
}

/// xorshift on a seed, 0..1.
fn rnd(seed: &mut u32) -> f32 {
    let mut x = (*seed).max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *seed = x;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// A fractal lightning path from `a` to `b`: midpoint displacement for `depth` halvings, each
/// offsetting the midpoint perpendicular to its segment by up to `jag` × its length, with a
/// `branch` chance per split of a side bolt (half as long, wandering off at up to 45°).
/// Returns segments with a width weight (1 for the trunk, less for branches).
pub fn lightning_path(a: Vec3, b: Vec3, depth: u32, jag: f32, branch: f32, seed: u32) -> Vec<(Vec3, Vec3, f32)> {
    let mut seed = seed;
    let mut out = Vec::new();
    let mut stack = vec![(a, b, depth, 1.0f32)];
    while let Some((p, q, d, w)) = stack.pop() {
        if d == 0 {
            out.push((p, q, w));
            continue;
        }
        let along = q - p;
        let len = along.length();
        let n1 = along.any_orthonormal_vector();
        let n2 = along.normalize_or(Vec3::Y).cross(n1);
        let (s, t) = (rnd(&mut seed) * 2.0 - 1.0, rnd(&mut seed) * 2.0 - 1.0);
        let mid = (p + q) * 0.5 + (n1 * s + n2 * t) * jag * len;
        stack.push((p, mid, d - 1, w));
        stack.push((mid, q, d - 1, w));
        if w > 0.3 && rnd(&mut seed) < branch {
            let side = (n1 * (rnd(&mut seed) * 2.0 - 1.0) + n2 * (rnd(&mut seed) * 2.0 - 1.0)).normalize_or(n1);
            let dir = (along.normalize_or(Vec3::Y) + side * rnd(&mut seed)).normalize_or(side);
            stack.push((mid, mid + dir * len * 0.5, d.saturating_sub(1), w * 0.55));
        }
    }
    out
}

/// Crossed ribbons (two perpendicular quads) along each segment, `width` m at weight 1.
pub fn ribbons(segments: &[(Vec3, Vec3, f32)], width: f32) -> Mesh {
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for &(a, b, w) in segments {
        let d = (b - a).normalize_or(Vec3::Y);
        let n1 = d.any_orthonormal_vector() * width * w * 0.5;
        let n2 = d.cross(n1);
        for n in [n1, n2] {
            let i = pos.len() as u32;
            pos.extend([(a - n).to_array(), (a + n).to_array(), (b + n).to_array(), (b - n).to_array()]);
            idx.extend([i, i + 1, i + 2, i, i + 2, i + 3]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(idx))
}

/// Brightness flickers without replacing and re-uploading the ribbon mesh each frame.
#[derive(Component)]
pub struct Bolt {
    phase: f32,
}

/// Spawn a flickering bolt from `a` to `b`.
pub fn spawn_bolt(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    a: Vec3,
    b: Vec3,
    seed: u32,
) {
    let path = lightning_path(a, b, 4, 0.18, 0.15, seed);
    let color = Element::Lightning.core() * 0.9;
    commands.spawn((
        Name::new("Bolt"),
        Mesh3d(meshes.add(ribbons(&path, 0.07))),
        MeshMaterial3d(glow(materials, color)),
        Transform::default(),
        Bolt { phase: (seed % 1024) as f32 },
        Flash {
            age: 0.0,
            life: BOLT_LIFE,
            grow: 0.0,
            color,
        },
        bevy::light::NotShadowCaster,
    ));
}

/// Frost/arcane lances use a straight narrow ray, while lightning keeps its branching silhouette.
pub fn spawn_ray(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>,
    e: Element, a: Vec3, b: Vec3, seed: u32) {
    if e == Element::Lightning {
        spawn_bolt(commands, meshes, materials, a, b, seed);
        return;
    }
    let color = e.core() * 0.9;
    commands.spawn((
        Name::new("Element lance"),
        Mesh3d(meshes.add(ribbons(&[(a, b, 1.0)], 0.08))),
        MeshMaterial3d(glow(materials, color)),
        Transform::default(),
        Flash { age: 0.0, life: 0.24, grow: 0.0, color },
        bevy::light::NotShadowCaster,
    ));
}

/// A tapered crescent in its local XZ plane, opening toward −Z (the blade's leading edge is +Z),
/// `radius` m, `arc` rad, `width` m thick at the middle.
pub fn crescent(radius: f32, arc: f32, width: f32) -> Mesh {
    let n = 24;
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for i in 0..=n {
        let u = i as f32 / n as f32;
        let a = (u - 0.5) * arc;
        let taper = (u * std::f32::consts::PI).sin().powf(0.7);
        let dir = Vec3::new(a.sin(), 0.0, a.cos());
        let outer = dir * radius;
        let inner = dir * (radius - width * taper);
        pos.push(outer.to_array());
        pos.push(inner.to_array());
        if i < n {
            let k = (i * 2) as u32;
            idx.extend([k, k + 1, k + 2, k + 1, k + 3, k + 2]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(idx))
}

/// A flat annulus in XZ.
pub fn ring(inner: f32, outer: f32) -> Mesh {
    let n = 64;
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        pos.push((d * outer).to_array());
        pos.push((d * inner).to_array());
        if i < n {
            let k = (i * 2) as u32;
            idx.extend([k, k + 1, k + 2, k + 1, k + 3, k + 2]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(idx))
}

/// A soft flat ring (XZ plane, normal +Y): white at radius `peak`, fading to black at `inner` and `outer` through
/// vertex colours. On an additive material black adds nothing, so its edges melt into the scene.
pub fn soft_ring(inner: f32, peak: f32, outer: f32) -> Mesh {
    let n = 48;
    let radii = [(inner, 0.0), (peak, 1.0), (outer, 0.0)];
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut col: Vec<[f32; 4]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        for (r, c) in radii {
            pos.push((d * r).to_array());
            col.push([c, c, c, 1.0]);
        }
        if i < n {
            let k = (i * 3) as u32;
            for j in 0..2 {
                let (a0, a1, b0, b1) = (k + j, k + j + 1, k + 3 + j, k + 3 + j + 1);
                idx.extend([a0, a1, b0, a1, b1, b0]);
            }
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lightning_reaches_both_ends_and_stays_near_the_line() {
        let (a, b) = (Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0));
        let path = lightning_path(a, b, 6, 0.22, 0.35, 7);
        let trunk: Vec<_> = path.iter().filter(|s| s.2 == 1.0).collect();
        assert_eq!(trunk.len(), 64, "2^depth trunk segments");
        assert!(trunk.iter().any(|s| s.0.distance(a) < 1e-4 || s.1.distance(a) < 1e-4));
        assert!(trunk.iter().any(|s| s.0.distance(b) < 1e-4 || s.1.distance(b) < 1e-4));
        // Sum of the displacement series bounds the wander: jag·L·(1/2 + 1/4 + ...)·√2.
        for s in &trunk {
            let off = Vec2::new(s.0.x, s.0.y).length();
            assert!(off < 0.22 * 10.0 * 1.5, "wandered {off}");
        }
        assert!(path.len() > trunk.len(), "branches grow");
        assert_ne!(path, lightning_path(a, b, 6, 0.22, 0.35, 8), "seeded");
    }

    #[test]
    fn sprites_are_bounded_and_shaped() {
        for e in Element::ALL {
            let centre = sprite_alpha(e, 0.0, 0.0);
            assert!(centre > 0.2 || e == Element::Arcane, "{e:?} centre {centre}");
            for i in 0..=20 {
                let t = i as f32 / 20.0 * 2.0 - 1.0;
                for (x, y) in [(t, 1.0), (1.0, t), (t, -1.0), (-1.0, t)] {
                    assert!(sprite_alpha(e, x, y) < 0.02, "{e:?} edge");
                }
            }
        }
        // The snowflake is six-fold: equal at angles 60° apart.
        let at = |deg: f32| sprite_alpha(Element::Frost, 0.5 * deg.to_radians().cos(), 0.5 * deg.to_radians().sin());
        assert!((at(10.0) - at(70.0)).abs() < 1e-3 && (at(10.0) - at(250.0)).abs() < 1e-3);
        // The ring is brighter on the ring than at its centre.
        assert!(sprite_alpha(Element::Arcane, 0.62, 0.0) > sprite_alpha(Element::Arcane, 0.0, 0.0));
    }

    #[test]
    fn meshes_are_well_formed() {
        for m in [crescent(1.2, 2.2, 0.35), ring(0.8, 1.0), ribbons(&[(Vec3::ZERO, Vec3::Z, 1.0)], 0.1)] {
            let n = m.count_vertices();
            let Some(Indices::U32(idx)) = m.indices() else { panic!("indices") };
            assert!(idx.len() % 3 == 0 && idx.iter().all(|&i| (i as usize) < n));
        }
    }
}
