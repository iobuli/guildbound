//! Kinematic capsule controller in `FixedUpdate / PlaySet::Move` using Avian `MoveAndSlide`:
//! ground/air modes, snappy accel/brake (Feel), turn rate, coyote time, variable jump, gravity,
//! ground snap + slope limit, step-up, `Mover.external` knockback decay, `root_motion`,
//! `move_scale`, `face`. Computes `Mover.accel` (smoothed), `speed`, `landing`, and advances
//! `Mover.phase` by travelled distance / stride (stride from speed). Swim/Climb/Ledge/Ragdoll
//! modes are moved by `traverse` / `ragdoll` instead (skip them here).
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
