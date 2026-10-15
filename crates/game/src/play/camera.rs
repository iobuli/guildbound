//! Third-person orbit camera (`CameraRig` resource: yaw, pitch, distance, smoothed focus).
//! Mouse/right-stick orbit, wheel zoom, follows the player's INTERPOLATED Transform, lock-on
//! framing between player and target, over-shoulder offset while aiming, small shake on hits
//! (`CameraShake` resource other modules can add trauma to). Runs in PostUpdate after
//! TransformSystems::Propagate is NOT needed: read root `Transform` directly.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
