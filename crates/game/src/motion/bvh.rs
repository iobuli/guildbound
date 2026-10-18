//! BVH donors: motion-capture takes (the CMU database, `assets/anim/cmu/*.bvh`, written by
//! `tools/cmu/subset.py`) parsed into the same `DonorLib` the GLB donors produce, so `retarget`
//! maps them onto the canonical skeleton like any other library (`retarget::Mode::Cmu`, a
//! bind-delta retarget with the CMU name table).
//!
//! One file = one take = one `DonorLib` (every CMU subject has its own bone offsets) holding one
//! clip named after the file stem; all are in library `lib` (tracks `"cmu/135_02_empi"`).
//! Each joint becomes a node; End Sites are dropped. The bind (`DonorNode.rest`) is frame one,
//! the T-pose every cgspeed take starts with, so `retarget` can take bind deltas against the
//! canonical T-pose; the clip itself starts at frame two (its time 0). Rotations follow the
//! channel order (`Zrotation Yrotation Xrotation` = Rz·Ry·Rx, degrees); the root's position
//! channels become its translation curve. Units stay as in the file: `retarget` scales the hips'
//! travel by the leg-length ratio.
use super::donor::{Curve, DonorClip, DonorLib, DonorNode};
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::Path;

struct Joint {
    name: String,
    parent: Option<usize>,
    offset: Vec3,
    channels: Vec<String>,
}

/// Parse BVH `text` as one clip `clip` of library `lib`.
pub fn parse(lib: &str, clip: &str, text: &str) -> Result<DonorLib, String> {
    let mut tokens = text.split_whitespace().peekable();
    let mut joints: Vec<Joint> = Vec::new();
    // Stack of open blocks: Some(joint) or None for an End Site.
    let mut stack: Vec<Option<usize>> = Vec::new();
    let mut pending: Option<(String, Option<usize>)> = None;
    let number = |t: Option<&str>| -> Result<f32, String> {
        t.ok_or("unexpected end of file")?.parse::<f32>().map_err(|e| format!("bad number: {e}"))
    };
    loop {
        let Some(token) = tokens.next() else { return Err("no MOTION section".into()) };
        match token {
            "HIERARCHY" => {}
            "ROOT" | "JOINT" => {
                let name = tokens.next().ok_or("joint without a name")?.to_string();
                let parent = stack.iter().rev().find_map(|s| *s);
                pending = Some((name, parent));
            }
            "End" => {
                tokens.next();
                pending = None;
                stack.push(None);
                // `{` follows; skip it here so the block is counted once.
                if tokens.next() != Some("{") {
                    return Err("End Site without a block".into());
                }
            }
            "{" => {
                let (name, parent) = pending.take().ok_or("block without a joint")?;
                joints.push(Joint { name, parent, offset: Vec3::ZERO, channels: Vec::new() });
                stack.push(Some(joints.len() - 1));
            }
            "}" => {
                stack.pop().ok_or("unbalanced braces")?;
            }
            "OFFSET" => {
                let v = Vec3::new(number(tokens.next())?, number(tokens.next())?, number(tokens.next())?);
                if let Some(Some(j)) = stack.last() {
                    joints[*j].offset = v;
                }
            }
            "CHANNELS" => {
                let n = number(tokens.next())? as usize;
                let names: Vec<String> = (0..n).map(|_| tokens.next().unwrap_or("").to_string()).collect();
                if let Some(Some(j)) = stack.last() {
                    joints[*j].channels = names;
                }
            }
            "MOTION" => break,
            other => return Err(format!("unexpected token {other:?}")),
        }
    }
    if joints.is_empty() {
        return Err("no joints".into());
    }
    if tokens.next() != Some("Frames:") {
        return Err("expected Frames:".into());
    }
    let frames = number(tokens.next())? as usize;
    if tokens.next() != Some("Frame") || tokens.next() != Some("Time:") {
        return Err("expected Frame Time:".into());
    }
    let frame_time = number(tokens.next())?;
    if frames < 3 || !(frame_time > 0.0) {
        return Err(format!("{frames} frames at {frame_time} s"));
    }
    let width: usize = joints.iter().map(|j| j.channels.len()).sum();
    let values: Vec<f32> = tokens.take(frames * width).map(|t| t.parse::<f32>().unwrap_or(0.0)).collect();
    if values.len() < frames * width {
        return Err(format!("{} values for {frames} frames of {width}", values.len()));
    }

    // Per frame, per joint: local rotation and (root) translation.
    let sample = |f: usize, j: usize, start: usize| -> (Quat, Option<Vec3>) {
        let row = &values[f * width + start..f * width + start + joints[j].channels.len()];
        let mut rotation = Quat::IDENTITY;
        let mut position = Vec3::ZERO;
        let mut has_position = false;
        for (name, v) in joints[j].channels.iter().zip(row) {
            match name.as_str() {
                "Xposition" => (position.x, has_position) = (*v, true),
                "Yposition" => (position.y, has_position) = (*v, true),
                "Zposition" => (position.z, has_position) = (*v, true),
                "Xrotation" => rotation *= Quat::from_rotation_x(v.to_radians()),
                "Yrotation" => rotation *= Quat::from_rotation_y(v.to_radians()),
                "Zrotation" => rotation *= Quat::from_rotation_z(v.to_radians()),
                _ => {}
            }
        }
        (rotation.normalize(), has_position.then_some(position))
    };
    let mut starts = Vec::with_capacity(joints.len());
    let mut at = 0;
    for j in &joints {
        starts.push(at);
        at += j.channels.len();
    }

    let mut nodes = Vec::with_capacity(joints.len());
    let mut rotations = HashMap::new();
    let mut translations = HashMap::new();
    for (j, joint) in joints.iter().enumerate() {
        let (bind_rot, bind_pos) = sample(0, j, starts[j]);
        let translation = joint.offset + bind_pos.unwrap_or(Vec3::ZERO);
        nodes.push(DonorNode { name: joint.name.clone(), parent: joint.parent, rest: Transform::from_translation(translation).with_rotation(bind_rot) });
        let mut rot = Curve { times: Vec::with_capacity(frames - 1), values: Vec::with_capacity(frames - 1), step: false };
        let mut pos = Curve { times: Vec::new(), values: Vec::new(), step: false };
        for f in 1..frames {
            let t = (f - 1) as f32 * frame_time;
            let (q, p) = sample(f, j, starts[j]);
            rot.times.push(t);
            rot.values.push(q);
            if let Some(p) = p {
                pos.times.push(t);
                pos.values.push(joint.offset + p);
            }
        }
        rotations.insert(j, rot);
        if !pos.times.is_empty() {
            translations.insert(j, pos);
        }
    }
    let duration = (frames - 2) as f32 * frame_time;
    Ok(DonorLib {
        lib: lib.to_string(),
        joints: (0..nodes.len()).collect(),
        nodes,
        clips: vec![DonorClip { name: clip.to_string(), duration, rotations, translations }],
    })
}

