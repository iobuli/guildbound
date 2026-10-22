//! Energy beams: a port of the MIT "Energy Beams FX" for Godot (assets/vfx/beams/LICENSE_energy_beams_fx.md).
//! One beam is four additive tube layers (core, body, halo, aura) sharing a radius envelope, a muzzle flare, a tip flare
//! and a contact shock ring that keeps blooming where it lands. `assets/shaders/energy_beam.wgsl` bends the tube (wave,
//! arc, helix, coil, whip), runs radial pulses along it and paints one of ten patterns; `energy_quad.wgsl` draws the
//! camera-facing flares and shock. Ten presets (`PRESETS`) carry the Godot scenes' values.
//!
//! - `spawn_beam(.., preset, from, to, life)`: a beam from `from` toward `to` (it points down its local -Z), fading in,
//!   living `life` s, fading out, then despawning.
//! - `GB_BEAM=<preset index or name>[:<length m>]` (smoke runs, recordings): from 2 s the player fires that preset ahead
//!   every few seconds. Unset: nothing happens.
use super::Player;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use std::f32::consts::TAU;

/// Radius envelope shared by every tube layer (Godot `start_narrow`, `tip_flare`).
const START_NARROW: f32 = 0.30;
const TIP_FLARE: f32 = 0.14;
const FADE_IN: f32 = 0.32;
const FADE_OUT: f32 = 0.30;

/// One look: colours, pattern, bend, pulse and flare, from the Godot preset scenes.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub name: &'static str,
    pub color: [f32; 3],
    pub core: [f32; 3],
    pub style: u32,
    pub radius: f32,
    pub intensity: f32,
    pub pattern_scale: f32,
    pub motion: u32,
    pub wave_amp: f32,
    pub wave_freq: f32,
    pub wave_speed: f32,
    pub twist: f32,
    pub pulse_amp: f32,
    pub pulse_freq: f32,
    pub pulse_speed: f32,
    pub end_pulse: f32,
    pub emitter: u32,
    pub emitter_spin: f32,
    pub flow_speed: f32,
    pub flicker: f32,
}

macro_rules! preset {
    ($n:expr, $c:expr, $k:expr, $st:expr, $r:expr, $i:expr, $ps:expr, $m:expr, $wa:expr, $wf:expr, $ws:expr, $tw:expr, $pa:expr, $pf:expr, $pS:expr, $ep:expr, $em:expr, $es:expr, $fs:expr, $fl:expr) => {
        Preset { name: $n, color: $c, core: $k, style: $st, radius: $r, intensity: $i, pattern_scale: $ps, motion: $m, wave_amp: $wa, wave_freq: $wf, wave_speed: $ws, twist: $tw, pulse_amp: $pa, pulse_freq: $pf, pulse_speed: $pS, end_pulse: $ep, emitter: $em, emitter_spin: $es, flow_speed: $fs, flicker: $fl }
    };
}

