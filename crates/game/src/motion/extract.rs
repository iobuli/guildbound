//! Extreme extraction: reduce a dense `Clip` to the few poses that carry the action.
//! A frame is a candidate extreme when the weighted angular speed of the major bones is a local
//! minimum (motion reverses / holds), or at a foot/hand contact flip (lowest foot switches,
//! fastest hand peaks then stops). First and last frames are always keys. Candidates closer
//! than ~0.08 s merge. Loops get at least 4 keys (both foot contacts + both passing poses).
use super::retarget::Clip;
use super::{Bones, Pose, Skeleton};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    Start,
    Anticipation,
    /// Foot plant (locomotion) or strike contact.
    Contact,
    Passing,
    Apex,
    /// Fastest limb peak right before stop (attacks).
    Impact,
    Recoil,
    Hold,
    End,
}

#[derive(Clone, Debug)]
pub struct Key {
    /// Seconds in the source clip.
    pub time: f32,
    pub pose: Pose,
    pub tag: Tag,
}

/// The reduced, stylizable representation of a clip.
#[derive(Clone, Debug)]
pub struct Track {
    pub name: String,
    pub duration: f32,
    pub looping: bool,
    /// Sorted by time; never empty.
    pub keys: Vec<Key>,
    /// For locomotion loops: normalized phase (0..1) where the LEFT foot plants.
    pub left_contact: Option<f32>,
    /// The dense clip, kept for raw sampling, root travel and debugging.
    pub dense: Clip,
}

pub fn extract(clip: &Clip, skeleton: &Skeleton, bones: &Bones) -> Track {
    todo!("extract::extract {} {} {}", clip.track, skeleton.len(), bones.pelvis)
}
