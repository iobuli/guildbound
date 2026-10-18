//! Character looks: the UAL mannequin made into distinct people, all in code (no new assets).
//! Once a character is bound and has its skin (`skin::Skin`) and its pose has settled for
//! `SETTLE` s, `dress` gives it the `Look` for its name (`look_for`):
//! - Proportions: the model scales about the feet by `height` and `breadth` (modest, so IK and
//!   the capsule still fit).
//! - Accent: the colour the skin's shadow bands lean toward (`Skin.accent`).
//! - Pieces: procedural meshes on bones: a kasa (straw hat), hood, topknot, headband, scarf with
//!   tails, sash with a tail, pauldrons, pouch. Each is placed by the world pose it should have
//!   (head top, neck, pelvis, shoulders, relative to the body's facing) converted into its bone's
//!   frame once, so it then rides the animation. Pieces use the skin material too (ink look,
//!   presence rim). Debug view 13 hides them.
//! - Tails (`Flap`): scarf and sash tails hang from a pivot and swing on a spring, trailing the
//!   body's velocity and acceleration (`TAIL_SWING`, `TAIL_STIFF`, `TAIL_DAMP`).
use super::debug::{DebugToggles, View};
use super::skin::{Skin, SkinExt, SkinMaterial, SkinParams};
use super::Mover;
use crate::motion::Rig;
use crate::motion::bank::PoseBank;
use bevy::prelude::*;

/// Seconds after binding before the pose is trusted for placing pieces.
const SETTLE: f32 = 0.6;
/// Tails: swing per m/s of speed (rad), spring stiffness and damping, limit (rad).
const TAIL_SWING: f32 = 0.22;
const TAIL_ACCEL: f32 = 0.035;
const TAIL_STIFF: f32 = 40.0;
const TAIL_DAMP: f32 = 7.0;
const TAIL_MAX: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Piece {
    Kasa,
    Hood,
    Topknot,
    Headband,
    Scarf,
    Sash,
    Pauldrons,
    Pouch,
}

/// One character's look.
#[derive(Clone, Debug)]
pub struct Look {
    pub height: f32,
    pub breadth: f32,
    /// Shadow accent (linear rgb) and amount.
    pub accent: Vec4,
    /// Cloth and trim colours for the pieces.
    pub cloth: Color,
    pub trim: Color,
    pub pieces: Vec<Piece>,
}

/// The look for a character by name; unknown names get a plain sash.
pub fn look_for(name: &str) -> Look {
    use Piece::*;
    let l = |height, breadth, accent: [f32; 3], cloth: [f32; 3], trim: [f32; 3], pieces: Vec<Piece>| Look {
        height,
        breadth,
        accent: Vec4::new(accent[0], accent[1], accent[2], 0.55),
        cloth: Color::srgb(cloth[0], cloth[1], cloth[2]),
        trim: Color::srgb(trim[0], trim[1], trim[2]),
        pieces,
    };
    match name {
        "Player" => l(1.0, 1.0, [0.35, 0.4, 0.9], [0.16, 0.18, 0.38], [0.85, 0.75, 0.5], vec![Kasa, Scarf, Sash]),
        "Opponent" => l(1.02, 1.08, [0.9, 0.3, 0.25], [0.55, 0.12, 0.1], [0.2, 0.2, 0.22], vec![Headband, Pauldrons, Sash]),
        "Varro" => l(1.05, 1.1, [0.8, 0.6, 0.3], [0.2, 0.22, 0.25], [0.75, 0.6, 0.3], vec![Topknot, Pauldrons, Sash]),
        "Kael" => l(1.0, 1.06, [0.7, 0.45, 0.25], [0.45, 0.3, 0.18], [0.3, 0.25, 0.2], vec![Headband, Scarf, Pouch]),
        "Sena" => l(0.97, 0.94, [0.3, 0.6, 0.7], [0.2, 0.32, 0.38], [0.8, 0.8, 0.82], vec![Hood, Scarf, Pouch]),
        "Mira" => l(0.95, 0.93, [0.85, 0.5, 0.6], [0.7, 0.62, 0.5], [0.6, 0.2, 0.3], vec![Topknot, Scarf]),
        "Smith" => l(0.98, 1.1, [0.8, 0.4, 0.2], [0.3, 0.22, 0.15], [0.5, 0.5, 0.5], vec![Headband, Pouch, Sash]),
        "Farmer" => l(0.98, 1.0, [0.5, 0.7, 0.3], [0.5, 0.45, 0.3], [0.3, 0.4, 0.2], vec![Kasa, Sash]),
        "Rival" => l(1.06, 1.08, [0.6, 0.3, 0.8], [0.12, 0.1, 0.16], [0.6, 0.15, 0.2], vec![Hood, Pauldrons, Sash]),
        "Hunter" => l(1.0, 0.97, [0.4, 0.6, 0.3], [0.22, 0.3, 0.18], [0.45, 0.35, 0.2], vec![Hood, Scarf, Pouch]),
        _ => l(1.0, 1.0, [0.5, 0.5, 0.5], [0.35, 0.3, 0.28], [0.6, 0.55, 0.45], vec![Sash]),
    }
}

