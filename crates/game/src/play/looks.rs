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
//! Characters wearing a modelled look (`appearance::Appearance`, the hero bodies) keep their own
//! proportions and outfit and get only the pieces their look lists (`Piece::from_name`).
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
    /// A round shield on the left forearm.
    Shield,
    /// An iron helm with a nasal guard.
    Helm,
    /// Prayer beads round the neck (monks).
    Beads,
    /// Cloth wraps on both forearms and knuckles (monks, brawlers).
    Wraps,
    /// Two horns (demons).
    Horns,
}

impl Piece {
    /// A piece by its name in `appearance.json` (`kasa`, `hood`, `helm`...).
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "kasa" => Self::Kasa,
            "hood" => Self::Hood,
            "topknot" => Self::Topknot,
            "headband" => Self::Headband,
            "scarf" => Self::Scarf,
            "sash" => Self::Sash,
            "pauldrons" => Self::Pauldrons,
            "pouch" => Self::Pouch,
            "shield" => Self::Shield,
            "helm" => Self::Helm,
            "beads" => Self::Beads,
            "wraps" => Self::Wraps,
            "horns" => Self::Horns,
            _ => return None,
        })
    }
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
    if is_monk(name) {
        return monk_look(name);
    }
    match name {
        "Oni" => l(1.18, 1.2, [0.95, 0.15, 0.1], [0.3, 0.05, 0.05], [0.9, 0.8, 0.6], vec![Horns, Wraps, Sash]),
        "Player" => l(1.0, 1.0, [0.35, 0.4, 0.9], [0.16, 0.18, 0.38], [0.85, 0.75, 0.5], vec![Kasa, Scarf, Sash]),
        "Opponent" => l(1.02, 1.08, [0.9, 0.3, 0.25], [0.55, 0.12, 0.1], [0.2, 0.2, 0.22], vec![Headband, Pauldrons, Sash]),
        "Varro" => l(1.05, 1.1, [0.8, 0.6, 0.3], [0.2, 0.22, 0.25], [0.75, 0.6, 0.3], vec![Topknot, Pauldrons, Sash]),
        "Kael" => l(1.0, 1.06, [0.7, 0.45, 0.25], [0.45, 0.3, 0.18], [0.3, 0.25, 0.2], vec![Headband, Scarf, Pouch]),
        "Sena" => l(0.97, 0.94, [0.3, 0.6, 0.7], [0.2, 0.32, 0.38], [0.8, 0.8, 0.82], vec![Hood, Scarf, Pouch]),
        "Mira" => l(0.95, 0.93, [0.85, 0.5, 0.6], [0.7, 0.62, 0.5], [0.6, 0.2, 0.3], vec![Topknot, Scarf]),
        "Smith" => l(0.98, 1.1, [0.8, 0.4, 0.2], [0.3, 0.22, 0.15], [0.5, 0.5, 0.5], vec![Headband, Pouch, Sash]),
        "Farmer" => l(0.98, 1.0, [0.5, 0.7, 0.3], [0.5, 0.45, 0.3], [0.3, 0.4, 0.2], vec![Kasa, Sash]),
        "Rival" => l(1.06, 1.08, [0.6, 0.3, 0.8], [0.12, 0.1, 0.16], [0.6, 0.15, 0.2], vec![Hood, Pauldrons, Sash]),
        "Thoral" => l(1.3, 1.28, [0.5, 0.5, 0.6], [0.18, 0.18, 0.2], [0.55, 0.12, 0.1], vec![Helm, Pauldrons, Shield, Sash]),
        "Hunter" => l(1.0, 0.97, [0.4, 0.6, 0.3], [0.22, 0.3, 0.18], [0.45, 0.35, 0.2], vec![Hood, Scarf, Pouch]),
        _ => l(1.0, 1.0, [0.5, 0.5, 0.5], [0.35, 0.3, 0.28], [0.6, 0.55, 0.45], vec![Sash]),
    }
}

/// Monks are named by their order: Brother, Sister, Elder, Novice.
pub fn is_monk(name: &str) -> bool {
    ["Brother ", "Sister ", "Elder ", "Novice "].iter().any(|p| name.starts_with(p))
}

