//! Voxel character skin: each physics segment is drawn as a merged mesh of small coloured cubes filling
//! its collider shape, parented to that segment's Avian body. Rest pose is axis-aligned (segment frame =
//! bone frame = world at rest), so the rest offset of every voxel is just its offset from the segment centre
//! and the skin follows the physics exactly.
//! Two looks, chosen by model name: Hero (master / hero: skin, hair, tunic, belt, trousers, boots, gloves)
//! and Mannequin (mannequin / dummy: pale wood with darker joint rings and a face plate).
//! Both looks are authored from the segment table, fitted to the selected source GLB.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::rig::{Seg, Shape};

/// Voxel edge length (m).
pub const VOXEL: f32 = 0.035;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Hero,
    Mannequin,
}

impl Style {
    pub fn for_model(model: &str) -> Style {
        let m = model.to_ascii_lowercase();
        if m.contains("mannequin") || m.contains("dummy") {
            Style::Mannequin
        } else {
            Style::Hero
        }
    }
}

/// Is body-local point p inside segment s (optionally inflated by `pad` m)?
fn inside(s: &Seg, p: Vec3, pad: f32) -> bool {
    let c = s.centre();
    match s.shape {
        Shape::Capsule(r) => {
            let (a, b) = (s.anchor - c, s.end - c);
            let half = (s.length() * 0.5 - r).max(0.01);
            let axis = (b - a).normalize_or(Vec3::Y);
            let t = p.dot(axis).clamp(-half, half);
            (p - axis * t).length() <= r + pad
        }
        Shape::Box(h) => {
            p.x.abs() <= h.x * 0.5 + pad
                && p.y.abs() <= h.y * 0.5 + pad
                && p.z.abs() <= h.z * 0.5 + pad
        }
    }
}

fn rgb(r: f32, g: f32, b: f32) -> [f32; 4] {
    [r, g, b, 1.0]
}

/// Colour of a surface voxel at body-local p (u = 0 at the joint anchor .. 1 at the segment end).
fn colour(style: Style, s: &Seg, p: Vec3, u: f32) -> [f32; 4] {
    let n = s.name;
    match style {
        Style::Mannequin => {
            let wood = rgb(0.86, 0.74, 0.55);
            let ring = rgb(0.55, 0.42, 0.28);
            if n.contains("Head") && p.z > 0.06 && p.y.abs() < 0.05 {
                return rgb(0.78, 0.66, 0.48);
            }
            if !n.contains("Foot") && !n.contains("Pelvis") && (u < 0.12 || u > 0.92) {
                return ring;
            }
            if n.contains("Pelvis") && p.y < -0.03 {
                return ring;
            }
            wood
        }
        Style::Hero => {
            let skin = rgb(0.93, 0.76, 0.62);
            let hair = rgb(0.30, 0.18, 0.10);
            let tunic = rgb(0.62, 0.20, 0.18);
            let trim = rgb(0.85, 0.68, 0.25);
            let trousers = rgb(0.22, 0.28, 0.45);
            let leather = rgb(0.30, 0.20, 0.12);
            if n.contains("Head") {
                if p.y > 0.03 || (p.z < -0.02 && p.y > -0.06) {
                    return hair;
                }
                if p.z > 0.08 && p.y > -0.01 && p.y < 0.03 && p.x.abs() > 0.02 && p.x.abs() < 0.06 {
                    return rgb(0.1, 0.1, 0.15);
                }
                return skin;
            }
            if n.contains("Spine") {
                return if u < 0.08 || (p.x.abs() < 0.02 && p.z > 0.0) {
                    trim
                } else {
                    tunic
                };
            }
            if n.contains("Pelvis") {
                return if u > 0.75 {
                    leather
                } else if p.x.abs() < 0.03 && p.z > 0.05 && u > 0.55 {
                    trim
                } else {
                    tunic
                };
            }
            if n.contains("UpperArm") {
                return if u < 0.45 { tunic } else { skin };
            }
            if n.contains("Forearm") {
                return if u > 0.7 { leather } else { skin };
            }
            if n.contains("Thigh") {
                return trousers;
            }
            if n.contains("Calf") {
                return if u > 0.45 { leather } else { trousers };
            }
            if n.contains("Foot") {
                return leather;
            }
            tunic
        }
    }
}

/// Coloured surface voxels (body-local centres) for segment s in the given style.
pub fn voxels(s: &Seg, style: Style) -> Vec<(Vec3, [f32; 4])> {
    let c = s.centre();
    let axis = (s.end - s.anchor).normalize_or(Vec3::Y);
    let len = s.length().max(1e-3);
    // Hero hair sits one voxel proud of the head capsule on top and at the back.
    let pad = |p: Vec3| -> f32 {
        if style == Style::Hero && s.name.contains("Head") && (p.y > 0.04 || p.z < -0.03) {
            VOXEL
        } else {
            0.0
        }
    };
    let inn = |p: Vec3| inside(s, p, pad(p));
    let ext = match s.shape {
        Shape::Capsule(r) => axis.abs() * (len * 0.5 - r).max(0.01) + Vec3::splat(r + VOXEL),
        Shape::Box(h) => h * 0.5,
    };
    let n = (ext / VOXEL).ceil().as_ivec3();
    let mut out = Vec::new();
    for x in -n.x..=n.x {
        for y in -n.y..=n.y {
            for z in -n.z..=n.z {
                let p = Vec3::new(x as f32, y as f32, z as f32) * VOXEL;
                if !inn(p) {
                    continue;
                }
                let shell = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z]
                    .iter()
                    .any(|d| !inn(p + *d * VOXEL));
                if !shell {
                    continue;
                }
                let u = ((p + c - s.anchor).dot(axis) / len).clamp(0.0, 1.0);
                out.push((p, colour(style, s, p, u)));
            }
        }
    }
    out
}

/// One mesh of cubes with per-vertex colours (flat-shaded faces).
pub fn mesh(cells: &[(Vec3, [f32; 4])]) -> Mesh {
    const FACES: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::NEG_Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::Z, Vec3::NEG_X),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
    ];
    let h = VOXEL * 0.47;
    let (mut pos, mut nor, mut col, mut idx) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::<u32>::new());
    for (c, k) in cells {
        for (nrm, a, b) in FACES {
            let base = pos.len() as u32;
            // corners ordered counter-clockwise seen from outside: a x b = n
            let (a, b) = if a.cross(b).dot(nrm) > 0.0 {
                (a, b)
            } else {
                (b, a)
            };
            for (sa, sb) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                pos.push((*c + (nrm + a * sa + b * sb) * h).to_array());
                nor.push(nrm.to_array());
                col.push(*k);
            }
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
    .with_inserted_indices(Indices::U32(idx))
}

/// Marks a voxel skin entity (child of a segment's Avian body).
#[derive(Component)]
pub struct Skin;

/// Attach the voxel skin of segment `s` as a child of its Avian body `body` (rest offset = identity,
/// because the voxel centres are already relative to the segment centre = body origin).
pub fn attach_skin(
    commands: &mut Commands,
    body: Entity,
    s: &Seg,
    style: Style,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) {
    let cells = voxels(s, style);
    if cells.is_empty() {
        return;
    }
    let rough = if style == Style::Mannequin { 0.6 } else { 0.85 };
    let skin = commands
        .spawn((
            Skin,
            Mesh3d(meshes.add(mesh(&cells))),
            MeshMaterial3d(mats.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: rough,
                ..default()
            })),
            Transform::default(),
        ))
        .id();
    commands.entity(body).add_child(skin);
}
