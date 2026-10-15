//! `PoseBank`: every donor track, stylized sampling, masks and the `poses.json` override file.
//!
//! `assets/anim/poses.json` (optional, hot-reloaded every ~0.5 s by mtime):
//! ```json
//! {
//!   "style": { "exaggeration": 1.15, "hold": 0.45, "bias": 0.55, "stepped_fps": null, "speed": 1.0 },
//!   "edits": { "ual1/Punch_Jab": { "2": { "hand_r": [x, y, z, w] } } },
//!   "authored": {
//!     "authored/bow_draw": {
//!       "duration": 0.6, "looping": false,
//!       "keys": [ { "time": 0.0, "tag": "start", "base": "ual1/Pistol_Aim_Neutral#0",
//!                   "bones": { "hand_l": [x, y, z, w] } } ]
//!     }
//!   }
//! }
//! ```
//! `edits` replace LOCAL rotations of a key (track → key index → bone name → quat).
//! `authored` tracks are built from a base key (`"{track}#{key}"` or `"rest"`) plus bone
//! overrides, and are sampled exactly like donor tracks.
use super::extract::Track;
use super::timing::Style;
use super::{Bones, Mask, Pose, Skeleton};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Resource)]
pub struct PoseBank {
    pub skeleton: Skeleton,
    pub bones: Bones,
    pub rest: Pose,
    pub tracks: HashMap<String, Track>,
    /// Default style (from poses.json `style`, else `Style::default()`).
    pub style: Style,
    /// Per-mask per-bone weights (len = skeleton.len()).
    pub masks: HashMap<Mask, Vec<f32>>,
}

impl PoseBank {
    /// Load all `super::LIBRARIES`, retarget, extract, apply poses.json. Logs a summary line
    /// `[bank] N tracks (ual1 a, ual2 b, sekiro c), K keys` on success.
    pub fn build() -> Result<Self, String> {
        todo!("bank::build")
    }
    pub fn has(&self, name: &str) -> bool {
        self.tracks.contains_key(name)
    }
    pub fn duration(&self, name: &str) -> f32 {
        self.tracks.get(name).map_or(0.0, |t| t.duration)
    }
    /// Stylized sample: find the surrounding keys, ease between them with `style`, then
    /// exaggerate from rest. Unknown names return rest (and warn once).
    pub fn sample(&self, name: &str, time: f32, looping: bool, style: &Style) -> Pose {
        todo!("bank::sample {name} {time} {looping} {style:?}")
    }
    /// Dense, un-stylized sample (for comparisons and root travel).
    pub fn sample_raw(&self, name: &str, time: f32, looping: bool) -> Pose {
        todo!("bank::sample_raw {name} {time} {looping}")
    }
    /// Model-space horizontal root travel between two clip times.
    pub fn travel(&self, name: &str, t0: f32, t1: f32) -> Vec3 {
        todo!("bank::travel {name} {t0} {t1}")
    }
    pub fn mask(&self, mask: Mask) -> &[f32] {
        &self.masks[&mask]
    }
    /// Write current `edits`/`authored`/`style` back to poses.json (used by `tweak`).
    pub fn save_overrides(&self, overrides: &Overrides) -> Result<(), String> {
        todo!("bank::save_overrides {}", overrides.edits.len())
    }
}

/// Parsed poses.json.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Overrides {
    #[serde(default)]
    pub style: Option<Style>,
    #[serde(default)]
    pub edits: HashMap<String, HashMap<String, HashMap<String, [f32; 4]>>>,
    #[serde(default)]
    pub authored: HashMap<String, AuthoredTrack>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AuthoredTrack {
    pub duration: f32,
    #[serde(default)]
    pub looping: bool,
    pub keys: Vec<AuthoredKey>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AuthoredKey {
    pub time: f32,
    pub tag: super::extract::Tag,
    pub base: String,
    #[serde(default)]
    pub bones: HashMap<String, [f32; 4]>,
}

pub fn plugin(app: &mut App) {
    let _ = app;
}
