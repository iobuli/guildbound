//! Numeric motion alignment plus the terrain-normal, contact-lock and two-bone maths from gb_live.
use crate::rig::{self, Seg, Shape};
use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

const GRID: f32 = 0.25;
const HALF: f32 = 10.0;

fn sample_height(x: f32, z: f32) -> f32 {
    // A hillside patch of the same courtyard height function as gb_live; the centre courtyard is flat.
    guildbound_sim::terrain::ground(x - 42.0, z + 14.0)
        - guildbound_sim::terrain::ground(-42.0, 14.0)
}

pub fn height(x: f32, z: f32) -> f32 {
    voxel_column(x, z)
        .map(|(center, h)| center.y + h * 0.5)
        .unwrap_or_else(|| base_height(x, z))
}

fn base_height(x: f32, z: f32) -> f32 {
    let gx = (x + HALF) / GRID;
    let gz = (z + HALF) / GRID;
    let x0 = gx.floor() * GRID - HALF;
    let z0 = gz.floor() * GRID - HALF;
    let (u, v) = (gx.fract(), gz.fract());
    let (a, b, c, d) = (
        sample_height(x0, z0),
        sample_height(x0 + GRID, z0),
        sample_height(x0, z0 + GRID),
        sample_height(x0 + GRID, z0 + GRID),
    );
    if u + v <= 1.0 {
        a + (b - a) * u + (c - a) * v
    } else {
        d + (c - d) * (1.0 - u) + (b - d) * (1.0 - v)
    }
}

pub fn normal(x: f32, z: f32) -> Vec3 {
    if voxel_column(x, z).is_some() {
        return Vec3::Y;
    }
    Vec3::new(
        height(x - 0.1, z) - height(x + 0.1, z),
        0.2,
        height(x, z - 0.1) - height(x, z + 0.1),
    )
    .normalize_or(Vec3::Y)
}

/// Shared sparse voxel column data for rendering, collision and foot/path queries.
fn voxel_column(x: f32, z: f32) -> Option<(Vec3, f32)> {
    if !(-4.0..4.0).contains(&x) || !(-3.5..-2.5).contains(&z) {
        return None;
    }
    let ix = ((x + 4.0) / GRID).floor() as i32;
    let iz = ((z + 3.5) / GRID).floor() as i32;
    let layers = (ix / 4).min((31 - ix) / 4).min(3);
    if layers == 0 {
        return None;
    }
    let x = -4.0 + (ix as f32 + 0.5) * GRID;
    let z = -3.5 + (iz as f32 + 0.5) * GRID;
    let bottom = (base_height(x, z) / GRID).floor() * GRID - 0.1;
    let top = (base_height(x, z) / GRID).ceil() * GRID + layers as f32 * GRID;
    Some((Vec3::new(x, (bottom + top) * 0.5, z), top - bottom))
}

pub fn safe_travel(start: Vec3, delta: Vec3, step: f32) -> Vec3 {
    let count = (delta.length() / 0.08).ceil().max(1.0) as usize;
    let mut accepted = start;
    let mut floor = height(start.x, start.z);
    let offsets = [
        Vec2::new(0.22, 0.0),
        Vec2::new(-0.22, 0.0),
        Vec2::new(0.0, 0.22),
        Vec2::new(0.0, -0.22),
    ];
    let initial_clearance = offsets
        .iter()
        .map(|s| height(start.x + s.x, start.z + s.y) - floor)
        .fold(0.0, f32::max);
    for i in 1..=count {
        let p = start + delta * (i as f32 / count as f32);
        if p.x.abs() > 8.4 || p.z.abs() > 8.4 {
            break;
        }
        let next = height(p.x, p.z);
        let blocked = [
            Vec2::new(0.22, 0.0),
            Vec2::new(-0.22, 0.0),
            Vec2::new(0.0, 0.22),
            Vec2::new(0.0, -0.22),
        ]
        .iter()
        .any(|s| height(p.x + s.x, p.z + s.y) > next + step);
        let clearance = offsets
            .iter()
            .map(|s| height(p.x + s.x, p.z + s.y) - next)
            .fold(0.0, f32::max);
        if next - floor > step
            || floor - next > 0.45
            || (blocked && clearance > initial_clearance + 0.001)
        {
            break;
        }
        accepted = p;
        floor = next;
    }
    (accepted - start).with_y(0.0)
}

pub fn trace(origin: Vec3, length: f32) -> Option<Vec3> {
    if origin.x.abs() > HALF || origin.z.abs() > HALF {
        return None;
    }
    let y = height(origin.x, origin.z);
    (origin.y >= y && origin.y - y <= length).then_some(Vec3::new(origin.x, y, origin.z))
}

