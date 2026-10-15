//! The single bone writer. For every character with a `Rig`:
//! 1. evaluate `MotionLayers` bottom→top with `PoseBank::sample` + masks,
//! 2. `Springs::step` (secondary motion, impulses, squash),
//! 3. `contact::apply` (feet/hands/lean/look) using Avian ray casts against `Phys::World`,
//! 4. write local rotations (+ pelvis translation) into the bone entities' `Transform`s.
//! Runs in `PostUpdate` / `MotionSet::Compose` (after Bevy animation, before propagation).
use bevy::prelude::*;

/// Global motion feel; edited by `play::tuning`.
#[derive(Resource, Clone, Debug)]
pub struct MotionFeel {
    pub style: super::timing::Style,
    pub springs: super::secondary::SpringParams,
    pub contact: super::contact::ContactParams,
    /// Debug: draw skeleton gizmos.
    pub debug: bool,
}

impl Default for MotionFeel {
    fn default() -> Self {
        Self {
            style: default(),
            springs: default(),
            contact: default(),
            debug: false,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<MotionFeel>();
}
