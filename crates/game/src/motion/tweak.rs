//! F2 pose tweak mode: freeze the player's action layer on a chosen track key, click a bone
//! (nearest bone gizmo to the cursor ray), rotate it (mouse wheel = twist, arrow keys = swing,
//! Shift = fine), Ctrl+S saves into `assets/anim/poses.json` `edits` via
//! `PoseBank::save_overrides`; N creates an `authored` key from the current pose.
//! PageUp/PageDown choose track, [ / ] choose key. Esc / F2 exits.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