pub fn spawn_ground(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) {
    let n = (2.0 * HALF / GRID) as usize;
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut triangles = Vec::new();
    for z in 0..=n {
        for x in 0..=n {
            let (x, z) = (-HALF + x as f32 * GRID, -HALF + z as f32 * GRID);
            vertices.push(Vec3::new(x, sample_height(x, z), z));
            normals.push(normal(x, z).to_array());
            let shade = if ((x * 2.0).floor() as i32 + (z * 2.0).floor() as i32).rem_euclid(2) == 0
            {
                0.29
            } else {
                0.34
            };
            colors.push([shade, shade + 0.09, shade + 0.04, 1.0]);
        }
    }
    for z in 0..n {
        for x in 0..n {
            let a = (z * (n + 1) + x) as u32;
            let b = a + 1;
            let c = a + (n + 1) as u32;
            let d = c + 1;
            triangles.extend([[a, c, b], [b, c, d]]);
        }
    }
    let collider = Collider::trimesh(vertices.clone(), triangles.clone());
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vertices.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(triangles.into_iter().flatten().collect()));
    commands.spawn((
        Name::new("gb_live terrain patch"),
        RigidBody::Static,
        Friction::new(0.8),
        collider,
        rig::floor_layers(),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(mats.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::default(),
    ));
    let cube = meshes.add(Cuboid::new(GRID, 1.0, GRID));
    let stone = mats.add(Color::srgb(0.38, 0.45, 0.53));
    for ix in 0..32 {
        for iz in 0..4 {
            let x = -4.0 + (ix as f32 + 0.5) * GRID;
            let z = -3.5 + (iz as f32 + 0.5) * GRID;
            if let Some((center, h)) = voxel_column(x, z) {
                commands
                    .spawn((
                        Name::new("Voxel practice path column"),
                        RigidBody::Static,
                        Collider::cuboid(GRID, h, GRID),
                        Friction::new(0.8),
                        rig::floor_layers(),
                        Visibility::default(),
                        Transform::from_translation(center),
                    ))
                    .with_child((
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(stone.clone()),
                        Transform::from_scale(Vec3::new(1.0, h, 1.0)),
                    ));
            }
        }
    }
}

pub fn neutral_segments() -> Vec<Seg> {
    let mut s = rig::segments();
    for segment in &mut s {
        if let Shape::Capsule(r) = &mut segment.shape {
            *r *= 0.72;
        }
    }
    s[8].end = s[11].anchor;
    s[10].end = s[12].anchor;
    s
}

pub fn attach(
    commands: &mut Commands,
    parent: Entity,
    s: &Seg,
    color: Color,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) {
    let (mesh, rotation) = match s.shape {
        Shape::Capsule(r) => (
            Mesh::from(Capsule3d::new(r, (s.length() - 2.0 * r).max(0.02))),
            s.inertia_frame(),
        ),
        Shape::Box(size) => (Mesh::from(Cuboid::from_size(size)), Quat::IDENTITY),
    };
    let child = commands
        .spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(mats.add(StandardMaterial {
                base_color: color,
                metallic: 0.15,
                perceptual_roughness: 0.55,
                ..default()
            })),
            Transform::from_rotation(rotation),
        ))
        .id();
    commands.entity(parent).add_child(child);
}

struct Sample {
    feet: [Vec3; 2],
    contact: [f32; 2],
    rotations: Vec<Quat>,
}
struct Clip {
    duration: f32,
    frames: Vec<Sample>,
}
pub struct Alignment {
    clips: Vec<Clip>,
    lengths: [[f32; 2]; 2],
    reach: f32,
    width: f32,
    roots: Vec<Vec3>,
}

pub struct Frame {
    pub root: Vec3,
    pub rotation: Quat,
    pub local: Vec<Quat>,
    pub feet: [Vec3; 2],
    pub contact: [f32; 2],
    pub raw: [Vec3; 2],
}

#[derive(Clone, Copy, Default)]
pub struct FootLock {
    pub anchor: Vec3,
    pub planted: bool,
    pub weight: f32,
    velocity: f32,
}
impl FootLock {
    fn step(&mut self, raw: Vec3, contact: f32, dt: f32) -> Vec3 {
        if !self.planted && contact > 0.65 {
            self.anchor = Vec3::new(raw.x, height(raw.x, raw.z) + 0.08, raw.z);
            self.planted = true;
        } else if self.planted && (contact < 0.35 || raw.distance(self.anchor) > 0.22) {
            self.planted = false;
        }
        let target = if self.planted { 1.0 } else { 0.0 };
        let omega = if self.planted { 65.0 } else { 40.0 };
        let offset = self.weight - target;
        let coupled = self.velocity + omega * offset;
        let decay = (-omega * dt).exp();
        self.weight = (target + (offset + coupled * dt) * decay).clamp(0.0, 1.0);
        self.velocity = (self.velocity - omega * coupled * dt) * decay;
        raw.lerp(self.anchor, self.weight)
    }
}

