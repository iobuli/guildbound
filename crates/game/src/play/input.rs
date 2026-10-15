//! Player input → `Intent` (keyboard + mouse + gamepad). Move direction is relative to the
//! camera yaw (`camera::CameraRig`). Controls: WASD move, Alt walk, Shift sprint, C crouch,
//! Space jump/climb, Ctrl dodge, LMB primary (tap/hold), RMB secondary (block/aim), E interact,
//! 1-9 loadout slot, Tab lock-on nearest enemy, R reset player.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
