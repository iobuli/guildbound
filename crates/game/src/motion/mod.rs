//! Stylized motion synthesis.
//!
//! Pipeline (all pure data until `compose`):
//! 1. `donor`    – parse a GLB (gltf crate) into a source skeleton + dense rotation/translation tracks.
//! 2. `retarget` – map donor clips onto the canonical UAL2 skeleton (`Skeleton`), sampled at
//!                 `SAMPLE_HZ` into `Clip`s of `Pose`s. UAL1/UAL2 map by name; Sekiro (Mixamo)
//!                 maps through a name table + world-space bind-delta retarget.
//! 3. `extract`  – reduce each `Clip` to a `Track` of a few tagged extremes (`Key`s).
//! 4. `bank`     – `PoseBank` resource: every track, stylized sampling (`timing` easing +
//!                 exaggeration), `assets/anim/poses.json` overrides with hot reload.
//! 5. `compose`  – the ONLY system that writes bone `Transform`s. Evaluates each character's
//!                 `MotionLayers`, runs `secondary` springs, `body` self-collision and
//!                 `contact` IK, `hand` grips (finger curl), writes bones.
//!
//! Conventions (shared by every module, do not change):
//! - Canonical skeleton = the joint list of `assets/anim/ual2.glb` skin 0, in that order.
//! - Model space: +Y up, character faces +Z, character's left is +X (glTF default).
//! - `Pose.rot[i]` is the LOCAL rotation of canonical bone `i` (parent-relative), the same
//!   value that goes into that bone entity's `Transform::rotation`.
//! - `Pose.pelvis` is the pelvis LOCAL translation offset from its rest translation.
//! - Track names are `"{lib}/{clip}"`, lib ∈ {`ual1`, `ual2`, `sekiro`, `rokoko`, `cmu`}, clip = glTF animation
//!   name verbatim, e.g. `"ual1/Punch_Jab"`, `"sekiro/Inward Slash"`. Times are seconds in the
//!   original clip.

pub mod bank;
pub mod body;
pub mod bvh;
pub mod compose;
pub mod contact;
pub mod donor;
pub mod extract;
pub mod hand;
pub mod retarget;
pub mod rig;
pub mod secondary;
pub mod timing;
pub mod tweak;

use bevy::prelude::*;
use std::collections::HashMap;

/// Dense sampling rate for retargeted clips.
pub const SAMPLE_HZ: f32 = 30.0;
/// The character model and canonical skeleton.
pub const MODEL_GLB: &str = "anim/ual2.glb";
/// Mocap takes (`*.bvh`, library `cmu`), relative to `crate::ASSETS`; see `bvh`.
pub const CMU_DIR: &str = "anim/cmu";
/// Donor libraries: (lib name, path relative to `crate::ASSETS`).
pub const LIBRARIES: [(&str, &str); 4] = [
    ("ual1", "anim/ual1.glb"),
    ("ual2", "anim/ual2.glb"),
    ("sekiro", "anim/sekiro.glb"),
    // Rokoko free mocap (Mixamo rig), converted from assets/anim/Rokoko_Free_Mocap_FBX_263.
    ("rokoko", "anim/rokoko.glb"),
];

pub fn plugin(app: &mut App) {
    app.configure_sets(
        PostUpdate,
        MotionSet::Compose
            .after(bevy::app::AnimationSystems)
            .before(bevy::transform::TransformSystems::Propagate),
    )
    .add_plugins((bank::plugin, rig::plugin, compose::plugin, tweak::plugin));
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MotionSet {
    /// `compose` runs here. Everything in `play` that fills `MotionLayers` runs in `Update`.
    Compose,
}

/// Canonical skeleton (UAL2 joint order).
#[derive(Clone, Debug)]
pub struct Skeleton {
    pub names: Vec<String>,
    /// Parent index within the skeleton (None for the skeleton root `root`).
    pub parent: Vec<Option<usize>>,
    /// Rest LOCAL transform of each bone.
    pub rest: Vec<Transform>,
    /// Model-space transform of the skeleton root's parent chain (armature node etc.).
    pub base: Transform,
    pub index: HashMap<String, usize>,
}

impl Skeleton {
    pub fn len(&self) -> usize {
        self.names.len()
    }
    pub fn find(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }
    pub fn rest_pose(&self) -> Pose {
        Pose {
            rot: self.rest.iter().map(|t| t.rotation).collect(),
            pelvis: Vec3::ZERO,
        }
    }
    /// Model-space transforms of every bone for `pose` (parents always precede children).
    pub fn fk(&self, pose: &Pose, pelvis: usize) -> Vec<Transform> {
        let mut out: Vec<Transform> = Vec::with_capacity(self.len());
        for i in 0..self.len() {
            let mut local = self.rest[i];
            local.rotation = pose.rot[i];
            if i == pelvis {
                local.translation += pose.pelvis;
            }
            let parent = match self.parent[i] {
                Some(p) => out[p],
                None => self.base,
            };
            out.push(parent.mul_transform(local));
        }
        out
    }
}

/// One full-body pose on the canonical skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub rot: Vec<Quat>,
    pub pelvis: Vec3,
}