pub const PRESETS: [Preset; 10] = [
    preset!("cyan_lance", [0.17, 0.85, 1.0], [1.0, 1.0, 1.0], 0, 0.15, 1.3, 1.4, 0, 0.0, 1.0, 1.0, 1.0, 0.22, 2.0, 1.6, 0.16, 0, 14.0, 1.8, 0.04),
    preset!("solar_gold", [1.0, 0.65, 0.12], [1.0, 0.945, 0.77], 1, 0.18, 1.6, 2.0, 1, 0.04, 1.2, 1.0, 1.0, 0.3, 1.4, 1.2, 0.24, 1, -22.0, 2.4, 0.05),
    preset!("violet_storm", [0.6, 0.36, 1.0], [0.94, 0.9, 1.0], 2, 0.17, 1.6, 1.5, 0, 0.0, 1.0, 1.0, 1.0, 0.42, 2.8, 2.2, 0.28, 2, 40.0, 3.0, 0.11),
    preset!("toxic_hex", [0.42, 1.0, 0.29], [0.92, 1.0, 0.88], 3, 0.16, 1.4, 1.1, 4, 0.0, 1.0, 1.0, 1.2, 0.34, 2.2, 1.0, 0.22, 3, 10.0, 1.6, 0.06),
    preset!("crimson_pulse", [1.0, 0.2, 0.33], [1.0, 0.86, 0.82], 4, 0.19, 1.5, 1.2, 2, 0.045, 1.0, 1.0, 1.0, 0.46, 1.8, 1.4, 0.3, 4, -30.0, 1.4, 0.07),
    preset!("glacier_shard", [0.66, 0.93, 1.0], [1.0, 1.0, 1.0], 5, 0.14, 1.2, 1.8, 0, 0.0, 1.0, 1.0, 1.0, 0.2, 3.4, 1.0, 0.14, 5, 6.0, 1.2, 0.03),
    preset!("prism_ray", [1.0, 1.0, 1.0], [1.0, 1.0, 1.0], 6, 0.15, 1.45, 1.0, 0, 0.0, 1.0, 1.0, 1.0, 0.28, 1.2, 1.8, 0.2, 0, 26.0, 0.8, 0.02),
    preset!("plasma_storm", [0.24, 0.55, 1.0], [0.92, 0.96, 1.0], 7, 0.18, 1.6, 1.0, 1, 0.04, 2.0, 1.6, 1.0, 0.5, 3.6, 2.4, 0.32, 2, 55.0, 2.6, 0.09),
    preset!("amber_drill", [1.0, 0.48, 0.09], [1.0, 0.91, 0.76], 8, 0.17, 1.5, 1.6, 4, 0.0, 1.0, 1.0, 1.6, 0.38, 2.4, 1.8, 0.24, 1, -16.0, 4.0, 0.05),
    preset!("nova_white", [1.0, 0.96, 0.82], [1.0, 1.0, 1.0], 9, 0.13, 1.7, 2.2, 2, 0.04, 1.0, 1.0, 1.0, 0.3, 1.6, 1.2, 0.2, 4, 20.0, 3.2, 0.04),
];

/// Index of a preset by name or number.
pub fn preset_index(s: &str) -> Option<usize> {
    s.trim().parse::<usize>().ok().filter(|i| *i < PRESETS.len()).or_else(|| PRESETS.iter().position(|p| p.name == s.trim()))
}

