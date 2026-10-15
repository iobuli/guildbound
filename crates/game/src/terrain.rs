//! The ground: chunked meshes of `guildbound_sim::terrain::ground`, finer near the centre (1 m) than at the
//! horizon (4 m), with skirts hiding the cracks between detail levels. Colour comes from slope and height (grass,
//! earth, rock, pale peaks), painted into the vertices: no splat textures, one material for the whole world.
//! Physics gets one heightfield over the play area.
use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use guildbound_sim::terrain::ground;

const CHUNK: f32 = 32.0;
/// Chunks out from the centre on each side (6 -> 384 m square).
const REACH: i32 = 6;
/// Physics heightfield: half size and spacing (m).
const FIELD_HALF: f32 = 128.0;
const FIELD_STEP: f32 = 1.0;

pub fn ground_at(p: Vec3) -> f32 {
    ground(p.x, p.z)
}

fn normal(x: f32, z: f32) -> Vec3 {
    let e = 0.5;
    Vec3::new(ground(x - e, z) - ground(x + e, z), 2.0 * e, ground(x, z - e) - ground(x, z + e)).normalize()
}

fn colour(x: f32, z: f32, n: Vec3) -> [f32; 4] {
    let h = ground(x, z);
    // a little variation so grass is not one flat green
    let v = (x * 0.37).sin() * (z * 0.29).cos() * 0.5 + (x * 0.071 + z * 0.053).sin() * 0.5;
    let grass = Vec3::new(0.27 + 0.03 * v, 0.42 + 0.05 * v, 0.17);
    let earth = Vec3::new(0.40, 0.33, 0.22);
    let rock = Vec3::new(0.43, 0.42, 0.40);
    let peak = Vec3::new(0.78, 0.79, 0.80);
    let steep = ((0.93 - n.y) / 0.12).clamp(0.0, 1.0);
    let worn = if x.abs() < 2.5 || z.abs() < 2.5 { 0.6 } else { 0.0 };
    let mut c = grass.lerp(earth, worn).lerp(rock, steep);
    c = c.lerp(peak, ((h - 22.0) / 6.0).clamp(0.0, 1.0));
    [c.x, c.y, c.z, 1.0]
}

fn chunk_mesh(x0: f32, z0: f32, step: f32) -> Mesh {
    let n = (CHUNK / step) as usize;
    let (mut pos, mut nor, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::<u32>::new());
    for j in 0..=n {
        for i in 0..=n {
            let (x, z) = (x0 + i as f32 * step, z0 + j as f32 * step);
            let nn = normal(x, z);
            // under the courtyard's stone slab (top at 0) the ground sits 3 cm lower, or the two flicker
            let under_slab = x.abs() <= 10.6 && z.abs() <= 10.6;
            pos.push([x, ground(x, z) - if under_slab { 0.03 } else { 0.0 }, z]);
            nor.push(nn.to_array());
            col.push(colour(x, z, nn));
        }
    }
    let at = |i: usize, j: usize| (j * (n + 1) + i) as u32;
    for j in 0..n {
        for i in 0..n {
            idx.extend([at(i, j), at(i, j + 1), at(i + 1, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1)]);
        }
    }
    // skirts: each edge dropped 3 m, so a coarser neighbour never shows a gap
    let edges: [Vec<u32>; 4] = [(0..=n).map(|i| at(i, 0)).collect(), (0..=n).map(|j| at(n, j)).collect(), (0..=n).rev().map(|i| at(i, n)).collect(), (0..=n).rev().map(|j| at(0, j)).collect()];
    for edge in edges {
        let base = pos.len() as u32;
        for &v in &edge {
            let p = pos[v as usize];
            pos.push([p[0], p[1] - 3.0, p[2]]);
            nor.push(nor[v as usize]);
            col.push(col[v as usize]);
        }
        for k in 0..edge.len() - 1 {
            let (a, b) = (edge[k], edge[k + 1]);
            let (a2, b2) = (base + k as u32, base + k as u32 + 1);
            idx.extend([a, b, a2, b, b2, a2]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

pub fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let material = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, ..default() });
    for cz in -REACH..REACH {
        for cx in -REACH..REACH {
            let (x0, z0) = (cx as f32 * CHUNK, cz as f32 * CHUNK);
            let centre = Vec2::new(x0 + CHUNK * 0.5, z0 + CHUNK * 0.5).length();
            let step = if centre < 100.0 { 1.0 } else if centre < 170.0 { 2.0 } else { 4.0 };
            commands.spawn((Name::new("terrain chunk"), Mesh3d(meshes.add(chunk_mesh(x0, z0, step))), MeshMaterial3d(material.clone()), Transform::default()));
        }
    }
    // physics: rows along X, columns along Z, scale = the field's full size
    let n = (2.0 * FIELD_HALF / FIELD_STEP) as usize + 1;
    let heights: Vec<Vec<f32>> = (0..n).map(|i| (0..n).map(|j| ground(-FIELD_HALF + i as f32 * FIELD_STEP, -FIELD_HALF + j as f32 * FIELD_STEP)).collect()).collect();
    commands.spawn((Name::new("terrain collider"), RigidBody::Static, Collider::heightfield(heights, Vec3::new(2.0 * FIELD_HALF, 1.0, 2.0 * FIELD_HALF)), Transform::default()));
}
