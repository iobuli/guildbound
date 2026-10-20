//! Smear frames (assets/shaders/smear.wgsl): a fast limb leaves ragged brush streaks on its trailing side, like the
//! hand-drawn smears of anime fights, worked out from how the limb is moving.
//!
//! Per fighter, per frame, on four chains (each arm: forearm and hand, each leg: shin and foot):
//! - Samples: `SAMPLES` points along the chain's last two segments, in world space RELATIVE TO THE ROOT, so walking
//!   and running don't smear, only the limb's own motion does.
//! - Speed: each sample's velocity v = dp / dt. Below `V_MIN` nothing; full strength at `V_FULL`.
//! - Extension: the end joint moving away from its anchor (fist from shoulder, foot from hip), d|B - A| / dt. A strike
//!   going out smears fully; a limb coming back (pullback) at `RETRACT` strength.
//! - Duration: only while the fighter is in an attack (`Melee::attacking`), ramping in over `RAMP` s from the swing's
//!   start. `GB_SMEAR=1` also smears the player outside attacks (for clips played with `GB_CLIP`).
//! - Streak length = speed x `TAU` (the eye's persistence, ~2 frames), capped at `L_MAX`, times a clumped random
//!   factor per streak (0.25..1.2) so lengths are uneven (brush 1.5), never even. Streaks start on the limb's
//!   trailing surface (radius per segment, from Kev's body map) and run back along -v, as camera-facing ribbons.
//! One dynamic mesh per fighter holds every streak; it is rebuilt each frame and hidden when nothing smears.
use super::combat::Melee;
use super::{Fighter, Player};
use crate::motion::bank::PoseBank;
use crate::motion::Rig;
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

const SAMPLES: usize = 7;
const STREAKS_PER_SAMPLE: usize = 3;
pub const V_MIN: f32 = 3.5;
pub const V_FULL: f32 = 9.0;
pub const TAU: f32 = 0.04;
pub const L_MAX: f32 = 0.45;
const RETRACT: f32 = 0.4;
const RAMP: f32 = 0.05;
const WIDTH: f32 = 0.012;
/// (anchor, joint, joint, end, radius at the first joint, radius at the end) per chain; radii from Kev's body map
/// (`staging/kev/bodymap/ual2.json` radius profiles of lowerarm / hand / calf / foot).
const CHAINS: [(&str, &str, &str, &str, f32, f32); 4] = [
    ("upperarm_l", "lowerarm_l", "hand_l", "middle_03_l", 0.040, 0.045),
    ("upperarm_r", "lowerarm_r", "hand_r", "middle_03_r", 0.040, 0.045),
    ("thigh_l", "calf_l", "foot_l", "ball_leaf_l", 0.055, 0.045),
    ("thigh_r", "calf_r", "foot_r", "ball_leaf_r", 0.055, 0.045),
];

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct SmearMaterial {}

impl Material for SmearMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/smear.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

/// Per fighter: last frame's root-relative samples and end-to-anchor distances, the swing clock, its mesh.
#[derive(Component)]
struct Smearing {
    prev: Vec<[Vec3; SAMPLES]>,
    reach: [f32; 4],
    swing: f32,
    mesh: Entity,
    handle: Handle<Mesh>,
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<SmearMaterial>::default()).add_systems(PostUpdate, smear.after(TransformSystems::Propagate));
}

/// Clumped random factor in 0.25..1.2: neighbouring streaks share a lump, so lengths come in uneven bunches.
pub fn brush(i: usize, t: f32) -> f32 {
    let h = |x: f32| (x.sin() * 43758.547).fract().abs();
    let clump = 0.6 + 0.4 * ((i as f32) * 0.9 + t * 37.0).sin();
    0.25 + 0.95 * h(i as f32 * 12.989 + (t * 24.0).floor() * 78.233).powf(0.6) * clump
}

