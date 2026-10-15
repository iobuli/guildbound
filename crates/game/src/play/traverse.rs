//! Traversal: water (enter `Water` → `MoveMode::Swim`, buoyancy, swim speed, surface/dive),
//! climbing (`Climbable` faces: wall-climb with procedural hand/foot IK on a hold grid, ledge
//! hang + shimmy, climb-up / mantle onto the top using `ual2/ClimbUp_1m`), and auto-mantle
//! of waist-high obstacles. Owns movement while in Swim/Climb/Ledge modes.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