/// Dressing progress: seconds since the skin appeared.
#[derive(Component, Default)]
struct Dressing(f32);

/// Done: the look this character wears (pieces are children of its bones).
#[derive(Component)]
pub struct Dressed;

/// A visible piece (hidden by debug view 13).
#[derive(Component)]
struct PieceMesh;

/// A swinging tail: its owner and spring state (pitch, roll in the body's frame).
#[derive(Component)]
struct Flap {
    owner: Entity,
    angle: Vec2,
    rate: Vec2,
    /// Extra sideways bias (scarf tails part left and right).
    splay: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (dress, flaps, show_pieces).chain());
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn dress(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut skins: ResMut<Assets<SkinMaterial>>,
    mut characters: Query<(Entity, &Name, &Rig, &mut Skin, Option<&mut Dressing>), (Without<Dressed>, With<super::Fighter>)>,
    mut models: Query<&mut Transform>,
    globals: Query<&GlobalTransform>,
) {
    let Some(bank) = bank else { return };
    for (e, name, rig, mut skin, dressing) in &mut characters {
        let Some(mut d) = dressing else {
            commands.entity(e).try_insert(Dressing::default());
            // Proportions at once (the pose settles with them).
            let look = look_for(name.as_str());
            if let Ok(mut m) = models.get_mut(rig.model) {
                m.scale = Vec3::new(look.breadth, look.height, look.breadth);
            }
            continue;
        };
        d.0 += time.delta_secs();
        if d.0 < SETTLE {
            continue;
        }
        let look = look_for(name.as_str());
        skin.accent = look.accent;
        let Ok(root) = globals.get(e) else { continue };
        let at = |i: usize| {
            rig.bones
                .get(i)
                .copied()
                .filter(|b| *b != Entity::PLACEHOLDER)
                .and_then(|b| globals.get(b).ok().map(|g| (b, g.compute_transform())))
        };
        let b = &bank.bones;
        let (Some(head), Some(neck), Some(pelvis)) = (at(b.head), at(b.neck), at(b.pelvis)) else { continue };
        let face = root.rotation();
        let up = Vec3::Y;
        let fwd = face * Vec3::Z;
        let left = face * Vec3::X;
        let mut material = |c: Color, accent: Vec4| {
            let h = skins.add(SkinMaterial {
                base: StandardMaterial { base_color: c, perceptual_roughness: 0.85, ..default() },
                extension: SkinExt { skin: SkinParams { tint: accent, ..default() } },
            });
            skin.materials.push(h.clone());
            h
        };
        let cloth = material(look.cloth, look.accent);
        let trim = material(look.trim, look.accent);
        let straw = material(Color::srgb(0.78, 0.66, 0.4), look.accent);
        let hair = material(Color::srgb(0.06, 0.05, 0.05), look.accent);
        // Spawn `mesh` with world pose (`pos`, `rot`) as a child of `bone`.
        let place = |commands: &mut Commands, bone: (Entity, Transform), mesh: Handle<Mesh>, mat: Handle<SkinMaterial>, pos: Vec3, rot: Quat, scale: Vec3| {
            let parent = bone.1;
            let local = Transform {
                translation: parent.rotation.inverse() * (pos - parent.translation) / parent.scale,
                rotation: parent.rotation.inverse() * rot,
                scale: scale / parent.scale,
            };
            commands.spawn((PieceMesh, Mesh3d(mesh), MeshMaterial3d(mat), local, ChildOf(bone.0)));
        };
        // A tail: a pivot at `pos` on `bone`, the strip hanging below it.
        let tail = |commands: &mut Commands, bone: (Entity, Transform), mesh: Handle<Mesh>, mat: Handle<SkinMaterial>, pos: Vec3, len: f32, splay: f32| {
            let parent = bone.1;
            let pivot = commands
                .spawn((
                    Flap { owner: e, angle: Vec2::ZERO, rate: Vec2::ZERO, splay },
                    Transform::from_translation(parent.rotation.inverse() * (pos - parent.translation) / parent.scale),
                    Visibility::default(),
                    ChildOf(bone.0),
                ))
                .id();
            commands.spawn((
                PieceMesh,
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::from_xyz(0.0, -len * 0.5, 0.0),
                ChildOf(pivot),
            ));
        };
        let head_top = head.1.translation + up * 0.13;
        for piece in &look.pieces {
            match piece {
                Piece::Kasa => {
                    let m = meshes.add(Cone { radius: 0.36, height: 0.17 });
                    place(&mut commands, head, m, straw.clone(), head_top + up * 0.1, face, Vec3::ONE);
                    let rim = meshes.add(Cylinder::new(0.36, 0.012));
                    place(&mut commands, head, rim, straw.clone(), head_top + up * 0.02, face, Vec3::ONE);
                }
                Piece::Hood => {
                    let m = meshes.add(Sphere::new(1.0));
                    place(&mut commands, head, m, cloth.clone(), head_top - up * 0.05 - fwd * 0.025, face, Vec3::new(0.135, 0.15, 0.15));
                    let drape = meshes.add(Cylinder::new(0.15, 0.16));
                    place(&mut commands, neck, drape, cloth.clone(), neck.1.translation - up * 0.02, face, Vec3::ONE);
                }
                Piece::Topknot => {
                    let knot = meshes.add(Sphere::new(0.05));
                    place(&mut commands, head, knot, hair.clone(), head_top + up * 0.03 - fwd * 0.05, face, Vec3::ONE);
                    let tie = meshes.add(Cylinder::new(0.022, 0.05));
                    place(&mut commands, head, tie, trim.clone(), head_top - fwd * 0.04, face, Vec3::ONE);
                }
                Piece::Headband => {
                    let m = meshes.add(Torus { minor_radius: 0.016, major_radius: 0.115 });
                    place(&mut commands, head, m, trim.clone(), head_top - up * 0.06, face, Vec3::new(1.0, 1.0, 1.1));
                    let strip = meshes.add(Cuboid::new(0.03, 0.2, 0.012));
                    tail(&mut commands, head, strip, trim.clone(), head_top - up * 0.06 - fwd * 0.12, 0.2, 0.15);
                }
                Piece::Scarf => {
                    let m = meshes.add(Torus { minor_radius: 0.035, major_radius: 0.085 });
                    place(&mut commands, neck, m, cloth.clone(), neck.1.translation - up * 0.03, face, Vec3::ONE);
                    let strip = meshes.add(Cuboid::new(0.07, 0.42, 0.012));
                    for s in [-1.0f32, 1.0] {
                        tail(&mut commands, neck, strip.clone(), cloth.clone(), neck.1.translation - fwd * 0.09 + left * s * 0.03, 0.42, s * 0.12);
                    }
                }
                Piece::Sash => {
                    let m = meshes.add(Torus { minor_radius: 0.03, major_radius: 0.17 });
                    place(&mut commands, pelvis, m, trim.clone(), pelvis.1.translation + up * 0.06, face, Vec3::new(1.0, 1.0, 0.8));
                    let strip = meshes.add(Cuboid::new(0.06, 0.3, 0.012));
                    tail(&mut commands, pelvis, strip, trim.clone(), pelvis.1.translation + up * 0.05 + left * 0.14 + fwd * 0.04, 0.3, 0.08);
                }
                Piece::Pauldrons => {
                    for side in 0..2 {
                        let Some(arm) = at(b.upperarm[side]) else { continue };
                        let out = if side == 0 { left } else { -left };
                        let m = meshes.add(Sphere::new(1.0));
                        place(&mut commands, arm, m, trim.clone(), arm.1.translation + up * 0.05 + out * 0.03, face, Vec3::new(0.11, 0.07, 0.12));
                    }
                }
                Piece::Pouch => {
                    let m = meshes.add(Cuboid::new(0.09, 0.1, 0.05));
                    place(&mut commands, pelvis, m, cloth.clone(), pelvis.1.translation - left * 0.17 + up * 0.0 + fwd * 0.03, face, Vec3::ONE);
                }
            }
        }
        commands.entity(e).try_remove::<Dressing>().try_insert(Dressed);
        info!("[looks] {} dressed: {:?}", name.as_str(), look.pieces);
    }
}