impl Alignment {
    pub fn new(segments: &[Seg]) -> Self {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../../../assets/motion/neutral_tracks.json"))
                .expect("baked motion tracks");
        let vector = |v: &serde_json::Value| {
            Vec3::new(
                v[0].as_f64().unwrap() as f32,
                v[1].as_f64().unwrap() as f32,
                v[2].as_f64().unwrap() as f32,
            )
        };
        let clips = ["idle", "walk", "run"]
            .iter()
            .map(|name| {
                let c = &data["clips"][name];
                Clip {
                    duration: c["duration"].as_f64().unwrap() as f32,
                    frames: c["frames"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|f| Sample {
                            feet: [vector(&f["feet"][0]), vector(&f["feet"][1])],
                            contact: [
                                f["contact"][0].as_f64().unwrap() as f32,
                                f["contact"][1].as_f64().unwrap() as f32,
                            ],
                            rotations: f["rotations"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|q| {
                                    Quat::from_xyzw(
                                        q[0].as_f64().unwrap() as f32,
                                        q[1].as_f64().unwrap() as f32,
                                        q[2].as_f64().unwrap() as f32,
                                        q[3].as_f64().unwrap() as f32,
                                    )
                                    .normalize()
                                })
                                .collect(),
                        })
                        .collect(),
                }
            })
            .collect();
        let lengths = [
            [
                segments[7].anchor.distance(segments[8].anchor),
                segments[8].anchor.distance(segments[11].anchor),
            ],
            [
                segments[9].anchor.distance(segments[10].anchor),
                segments[10].anchor.distance(segments[12].anchor),
            ],
        ];
        Self {
            clips,
            lengths,
            reach: lengths[0].iter().sum(),
            width: segments[7].anchor.x.abs(),
            roots: segments.iter().map(|s| s.anchor).collect(),
        }
    }

    pub fn route(time: f32, speed: f32, lane: f32) -> (Vec3, Quat) {
        let radius = 0.72;
        let a = time * speed / radius;
        let mut p = Vec3::new(lane + radius * a.sin(), 0.0, radius * a.cos());
        p.y = height(p.x, p.z) + 0.91;
        (p, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 + a))
    }

    pub fn frame(
        &self,
        time: f32,
        travel: f32,
        speed: f32,
        lane: f32,
        locks: &mut [FootLock; 2],
        dt: f32,
    ) -> Frame {
        let clip = &self.clips[if speed > 2.0 {
            2
        } else if speed > 0.1 {
            1
        } else {
            0
        }];
        let f = time.rem_euclid(clip.duration) / clip.duration * clip.frames.len() as f32;
        let i = f.floor() as usize % clip.frames.len();
        let j = (i + 1) % clip.frames.len();
        let u = f.fract();
        let contact = std::array::from_fn(|s| {
            clip.frames[i].contact[s] + (clip.frames[j].contact[s] - clip.frames[i].contact[s]) * u
        });
        let mut local: Vec<_> = clip.frames[i]
            .rotations
            .iter()
            .zip(&clip.frames[j].rotations)
            .map(|(a, b)| a.slerp(*b, u))
            .collect();
        let (mut root, rotation) = Self::route(
            travel,
            if speed > 0.1 {
                speed.min(1.2) * 0.5
            } else {
                0.0
            },
            lane,
        );
        let mut raw = [Vec3::ZERO; 2];
        let mut feet = [Vec3::ZERO; 2];
        for s in 0..2 {
            let track = clip.frames[i].feet[s].lerp(clip.frames[j].feet[s], u);
            // Align normalized trajectories once to the target reach and hip width, not source bone scales.
            let horizontal = rotation
                * Vec3::new(
                    if s == 0 { self.width } else { -self.width },
                    0.0,
                    track.z * self.reach,
                );
            raw[s] = root + horizontal;
            raw[s].y = height(raw[s].x, raw[s].z) + 0.08 + track.y * self.reach;
            feet[s] = locks[s].step(raw[s], contact[s], dt);
            feet[s].y = feet[s].y.max(height(feet[s].x, feet[s].z) + 0.08);
        }
        // Lower the pelvis if either immutable leg would otherwise stretch (gb_live support reach rule).
        let mut drop = 0.0f32;
        for s in 0..2 {
            let hip = root + rotation * (self.roots[if s == 0 { 7 } else { 9 }] - self.roots[0]);
            let delta = hip - feet[s];
            let reach = self.lengths[s].iter().sum::<f32>() - 0.003;
            let vertical = (reach * reach - Vec2::new(delta.x, delta.z).length_squared())
                .max(0.0)
                .sqrt();
            drop = drop.max((delta.y - vertical).max(0.0));
        }
        root.y -= drop.min(0.1);
        for s in 0..2 {
            let (hip, knee, foot) = if s == 0 { (7, 8, 11) } else { (9, 10, 12) };
            let h = root + rotation * (self.roots[hip] - self.roots[0]);
            let (k, tip) = solve(h, feet[s], rotation * Vec3::Z, self.lengths[s]);
            feet[s] = tip;
            let upper = (rotation
                * Quat::from_rotation_arc(
                    (self.roots[knee] - self.roots[hip]).normalize(),
                    rotation.inverse() * (k - h).normalize(),
                ))
            .normalize();
            let lower = (rotation
                * Quat::from_rotation_arc(
                    (self.roots[foot] - self.roots[knee]).normalize(),
                    rotation.inverse() * (tip - k).normalize(),
                ))
            .normalize();
            let foot_rotation = ground_rotation(rotation, normal(tip.x, tip.z));
            local[hip] = rotation.inverse() * upper;
            local[knee] = upper.inverse() * lower;
            local[foot] = lower.inverse() * foot_rotation;
        }
        Frame {
            root,
            rotation,
            local,
            feet,
            contact,
            raw,
        }
    }
}