impl Pose {
    pub fn lerp(&self, other: &Pose, t: f32) -> Pose {
        Pose {
            rot: self
                .rot
                .iter()
                .zip(&other.rot)
                .map(|(a, b)| a.slerp(*b, t))
                .collect(),
            pelvis: self.pelvis.lerp(other.pelvis, t),
        }
    }
    /// Per-bone blend toward `other`: bone `i` moves by `t * mask[i]`.
    pub fn blend_masked(&self, other: &Pose, t: f32, mask: &[f32], pelvis_weight: f32) -> Pose {
        Pose {
            rot: self
                .rot
                .iter()
                .zip(&other.rot)
                .zip(mask)
                .map(|((a, b), m)| a.slerp(*b, (t * m).clamp(0.0, 1.0)))
                .collect(),
            pelvis: self.pelvis.lerp(other.pelvis, (t * pelvis_weight).clamp(0.0, 1.0)),
        }
    }
}

/// Named indices of the bones the runtime cares about. Built once from the skeleton.
#[derive(Clone, Copy, Debug)]
pub struct Bones {
    pub root: usize,
    pub pelvis: usize,
    pub spine: [usize; 3],
    pub neck: usize,
    pub head: usize,
    /// [left, right] for every pair below.
    pub clavicle: [usize; 2],
    pub upperarm: [usize; 2],
    pub lowerarm: [usize; 2],
    pub hand: [usize; 2],
    pub thigh: [usize; 2],
    pub calf: [usize; 2],
    pub foot: [usize; 2],
    pub ball: [usize; 2],
}

impl Bones {
    pub fn new(s: &Skeleton) -> Option<Self> {
        let f = |n: &str| s.find(n);
        let pair = |a: &str| Some([f(&format!("{a}_l"))?, f(&format!("{a}_r"))?]);
        Some(Self {
            root: f("root")?,
            pelvis: f("pelvis")?,
            spine: [f("spine_01")?, f("spine_02")?, f("spine_03")?],
            neck: f("neck_01")?,
            head: f("Head")?,
            clavicle: pair("clavicle")?,
            upperarm: pair("upperarm")?,
            lowerarm: pair("lowerarm")?,
            hand: pair("hand")?,
            thigh: pair("thigh")?,
            calf: pair("calf")?,
            foot: pair("foot")?,
            ball: pair("ball")?,
        })
    }
}

/// Which bones a layer drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mask {
    #[default]
    Full,
    /// pelvis children above the hips: spine, neck, head, arms, fingers.
    Upper,
    /// pelvis translation, legs.
    Lower,
    ArmL,
    ArmR,
    Arms,
    Head,
}

/// What a layer samples.
#[derive(Clone, Debug)]
pub enum Source {
    /// Stylized sample of a bank track at `time` seconds (clamped or wrapped per `looping`).
    Track { name: String, time: f32, looping: bool },
    /// A pose computed by play code (e.g. a procedural aim pose).
    Pose(Pose),
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub source: Source,
    /// 0..1 blend toward this layer from everything below it.
    pub weight: f32,
    pub mask: Mask,
    /// Timing feel for this layer (see `timing::Style`); `None` = bank default.
    pub style: Option<timing::Style>,
}

