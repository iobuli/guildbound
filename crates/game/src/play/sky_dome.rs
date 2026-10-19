//! The sky dome (E6): a large unlit shell round the camera, its colour running from the horizon
//! (the fog colour, so far hills melt into it) up to a deeper zenith, with a sun disc by day and a
//! moon by night. It follows `sky::light_at` and the weather (`weather::shade`): clouds grey the
//! whole dome and hide the discs. Fog never touches it (`fog_enabled: false`); it casts and takes
//! no shadow. The vertex colours are rewritten only when the colours move by `RECOLOUR` (the dome
//! is a few hundred vertices).
use super::camera::MainCamera;
use super::sky::{Clock, light_at};
use super::weather::{Weather, shade};
use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

/// Dome radius (m): inside the camera's far plane, far past the fog's end.
const RADIUS: f32 = 400.0;
/// Rings from the horizon (just below it) to the zenith, and segments round.
const RINGS: usize = 16;
const SEGMENTS: usize = 32;
/// The dome dips this far below the horizon (rad) so the ground's edge never shows a gap.
const BELOW: f32 = 0.25;
/// Disc sizes (m at `RADIUS`) and distance in from the dome.
const SUN_SIZE: f32 = 14.0;
const MOON_SIZE: f32 = 10.0;
const DISC_AT: f32 = RADIUS * 0.9;
/// Smallest colour change worth rewriting the dome for.
const RECOLOUR: f32 = 0.004;

#[derive(Component)]
struct Dome;
#[derive(Component)]
struct SunDisc;
#[derive(Component)]
struct MoonDisc;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, follow);
}

/// Horizon and zenith colours for a sky colour (the fog / clear colour of the hour): the zenith is
/// deeper and more saturated, the horizon a little lighter.
pub fn gradient(sky: Color) -> (Color, Color) {
    let s = sky.to_srgba();
    let horizon = Color::srgb((s.red * 1.04 + 0.02).min(1.0), (s.green * 1.04 + 0.02).min(1.0), (s.blue * 1.03 + 0.02).min(1.0));
    let lum = 0.3 * s.red + 0.59 * s.green + 0.11 * s.blue;
    let deepen = |c: f32| ((c - lum) * 1.35 + lum) * 0.78;
    let zenith = Color::srgb(deepen(s.red).clamp(0.0, 1.0), deepen(s.green).clamp(0.0, 1.0), (deepen(s.blue) + 0.04).clamp(0.0, 1.0));
    (horizon, zenith)
}

/// Colour at elevation `el` (rad, 0 = horizon): horizon below a little above the horizon, easing
/// to the zenith overhead.
pub fn colour_at(horizon: Color, zenith: Color, el: f32) -> Color {
    let t = (el.max(0.0) / std::f32::consts::FRAC_PI_2).clamp(0.0, 1.0).powf(0.55);
    let (h, z) = (horizon.to_linear(), zenith.to_linear());
    Color::linear_rgb(h.red + (z.red - h.red) * t, h.green + (z.green - h.green) * t, h.blue + (z.blue - h.blue) * t)
}

/// The dome mesh (normals inward) and each vertex's elevation.
fn dome_mesh() -> (Mesh, Vec<f32>) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    let mut positions = Vec::new();
    let mut elevations = Vec::new();
    for r in 0..=RINGS {
        let el = -BELOW + (FRAC_PI_2 + BELOW) * r as f32 / RINGS as f32;
        for s in 0..=SEGMENTS {
            let az = TAU * s as f32 / SEGMENTS as f32;
            positions.push([el.cos() * az.cos() * RADIUS, el.sin() * RADIUS, el.cos() * az.sin() * RADIUS]);
            elevations.push(el);
        }
    }
    let row = SEGMENTS + 1;
    let mut indices = Vec::new();
    for r in 0..RINGS {
        for s in 0..SEGMENTS {
            let a = (r * row + s) as u32;
            let b = a + row as u32;
            // Wound to face inward (seen from the centre).
            indices.extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
        }
    }
    let normals: Vec<[f32; 3]> = positions.iter().map(|p| (-Vec3::from(*p).normalize()).to_array()).collect();
    let colours = vec![[1.0f32, 1.0, 1.0, 1.0]; positions.len()];
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_indices(Indices::U32(indices));
    (mesh, elevations)
}

