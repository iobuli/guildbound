//! The standing rig: one table of segments, turned into a `sim::api::RigProfile` and into Avian bodies.
//! Segment frames are world-aligned at spawn. Source GLB positions fit the geometry;
//! collider axes and inertia frames follow each fitted segment.

use avian3d::prelude::*;
use bevy::prelude::*;
use guildbound_sim::api::{JointRole, RigJoint, RigProfile};
use guildbound_sim::math::{Mat3, Vec3 as SVec3};

pub enum Shape {
    Capsule(f32),
    Box(Vec3),
}

/// One segment: name, parent index, role, joint anchor (world, rest), segment end (world, rest), shape, mass, torque cap.
pub struct Seg {
    pub name: &'static str,
    pub parent: Option<usize>,
    pub role: JointRole,
    pub anchor: Vec3,
    pub end: Vec3,
    pub shape: Shape,
    pub mass: f32,
    pub cap: f32,
}

/// Total body mass (kg): one constant for every model, never scaled per model or at runtime.
pub const BODY_MASS: f32 = 79.4;

/// Available profiles, with the source peasant first. Geometry comes from each GLB;
/// masses retain the sandbox's shared BODY_MASS tuning.
pub fn available_models() -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../staging/phys/rig.json");
    let mut out: Vec<String> = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| {
            let i = t.find("\"rigs\"")?;
            let a = t[i..].find('{')? + i;
            let b = t[a..].find('}')? + a;
            Some(
                t[a + 1..b]
                    .split(',')
                    .filter_map(|kv| kv.split(':').next())
                    .map(|k| k.trim().trim_matches('"').to_string())
                    .filter(|k| !k.is_empty())
                    .collect(),
            )
        })
        .unwrap_or_default();
    if out.is_empty() {
        out = vec!["master".into(), "mannequin".into(), "hero".into()];
    }
    out.insert(0, "peasant".into());
    out
}