#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default)]
pub struct BeamParams {
    pub color: Vec4,
    pub core: Vec4,
    pub shape: Vec4,
    pub bend: Vec4,
    pub pulse: Vec4,
    pub misc: Vec4,
    pub swell: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct BeamMaterial {
    #[uniform(0)]
    pub params: BeamParams,
}

impl Material for BeamMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/energy_beam.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/energy_beam.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default)]
pub struct QuadParams {
    pub color: Vec4,
    pub core: Vec4,
    pub a: Vec4,
    pub b: Vec4,
    pub c: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct QuadMaterial {
    #[uniform(0)]
    pub params: QuadParams,
}

impl Material for QuadMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/energy_quad.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/energy_quad.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

/// The tube layers: radius factor, rings, segments, profile power, tip gain, thickness power, core energy factor, intensity factor.
const LAYERS: [(f32, usize, usize, f32, f32, f32, f32, f32); 4] = [
    (0.62, 32, 24, 1.0, 1.0, 1.85, 0.25, 1.0),
    (0.34, 30, 20, 0.65, 1.0, 0.75, 1.4, 1.15),
    (1.55, 26, 24, 1.60, 0.0, 0.50, 0.0, 0.30),
    (1.70, 22, 20, 1.45, 0.0, 0.28, 0.0, 0.07),
];

fn profile(t: f32, power: f32, tip_gain: f32) -> f32 {
    let grow = smooth(0.0, 0.62, t);
    let width = START_NARROW + (1.0 - START_NARROW) * grow;
    (width * (1.0 + TIP_FLARE * tip_gain * smooth(0.62, 1.0, t))).powf(power)
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// An open tube of unit length down -Z: uv.x around, uv.y muzzle to tip (the shader reads both), outward normals.
fn tube_mesh(radius: f32, rings: usize, segments: usize, power: f32, tip_gain: f32) -> Mesh {
    let mut pos = Vec::with_capacity((rings + 1) * (segments + 1));
    let mut nor = Vec::with_capacity(pos.capacity());
    let mut uv = Vec::with_capacity(pos.capacity());
    for i in 0..=rings {
        let t = i as f32 / rings as f32;
        let r = radius * profile(t, power, tip_gain);
        for j in 0..=segments {
            let a = TAU * j as f32 / segments as f32;
            pos.push([a.cos() * r, a.sin() * r, -t]);
            nor.push([a.cos(), a.sin(), 0.0]);
            uv.push([j as f32 / segments as f32, t]);
        }
    }
    let mut idx = Vec::with_capacity(rings * segments * 6);
    for i in 0..rings {
        for j in 0..segments {
            let a = (i * (segments + 1) + j) as u32;
            let b = a + 1;
            let c = a + (segments + 1) as u32;
            let d = c + 1;
            idx.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(bevy::mesh::Indices::U32(idx))
}

/// A beam in flight: its preset, clock and the material handles it animates.
#[derive(Component)]
pub struct EnergyBeam {
    preset: usize,
    length: f32,
    age: f32,
    life: f32,
    seed: f32,
    tubes: Vec<Handle<BeamMaterial>>,
    muzzle: (Entity, Handle<QuadMaterial>),
    tip: (Entity, Handle<QuadMaterial>),
    contact: (Entity, Handle<QuadMaterial>),
}

fn color4(c: [f32; 3], w: f32) -> Vec4 {
    Vec4::new(c[0], c[1], c[2], w)
}

fn tube_params(p: &Preset, layer: usize, length: f32, seed: f32) -> BeamParams {
    let l = LAYERS[layer];
    BeamParams {
        color: color4(if layer == 1 { lerp3(p.core, p.color, 0.25) } else { p.color }, 1.0),
        core: color4(p.core, 0.0),
        shape: Vec4::new(length, p.pattern_scale, p.flow_speed, 0.0),
        bend: Vec4::new(p.motion as f32, p.wave_amp, p.wave_freq, p.wave_speed),
        pulse: Vec4::new(p.pulse_amp, p.pulse_freq, p.pulse_speed, p.twist),
        misc: Vec4::new(p.style as f32, seed, l.5, 0.0),
        swell: Vec4::new(1.0, 1.0, 0.0, 0.0),
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn quad_params(p: &Preset, shock: bool) -> QuadParams {
    QuadParams {
        color: color4(p.color, 1.0),
        core: color4(p.core, 1.0),
        a: Vec4::new(if shock { 1.0 } else { 0.0 }, p.emitter as f32, 1.0, 1.0),
        b: Vec4::new(6.0, 26.0, 0.0, 0.0),
        c: Vec4::new(0.055, 0.0, 0.0, 0.0),
    }
}

/// Fire a beam of `preset` from `from` toward `to`. It fades in, lives `life` s, fades out and despawns.
#[allow(clippy::too_many_arguments)]
pub fn spawn_beam(commands: &mut Commands, meshes: &mut Assets<Mesh>, beams: &mut Assets<BeamMaterial>, quads: &mut Assets<QuadMaterial>, preset: usize, from: Vec3, to: Vec3, life: f32) -> Entity {
    let p = PRESETS[preset % PRESETS.len()];
    let length = from.distance(to).max(0.5);
    let dir = (to - from).normalize_or(Vec3::NEG_Z);
    let up = if dir.y.abs() > 0.98 { Vec3::X } else { Vec3::Y };
    let seed = (from.x * 7.0 + from.z * 3.0).fract().abs();
    let root_tf = Transform::from_translation(from).looking_to(dir, up);
    let tubes: Vec<Handle<BeamMaterial>> = (0..LAYERS.len()).map(|l| beams.add(BeamMaterial { params: tube_params(&p, l, length, seed) })).collect();
    let flare_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let (m_mat, t_mat, c_mat) = (quads.add(QuadMaterial { params: quad_params(&p, false) }), quads.add(QuadMaterial { params: quad_params(&p, false) }), quads.add(QuadMaterial { params: quad_params(&p, true) }));
    let mut muzzle = Entity::PLACEHOLDER;
    let mut tip = Entity::PLACEHOLDER;
    let mut contact = Entity::PLACEHOLDER;
    let root = commands
        .spawn((root_tf, Visibility::default(), Name::new("EnergyBeam")))
        .with_children(|c| {
            c.spawn((Transform::from_scale(Vec3::new(1.0, 1.0, length)), Visibility::default())).with_children(|t| {
                for (l, h) in tubes.iter().enumerate() {
                    let (rf, rings, segs, power, tip_gain, ..) = LAYERS[l];
                    t.spawn((Mesh3d(meshes.add(tube_mesh(p.radius * rf, rings, segs, power, tip_gain))), MeshMaterial3d(h.clone()), Transform::default(), bevy::light::NotShadowCaster));
                }
            });
            muzzle = c.spawn((Mesh3d(flare_mesh.clone()), MeshMaterial3d(m_mat.clone()), Transform::from_scale(Vec3::splat(p.radius * 3.0)), bevy::light::NotShadowCaster)).id();
            tip = c.spawn((Mesh3d(flare_mesh.clone()), MeshMaterial3d(t_mat.clone()), Transform::from_translation(Vec3::new(0.0, 0.0, -length)).with_scale(Vec3::splat(p.radius * 2.4)), bevy::light::NotShadowCaster)).id();
            contact = c.spawn((Mesh3d(flare_mesh.clone()), MeshMaterial3d(c_mat.clone()), Transform::from_translation(Vec3::new(0.0, 0.0, -length + 0.03)).with_scale(Vec3::splat(p.radius * 4.2)), bevy::light::NotShadowCaster)).id();
        })
        .id();
    commands.entity(root).insert(EnergyBeam { preset, length, age: 0.0, life, seed, tubes, muzzle: (muzzle, m_mat), tip: (tip, t_mat), contact: (contact, c_mat) });
    root
}

fn animate(time: Res<Time>, mut commands: Commands, mut q: Query<(Entity, &mut EnergyBeam)>, mut beams: ResMut<Assets<BeamMaterial>>, mut quads: ResMut<Assets<QuadMaterial>>, mut tf: Query<&mut Transform, Without<EnergyBeam>>) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut b) in &mut q {
        b.age += dt;
        if b.age >= b.life + FADE_OUT {
            commands.entity(entity).despawn();
            continue;
        }
        let p = PRESETS[b.preset % PRESETS.len()];
        let rise = (b.age / FADE_IN).clamp(0.0, 1.0);
        let fall = ((b.life + FADE_OUT - b.age) / FADE_OUT).clamp(0.0, 1.0);
        let energy = rise.min(fall);
        let t = b.age;
        let flick = 1.0 + p.flicker * ((t * 37.0 + b.length).sin() * 0.5 + (t * 23.0 + p.radius * 11.0).sin() * 0.5);
        let head_swell = 1.32 + (0.10 + p.end_pulse * 0.45) * (t * 3.4).sin();
        let tip_swell = 1.55 + (0.16 + p.end_pulse * 0.55) * (t * 4.2 + 1.3).sin();
        let lum = [1.0, 1.15, 0.30, 0.07];
        for (l, h) in b.tubes.iter().enumerate() {
            if let Some(mut m) = beams.get_mut(h) {
                let q = &mut m.params;
                q.shape.w = t;
                q.swell = Vec4::new(head_swell, tip_swell, p.intensity * lum[l] * energy * flick, 0.0);
                q.core.w = if l == 0 { p.intensity * 0.25 * energy } else if l == 1 { p.intensity * 1.4 * energy * 0.0 + 1.4 * energy } else { 0.0 };
                q.misc.y = b.seed;
            }
        }
        let roll = t * p.emitter_spin.to_radians();
        let pop = 1.0 + (1.0 - energy.min(1.0)) * 1.2;
        if let Ok(mut f) = tf.get_mut(b.muzzle.0) {
            f.scale = Vec3::splat(p.radius * 3.0 * (pop + 0.05 * (t * 26.0).sin()) * head_swell.clamp(0.6, 2.6));
        }
        if let Ok(mut f) = tf.get_mut(b.tip.0) {
            f.scale = Vec3::splat(p.radius * 2.4 * (1.0 + 0.10 * (t * 31.0 + 1.7).sin()) * tip_swell.clamp(0.6, 3.4));
        }
        let cp = (t % 0.8) / 0.8;
        if let Ok(mut f) = tf.get_mut(b.contact.0) {
            f.scale = Vec3::splat(p.radius * 4.2 * (0.55 + 0.85 * cp));
        }
        if let Some(mut m) = quads.get_mut(&b.muzzle.1) {
            m.params.a.z = 1.8 * energy * flick;
            m.params.a.w = 2.8 * energy;
            m.params.b.z = roll;
        }
        if let Some(mut m) = quads.get_mut(&b.tip.1) {
            m.params.a.y = 0.0;
            m.params.a.z = 0.7 * energy * flick;
            m.params.a.w = 1.1 * energy;
            m.params.b.z = roll;
        }
        if let Some(mut m) = quads.get_mut(&b.contact.1) {
            m.params.a.z = (1.0 - cp) * (1.0 - cp) * 1.15 * energy;
            m.params.b.w = cp;
            m.params.c.y = b.seed * 3.0;
        }
    }
}

#[derive(Resource, Clone, Copy)]
struct Smoke {
    preset: usize,
    length: f32,
}

fn smoke_setting() -> Option<Smoke> {
    let v = std::env::var("GB_BEAM").ok()?;
    let (name, len) = v.split_once(':').map_or((v.as_str(), None), |(a, b)| (a, Some(b)));
    Some(Smoke { preset: preset_index(name)?, length: len.and_then(|l| l.trim().parse::<f32>().ok()).filter(|l| l.is_finite() && *l > 0.5).unwrap_or(6.0) })
}

/// `GB_BEAM`: from 2 s the player fires the preset ahead every 3.2 s.
fn beam_smoke(time: Res<Time>, smoke: Option<Res<Smoke>>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut beams: ResMut<Assets<BeamMaterial>>, mut quads: ResMut<Assets<QuadMaterial>>, players: Query<(&Transform, &super::Mover), With<Player>>, mut last: Local<Option<f32>>) {
    let Some(smoke) = smoke else { return };
    let t = time.elapsed_secs();
    if t < 2.0 || last.is_some_and(|l| t - l < 3.2) {
        return;
    }
    let Ok((tf, mover)) = players.single() else { return };
    *last = Some(t);
    let fwd = Vec3::new(mover.yaw.sin(), 0.0, mover.yaw.cos());
    let from = tf.translation + Vec3::Y * 0.25 + fwd * 0.7;
    spawn_beam(&mut commands, &mut meshes, &mut beams, &mut quads, smoke.preset, from, from + fwd * smoke.length, 2.6);
    info!("[energy] beam {} ({:.1} m)", PRESETS[smoke.preset].name, smoke.length);
}

pub fn plugin(app: &mut App) {
    app.add_plugins((MaterialPlugin::<BeamMaterial>::default(), MaterialPlugin::<QuadMaterial>::default()));
    if let Some(s) = smoke_setting() {
        app.insert_resource(s);
    }
    app.add_systems(Update, (animate, beam_smoke));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_resolve_by_name_and_number() {
        assert_eq!(preset_index("prism_ray"), Some(6));
        assert_eq!(preset_index("3"), Some(3));
        assert_eq!(preset_index("99"), None);
        assert_eq!(PRESETS.len(), 10);
    }

    #[test]
    fn tube_runs_down_negative_z_and_narrows_at_the_muzzle() {
        let m = tube_mesh(0.2, 8, 12, 1.0, 1.0);
        let pos: Vec<[f32; 3]> = m.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap().to_vec();
        assert_eq!(pos.len(), 9 * 13);
        assert!((pos[0][2] - 0.0).abs() < 1e-6 && (pos.last().unwrap()[2] + 1.0).abs() < 1e-6);
        let r0 = (pos[0][0].powi(2) + pos[0][1].powi(2)).sqrt();
        let r_mid = (pos[4 * 13][0].powi(2) + pos[4 * 13][1].powi(2)).sqrt();
        assert!(r0 < r_mid, "the muzzle is narrower than the middle: {r0} vs {r_mid}");
    }
}