/// Filled every frame by `play::select` for each animated character; consumed by `compose`.
#[derive(Component, Clone, Debug, Default)]
pub struct MotionLayers {
    /// Evaluated bottom to top. `layers[0]` should be Full-mask.
    pub layers: Vec<Layer>,
    /// Contact / procedural requests for this frame.
    pub contact: ContactRequest,
    /// Angular impulses (canonical bone index, world-space axis*angle rad/s) for the springs.
    /// Appended by `play` systems (combat hits, landings); drained by `compose` each frame.
    pub impulses: Vec<(usize, Vec3)>,
    /// Bypass springs and contact (ragdoll feeds a physics-driven `Source::Pose`).
    pub raw: bool,
}

#[derive(Clone, Debug)]
pub struct ContactRequest {
    /// 0..1 weight of foot IK onto the ground under each foot.
    pub foot_ik: f32,
    /// World-space hand targets with weights (climb holds, bow string, two-hand grip, levers).
    pub hands: [Option<(Vec3, f32)>; 2],
    /// World-space acceleration of the character (m/s²): horizontal for COP lean (Kwon 2020:
    /// torso over COP + h·a/g) and the arm trail; vertical = the step bounce's acceleration,
    /// which the head and chest nod against.
    pub accel: Vec3,
    /// Step bounce (m, ≤ 0): extra pelvis dip after each foot plant. Feet stay planted.
    pub bob: f32,
    /// Extra spine squash (−) / stretch (+), roughly −0.3..0.3.
    pub squash: f32,
    /// World point the head/chest should look toward (lock-on, aiming).
    pub look_at: Option<Vec3>,
    /// `Mover.step_ease`: running total of the drawn root's vertical easing over steps (m).
    pub root_ease: f32,
    /// World-space ankle targets with weights (climb holds), applied after the ground IK: knees
    /// bend toward the body's front turned out, feet keep their animated world orientation.
    pub feet: [Option<(Vec3, f32)>; 2],
    /// Narrow stance (balance beam): the farthest a foot on the ground may stand from the body's
    /// mid-plane (m); `f32::INFINITY` keeps the animated stance.
    pub stance: f32,
    /// World direction a held ranged weapon aims along: the spine bends so the chest follows the
    /// aim's pitch (the clips shoot level). `None` eases back.
    pub aim: Option<Vec3>,
    /// Finger grips `[left, right]` (`hand::Grip`): door knobs, levers, weapon handles, flat
    /// pushes. A world-space grip also places and turns the wrist (it replaces `hands`).
    pub grips: [Option<hand::Grip>; 2],
}

impl Default for ContactRequest {
    fn default() -> Self {
        Self {
            foot_ik: 1.0,
            hands: [None, None],
            accel: Vec3::ZERO,
            bob: 0.0,
            squash: 0.0,
            look_at: None,
            root_ease: 0.0,
            feet: [None, None],
            stance: f32::INFINITY,
            aim: None,
            grips: [None, None],
        }
    }
}

/// Lives on the character ROOT entity (the capsule). Created by `rig` when the model is ready.
#[derive(Component, Clone, Debug)]
pub struct Rig {
    /// The model scene entity (child of the root). Its local transform places the feet at the
    /// bottom of the capsule.
    pub model: Entity,
    /// Bone entity per canonical skeleton index (`Entity::PLACEHOLDER` when missing).
    pub bones: Vec<Entity>,
}

/// Marker on every bone entity bound in a `Rig`.
#[derive(Component)]
pub struct RigBone;

/// The pose `compose` just wrote, as model-space transforms (one per canonical bone, relative to
/// the `Rig.model` entity). Written by `compose` only, every frame, before transform propagation,
/// so things riding on the body (armour plates) can follow it in the same frame.
#[derive(Component, Clone, Debug, Default)]
pub struct RigPose(pub Vec<Transform>);

/// Request a model to be spawned under a character root. `rig` handles it.
#[derive(Component, Clone, Debug)]
pub struct ModelRequest {
    /// Asset path relative to `crate::ASSETS`, e.g. `MODEL_GLB`.
    pub path: String,
    /// Local offset of the model under the root (feet at capsule bottom).
    pub offset: Vec3,
    pub tint: Option<Color>,
}
