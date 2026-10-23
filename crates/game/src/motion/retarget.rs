//! Donor clip → canonical skeleton.
//! - Same-skeleton libraries (`ual1`, `ual2`): copy local rotations by bone name.
//! - Mixamo (`sekiro`): name map (`MIXAMO_MAP` /
//!   `CMU_MAP`), then per mapped bone in model space
//!   `W_dst(t) = W_src(t) · W_src_bind⁻¹ · W_dst_bind`, converted back to local through the
//!   destination parent chain. Both binds are T-pose, +Z forward, so this removes axis-convention
//!   differences. Unmapped destination bones keep rest. Pelvis translation is the source hips
//!   model-space offset from bind, scaled by the leg-length ratio, expressed in pelvis-parent space.
use super::donor::{DonorClip, DonorLib, safe_normalize};
use super::{Bones, Pose, SAMPLE_HZ, Skeleton};
use bevy::prelude::*;

/// A retargeted dense clip on the canonical skeleton.
#[derive(Clone, Debug)]
pub struct Clip {
    /// `"{lib}/{clip}"`.
    pub track: String,
    pub duration: f32,
    /// Heuristic: name contains `Loop`, or is a known cyclic locomotion clip
    /// (Walk/Run/Jog/Sprint/Swim/Crouch_Fwd/Strafe/`Magic Walk`), or first≈last frame (for a
    /// clip that stays near its first pose or is named like an idle; attacks that return to
    /// their start pose stay one-shots).
    pub looping: bool,
    /// Frames at `SAMPLE_HZ`, frame i at time `i / SAMPLE_HZ` (last frame clamped to duration).
    /// Loops whose donor duplicates its first key (Mixamo) drop that key so cycles do not hitch.
    pub frames: Vec<Pose>,
    /// Model-space horizontal pelvis travel relative to frame 0, per frame (root motion source).
    /// The `frames[i].pelvis` have this horizontal travel REMOVED (in-place poses).
    pub travel: Vec<Vec3>,
}

/// (mixamo bone without the `mixamorig:` prefix, canonical UAL name).
pub const MIXAMO_MAP: &[(&str, &str)] = &[
    ("Hips", "pelvis"),
    ("Spine", "spine_01"),
    ("Spine1", "spine_02"),
    ("Spine2", "spine_03"),
    ("Neck", "neck_01"),
    ("Head", "Head"),
    ("LeftShoulder", "clavicle_l"),
    ("LeftArm", "upperarm_l"),
    ("LeftForeArm", "lowerarm_l"),
    ("LeftHand", "hand_l"),
    ("RightShoulder", "clavicle_r"),
    ("RightArm", "upperarm_r"),
    ("RightForeArm", "lowerarm_r"),
    ("RightHand", "hand_r"),
    ("LeftUpLeg", "thigh_l"),
    ("LeftLeg", "calf_l"),
    ("LeftFoot", "foot_l"),
    ("LeftToeBase", "ball_l"),
    ("RightUpLeg", "thigh_r"),
    ("RightLeg", "calf_r"),
    ("RightFoot", "foot_r"),
    ("RightToeBase", "ball_r"),
    ("LeftHandThumb1", "thumb_01_l"),
    ("LeftHandThumb2", "thumb_02_l"),
    ("LeftHandThumb3", "thumb_03_l"),
    ("LeftHandIndex1", "index_01_l"),
    ("LeftHandIndex2", "index_02_l"),
    ("LeftHandIndex3", "index_03_l"),
    ("LeftHandMiddle1", "middle_01_l"),
    ("LeftHandMiddle2", "middle_02_l"),
    ("LeftHandMiddle3", "middle_03_l"),
    ("LeftHandRing1", "ring_01_l"),
    ("LeftHandRing2", "ring_02_l"),
    ("LeftHandRing3", "ring_03_l"),
    ("LeftHandPinky1", "pinky_01_l"),
    ("LeftHandPinky2", "pinky_02_l"),
    ("LeftHandPinky3", "pinky_03_l"),
    ("RightHandThumb1", "thumb_01_r"),
    ("RightHandThumb2", "thumb_02_r"),
    ("RightHandThumb3", "thumb_03_r"),
    ("RightHandIndex1", "index_01_r"),
    ("RightHandIndex2", "index_02_r"),
    ("RightHandIndex3", "index_03_r"),
    ("RightHandMiddle1", "middle_01_r"),
    ("RightHandMiddle2", "middle_02_r"),
    ("RightHandMiddle3", "middle_03_r"),
    ("RightHandRing1", "ring_01_r"),
    ("RightHandRing2", "ring_02_r"),
    ("RightHandRing3", "ring_03_r"),
    ("RightHandPinky1", "pinky_01_r"),
    ("RightHandPinky2", "pinky_02_r"),
    ("RightHandPinky3", "pinky_03_r"),
];