/// Tail spring: the target angles for a body moving at `velocity` with `accel` (body frame:
/// x = left, z = forward), plus the tail's own splay. Tails trail motion (swing back, out).
pub fn tail_target(local_velocity: Vec3, local_accel: Vec3, splay: f32) -> Vec2 {
    let pitch = (local_velocity.z * TAIL_SWING + local_accel.z * TAIL_ACCEL).clamp(-TAIL_MAX, TAIL_MAX);
    let roll = (-local_velocity.x * TAIL_SWING * 0.6 - local_accel.x * TAIL_ACCEL + splay).clamp(-TAIL_MAX, TAIL_MAX);
    Vec2::new(pitch, roll)
}

fn flaps(
    time: Res<Time>,
    mut tails: Query<(&mut Flap, &mut Transform, &ChildOf)>,
    owners: Query<(&GlobalTransform, &Mover)>,
    globals: Query<&GlobalTransform>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.05);
    for (mut f, mut tf, parent) in &mut tails {
        let (Ok((root, mover)), Ok(bone)) = (owners.get(f.owner), globals.get(parent.parent())) else { continue };
        let inv = root.rotation().inverse();
        let target = tail_target(inv * mover.velocity, inv * mover.accel, f.splay);
        let accel = (target - f.angle) * TAIL_STIFF - f.rate * TAIL_DAMP;
        f.rate += accel * dt;
        let rate = f.rate;
        f.angle += rate * dt;
        // Hang straight down in the body's frame, swung by the spring: positive pitch lifts the
        // tail backward (it trails a forward run).
        let world = root.rotation() * Quat::from_rotation_x(f.angle.x) * Quat::from_rotation_z(f.angle.y);
        tf.rotation = bone.rotation().inverse() * world;
    }
}