/// Log a loaded model's joint names (JointId order) and record them in hive/RIG_BONES.md, one section per model.
pub fn log_rig(model: &str, segs: &[Seg]) {
    let names: Vec<&str> = segs.iter().map(|s| s.name).collect();
    println!("[anim_rig] {model}: {}", names.join(", "));
    let path = "/workspace/hive/RIG_BONES.md";
    let head = format!("## Loaded model: {model}");
    let body = format!(
        "{head}\nJointId order (game rig, total {BODY_MASS} kg): {}\n",
        names
            .iter()
            .enumerate()
            .map(|(i, n)| format!("{i} {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let Ok(old) = std::fs::read_to_string(path) else {
        return;
    };
    let new = match old.find(&head) {
        Some(a) => {
            let rest = &old[a + head.len()..];
            let b = rest
                .find("\n## ")
                .map(|x| a + head.len() + x + 1)
                .unwrap_or(old.len());
            format!("{}{}{}", &old[..a], body, &old[b..])
        }
        None => format!("{}\n{}", old.trim_end(), "\n".to_string() + &body),
    };
    if new != old {
        let _ = std::fs::write(path, new);
    }
}

/// Segment table for a named model (scaled so the masses always total BODY_MASS).
pub fn segments_for(model: &str) -> Vec<Seg> {
    let mut s = segments();
    match crate::source::SourceMotion::load(model) {
        Ok(source) => source.fit(&mut s),
        Err(e) => eprintln!("[source] {e}; using fallback geometry"),
    }
    let total: f32 = s.iter().map(|x| x.mass).sum();
    for x in s.iter_mut() {
        x.mass *= BODY_MASS / total;
    }
    s
}

/// The rig, parent before child; [0] = pelvis. +x is the body's left (side 0), +z forward. Feet soles at y = 0.
pub fn segments() -> Vec<Seg> {
    use JointRole::*;
    let s = |name, parent, role, a: [f32; 3], e: [f32; 3], shape, mass, cap| Seg {
        name,
        parent,
        role,
        anchor: Vec3::from(a),
        end: Vec3::from(e),
        shape,
        mass,
        cap,
    };
    vec![
        s(
            "B_Pelvis",
            None,
            Pelvis,
            [0.0, 1.00, 0.0],
            [0.0, 1.18, 0.0],
            Shape::Capsule(0.13),
            16.0,
            400.0,
        ),
        s(
            "B_Spine2",
            Some(0),
            Other,
            [0.0, 1.18, 0.0],
            [0.0, 1.50, 0.0],
            Shape::Capsule(0.15),
            20.0,
            400.0,
        ),
        s(
            "B_Head",
            Some(1),
            Other,
            [0.0, 1.52, 0.0],
            [0.0, 1.74, 0.0],
            Shape::Capsule(0.10),
            5.0,
            60.0,
        ),
        s(
            "B_L_UpperArm",
            Some(1),
            Arm(0),
            [0.24, 1.46, 0.0],
            [0.24, 1.18, 0.0],
            Shape::Capsule(0.05),
            3.0,
            80.0,
        ),
        s(
            "B_L_Forearm",
            Some(3),
            Arm(0),
            [0.24, 1.18, 0.0],
            [0.24, 0.92, 0.0],
            Shape::Capsule(0.045),
            2.0,
            50.0,
        ),
        s(
            "B_R_UpperArm",
            Some(1),
            Arm(1),
            [-0.24, 1.46, 0.0],
            [-0.24, 1.18, 0.0],
            Shape::Capsule(0.05),
            3.0,
            80.0,
        ),
        s(
            "B_R_Forearm",
            Some(5),
            Arm(1),
            [-0.24, 1.18, 0.0],
            [-0.24, 0.92, 0.0],
            Shape::Capsule(0.045),
            2.0,
            50.0,
        ),
        s(
            "B_L_Thigh",
            Some(0),
            Hip(0),
            [0.10, 0.98, 0.0],
            [0.10, 0.54, 0.0],
            Shape::Capsule(0.075),
            8.0,
            400.0,
        ),
        s(
            "B_L_Calf",
            Some(7),
            Knee(0),
            [0.10, 0.54, 0.0],
            [0.10, 0.10, 0.0],
            Shape::Capsule(0.06),
            5.0,
            350.0,
        ),
        s(
            "B_R_Thigh",
            Some(0),
            Hip(1),
            [-0.10, 0.98, 0.0],
            [-0.10, 0.54, 0.0],
            Shape::Capsule(0.075),
            8.0,
            400.0,
        ),
        s(
            "B_R_Calf",
            Some(9),
            Knee(1),
            [-0.10, 0.54, 0.0],
            [-0.10, 0.10, 0.0],
            Shape::Capsule(0.06),
            5.0,
            350.0,
        ),
        s(
            "B_L_Foot",
            Some(8),
            Ankle(0),
            [0.10, 0.08, 0.0],
            [0.10, 0.0, 0.0],
            Shape::Box(Vec3::new(0.10, 0.07, 0.26)),
            1.2,
            250.0,
        ),
        s(
            "B_R_Foot",
            Some(10),
            Ankle(1),
            [-0.10, 0.08, 0.0],
            [-0.10, 0.0, 0.0],
            Shape::Box(Vec3::new(0.10, 0.07, 0.26)),
            1.2,
            250.0,
        ),
    ]
}

impl Seg {
    /// Body centre (= centre of mass) in world at rest.
    pub fn centre(&self) -> Vec3 {
        match self.shape {
            Shape::Capsule(_) => (self.anchor + self.end) * 0.5,
            // foot box: sole on the floor, pushed forward so the toe leads
            Shape::Box(h) => Vec3::new(self.anchor.x, h.y * 0.5, self.anchor.z + 0.05),
        }
    }
    /// Capsule length between hemisphere centres.
    pub fn length(&self) -> f32 {
        (self.end - self.anchor).length()
    }
    pub fn collider(&self) -> Collider {
        match self.shape {
            Shape::Capsule(r) => {
                let axis = (self.end - self.anchor).normalize_or(Vec3::Y);
                let half = (self.length() * 0.5 - r).max(0.01);
                Collider::capsule_endpoints(r, -axis * half, axis * half)
            }
            Shape::Box(h) => Collider::cuboid(h.x, h.y, h.z),
        }
    }
    /// Principal inertia about the centre (solid cylinder / box approximation).
    pub fn inertia(&self) -> Vec3 {
        let m = self.mass;
        match self.shape {
            Shape::Capsule(r) => {
                let l = self.length();
                let p = m * (3.0 * r * r + l * l) / 12.0;
                Vec3::new(p, m * r * r * 0.5, p)
            }
            Shape::Box(h) => {
                Vec3::new(
                    m * (h.y * h.y + h.z * h.z),
                    m * (h.x * h.x + h.z * h.z),
                    m * (h.x * h.x + h.y * h.y),
                ) / 12.0
            }
        }
    }

    pub fn inertia_frame(&self) -> Quat {
        match self.shape {
            Shape::Capsule(_) => {
                Quat::from_rotation_arc(Vec3::Y, (self.end - self.anchor).normalize_or(Vec3::Y))
            }
            Shape::Box(_) => Quat::IDENTITY,
        }
    }
}

fn sv(v: Vec3) -> SVec3 {
    SVec3::new(v.x as f64, v.y as f64, v.z as f64)
}

/// RigProfile for sim::api: offsets in the parent frame (= world at rest), com relative to the joint anchor.
pub fn profile(segs: &[Seg]) -> RigProfile {
    RigProfile {
        joints: segs
            .iter()
            .map(|s| {
                let offset = s
                    .parent
                    .map(|p| s.anchor - segs[p].anchor)
                    .unwrap_or(Vec3::ZERO);
                let i = s.inertia();
                let r = bevy::math::Mat3::from_quat(s.inertia_frame());
                let tensor = r * bevy::math::Mat3::from_diagonal(i) * r.transpose();
                let cols = tensor.to_cols_array_2d();
                RigJoint {
                    name: s.name.into(),
                    parent: s.parent.map(|p| p as u16),
                    offset: sv(offset),
                    mass: s.mass as f64,
                    com: sv(s.centre() - s.anchor),
                    inertia: Mat3(std::array::from_fn(|r| {
                        std::array::from_fn(|c| cols[c][r] as f64)
                    })),
                    role: s.role,
                    torque_cap: s.cap as f64,
                }
            })
            .collect(),
        // sole = the foot box collider (0.10 x 0.07 x 0.26 m)
        sole: guildbound_sim::api::SoleBox {
            half_length: 0.13,
            half_width: 0.05,
            half_height: 0.035,
        },
    }
}

const FLOOR: u32 = 1;
const BODY: u32 = 2;
/// Body parts collide with the floor only (self-collision off: joints already keep them apart).
pub fn body_layers() -> CollisionLayers {
    CollisionLayers::new(LayerMask(BODY), LayerMask(FLOOR))
}
pub fn floor_layers() -> CollisionLayers {
    CollisionLayers::new(LayerMask(FLOOR), LayerMask(BODY | 4 | 8))
}

/// Swing cone half-angle for each joint's Avian limit (safety only; the motor holds the pose).
pub fn swing_limit(role: JointRole, _name: &str) -> f32 {
    match role {
        JointRole::Knee(_) => 1.4,
        JointRole::Ankle(_) => 0.6,
        JointRole::Hip(_) => 1.2,
        JointRole::Arm(_) => 1.4,
        _ => 0.5,
    }
}