/// Smear strength (0..1) from a sample's speed and whether the end joint is reaching out or coming back.
pub fn strength(speed: f32, extending: bool) -> f32 {
    let k = ((speed - V_MIN) / (V_FULL - V_MIN)).clamp(0.0, 1.0);
    if extending { k } else { k * RETRACT }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn smear(
    mut commands: Commands,
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SmearMaterial>>,
    mut mat: Local<Option<Handle<SmearMaterial>>>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    bones: Query<&GlobalTransform>,
    mut fighters: Query<(Entity, &GlobalTransform, &Rig, Option<&Melee>, Has<Player>, Option<&mut Smearing>), With<Fighter>>,
    mut vis: Query<&mut Visibility>,
) {
    let Some(bank) = bank else { return };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mat = mat.get_or_insert_with(|| materials.add(SmearMaterial {})).clone();
    let eye = cameras.iter().next().map(|c| c.translation()).unwrap_or(Vec3::Y * 10.0);
    let forced = std::env::var_os("GB_SMEAR").is_some();
    let t = time.elapsed_secs();
    for (e, root, rig, melee, player, state) in &mut fighters {
        let attacking = melee.is_some_and(|m| m.attacking());
        let on = attacking || (forced && player);
        let origin = root.translation();
        let joint = |name: &str| -> Option<Vec3> {
            let i = bank.skeleton.find(name)?;
            let b = *rig.bones.get(i)?;
            bones.get(b).ok().map(|g| g.translation())
        };
        // Root-relative samples along each chain's last two segments.
        let mut now: Vec<[Vec3; SAMPLES]> = Vec::with_capacity(4);
        let mut reach = [0.0; 4];
        let mut ok = true;
        for (c, (anchor, a, b, end, _, _)) in CHAINS.iter().enumerate() {
            let (Some(pa), Some(p1), Some(p2), Some(p3)) = (joint(anchor), joint(a), joint(b), joint(end)) else {
                ok = false;
                break;
            };
            reach[c] = p3.distance(pa);
            let mut s = [Vec3::ZERO; SAMPLES];
            for (k, slot) in s.iter_mut().enumerate() {
                let u = k as f32 / (SAMPLES - 1) as f32 * 2.0;
                let p = if u <= 1.0 { p1.lerp(p2, u) } else { p2.lerp(p3, u - 1.0) };
                *slot = p - origin;
            }
            now.push(s);
        }
        if !ok {
            continue;
        }
        let Some(mut st) = state else {
            let handle = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
            let mesh = commands
                .spawn((Name::new("Smear"), Mesh3d(handle.clone()), MeshMaterial3d(mat.clone()), Transform::default(), Visibility::Hidden, NotShadowCaster))
                .id();
            commands.entity(e).insert(Smearing { prev: now, reach, swing: 0.0, mesh, handle });
            continue;
        };
        st.swing = if on { st.swing + dt } else { 0.0 };
        let ramp = (st.swing / RAMP).clamp(0.0, 1.0);
        let mut pos: Vec<[f32; 3]> = Vec::new();
        let mut uv: Vec<[f32; 2]> = Vec::new();
        let mut col: Vec<[f32; 4]> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        if on && ramp > 0.0 {
            for (c, (_, _, _, _, r0, r1)) in CHAINS.iter().enumerate() {
                let extending = reach[c] > st.reach[c] + 1e-4;
                for k in 0..SAMPLES {
                    let v = (now[c][k] - st.prev[c][k]) / dt;
                    let speed = v.length();
                    let s = strength(speed, extending) * ramp;
                    if s <= 0.01 {
                        continue;
                    }
                    let back = -v / speed;
                    let p = now[c][k] + origin;
                    let to_eye = (eye - p).normalize_or(Vec3::Y);
                    let across = back.cross(to_eye).normalize_or(Vec3::X);
                    let r = r0 + (r1 - r0) * k as f32 / (SAMPLES - 1) as f32;
                    for j in 0..STREAKS_PER_SAMPLE {
                        let id = c * 100 + k * 10 + j;
                        let o = (j as f32 / (STREAKS_PER_SAMPLE - 1) as f32 - 0.5) * 1.6;
                        let len = (speed * TAU).min(L_MAX) * brush(id, t);
                        let start = p + across * (o * r) - back * (r * 0.2);
                        let tail = start + back * len;
                        let w = WIDTH * (0.6 + 0.8 * brush(id + 7, t));
                        let base = pos.len() as u32;
                        for (q, u, side) in [(start, 0.0, -1.0), (start, 0.0, 1.0), (tail, 1.0, -1.0), (tail, 1.0, 1.0)] {
                            pos.push((q + across * (side * w * 0.5)).to_array());
                            uv.push([u, if side < 0.0 { 0.0 } else { 1.0 }]);
                            // Tint: the limb's brown-red; green's fraction carries the streak's seed.
                            col.push([0.55, 0.25 + (id as f32 * 0.6180339).fract() / 97.0, 0.2, s]);
                        }
                        idx.extend([base, base + 1, base + 2, base + 1, base + 3, base + 2]);
                    }
                }
            }
        }
        if !idx.is_empty() && std::env::var_os("GB_SMEAR").is_some() && (t * 10.0) as i32 % 10 == 0 {
            info!("[smear] {} streaks on {e}", idx.len() / 6);
        }
        if let Ok(mut v) = vis.get_mut(st.mesh) {
            *v = if idx.is_empty() { Visibility::Hidden } else { Visibility::Visible };
        }
        if !idx.is_empty() {
            if let Some(mut m) = meshes.get_mut(&st.handle) {
                let n = pos.len();
                *m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
                    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
                    .with_inserted_indices(Indices::U32(idx));
            }
        }
        st.prev = now;
        st.reach = reach;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_starts_at_the_threshold_and_pullbacks_smear_less() {
        assert_eq!(strength(V_MIN - 0.1, true), 0.0);
        assert!((strength(V_FULL, true) - 1.0).abs() < 1e-6);
        assert!((strength(V_FULL, false) - RETRACT).abs() < 1e-6);
    }

    #[test]
    fn brush_lengths_are_uneven_within_their_range() {
        let v: Vec<f32> = (0..60).map(|i| brush(i, 0.3)).collect();
        assert!(v.iter().all(|x| (0.0..=1.21).contains(x)));
        let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        assert!(hi - lo > 0.4, "{lo}..{hi}");
    }
}