fn ground_rotation(rotation: Quat, normal: Vec3) -> Quat {
    let correction = Quat::from_rotation_arc(rotation * Vec3::Y, normal);
    let (axis, angle) = correction.to_axis_angle();
    (Quat::from_axis_angle(axis, angle.min(30f32.to_radians())) * rotation).normalize()
}

fn solve(root: Vec3, target: Vec3, pole: Vec3, lengths: [f32; 2]) -> (Vec3, Vec3) {
    let [a, b] = lengths;
    let offset = target - root;
    let axis = offset.normalize_or(Vec3::NEG_Y);
    let distance = offset
        .length()
        .clamp((a - b).abs() + 0.0001, a + b - 0.0001);
    let pole = (pole - axis * pole.dot(axis)).normalize_or(Vec3::Z);
    let along = (a * a - b * b + distance * distance) / (2.0 * distance);
    let height = (a * a - along * along).max(0.0).sqrt();
    (root + axis * along + pole * height, root + axis * distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traces_hit_surface_and_aligned_legs_keep_their_lengths() {
        let segments = neutral_segments();
        let alignment = Alignment::new(&segments);
        let mut locks = [FootLock::default(); 2];
        for i in 0..1200 {
            let f = alignment.frame(
                i as f32 / 120.0,
                i as f32 / 120.0,
                1.5,
                0.0,
                &mut locks,
                1.0 / 120.0,
            );
            for s in 0..2 {
                let foot = f.feet[s];
                let hit = trace(foot + Vec3::Y, 2.0).unwrap();
                assert!((hit.y - height(foot.x, foot.z)).abs() < 1e-5);
                assert!(foot.y >= hit.y + 0.079, "foot penetrated terrain");
                assert!(f.local.iter().all(|q| q.is_finite()));
            }
        }
        let (k, f) = solve(Vec3::Y, Vec3::new(0.1, 0.1, 0.2), Vec3::Z, [0.5, 0.5]);
        assert!((k.distance(Vec3::Y) - 0.5).abs() < 1e-5);
        assert!((k.distance(f) - 0.5).abs() < 1e-5);
    }
    #[test]
    fn floor_probe_agrees_with_the_actual_collision_triangles() {
        for (x, z) in [(-0.25, -0.25), (3.0, 1.0), (-5.0, -2.0)] {
            let vertices = vec![
                Vec3::new(x, sample_height(x, z), z),
                Vec3::new(x + GRID, sample_height(x + GRID, z), z),
                Vec3::new(x, sample_height(x, z + GRID), z + GRID),
                Vec3::new(x + GRID, sample_height(x + GRID, z + GRID), z + GRID),
            ];
            let collider = Collider::trimesh(vertices, vec![[0, 2, 1], [1, 2, 3]]);
            for (u, v) in [(0.1, 0.2), (0.8, 0.7), (0.4, 0.5)] {
                let origin = Vec3::new(x + u * GRID, 5.0, z + v * GRID);
                let (distance, _) = collider
                    .cast_ray(Vec3::ZERO, Quat::IDENTITY, origin, Vec3::NEG_Y, 10.0, true)
                    .unwrap();
                assert!((origin.y - distance - height(origin.x, origin.z)).abs() < 1e-5);
            }
        }
    }
}