/// Every `*.bvh` in `dir` as library `lib`, sorted by name. A missing directory is no takes;
/// a file that fails to parse is reported and skipped.
pub fn load_dir(lib: &str, dir: &Path) -> Vec<DonorLib> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "bvh")).collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|path| {
            let stem = path.file_stem()?.to_string_lossy().to_string();
            let text = std::fs::read_to_string(path).map_err(|e| warn!("[bvh] {}: {e}", path.display())).ok()?;
            parse(lib, &stem, &text).map_err(|e| warn!("[bvh] {}: {e}", path.display())).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TINY: &str = "HIERARCHY
ROOT Hips
{
  OFFSET 0 0 0
  CHANNELS 6 Xposition Yposition Zposition Zrotation Yrotation Xrotation
  JOINT LeftUpLeg
  {
    OFFSET 1 -1 0
    CHANNELS 3 Zrotation Yrotation Xrotation
    End Site
    {
      OFFSET 0 -2 0
    }
  }
}
MOTION
Frames: 3
Frame Time: 0.5
0 10 0 0 0 0 0 0 0
1 10 2 0 90 0 0 0 30
2 10 4 0 0 0 0 0 0
";

    #[test]
    fn parses_hierarchy_bind_and_curves() {
        let lib = parse("cmu", "tiny", TINY).unwrap();
        assert_eq!(lib.nodes.len(), 2, "End Sites are not nodes");
        assert_eq!(lib.nodes[1].parent, Some(0));
        assert_eq!(lib.nodes[1].rest.translation, Vec3::new(1.0, -1.0, 0.0));
        // Frame one is the bind: hips at (0, 10, 0).
        assert_eq!(lib.nodes[0].rest.translation, Vec3::new(0.0, 10.0, 0.0));
        let clip = &lib.clips[0];
        assert_eq!(clip.name, "tiny");
        assert!((clip.duration - 0.5).abs() < 1e-6);
        let hips = &clip.translations[&0];
        assert_eq!(hips.values[0], Vec3::new(1.0, 10.0, 2.0));
        assert_eq!(hips.times, vec![0.0, 0.5]);
        // Zrotation Yrotation Xrotation: Ry(90) on the hips at time 0.
        let q = clip.rotations[&0].values[0];
        assert!(q.angle_between(Quat::from_rotation_y(90f32.to_radians())) < 1e-5);
        let leg = clip.rotations[&1].values[0];
        assert!(leg.angle_between(Quat::from_rotation_x(30f32.to_radians())) < 1e-5);
    }

    #[test]
    fn rejects_truncated_motion() {
        let cut = &TINY[..TINY.len() - 20];
        assert!(parse("cmu", "cut", cut).is_err());
    }

    #[test]
    fn every_shipped_take_parses_with_a_t_pose_bind() {
        let dir = Path::new(crate::ASSETS).join("anim/cmu");
        let libs = load_dir("cmu", &dir);
        assert!(libs.len() >= 60, "{} takes", libs.len());
        for lib in &libs {
            let arm = |a: &str, b: &str| {
                let (a, b) = (lib.node(a).unwrap(), lib.node(b).unwrap());
                (lib.rest_world(b).translation - lib.rest_world(a).translation).normalize()
            };
            let l = arm("LeftArm", "LeftForeArm");
            assert!(l.x > 0.95, "{}: left arm at bind {l}", lib.clips[0].name);
            assert!(arm("RightArm", "RightForeArm").x < -0.95);
            assert!(arm("LeftUpLeg", "LeftLeg").y < -0.95);
        }
    }
}