fn show_pieces(toggles: Res<DebugToggles>, mut pieces: Query<&mut Visibility, With<PieceMesh>>) {
    if !toggles.is_changed() {
        return;
    }
    let want = if toggles.get(View::ModelPieces) { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in &mut pieces {
        *v = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_character_has_a_distinct_look() {
        let names = ["Player", "Opponent", "Varro", "Kael", "Sena", "Mira", "Smith", "Farmer", "Rival", "Hunter"];
        for (i, a) in names.iter().enumerate() {
            let la = look_for(a);
            assert!((0.9..=1.1).contains(&la.height) && (0.9..=1.12).contains(&la.breadth), "{a}");
            for b in &names[i + 1..] {
                let lb = look_for(b);
                assert!(la.pieces != lb.pieces || la.cloth != lb.cloth, "{a} and {b} look alike");
            }
        }
    }

    #[test]
    fn tails_trail_the_motion() {
        let still = tail_target(Vec3::ZERO, Vec3::ZERO, 0.0);
        assert_eq!(still, Vec2::ZERO);
        let running = tail_target(Vec3::Z * 5.0, Vec3::ZERO, 0.0);
        assert!(running.x > 0.5, "a run swings the tail back");
        assert!(tail_target(Vec3::Z * 50.0, Vec3::ZERO, 0.0).x <= TAIL_MAX);
    }
}
