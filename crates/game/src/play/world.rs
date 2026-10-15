//! World setup: camera entity, sun + ambient, a ~120x120 m heightfield terrain (hills from
//! `guildbound_sim::terrain::ground`, flat training courtyard at the centre), a pond with a
//! `Water` volume, climbable walls/cliffs/ledges (`Climbable`), stairs and boxes for mantling,
//! interactables (`Interactable`: chest, crop plot, tree, well, table, chair, anvil, lever,
//! torch), training dummies / an opponent, and the player via `spawn_character`.
//! Exposes `pub fn ground_height(x, z) -> f32` for the analytic terrain.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
