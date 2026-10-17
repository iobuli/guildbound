//! Impact-only active ragdoll. On request (`RagdollRequest` component from combat): spawn
//! dynamic capsule segments for pelvis, spine_02, Head, upper/lower arms, thighs, calves with
//! spherical joints from the current composed pose, carry the hit velocity, set
//! `Mover.mode = Ragdoll`, feed the bodies' rotations back as a full `Source::Pose` layer with
//! `MotionLayers.raw = true`, PD-drive toward the animated pose with gains rising over time,
//! then blend back (get-up track chosen by facing) and despawn segments.
use bevy::prelude::*;

/// Hand a character's body to physics: inserted by `combat` (knockback above
/// `Feel.ragdoll_threshold`) or a debug key. `ragdoll` consumes it (removes it when it takes
/// over); from then until the get-up ends it owns the body, `Mover.mode` (`Ragdoll`) and
/// `MotionLayers.raw`, and the get-up `Action` (`Owner::Ragdoll`, `ActionKind::GetUp`).
#[derive(Component, Clone, Copy, Debug)]
pub struct RagdollRequest {
    /// World velocity (m/s) the body leaves with.
    pub velocity: Vec3,
    /// World point the blow landed on (the segment nearest it gets the extra push).
    pub point: Vec3,
}

pub fn plugin(app: &mut App) {
    let _ = app;
}
