//! The motion selector (`Update / PlaySet::Select`): the ONLY writer of `MotionLayers.layers`.
//! Builds per character: locomotion base (idle / walk / jog / run / sprint / crouch / strafe /
//! back / turn, phase-synced by `Mover.phase` and blended by speed; air start/loop/fall/land;
//! swim; climb), the equipped moveset's `Stance` overlay, then `Action.layer` and
//! `Action.overlay`. Fills `ContactRequest` (foot IK off in air/swim/ragdoll, accel, squash from
//! landing/jump, look_at from Intent.target). Never touches `MotionLayers.impulses` / `raw`.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
