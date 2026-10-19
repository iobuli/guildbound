//! Spell marks: scorches, frost and cracks left on the ground where magic lands.
//! - `vfx::impact` spawns a `MarkRequest` (element, size) at every magic impact; `place` turns it
//!   into a flat decal on the surface below (a ray down at most `MAX_DROP` m, so blows that land
//!   on a body leave nothing), lying along the surface normal, turned at random, then despawns the
//!   request.
//! - Which mark: fire → scorch, lightning and arcane → char, frost → frost, aura (Quake, a ward
//!   turned) → crack (with its normal map).
//! - A mark holds `HOLD` s, fades over `FADE` s and goes; at most `MAX_MARKS` stay (the oldest go
//!   first).
//!
//! Textures: `assets/textures/marks/{scorch,char,frost,crack}.png` (+ `crack_n.png`).
//! Ownership: `Mark` entities and their materials (written here only).
use super::Phys;
use super::vfx::Element;
use avian3d::prelude::*;
use bevy::prelude::*;

/// Farthest a mark is laid below the impact (m).
const MAX_DROP: f32 = 1.0;
/// Seconds a mark stays at full strength, then fades out over.
const HOLD: f32 = 20.0;
const FADE: f32 = 10.0;
const MAX_MARKS: usize = 48;
/// Mark width per unit of impact size (m), and its bounds.
const SIZE_PER_SCALE: f32 = 1.3;
const SIZE_RANGE: (f32, f32) = (0.6, 7.0);
/// Lift off the surface (m) and the depth bias, so marks never flicker into the ground.
const LIFT: f32 = 0.015;
const DEPTH_BIAS: f32 = 40.0;

/// A request for a mark under `Transform`'s translation (see the module docs).
#[derive(Component, Clone, Copy, Debug)]
pub struct MarkRequest {
    pub element: Element,
    /// Impact size (`vfx::impact`'s scale).
    pub size: f32,
}

#[derive(Component, Clone, Copy, Debug)]
struct Mark {
    age: f32,
    /// Placement order (oldest go first past `MAX_MARKS`).
    order: u64,
}

#[derive(Resource)]
struct MarkAssets {
    quad: Handle<Mesh>,
    scorch: Handle<Image>,
    char: Handle<Image>,
    frost: Handle<Image>,
    crack: Handle<Image>,
    crack_n: Handle<Image>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load).add_systems(Update, (place, fade).chain());
}

fn load(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(MarkAssets {
        quad: meshes.add(Plane3d::default().mesh().size(1.0, 1.0)),
        scorch: assets.load("textures/marks/scorch.png"),
        char: assets.load("textures/marks/char.png"),
        frost: assets.load("textures/marks/frost.png"),
        crack: assets.load("textures/marks/crack.png"),
        crack_n: assets.load("textures/marks/crack_n.png"),
    });
}

/// A rotation that turns +Y onto `normal`, then spins `spin` rad about it.
fn lay(normal: Vec3, spin: f32) -> Quat {
    Quat::from_rotation_arc(Vec3::Y, normal.normalize_or(Vec3::Y)) * Quat::from_rotation_y(spin)
}

#[allow(clippy::too_many_arguments)]
fn place(
    mut commands: Commands,
    assets: Option<Res<MarkAssets>>,
    spatial: SpatialQuery,
    requests: Query<(Entity, &Transform, &MarkRequest)>,
    marks: Query<(Entity, &Mark)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut placed: Local<u64>,
) {
    let Some(assets) = assets else { return };
    if requests.is_empty() {
        return;
    }
    let filter = SpatialQueryFilter::from_mask(Phys::World);
    let mut count = marks.iter().count();
    let mut oldest: Vec<(u64, Entity)> = marks.iter().map(|(e, m)| (m.order, e)).collect();
    oldest.sort_unstable_by_key(|(o, _)| *o);
    let mut oldest = oldest.into_iter();
    for (entity, tf, req) in &requests {
        commands.entity(entity).try_despawn();
        let from = tf.translation + Vec3::Y * 0.2;
        let Some(hit) = spatial.cast_ray(from, Dir3::NEG_Y, MAX_DROP + 0.2, true, &filter) else { continue };
        let at = from - Vec3::Y * hit.distance;
        if !at.is_finite() {
            continue;
        }
        while count >= MAX_MARKS {
            let Some((_, old)) = oldest.next() else { break };
            commands.entity(old).try_despawn();
            count -= 1;
        }
        let (image, normal_map, tint) = match req.element {
            Element::Fire => (&assets.scorch, None, Color::WHITE),
            Element::Lightning => (&assets.char, None, Color::WHITE),
            Element::Arcane => (&assets.char, None, Color::srgb(0.85, 0.7, 1.0)),
            Element::Frost => (&assets.frost, None, Color::WHITE),
            Element::Aura => (&assets.crack, Some(&assets.crack_n), Color::WHITE),
        };
        let material = materials.add(StandardMaterial {
            base_color: tint,
            base_color_texture: Some(image.clone()),
            normal_map_texture: normal_map.cloned(),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            depth_bias: DEPTH_BIAS,
            ..default()
        });
        *placed += 1;
        // A spin from the placement count and the spot, so marks never line up.
        let spin = (*placed as f32 * 2.399 + at.x * 1.7 + at.z * 0.9) % std::f32::consts::TAU;
        let size = (req.size * SIZE_PER_SCALE).clamp(SIZE_RANGE.0, SIZE_RANGE.1);
        let normal = hit.normal.normalize_or(Vec3::Y);
        commands.spawn((
            Name::new("Spell mark"),
            Mark { age: 0.0, order: *placed },
            Mesh3d(assets.quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(at + normal * LIFT).with_rotation(lay(normal, spin)).with_scale(Vec3::new(size, 1.0, size)),
            bevy::light::NotShadowCaster,
        ));
        count += 1;
    }
}

fn fade(
    time: Res<Time>,
    mut commands: Commands,
    mut marks: Query<(Entity, &mut Mark, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    for (entity, mut mark, material) in &mut marks {
        mark.age += dt;
        if mark.age >= HOLD + FADE {
            materials.remove(&material.0);
            commands.entity(entity).try_despawn();
        } else if mark.age > HOLD
            && let Some(mut m) = materials.get_mut(&material.0)
        {
            let a = 1.0 - (mark.age - HOLD) / FADE;
            m.base_color.set_alpha(a.clamp(0.0, 1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_lies_along_the_ground() {
        let up = lay(Vec3::Y, 1.0) * Vec3::Y;
        assert!(up.distance(Vec3::Y) < 1e-5);
        let slope = Vec3::new(0.3, 1.0, 0.0).normalize();
        assert!((lay(slope, 2.0) * Vec3::Y).distance(slope) < 1e-5);
    }
}
