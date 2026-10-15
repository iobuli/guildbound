//! Impact-only active ragdoll. On request (`RagdollRequest` component from combat): spawn
//! dynamic capsule segments for pelvis, spine_02, Head, upper/lower arms, thighs, calves with
//! spherical joints from the current composed pose, carry the hit velocity, set
//! `Mover.mode = Ragdoll`, feed the bodies' rotations back as a full `Source::Pose` layer with
//! `MotionLayers.raw = true`, PD-drive toward the animated pose with gains rising over time,
//! then blend back (get-up track chosen by facing) and despawn segments.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