/// A monk's look, varied by name so no two of the order match: build, robe dye, sash, and
/// which of the order's marks they wear (beads, wraps, headband, topknot or shaved, hood,
/// scarf). Always the sash and at least one of beads / wraps.
pub fn monk_look(name: &str) -> Look {
    use Piece::*;
    let mut h = 0x811C_9DC5u32;
    for b in name.bytes() {
        h = (h ^ b as u32).wrapping_mul(0x0100_0193);
    }
    let mut pick = |n: u32| {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        h % n
    };
    let elder = name.starts_with("Elder ");
    let novice = name.starts_with("Novice ");
    const ROBES: [[f32; 3]; 6] = [[0.85, 0.5, 0.12], [0.55, 0.22, 0.1], [0.32, 0.32, 0.34], [0.18, 0.22, 0.4], [0.7, 0.62, 0.48], [0.25, 0.35, 0.22]];
    const SASHES: [[f32; 3]; 4] = [[0.12, 0.1, 0.08], [0.75, 0.6, 0.25], [0.55, 0.1, 0.1], [0.85, 0.85, 0.8]];
    let robe = ROBES[pick(ROBES.len() as u32) as usize];
    let sash = SASHES[pick(SASHES.len() as u32) as usize];
    let height = if novice { 0.93 } else { 0.95 } + pick(12) as f32 * 0.01;
    let breadth = if elder { 0.9 } else { 0.92 } + pick(22) as f32 * 0.01;
    let mut pieces = vec![Sash];
    match pick(3) {
        0 => pieces.push(Beads),
        1 => pieces.push(Wraps),
        _ => pieces.extend([Beads, Wraps]),
    }
    match pick(4) {
        0 => pieces.push(Headband),
        1 => pieces.push(Topknot),
        2 if elder => pieces.push(Hood),
        _ => {} // shaved
    }
    if pick(3) == 0 {
        pieces.push(Scarf);
    }
    if elder && !pieces.contains(&Beads) {
        pieces.push(Beads);
    }
    Look {
        height,
        breadth,
        accent: Vec4::new(robe[0], robe[1], robe[2], 0.5),
        cloth: Color::srgb(robe[0], robe[1], robe[2]),
        trim: Color::srgb(sash[0], sash[1], sash[2]),
        pieces,
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

/// The kasa's meshes (a helm replaces it).
#[derive(Component)]
struct KasaMesh;

/// Wear the Iron Rider's helm (Thoral's trophy): it goes on over the head, the kasa comes off.
#[derive(Component)]
pub struct WearHelm;

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
    app.add_systems(Update, (undress, dress, wear_helm, flaps, show_pieces).chain());
}

/// A modelled look changed (the class choice re-dresses the player): take the pieces off so
/// `dress` puts on the new look's.
#[allow(clippy::type_complexity)]
fn undress(
    mut commands: Commands,
    changed: Query<(Entity, &Rig), (Changed<super::appearance::Appearance>, With<Dressed>)>,
    children: Query<&Children>,
    pieces: Query<(), Or<(With<PieceMesh>, With<Flap>)>>,
) {
    for (e, rig) in &changed {
        for node in children.iter_descendants(rig.model) {
            if pieces.contains(node) {
                commands.entity(node).try_despawn();
            }
        }
        commands.entity(e).try_remove::<(Dressed, Dressing)>();
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn dress(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut skins: ResMut<Assets<SkinMaterial>>,
    mut characters: Query<(Entity, &Name, &Rig, &mut Skin, Option<&mut Dressing>, Option<&super::appearance::Appearance>), (Without<Dressed>, With<super::Fighter>)>,
    mut models: Query<&mut Transform>,
    globals: Query<&GlobalTransform>,
) {
    let Some(bank) = bank else { return };
    for (e, name, rig, mut skin, dressing, worn) in &mut characters {
        let Some(mut d) = dressing else {
            commands.entity(e).try_insert(Dressing::default());
            // Proportions at once (the pose settles with them); a modelled look has its own.
            let look = look_for(name.as_str());
            if worn.is_none()
                && let Ok(mut m) = models.get_mut(rig.model)
            {
                m.scale = Vec3::new(look.breadth, look.height, look.breadth);
            }
            continue;
        };
        d.0 += time.delta_secs();
        if d.0 < SETTLE {
            continue;
        }
        let mut look = look_for(name.as_str());
        if let Some(worn) = worn {
            // A modelled look: its outfit is its own; only the pieces it asks for go on.
            look.pieces = worn.pieces.iter().filter_map(|p| Piece::from_name(p)).collect();
        }
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
        // Pieces are sized for an ordinary build: bigger people get bigger pieces.
        let k = look.height;
        let place = |commands: &mut Commands, bone: (Entity, Transform), mesh: Handle<Mesh>, mat: Handle<SkinMaterial>, pos: Vec3, rot: Quat, scale: Vec3| {
            let parent = bone.1;
            let scale = scale * k;
            let local = Transform {
                translation: parent.rotation.inverse() * (pos - parent.translation) / parent.scale,
                rotation: parent.rotation.inverse() * rot,
                scale: scale / parent.scale,
            };
            commands.spawn((PieceMesh, Mesh3d(mesh), MeshMaterial3d(mat), local, ChildOf(bone.0))).id()
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
        let head_top = head.1.translation + up * 0.13 * k;
        for piece in &look.pieces {
            match piece {
                Piece::Kasa => {
                    let m = meshes.add(Cone { radius: 0.36, height: 0.17 });
                    let a = place(&mut commands, head, m, straw.clone(), head_top + up * 0.1, face, Vec3::ONE);
                    let rim = meshes.add(Cylinder::new(0.36, 0.012));
                    let b = place(&mut commands, head, rim, straw.clone(), head_top + up * 0.02, face, Vec3::ONE);
                    commands.entity(a).try_insert(KasaMesh);
                    commands.entity(b).try_insert(KasaMesh);
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
                Piece::Shield => {
                    let (Some(fore), Some(hand)) = (at(b.lowerarm[0]), at(b.hand[0])) else { continue };
                    let mid = fore.1.translation.lerp(hand.1.translation, 0.45) + left * 0.1;
                    let disc = Quat::from_rotation_arc(Vec3::Y, left);
                    let m = meshes.add(Cylinder::new(0.42, 0.05));
                    place(&mut commands, fore, m, cloth.clone(), mid, disc, Vec3::ONE);
                    let rim = meshes.add(Torus { minor_radius: 0.025, major_radius: 0.42 });
                    place(&mut commands, fore, rim, trim.clone(), mid, disc, Vec3::ONE);
                    let boss = meshes.add(Sphere::new(0.08));
                    place(&mut commands, fore, boss, trim.clone(), mid + left * 0.04, disc, Vec3::ONE);
                }
                Piece::Helm => {
                    let m = meshes.add(Sphere::new(1.0));
                    place(&mut commands, head, m, trim.clone(), head_top - up * 0.035, face, Vec3::new(0.135, 0.12, 0.14));
                    let nasal = meshes.add(Cuboid::new(0.025, 0.11, 0.02));
                    place(&mut commands, head, nasal, trim.clone(), head_top - up * 0.1 + fwd * 0.13, face, Vec3::ONE);
                    let crest = meshes.add(Cuboid::new(0.02, 0.06, 0.24));
                    place(&mut commands, head, crest, cloth.clone(), head_top + up * 0.08, face, Vec3::ONE);
                }
                Piece::Beads => {
                    let bead = meshes.add(Sphere::new(0.016));
                    let wood = material(Color::srgb(0.28, 0.16, 0.08), look.accent);
                    let centre = neck.1.translation - up * 0.06;
                    for i in 0..18 {
                        let a = i as f32 / 18.0 * std::f32::consts::TAU;
                        // Hangs lower at the front.
                        let drop = 0.05 * (1.0 + a.cos()) * 0.5;
                        let at = centre + fwd * (a.cos() * 0.095) + left * (a.sin() * 0.085) - up * drop;
                        place(&mut commands, neck, bead.clone(), wood.clone(), at, face, Vec3::ONE);
                    }
                }
                Piece::Wraps => {
                    let wrap = meshes.add(Cylinder::new(0.042, 0.12));
                    for side in 0..2 {
                        let (Some(fore), Some(hand)) = (at(b.lowerarm[side]), at(b.hand[side])) else { continue };
                        let dir = (hand.1.translation - fore.1.translation).normalize_or(Vec3::NEG_Y);
                        let rot = Quat::from_rotation_arc(Vec3::Y, dir);
                        place(&mut commands, fore, wrap.clone(), trim.clone(), fore.1.translation.lerp(hand.1.translation, 0.78), rot, Vec3::ONE);
                    }
                }
                Piece::Horns => {
                    let horn = meshes.add(Cone { radius: 0.035, height: 0.16 });
                    let bone = material(Color::srgb(0.85, 0.8, 0.65), look.accent);
                    for s in [-1.0f32, 1.0] {
                        let rot = Quat::from_rotation_arc(Vec3::Y, (up + left * s * 0.45 - fwd * 0.2).normalize());
                        place(&mut commands, head, horn.clone(), bone.clone(), head_top - up * 0.02 + left * s * 0.07 + fwd * 0.03, rot, Vec3::ONE);
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

/// Put the Iron Rider's helm on a dressed character (the kasa comes off).
#[allow(clippy::type_complexity)]
fn wear_helm(
    bank: Option<Res<PoseBank>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut skins: ResMut<Assets<SkinMaterial>>,
    mut wearers: Query<(Entity, &Name, &Rig, &mut Skin), (With<Dressed>, Added<WearHelm>)>,
    kasas: Query<(Entity, &ChildOf), With<KasaMesh>>,
    globals: Query<&GlobalTransform>,
) {
    let Some(bank) = bank else { return };
    for (e, name, rig, mut skin) in &mut wearers {
        let Some(head_bone) = rig.bones.get(bank.bones.head).copied() else { continue };
        let (Ok(head), Ok(root)) = (globals.get(head_bone), globals.get(e)) else { continue };
        for (k, parent) in &kasas {
            if parent.parent() == head_bone {
                commands.entity(k).try_despawn();
            }
        }
        let head = head.compute_transform();
        let k = look_for(name.as_str()).height;
        let face = root.rotation();
        let (up, fwd) = (Vec3::Y, face * Vec3::Z);
        let top = head.translation + up * 0.13 * k;
        let mut mat = |c: Color| {
            let h = skins.add(SkinMaterial {
                base: StandardMaterial { base_color: c, perceptual_roughness: 0.45, metallic: 0.6, ..default() },
                extension: SkinExt { skin: SkinParams::default() },
            });
            skin.materials.push(h.clone());
            h
        };
        let (iron, crest) = (mat(Color::srgb(0.32, 0.32, 0.35)), mat(Color::srgb(0.6, 0.1, 0.08)));
        let mut put = |mesh: Mesh, m: &Handle<SkinMaterial>, pos: Vec3, scale: Vec3| {
            let local = Transform {
                translation: head.rotation.inverse() * (pos - head.translation) / head.scale,
                rotation: head.rotation.inverse() * face,
                scale: scale * k / head.scale,
            };
            commands.spawn((PieceMesh, Mesh3d(meshes.add(mesh)), MeshMaterial3d(m.clone()), local, ChildOf(head_bone)));
        };
        put(Sphere::new(1.0).into(), &iron, top - up * 0.035 * k, Vec3::new(0.135, 0.12, 0.14));
        put(Cuboid::new(0.025, 0.11, 0.02).into(), &iron, top - up * 0.1 * k + fwd * 0.13 * k, Vec3::ONE);
        put(Cuboid::new(0.02, 0.06, 0.24).into(), &crest, top + up * 0.08 * k, Vec3::ONE);
        info!("[looks] {} wears the Iron Rider's helm", name.as_str());
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
    fn monks_vary_but_keep_the_order_marks() {
        let names = ["Brother Ishin", "Sister Ren", "Elder Tadao", "Novice Kenji", "Brother Sho"];
        let looks: Vec<Look> = names.iter().map(|n| look_for(n)).collect();
        for l in &looks {
            assert!(l.pieces.contains(&Piece::Sash));
            assert!(l.pieces.contains(&Piece::Beads) || l.pieces.contains(&Piece::Wraps));
        }
        let distinct = |f: &dyn Fn(&Look) -> String| {
            let mut v: Vec<String> = looks.iter().map(f).collect();
            v.sort();
            v.dedup();
            v.len()
        };
        assert!(distinct(&|l| format!("{:?}{:.2}{:.2}", l.pieces, l.height, l.breadth)) == names.len(), "no two monks alike");
        assert!(distinct(&|l| format!("{:?}", l.cloth)) >= 3, "robes vary");
        assert!(!is_monk("Opponent") && is_monk("Elder Tadao"));
    }

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