/// (Kevin Iglesias `HumanM` bone, canonical UAL name). Kevin's spine is two bones (`B-spine`,
/// `B-chest`): they drive `spine_01` and `spine_03`, `spine_02` keeps rest. The packs are converted
/// by `tools/model/kev_pack.py` (Y-up metres, T-pose bind facing +Z, like the other donors).
pub const KEVIN_MAP: &[(&str, &str)] = &[
    ("B-hips", "pelvis"),
    ("B-spine", "spine_01"),
    ("B-chest", "spine_03"),
    ("B-neck", "neck_01"),
    ("B-head", "Head"),
    ("B-shoulder.L", "clavicle_l"),
    ("B-upperArm.L", "upperarm_l"),
    ("B-forearm.L", "lowerarm_l"),
    ("B-hand.L", "hand_l"),
    ("B-shoulder.R", "clavicle_r"),
    ("B-upperArm.R", "upperarm_r"),
    ("B-forearm.R", "lowerarm_r"),
    ("B-hand.R", "hand_r"),
    ("B-thigh.L", "thigh_l"),
    ("B-shin.L", "calf_l"),
    ("B-foot.L", "foot_l"),
    ("B-toe.L", "ball_l"),
    ("B-thigh.R", "thigh_r"),
    ("B-shin.R", "calf_r"),
    ("B-foot.R", "foot_r"),
    ("B-toe.R", "ball_r"),
    ("B-thumb01.L", "thumb_01_l"),
    ("B-thumb02.L", "thumb_02_l"),
    ("B-thumb03.L", "thumb_03_l"),
    ("B-indexFinger01.L", "index_01_l"),
    ("B-indexFinger02.L", "index_02_l"),
    ("B-indexFinger03.L", "index_03_l"),
    ("B-middleFinger01.L", "middle_01_l"),
    ("B-middleFinger02.L", "middle_02_l"),
    ("B-middleFinger03.L", "middle_03_l"),
    ("B-ringFinger01.L", "ring_01_l"),
    ("B-ringFinger02.L", "ring_02_l"),
    ("B-ringFinger03.L", "ring_03_l"),
    ("B-pinky01.L", "pinky_01_l"),
    ("B-pinky02.L", "pinky_02_l"),
    ("B-pinky03.L", "pinky_03_l"),
    ("B-thumb01.R", "thumb_01_r"),
    ("B-thumb02.R", "thumb_02_r"),
    ("B-thumb03.R", "thumb_03_r"),
    ("B-indexFinger01.R", "index_01_r"),
    ("B-indexFinger02.R", "index_02_r"),
    ("B-indexFinger03.R", "index_03_r"),
    ("B-middleFinger01.R", "middle_01_r"),
    ("B-middleFinger02.R", "middle_02_r"),
    ("B-middleFinger03.R", "middle_03_r"),
    ("B-ringFinger01.R", "ring_01_r"),
    ("B-ringFinger02.R", "ring_02_r"),
    ("B-ringFinger03.R", "ring_03_r"),
    ("B-pinky01.R", "pinky_01_r"),
    ("B-pinky02.R", "pinky_02_r"),
    ("B-pinky03.R", "pinky_03_r"),
];

/// (CMU BVH joint, canonical UAL name). CMU's spine is LowerBack → Spine → Spine1 and its neck
/// Neck → Neck1 → Head; fingers are a single base bone, left unmapped (the hand rig poses them).
pub const CMU_MAP: &[(&str, &str)] = &[
    ("Hips", "pelvis"),
    ("LowerBack", "spine_01"),
    ("Spine", "spine_02"),
    ("Spine1", "spine_03"),
    ("Neck", "neck_01"),
    ("Head", "Head"),
    ("LeftShoulder", "clavicle_l"),
    ("LeftArm", "upperarm_l"),
    ("LeftForeArm", "lowerarm_l"),
    ("LeftHand", "hand_l"),
    ("RightShoulder", "clavicle_r"),
    ("RightArm", "upperarm_r"),
    ("RightForeArm", "lowerarm_r"),
    ("RightHand", "hand_r"),
    ("LeftUpLeg", "thigh_l"),
    ("LeftLeg", "calf_l"),
    ("LeftFoot", "foot_l"),
    ("LeftToeBase", "ball_l"),
    ("RightUpLeg", "thigh_r"),
    ("RightLeg", "calf_r"),
    ("RightFoot", "foot_r"),
    ("RightToeBase", "ball_r"),
];

/// Bind / reference clips that carry no motion.
const SKIPPED: [&str; 2] = ["A_TPose", "T-Pose"];
/// Name fragments of cyclic clips.
const LOOP_WORDS: [&str; 8] = [
    "Loop",
    "Walk",
    "Run",
    "Jog",
    "Sprint",
    "Swim",
    "Crouch_Fwd",
    "Strafe",
];
/// First≈last tolerance for unnamed loops: per-bone angle (rad) and pelvis height (m).
const LOOP_ANGLE: f32 = 0.06;
const LOOP_HEIGHT: f32 = 0.02;
/// A closing clip whose body never leaves its first pose by more than this (rad) is a loop.
const LOOP_EXCURSION: f32 = 0.35;
const MIXAMO_PREFIX: &str = "mixamorig:";

/// Number of dense frames for a clip of `duration` seconds at `SAMPLE_HZ`.
pub fn frame_count(duration: f32) -> usize {
    if !(duration > 0.0) || !duration.is_finite() {
        return 1;
    }
    (duration * SAMPLE_HZ - 1e-3).ceil().max(1.0) as usize + 1
}