/// Each dome vertex's elevation (for recolouring).
#[derive(Component)]
struct Elevations(Vec<f32>);

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (mesh, elevations) = dome_mesh();
    let unlit = |c: Color| StandardMaterial { base_color: c, unlit: true, fog_enabled: false, cull_mode: None, ..default() };
    commands.spawn((
        Name::new("Sky dome"),
        Dome,
        Elevations(elevations),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(unlit(Color::WHITE))),
        Transform::default(),
        NotShadowCaster,
        NotShadowReceiver,
    ));
    let disc = meshes.add(Circle::new(0.5));
    commands.spawn((
        Name::new("Sun disc"),
        SunDisc,
        Mesh3d(disc.clone()),
        MeshMaterial3d(materials.add(unlit(Color::srgb(4.0, 3.6, 3.0)))),
        Transform::default(),
        NotShadowCaster,
        NotShadowReceiver,
    ));
    commands.spawn((
        Name::new("Moon disc"),
        MoonDisc,
        Mesh3d(disc),
        MeshMaterial3d(materials.add(unlit(Color::srgb(1.4, 1.5, 1.7)))),
        Transform::default(),
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// Keep the dome and the discs round the camera; recolour the dome for the hour and the weather.
#[allow(clippy::type_complexity)]
fn follow(
    clock: Option<Res<Clock>>,
    weather: Option<Res<Weather>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut dome: Query<(&mut Transform, &Mesh3d, &Elevations), (With<Dome>, Without<SunDisc>, Without<MoonDisc>)>,
    mut sun: Query<(&mut Transform, &mut Visibility), (With<SunDisc>, Without<Dome>, Without<MoonDisc>)>,
    mut moon: Query<(&mut Transform, &mut Visibility), (With<MoonDisc>, Without<Dome>, Without<SunDisc>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut last: Local<Option<(Color, Color)>>,
) {
    let (Some(clock), Ok(cam)) = (clock, camera.single()) else { return };
    let c = cam.translation();
    let l = light_at(clock.hour);
    let (sky, cloud) = match weather.as_deref() {
        Some(w) => (shade(l.lux, l.ambient, l.sky, w).2, w.cloud),
        None => (l.sky, 0.0),
    };
    let (horizon, zenith) = gradient(sky);
    for (mut tf, mesh, elevations) in &mut dome {
        tf.translation = c;
        let moved = last.is_none_or(|(h, z)| colour_distance(h, horizon) > RECOLOUR || colour_distance(z, zenith) > RECOLOUR);
        if moved && let Some(m) = meshes.get_mut(&mesh.0) {
            let colours: Vec<[f32; 4]> = elevations.0.iter().map(|el| colour_at(horizon, zenith, *el).to_linear().to_f32_array()).collect();
            m.insert_attribute(Mesh::ATTRIBUTE_COLOR, VertexAttributeValues::Float32x4(colours));
            *last = Some((horizon, zenith));
        }
    }
    // The light travels along `l.dir`; the body it comes from is the other way. By day that is the
    // sun, by night the moon; heavy cloud hides either.
    let day = super::sky::elevation(clock.hour) >= 0.0;
    let toward = -l.dir.normalize_or(Vec3::Y);
    let clear = cloud < 0.6;
    let place = |tf: &mut Transform, size: f32| {
        let at = c + toward * DISC_AT;
        *tf = Transform::from_translation(at).looking_at(c, Vec3::Y).with_scale(Vec3::splat(size));
    };
    for (mut tf, mut vis) in &mut sun {
        let show = day && clear && toward.y > -0.05;
        vis.set_if_neq(if show { Visibility::Visible } else { Visibility::Hidden });
        if show {
            place(&mut tf, SUN_SIZE);
        }
    }
    for (mut tf, mut vis) in &mut moon {
        let show = !day && clear && toward.y > -0.05;
        vis.set_if_neq(if show { Visibility::Visible } else { Visibility::Hidden });
        if show {
            place(&mut tf, MOON_SIZE);
        }
    }
}

fn colour_distance(a: Color, b: Color) -> f32 {
    let (a, b) = (a.to_linear(), b.to_linear());
    (a.red - b.red).abs().max((a.green - b.green).abs()).max((a.blue - b.blue).abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_zenith_is_deeper_than_the_horizon() {
        let (h, z) = gradient(Color::srgb(0.55, 0.68, 0.82));
        let (h, z) = (h.to_srgba(), z.to_srgba());
        assert!(z.red + z.green + z.blue < h.red + h.green + h.blue, "darker overhead");
        assert!(z.blue - z.red > h.blue - h.red, "bluer overhead");
        // Night stays dark at both ends.
        let (h, z) = gradient(Color::srgb(0.035, 0.05, 0.11));
        assert!(h.to_srgba().blue < 0.2 && z.to_srgba().blue < 0.2);
    }

    #[test]
    fn colour_runs_from_horizon_to_zenith() {
        let (h, z) = (Color::srgb(1.0, 1.0, 1.0), Color::srgb(0.0, 0.0, 0.0));
        assert!(colour_distance(colour_at(h, z, -0.2), h) < 1e-5, "below the horizon: horizon colour");
        assert!(colour_distance(colour_at(h, z, std::f32::consts::FRAC_PI_2), z) < 1e-5);
        let mid = colour_at(h, z, 0.6).to_linear().red;
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn the_dome_is_closed_and_faces_in() {
        let (mesh, el) = dome_mesh();
        let n = mesh.count_vertices();
        assert_eq!(n, (RINGS + 1) * (SEGMENTS + 1));
        assert_eq!(el.len(), n);
        assert!(el.iter().cloned().fold(f32::MAX, f32::min) < 0.0, "dips below the horizon");
        if let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            assert!(p.iter().all(|v| (Vec3::from(*v).length() - RADIUS).abs() < 0.01));
        }
    }
}
