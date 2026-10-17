//! Title insignia: anyone holding a title (`skills::Sheet::titles`) wears a small glowing badge on
//! the left breast, coloured by their newest title (a hue from its id), so standing shows on the
//! body; an honoured one (`contracts::Honours`) wears it in gold. The badge is a free entity placed each frame from the chest bone and the body's facing.
use super::contracts::Honours;
use super::skills::Sheet;
use crate::motion::Rig;
use crate::motion::bank::PoseBank;
use bevy::prelude::*;

/// Badge size (m) and where it sits from the chest bone: forward, left, up.
const SIZE: f32 = 0.06;
const FORWARD: f32 = 0.12;
const LEFT: f32 = 0.075;
const UP: f32 = 0.02;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (wear, place).chain());
}

/// The badge of one character; `title` is the one it shows.
#[derive(Component)]
struct Badge {
    owner: Entity,
    title: String,
    gold: bool,
}

/// An honoured character's badge (HDR gold).
const GOLD: LinearRgba = LinearRgba::rgb(3.2, 2.2, 0.5);

/// A colour per title: hue from the id, bright enough to bloom a little.
pub fn hue(id: &str) -> LinearRgba {
    let h = id.bytes().fold(2166136261u32, |a, b| (a ^ b as u32).wrapping_mul(16777619));
    let c = Color::hsl((h % 360) as f32, 0.85, 0.6).to_linear();
    LinearRgba::rgb(c.red * 2.2, c.green * 2.2, c.blue * 2.2)
}

/// Badges follow titles: spawned, recoloured, or dropped with their owner.
fn wear(
    mut commands: Commands,
    sheets: Query<(Entity, &Sheet, Option<&Honours>), Or<(Changed<Sheet>, Changed<Honours>)>>,
    mut badges: Query<(Entity, &mut Badge, &MeshMaterial3d<StandardMaterial>)>,
    owners: Query<(), With<Sheet>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
) {
    for (e, b, _) in &badges {
        if !owners.contains(b.owner) {
            commands.entity(e).try_despawn();
        }
    }
    for (owner, sheet, honours) in &sheets {
        let Some(title) = sheet.titles.last() else { continue };
        let gold = honours.is_some_and(|h| !h.0.is_empty());
        let color = if gold { GOLD } else { hue(title) };
        if let Some((_, mut b, mat)) = badges.iter_mut().find(|(_, b, _)| b.owner == owner) {
            if b.title != *title || b.gold != gold {
                b.title = title.clone();
                b.gold = gold;
                if let Some(mut m) = materials.get_mut(&mat.0) {
                    m.base_color = Color::LinearRgba(color);
                }
            }
            continue;
        }
        info!("[insignia] {owner} wears {title}");
        let mesh = mesh.get_or_insert_with(|| meshes.add(Cuboid::new(SIZE, SIZE, SIZE * 0.35))).clone();
        commands.spawn((
            Name::new("Title badge"),
            Badge {
                owner,
                title: title.clone(),
                gold,
            },
            Mesh3d(mesh),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::LinearRgba(color),
                unlit: true,
                ..default()
            })),
            Transform::from_scale(Vec3::ZERO),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// On the chest, turned to the body's facing and set on its point (a diamond).
fn place(
    bank: Option<Res<PoseBank>>,
    mut badges: Query<(&Badge, &mut Transform)>,
    bodies: Query<(&Transform, Option<&Rig>), Without<Badge>>,
    globals: Query<&GlobalTransform>,
) {
    let Some(bank) = bank else { return };
    for (b, mut tf) in &mut badges {
        let Ok((body, rig)) = bodies.get(b.owner) else { continue };
        let chest = rig
            .and_then(|r| r.bones.get(bank.bones.spine[2]).copied())
            .filter(|e| *e != Entity::PLACEHOLDER)
            .and_then(|e| globals.get(e).ok())
            .map(|g| g.translation())
            .filter(|p| p.is_finite());
        let Some(chest) = chest else {
            tf.scale = Vec3::ZERO;
            continue;
        };
        let rot = body.rotation;
        let (fwd, left) = (rot * Vec3::Z, rot * Vec3::X);
        tf.translation = chest + fwd * FORWARD + left * LEFT + Vec3::Y * UP;
        tf.rotation = rot * Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
        tf.scale = Vec3::ONE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hues_are_stable_and_differ() {
        assert_eq!(hue("duelist"), hue("duelist"));
        assert_ne!(hue("duelist"), hue("pyromancer"));
    }
}