/// Time of dense frame `i` (the last frame is clamped to `duration`).
pub fn frame_time(i: usize, duration: f32) -> f32 {
    (i as f32 / SAMPLE_HZ).min(duration.max(0.0))
}

/// Wrap (looping) or clamp a clip time into `[0, duration]`. NaN maps to 0.
pub fn clip_time(time: f32, duration: f32, looping: bool) -> f32 {
    if !(duration > 0.0) || time.is_nan() {
        return 0.0;
    }
    if looping && time.is_finite() {
        let t = time.rem_euclid(duration);
        // rem_euclid can round up to `duration` for tiny negative inputs.
        if t >= duration { 0.0 } else { t }
    } else {
        time.clamp(0.0, duration)
    }
}

/// How a library's bones are found for each canonical bone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Same skeleton: copy local rotations.
    ByName,
    /// Mixamo rig: model-space bind-delta.
    Mixamo,
    /// CMU mocap (BVH): model-space bind-delta with `CMU_MAP`.
    Cmu,
    /// Kevin Iglesias rig (`B-hips`, ...): model-space bind-delta with `KEVIN_MAP`.
    Kevin,
}

/// Precomputed per-library mapping onto the canonical skeleton.
struct Mapping {
    mode: Mode,
    /// Source node per canonical bone.
    src: Vec<Option<usize>>,
    /// Mixamo: `W_src_bind⁻¹ · W_dst_bind` rotation per canonical bone.
    bind_delta: Vec<Quat>,
    /// Source hips node and its model-space bind position.
    hips: Option<usize>,
    hips_bind: Vec3,
    /// Destination / source leg length.
    scale: f32,
    /// Canonical bones in parent-first order.
    order: Vec<usize>,
    /// Rest model-space scale of the pelvis parent (to express offsets in its space).
    pelvis_parent_scale: Vec3,
}

impl Mapping {
    fn new(lib: &DonorLib, target: &Skeleton, bones: &Bones) -> Self {
        let mode = if lib.node(&format!("{MIXAMO_PREFIX}Hips")).is_some() {
            Mode::Mixamo
        } else if lib.node("LowerBack").is_some() && lib.node("LHipJoint").is_some() {
            Mode::Cmu
        } else if lib.node("B-hips").is_some() && lib.node("B-chest").is_some() {
            Mode::Kevin
        } else {
            Mode::ByName
        };
        let node_of: std::collections::HashMap<&str, usize> = lib
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.name.as_str(), i))
            .collect();
        let mut src = vec![None; target.len()];
        match mode {
            Mode::ByName => {
                for (i, name) in target.names.iter().enumerate() {
                    src[i] = node_of.get(name.as_str()).copied();
                }
            }
            Mode::Mixamo => {
                for (mixamo, canonical) in MIXAMO_MAP {
                    if let Some(i) = target.find(canonical) {
                        src[i] = node_of.get(format!("{MIXAMO_PREFIX}{mixamo}").as_str()).copied();
                    }
                }
            }
            Mode::Kevin => {
                for (kev, canonical) in KEVIN_MAP {
                    if let Some(i) = target.find(canonical) {
                        src[i] = node_of.get(kev).copied();
                    }
                }
            }
            Mode::Cmu => {
                for (cmu, canonical) in CMU_MAP {
                    if let Some(i) = target.find(canonical) {
                        src[i] = node_of.get(cmu).copied();
                    }
                }
            }
        }

        let dst_rest = target.fk(&target.rest_pose(), bones.pelvis);
        let src_rest: Vec<Option<Transform>> =
            src.iter().map(|s| s.map(|n| lib.rest_world(n))).collect();
        let bind_delta = (0..target.len())
            .map(|i| match src_rest[i] {
                Some(sw) => safe_normalize(sw.rotation.inverse() * dst_rest[i].rotation),
                None => Quat::IDENTITY,
            })
            .collect();

        // Leg length ratio (both sides averaged) scales the hips offset and travel.
        let leg = |pos: &dyn Fn(usize) -> Option<Vec3>| -> Option<f32> {
            let mut sum = 0.0;
            for side in 0..2 {
                let (a, b, c) = (
                    pos(bones.thigh[side])?,
                    pos(bones.calf[side])?,
                    pos(bones.foot[side])?,
                );
                sum += a.distance(b) + b.distance(c);
            }
            Some(sum * 0.5)
        };
        let dst_leg = leg(&|i| Some(dst_rest[i].translation));
        let src_leg = leg(&|i| src_rest[i].map(|t| t.translation));
        let scale = match (dst_leg, src_leg) {
            (Some(d), Some(s)) if s > 1e-3 && (d / s).is_finite() => d / s,
            _ => 1.0,
        };

        let hips = src[bones.pelvis];
        let hips_bind = hips.map_or(Vec3::ZERO, |h| lib.rest_world(h).translation);
        let pelvis_parent_scale = target.parent[bones.pelvis]
            .map_or(target.base.scale, |p| dst_rest[p].scale)
            .max(Vec3::splat(1e-4));

        Self {
            mode,
            src,
            bind_delta,
            hips,
            hips_bind,
            scale,
            order: parent_first(target),
            pelvis_parent_scale,
        }
    }
}

