//! Ranged weapons (moveset kind `ranged`): aim (secondary held) with upper-body `Aim` overlay,
//! bow draw with procedural string-hand IK (`ContactRequest.hands`), crossbow shoot/reload,
//! throwing knives (overhand throw, fan count). `Projectile` entities (ballistic, gravity,
//! sweep against characters + world, stick into world for arrows/bolts/knives), emitting `Hit`
//! with `HitKind::Projectile`. Aim ray from the camera; trajectory preview gizmo.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
