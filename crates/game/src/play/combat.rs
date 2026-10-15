//! Melee combat FSM + the ONLY `Hit` resolver. Light chain / heavy charge / sprint and air
//! attacks from the equipped `Moveset` (kinds unarmed/melee), guard + parry, dodge i-frames,
//! hit tests (swept capsule of the hitbox bone/blade between frames vs other characters'
//! capsules), damage scaled by closing speed, poise → stagger, hitstop (`Time<Virtual>`
//! relative speed), knockback via `Mover.external`, impulses for secondary springs, death,
//! knockdown/get-up, ragdoll trigger above `Feel.ragdoll_threshold`. Weapon mesh attachment.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