/// Canonical bone indices ordered so every parent precedes its children.
fn parent_first(s: &Skeleton) -> Vec<usize> {
    let n = s.len();
    let mut placed = vec![false; n];
    let mut order = Vec::with_capacity(n);
    let mut stack = Vec::new();
    for i in 0..n {
        let mut cursor = Some(i);
        while let Some(c) = cursor {
            if placed[c] || stack.len() > n {
                break;
            }
            stack.push(c);
            cursor = s.parent[c];
        }
        while let Some(c) = stack.pop() {
            if !placed[c] {
                placed[c] = true;
                order.push(c);
            }
        }
    }
    order
}

/// Model-space translation of one source node at time `t` (walks only its parent chain).
fn node_world_translation(lib: &DonorLib, clip: &DonorClip, node: usize, t: f32) -> Vec3 {
    let local = |i: usize| {
        let mut l = lib.nodes[i].rest;
        if let Some(c) = clip.rotations.get(&i)
            && !c.times.is_empty()
        {
            l.rotation = c.sample(t);
        }
        if let Some(c) = clip.translations.get(&i)
            && !c.times.is_empty()
        {
            l.translation = c.sample(t);
        }
        l
    };
    let mut world = local(node);
    let mut cursor = lib.nodes[node].parent;
    let mut guard = lib.nodes.len();
    while let Some(p) = cursor
        && guard > 0
    {
        guard -= 1;
        world = local(p).mul_transform(world);
        cursor = lib.nodes[p].parent;
    }
    world.translation
}

/// One retargeted frame before travel extraction.
struct RawFrame {
    rot: Vec<Quat>,
    /// Hips offset from bind in model space (scaled).
    offset: Vec3,
    /// Model-space rotation of the pelvis parent this frame.
    parent_rot: Quat,
}

fn retarget_frame(
    lib: &DonorLib,
    clip: &DonorClip,
    t: f32,
    target: &Skeleton,
    bones: &Bones,
    map: &Mapping,
) -> RawFrame {
    let n = target.len();
    let mut rot: Vec<Quat> = target.rest.iter().map(|r| r.rotation).collect();
    let mut world_rot = vec![Quat::IDENTITY; n];
    let src_world = match map.mode {
        Mode::Mixamo | Mode::Cmu | Mode::Kevin => Some(lib.world_at(clip, t)),
        Mode::ByName => None,
    };
    for &i in &map.order {
        let parent_world = target.parent[i].map_or(target.base.rotation, |p| world_rot[p]);
        let local = match (map.mode, map.src[i]) {
            (Mode::ByName, Some(s)) => clip
                .rotations
                .get(&s)
                .filter(|c| !c.times.is_empty())
                .map_or(target.rest[i].rotation, |c| c.sample(t)),
            (Mode::Mixamo | Mode::Cmu | Mode::Kevin, Some(s)) => {
                let w = src_world.as_ref().map_or(Quat::IDENTITY, |w| w[s].rotation);
                parent_world.inverse() * (w * map.bind_delta[i])
            }
            (_, None) => target.rest[i].rotation,
        };
        let local = safe_normalize(local);
        rot[i] = local;
        world_rot[i] = safe_normalize(parent_world * local);
    }

    let offset = match (map.hips, &src_world) {
        (Some(h), Some(w)) => (w[h].translation - map.hips_bind) * map.scale,
        (Some(h), None) => (node_world_translation(lib, clip, h, t) - map.hips_bind) * map.scale,
        _ => Vec3::ZERO,
    };
    let offset = if offset.is_finite() { offset } else { Vec3::ZERO };
    let parent_rot = target.parent[bones.pelvis].map_or(target.base.rotation, |p| world_rot[p]);
    RawFrame {
        rot,
        offset,
        parent_rot,
    }
}

/// Max per-bone angle between two poses' rotations.
fn max_angle(a: &[Quat], b: &[Quat]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.angle_between(*y))
        .fold(0.0, f32::max)
}

/// Body bones (no fingers / leaves) used to judge how far a clip moves.
fn body_bones(b: &Bones) -> [usize; 14] {
    [
        b.pelvis,
        b.spine[0],
        b.spine[1],
        b.spine[2],
        b.neck,
        b.head,
        b.upperarm[0],
        b.upperarm[1],
        b.lowerarm[0],
        b.lowerarm[1],
        b.thigh[0],
        b.thigh[1],
        b.calf[0],
        b.calf[1],
    ]
}

/// Mixamo-style idle names ("Idle Active", "Dizzy Idle", "Sword_Idle"); UAL one-shots that
/// merely start from idle ("Idle_Shield_Break") do not count.
fn idle_named(name: &str) -> bool {
    let tokens: Vec<&str> = name
        .split(|c: char| c == ' ' || c == '_' || c == '-')
        .filter(|t| !t.is_empty())
        .collect();
    tokens.contains(&"Idle")
        && tokens
            .last()
            .is_some_and(|t| matches!(*t, "Idle" | "Active" | "Inactive"))
}