#[cfg(test)]
mod retarget_checks {
    use super::*;
    use crate::motion::{Bones, donor, retarget};

    fn canonical() -> (crate::motion::Skeleton, Bones) {
        let path = Path::new(crate::ASSETS).join(crate::motion::MODEL_GLB);
        let lib = donor::load("ual2", &path).unwrap();
        let s = donor::skeleton(&lib);
        let b = Bones::new(&s).unwrap();
        (s, b)
    }

    /// Retargeted takes keep a standing body's proportions: feet under the pelvis by about a leg,
    /// hands within reach of the shoulders, nothing sunk through the floor. And a front kick
    /// kicks forward (+Z), so the facing survived.
    #[test]
    fn retargeted_takes_keep_the_body_together() {
        let (skeleton, bones) = canonical();
        let rest = skeleton.fk(&skeleton.rest_pose(), bones.pelvis);
        let leg = rest[bones.pelvis].translation.y - rest[bones.foot[0]].translation.y;
        let arm = rest[bones.upperarm[0]].translation.distance(rest[bones.lowerarm[0]].translation)
            + rest[bones.lowerarm[0]].translation.distance(rest[bones.hand[0]].translation);
        let dir = Path::new(crate::ASSETS).join(crate::motion::CMU_DIR);
        let names = ["135_04_front_kick", "13_17_boxing", "144_20_punch_sequence", "78_28_defensive_zigzag"];
        for name in names {
            let text = std::fs::read_to_string(dir.join(format!("{name}.bvh"))).unwrap();
            let lib = parse("cmu", name, &text).unwrap();
            let clips = retarget::retarget(&lib, &skeleton, &bones);
            let clip = &clips[0];
            let mut kick_forward = f32::MIN;
            for (i, frame) in clip.frames.iter().enumerate().step_by(5) {
                let w = skeleton.fk(frame, bones.pelvis);
                let pelvis = w[bones.pelvis].translation;
                for side in 0..2 {
                    let shoulder = w[bones.upperarm[side]].translation;
                    let hand = w[bones.hand[side]].translation;
                    assert!(hand.distance(shoulder) <= arm * 1.05, "{name} f{i}: hand {} from the shoulder", hand.distance(shoulder));
                    let foot = w[bones.foot[side]].translation;
                    assert!(pelvis.y - foot.y < leg * 1.08, "{name} f{i}: leg stretched {}", pelvis.y - foot.y);
                    kick_forward = kick_forward.max(foot.z - pelvis.z);
                }
                assert!(w[bones.head].translation.y > pelvis.y, "{name} f{i}: head below the pelvis");
            }
            if name.contains("kick") {
                assert!(kick_forward > 0.45, "{name}: the kick never goes forward ({kick_forward})");
            }
            eprintln!("{name}: {} frames, {:.1}s, travel {:?}", clip.frames.len(), clip.duration, clip.travel.last());
        }
    }
}
