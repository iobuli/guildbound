//! The pose data PoseNet learns from and the numbers it trades in.
//!
//! The game samples every clip of the library on a plain mannequin (no procedural layers) and writes, per frame,
//! each body bone's position and rotation in the rig's own space plus its local (parent-relative) rotation:
//! exactly what Bevy's Transforms hold, so nothing is lost converting between tools.
//!
//! Features (16): head, hands and feet relative to the pelvis in the character's facing frame (hips give the
//! facing), and the pelvis height. Outputs (132): the pelvis rotation in that frame and every other bone's local
//! rotation, each in 6D form.
use crate::math::*;

/// Body bones, in order; the network owns these.
pub const BONES: [&str; 22] = [
    "B_Pelvis", "B_Spine", "B_Spine1", "B_Spine2", "B_Neck", "B_Head",
    "B_L_Clavicle", "B_L_UpperArm", "B_L_Forearm", "B_L_Forearm.001",
    "B_R_Clavicle", "B_R_UpperArm", "B_R_Forearm", "B_R_Hand",
    "B_L_Thigh", "B_L_Calf", "B_L_Foot", "B_L_Toe0",
    "B_R_Thigh", "B_R_Calf", "B_R_Foot", "B_R_Toe0",
];
/// The joints a person moves on purpose (the balls dragged in Possession): head, hands, feet.
pub const EFFECTORS: [usize; 5] = [5, 9, 13, 16, 20];
const PELVIS: usize = 0;
const L_THIGH: usize = 14;
const R_THIGH: usize = 18;
pub const FEATURES: usize = EFFECTORS.len() * 3 + 1;
pub const OUTPUTS: usize = BONES.len() * 6;

#[derive(Clone, Copy, Default)]
pub struct BonePose {
    /// In the rig's space (relative to the model's root).
    pub pos: V3,
    pub rot: Q,
    /// Relative to the parent bone.
    pub local: Q,
}

pub struct Frame {
    pub clip: u32,
    pub bones: Vec<BonePose>,
}

pub struct Dataset {
    pub clips: Vec<String>,
    pub frames: Vec<Frame>,
}

/// The facing yaw (radians) from the hips: left thigh minus right thigh, crossed with up.
pub fn facing_yaw(bones: &[BonePose]) -> f32 {
    let side = sub(bones[L_THIGH].pos, bones[R_THIGH].pos);
    let fwd = cross([side[0], 0.0, side[2]], [0.0, 1.0, 0.0]);
    fwd[0].atan2(fwd[2])
}

/// Network inputs from effector positions (rig space), the pelvis position and the facing yaw.
pub fn features(effectors: &[V3; 5], pelvis: V3, yaw: f32) -> Vec<f32> {
    let inv = qyaw(-yaw);
    let mut out = Vec::with_capacity(FEATURES);
    for e in effectors {
        out.extend(qrot(inv, sub(*e, pelvis)));
    }
    out.push(pelvis[1]);
    out
}

pub fn frame_features(bones: &[BonePose]) -> Vec<f32> {
    let yaw = facing_yaw(bones);
    let eff: [V3; 5] = std::array::from_fn(|i| bones[EFFECTORS[i]].pos);
    features(&eff, bones[PELVIS].pos, yaw)
}

pub fn frame_outputs(bones: &[BonePose]) -> Vec<f32> {
    let inv = qyaw(-facing_yaw(bones));
    let mut out = Vec::with_capacity(OUTPUTS);
    for (i, b) in bones.iter().enumerate() {
        out.extend(to6(if i == PELVIS { qmul(inv, b.rot) } else { b.local }));
    }
    out
}

/// Network outputs -> (pelvis rotation in rig space, every bone's local rotation; the pelvis entry unused).
pub fn decode(out: &[f32], yaw: f32) -> (Q, Vec<Q>) {
    let rots: Vec<Q> = out.chunks(6).map(from6).collect();
    (qmul(qyaw(yaw), rots[PELVIS]), rots)
}

impl Dataset {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = b"GBPOSE1\0".to_vec();
        let u = |b: &mut Vec<u8>, x: u32| b.extend_from_slice(&x.to_le_bytes());
        u(&mut b, BONES.len() as u32);
        u(&mut b, self.clips.len() as u32);
        for c in &self.clips {
            u(&mut b, c.len() as u32);
            b.extend_from_slice(c.as_bytes());
        }
        u(&mut b, self.frames.len() as u32);
        for f in &self.frames {
            u(&mut b, f.clip);
            for p in &f.bones {
                for x in p.pos.iter().chain(&p.rot).chain(&p.local) {
                    b.extend_from_slice(&x.to_le_bytes());
                }
            }
        }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.get(..8)? != b"GBPOSE1\0" {
            return None;
        }
        let mut at = 8;
        let u = |at: &mut usize| -> Option<u32> { let v = u32::from_le_bytes(b.get(*at..*at + 4)?.try_into().ok()?); *at += 4; Some(v) };
        let bones = u(&mut at)? as usize;
        if bones != BONES.len() {
            return None;
        }
        let n_clips = u(&mut at)?;
        let mut clips = Vec::new();
        for _ in 0..n_clips {
            let len = u(&mut at)? as usize;
            clips.push(String::from_utf8(b.get(at..at + len)?.to_vec()).ok()?);
            at += len;
        }
        let n = u(&mut at)?;
        let mut frames = Vec::with_capacity(n as usize);
        let float = |at: &mut usize| -> Option<f32> { let v = f32::from_le_bytes(b.get(*at..*at + 4)?.try_into().ok()?); *at += 4; Some(v) };
        for _ in 0..n {
            let clip = u(&mut at)?;
            let mut list = Vec::with_capacity(bones);
            for _ in 0..bones {
                let mut v = [0f32; 11];
                for x in &mut v {
                    *x = float(&mut at)?;
                }
                list.push(BonePose { pos: [v[0], v[1], v[2]], rot: [v[3], v[4], v[5], v[6]], local: [v[7], v[8], v[9], v[10]] });
            }
            frames.push(Frame { clip, bones: list });
        }
        Some(Self { clips, frames })
    }
}