/// Loop heuristic: cyclic name, or the clip closes (first≈last) and either stays near its
/// first pose or is named like an idle. One-shots that return to their start pose (attacks,
/// interactions) move far in between and stay one-shots.
fn is_loop(name: &str, raw: &[RawFrame], bones: &Bones) -> bool {
    if LOOP_WORDS.iter().any(|w| name.contains(w)) {
        return true;
    }
    let n = raw.len();
    if n < 3 {
        return false;
    }
    let (first, last) = (&raw[0], &raw[n - 1]);
    let closes = max_angle(&first.rot, &last.rot) < LOOP_ANGLE
        && (first.offset.y - last.offset.y).abs() < LOOP_HEIGHT;
    if !closes {
        return false;
    }
    let body = body_bones(bones);
    let excursion = raw
        .iter()
        .map(|f| {
            body.iter()
                .map(|&j| f.rot[j].angle_between(first.rot[j]))
                .fold(0.0, f32::max)
        })
        .fold(0.0, f32::max);
    excursion < LOOP_EXCURSION || idle_named(name)
}

/// Mixamo exports duplicate the first key, which would hitch every cycle of a loop: drop it
/// (the loop's time base moves by one frame; one-shots keep their original times).
fn trim_frozen_start(mut raw: Vec<RawFrame>, duration: f32, looping: bool) -> (Vec<RawFrame>, f32) {
    let n = raw.len();
    if !looping || n < 4 {
        return (raw, duration);
    }
    // Root travel keeps moving through the duplicate; only the pose is frozen.
    let frozen = max_angle(&raw[0].rot, &raw[1].rot) < 0.005
        && (raw[0].offset.y - raw[1].offset.y).abs() < 1e-3
        && max_angle(&raw[1].rot, &raw[n - 1].rot) < LOOP_ANGLE;
    if !frozen {
        return (raw, duration);
    }
    raw.remove(0);
    let trimmed = frame_time(n - 1, duration) - frame_time(1, duration);
    (raw, trimmed.max(0.0))
}

fn retarget_clip(
    lib: &DonorLib,
    clip: &DonorClip,
    target: &Skeleton,
    bones: &Bones,
    map: &Mapping,
) -> Clip {
    let duration = if clip.duration.is_finite() {
        clip.duration.max(0.0)
    } else {
        0.0
    };
    let n = frame_count(duration);
    let raw: Vec<RawFrame> = (0..n)
        .map(|i| retarget_frame(lib, clip, frame_time(i, duration), target, bones, map))
        .collect();

    let looping = is_loop(&clip.name, &raw, bones);
    let (raw, duration) = trim_frozen_start(raw, duration, looping);
    let n = raw.len();

    // Horizontal hips motion becomes travel: the linear trend for loops (keeps per-cycle
    // sway in the poses), everything relative to frame 0 for one-shots.
    let horizontal = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let h0 = horizontal(raw[0].offset);
    let cycle = horizontal(raw[n - 1].offset) - h0;
    let mut frames = Vec::with_capacity(n);
    let mut travel = Vec::with_capacity(n);
    for (i, f) in raw.into_iter().enumerate() {
        let moved = if looping {
            if duration > 0.0 {
                cycle * (frame_time(i, duration) / duration)
            } else {
                Vec3::ZERO
            }
        } else {
            horizontal(f.offset) - h0
        };
        let keep = f.offset - moved;
        let pelvis = (f.parent_rot.inverse() * keep) / map.pelvis_parent_scale;
        frames.push(Pose {
            rot: f.rot,
            pelvis: if pelvis.is_finite() { pelvis } else { Vec3::ZERO },
        });
        travel.push(moved);
    }

    Clip {
        track: format!("{}/{}", lib.lib, clip.name),
        duration,
        looping,
        frames,
        travel,
    }
}

/// Retarget every clip of `lib` onto `target`. Clips named `A_TPose` / `T-Pose` are skipped.
pub fn retarget(lib: &DonorLib, target: &Skeleton, bones: &Bones) -> Vec<Clip> {
    let map = Mapping::new(lib, target, bones);
    lib.clips
        .iter()
        .filter(|c| !SKIPPED.contains(&c.name.as_str()))
        .map(|c| retarget_clip(lib, c, target, bones, &map))
        .collect()
}

