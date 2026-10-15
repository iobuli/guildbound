//! Magic (moveset kind `magic`): schools/elements as movesets (fire, frost, storm, arcane...).
//! Cast (tap: small projectile), charged big projectile (hold), channel beam (held, ticks Hit
//! with `HitKind::Spell`), magic idle/walk stance, per-element VFX (emissive spheres, trails,
//! point lights) and timing/style variations. Reuses `ranged::spawn_projectile` for bolts.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