/// Dense sample with an explicit wrap/clamp choice (slerp between neighbouring frames).
pub fn sample_looping(clip: &Clip, time: f32, looping: bool) -> Pose {
    let n = clip.frames.len();
    match n {
        0 => {
            return Pose {
                rot: Vec::new(),
                pelvis: Vec3::ZERO,
            };
        }
        1 => return clip.frames[0].clone(),
        _ => {}
    }
    let t = clip_time(time, clip.duration, looping);
    let i0 = ((t * SAMPLE_HZ).floor().max(0.0) as usize).min(n - 2);
    let (t0, t1) = (frame_time(i0, clip.duration), frame_time(i0 + 1, clip.duration));
    let u = if t1 - t0 > 1e-6 {
        ((t - t0) / (t1 - t0)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Exact frames at the ends of a segment (slerp at 0/1 carries float noise).
    if u <= 1e-5 {
        clip.frames[i0].clone()
    } else if u >= 1.0 - 1e-5 {
        clip.frames[i0 + 1].clone()
    } else {
        clip.frames[i0].lerp(&clip.frames[i0 + 1], u)
    }
}

/// Sample a dense clip at `time` (wrap when looping, clamp otherwise) with slerp between frames.
pub fn sample(clip: &Clip, time: f32) -> Pose {
    sample_looping(clip, time, clip.looping)
}

/// Travel at a time inside `[0, duration]` (lerp between frames).
fn travel_at(clip: &Clip, t: f32) -> Vec3 {
    let n = clip.travel.len();
    if n == 0 {
        return Vec3::ZERO;
    }
    if n == 1 {
        return clip.travel[0];
    }
    let t = t.clamp(0.0, clip.duration.max(0.0));
    let i0 = ((t * SAMPLE_HZ).floor().max(0.0) as usize).min(n - 2);
    let (t0, t1) = (frame_time(i0, clip.duration), frame_time(i0 + 1, clip.duration));
    let u = if t1 - t0 > 1e-6 {
        ((t - t0) / (t1 - t0)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    clip.travel[i0].lerp(clip.travel[i0 + 1], u)
}

/// Horizontal travel between two times (handles wrap for looping clips).
/// Looping clips accept unwrapped times (whole cycles accumulate); `t1 < t0` with both inside
/// `[0, duration]` is read as one forward wrap.
pub fn travel(clip: &Clip, t0: f32, t1: f32) -> Vec3 {
    let d = clip.duration;
    if clip.travel.is_empty() || !(d > 0.0) || !t0.is_finite() || !t1.is_finite() {
        return Vec3::ZERO;
    }
    if !clip.looping {
        return travel_at(clip, t1) - travel_at(clip, t0);
    }
    let mut t1 = t1;
    if t1 < t0 && (0.0..=d).contains(&t0) && (0.0..=d).contains(&t1) {
        t1 += d;
    }
    let cycle = clip.travel[clip.travel.len() - 1] - clip.travel[0];
    let unwrapped = |t: f32| {
        let cycles = (t / d).floor();
        cycle * cycles + travel_at(clip, t - cycles * d)
    };
    unwrapped(t1) - unwrapped(t0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::donor::{self, tests::lib};
    use super::*;
    use std::sync::OnceLock;

    pub(crate) fn canonical() -> &'static (Skeleton, Bones) {
        static S: OnceLock<(Skeleton, Bones)> = OnceLock::new();
        S.get_or_init(|| {
            let s = donor::skeleton(lib("ual2"));
            let b = Bones::new(&s).expect("bones");
            (s, b)
        })
    }

    fn clip(lib_name: &str, name: &str) -> Clip {
        let (s, b) = canonical();
        let l = lib(lib_name);
        let map = Mapping::new(l, s, b);
        let c = l.clips.iter().find(|c| c.name == name).expect("clip");
        retarget_clip(l, c, s, b, &map)
    }

    /// Both binds are T-poses: every anatomical segment points the same way in model space.
    /// Joint-placement offsets (pelvis→thigh, chest→clavicle, clavicle→upperarm, lower spine
    /// curvature, palm→finger roots) differ between the rigs by design and are not segments of
    /// the same bone; the bind-delta retarget is insensitive to them.
    #[test]
    fn sekiro_bind_matches_canonical_bind() {
        let (s, b) = canonical();
        let l = lib("sekiro");
        let dst = s.fk(&s.rest_pose(), b.pelvis);
        let mixamo = |ual: &str| {
            let (mx, _) = MIXAMO_MAP.iter().find(|(_, u)| *u == ual).expect("mapped");
            l.rest_world(l.node(&format!("{MIXAMO_PREFIX}{mx}")).expect("node"))
                .translation
        };
        let mut segments: Vec<(String, String, f32)> = Vec::new();
        for side in ["l", "r"] {
            for (a, c) in [
                ("upperarm", "lowerarm"),
                ("lowerarm", "hand"),
                ("hand", "middle_01"),
                ("thigh", "calf"),
                ("calf", "foot"),
                ("foot", "ball"),
                ("thumb_01", "thumb_02"),
                ("thumb_02", "thumb_03"),
            ] {
                segments.push((format!("{a}_{side}"), format!("{c}_{side}"), 5.0));
            }
            for finger in ["index", "middle", "ring", "pinky"] {
                for (a, c) in [("01", "02"), ("02", "03")] {
                    segments.push((
                        format!("{finger}_{a}_{side}"),
                        format!("{finger}_{c}_{side}"),
                        5.0,
                    ));
                }
            }
        }
        for (a, c) in [("spine_02", "spine_03"), ("spine_03", "neck_01"), ("neck_01", "Head")] {
            segments.push((a.into(), c.into(), 8.0));
        }
        let mut worst = (0.0f32, String::new());
        for (a, c, limit) in &segments {
            let (ia, ic) = (s.find(a).unwrap(), s.find(c).unwrap());
            let dd = (dst[ic].translation - dst[ia].translation).normalize();
            let ds = (mixamo(c) - mixamo(a)).normalize();
            let angle = ds.angle_between(dd).to_degrees();
            if angle > worst.0 {
                worst = (angle, format!("{a}->{c}"));
            }
            assert!(angle < *limit, "{a}->{c} bind direction differs by {angle:.1}°");
        }
        println!("worst bind segment mismatch: {:.1}° ({})", worst.0, worst.1);
    }

    #[test]
    fn sekiro_tpose_clip_retargets_near_rest() {
        let (s, b) = canonical();
        let l = lib("sekiro");
        let map = Mapping::new(l, s, b);
        let tpose = l.clips.iter().find(|c| c.name == "T-Pose").expect("T-Pose clip");
        let clip = retarget_clip(l, tpose, s, b, &map);
        let rest = s.fk(&s.rest_pose(), b.pelvis);
        let mut worst = (0.0f32, "");
        for f in &clip.frames {
            let w = s.fk(f, b.pelvis);
            for &i in &body_bones(b) {
                let a = w[i].rotation.angle_between(rest[i].rotation).to_degrees();
                if a > worst.0 {
                    worst = (a, s.names[i].as_str());
                }
            }
        }
        println!("sekiro T-Pose clip vs canonical rest: worst {:.1}° ({})", worst.0, worst.1);
        assert!(worst.0 < 5.0, "T-Pose retarget off rest by {:.1}° at {}", worst.0, worst.1);
    }

    #[test]
    fn sekiro_bind_retargets_to_rest() {
        let (s, b) = canonical();
        let l = lib("sekiro");
        let map = Mapping::new(l, s, b);
        // A clip whose every channel holds the source bind values.
        let mut bind = donor::DonorClip {
            name: "bind".into(),
            duration: 1.0,
            rotations: Default::default(),
            translations: Default::default(),
        };
        for (i, n) in l.nodes.iter().enumerate() {
            bind.rotations.insert(
                i,
                donor::Curve {
                    times: vec![0.0],
                    values: vec![n.rest.rotation],
                    step: false,
                },
            );
        }
        let f = retarget_frame(l, &bind, 0.5, s, b, &map);
        for i in 0..s.len() {
            let a = f.rot[i].angle_between(s.rest[i].rotation);
            assert!(a < 0.01, "{} off rest by {a}", s.names[i]);
        }
        assert!(f.offset.length() < 1e-4);
        println!("sekiro leg scale {:.3}", map.scale);
    }

    #[test]
    fn kevin_idle_stands_arms_down_and_his_swing_moves_the_hand() {
        let (s, b) = canonical();
        let idle = clip("kev_melee", "Idle01");
        let w = s.fk(&idle.frames[0], b.pelvis);
        for side in 0..2 {
            let y = w[b.foot[side]].translation.y.min(w[b.ball[side]].translation.y);
            assert!(y.abs() < 0.12, "foot {side} at {y}");
            // A T-pose bind mapped right leaves the arms hanging at rest, hands well below the
            // shoulders (a wrong bind delta would leave them out level or twisted up).
            let drop = w[b.upperarm[side]].translation.y - w[b.hand[side]].translation.y;
            assert!(drop > 0.35, "hand {side} only {drop} m below the shoulder");
        }
        assert!(w[b.head].translation.y > 1.4, "head at {}", w[b.head].translation.y);
        let swing = clip("kev_melee", "Attack1H01_R");
        let hand = |f: &Pose| s.fk(f, b.pelvis)[b.hand[1]].translation;
        let start = hand(&swing.frames[0]);
        let reach = swing.frames.iter().map(|f| hand(f).distance(start)).fold(0.0f32, f32::max);
        assert!(reach > 0.5, "the right hand swings {reach} m");
        for f in idle.frames.iter().chain(&swing.frames) {
            assert!(f.rot.iter().all(|q| q.is_finite() && (q.length() - 1.0).abs() < 1e-3));
        }
    }

    #[test]
    fn sekiro_idle_stands_on_ground() {
        let (s, b) = canonical();
        let c = clip("sekiro", "Idle Active");
        assert!(c.looping, "idle should close as a loop");
        let w = s.fk(&c.frames[0], b.pelvis);
        for side in 0..2 {
            let y = w[b.foot[side]].translation.y.min(w[b.ball[side]].translation.y);
            assert!(y.abs() < 0.1, "foot {side} at {y}");
        }
        assert!(w[b.head].translation.y > 1.4, "head at {}", w[b.head].translation.y);
        for f in &c.frames {
            assert!(f.rot.iter().all(|q| q.is_finite() && (q.length() - 1.0).abs() < 1e-3));
        }
    }

    #[test]
    fn root_motion_becomes_travel() {
        let walk = clip("sekiro", "Walk-Forwards");
        assert!(walk.looping);
        let cycle = walk.travel.last().unwrap();
        assert!(cycle.z > 1.0, "walk travels forward: {cycle}");
        // In-place frames: pelvis horizontal offsets stay small.
        for f in &walk.frames {
            assert!(Vec2::new(f.pelvis.x, f.pelvis.y).length() < 0.3 || f.pelvis.length() < 0.3);
        }
        // Two cycles accumulate, wrapped times read as forward.
        let d = walk.duration;
        let two = travel(&walk, 0.0, 2.0 * d);
        assert!((two - *cycle * 2.0).length() < 1e-3);
        let wrapped = travel(&walk, d * 0.9, d * 0.1);
        assert!(wrapped.z > 0.0);

        let roll = clip("ual1", "Roll_RM");
        assert!(!roll.looping);
        assert!(roll.travel.last().unwrap().z > 1.0);
        assert!(travel(&roll, 0.0, roll.duration).z > 1.0);
        assert_eq!(roll.travel[0], Vec3::ZERO);
    }

    #[test]
    fn sampling_wraps_and_clamps() {
        let c = clip("ual1", "Walk_Loop");
        assert!(c.looping);
        assert_eq!(c.frames.len(), frame_count(c.duration));
        let a = sample(&c, 0.25);
        let b = sample(&c, 0.25 + c.duration);
        assert!(max_angle(&a.rot, &b.rot) < 2e-3);
        let jab = clip("ual1", "Punch_Jab");
        assert!(!jab.looping, "jab is a one-shot");
        let end = sample(&jab, jab.duration + 5.0);
        assert!(max_angle(&end.rot, &jab.frames.last().unwrap().rot) < 2e-3);
        let nan = sample(&jab, f32::NAN);
        assert!(nan.rot.iter().all(|q| q.is_finite()));
    }

    #[test]
    fn frame_counts() {
        let hz = SAMPLE_HZ as usize;
        assert_eq!(frame_count(0.0), 1);
        assert_eq!(frame_count(1.0), hz + 1);
        assert_eq!(frame_count((hz + 1) as f32 / SAMPLE_HZ), hz + 2);
        assert_eq!(frame_count(1.0 + 0.5 / SAMPLE_HZ), hz + 2);
        assert!((frame_time(hz + 1, 1.01) - 1.01).abs() < 1e-6);
    }
}

#[cfg(test)]
mod diag {
    use super::super::donor::{self, tests::lib};
    use super::*;
    #[test]
    fn diag_loops() {
        let s = donor::skeleton(lib("ual2"));
        let b = Bones::new(&s).unwrap();
        let major: Vec<usize> = [b.pelvis, b.spine[0], b.spine[1], b.spine[2], b.neck, b.head,
            b.upperarm[0], b.upperarm[1], b.lowerarm[0], b.lowerarm[1], b.thigh[0], b.thigh[1], b.calf[0], b.calf[1]].to_vec();
        for l in ["ual1", "ual2", "sekiro"] {
            let lb = lib(l);
            for c in retarget(lb, &s, &b) {
                let n = c.frames.len();
                let f0 = &c.frames[0];
                let fl = &c.frames[n - 1];
                let close = max_angle(&f0.rot, &fl.rot);
                let exc = c.frames.iter().map(|f| major.iter().map(|&j| f.rot[j].angle_between(f0.rot[j])).fold(0.0, f32::max)).fold(0.0, f32::max);
                let sv = |a: &Pose, b2: &Pose| major.iter().map(|&j| a.rot[j].angle_between(b2.rot[j])).fold(0.0, f32::max) * SAMPLE_HZ;
                let vs = sv(&c.frames[0], &c.frames[1]);
                let ve = sv(&c.frames[n - 2], &c.frames[n - 1]);
                println!("{:<40} loop={} n={:>4} close={:.3} exc={:.2} vs={:.2} ve={:.2} dy={:.3}", c.track, c.looping, n, close, exc, vs, ve, (f0.pelvis - fl.pelvis).length());
            }
        }
    }
    #[test]
    fn diag_bind() {
        let s = donor::skeleton(lib("ual2"));
        let b = Bones::new(&s).unwrap();
        let l = lib("sekiro");
        let dst = s.fk(&s.rest_pose(), b.pelvis);
        for (mixamo, ual) in MIXAMO_MAP {
            let i = s.find(ual).unwrap();
            for (cmx, cual) in MIXAMO_MAP.iter().filter(|(_, u)| s.find(u).is_some_and(|c| s.parent[c] == Some(i))) {
                let sa = l.rest_world(l.node(&format!("{MIXAMO_PREFIX}{mixamo}")).unwrap());
                let sb = l.rest_world(l.node(&format!("{MIXAMO_PREFIX}{cmx}")).unwrap());
                let c = s.find(cual).unwrap();
                let ds = (sb.translation - sa.translation).normalize();
                let dd = (dst[c].translation - dst[i].translation).normalize();
                println!("{:<12}->{:<12} {:>6.1}  src {:?} dst {:?}", ual, cual, ds.angle_between(dd).to_degrees(), ds, dd);
            }
        }
        for name in ["pelvis", "spine_01", "neck_01", "Head", "hand_l", "foot_l", "ball_l", "upperarm_l"] {
            let i = s.find(name).unwrap();
            println!("dst {name} {:?}", dst[i].translation);
        }
        for name in ["Hips", "Spine", "Neck", "Head", "LeftHand", "LeftFoot", "LeftToeBase", "LeftArm"] {
            let n = l.node(&format!("{MIXAMO_PREFIX}{name}")).unwrap();
            println!("src {name} {:?}", l.rest_world(n).translation);
        }
    }
}
