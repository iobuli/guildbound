//! Model spawning and bone binding.
//! When a `ModelRequest` is added to a character root: spawn the glTF scene
//! (`GltfAssetLabel::Scene(0)`, `WorldAssetRoot`) as a child with `request.offset`; on
//! `WorldInstanceReady` map descendant `Name`s to canonical skeleton indices, mark them
//! `RigBone`, REMOVE every `AnimationPlayer`/`AnimationGraphHandle` in the scene (clips are data
//! only), then insert `Rig`, `Springs`, `ContactState` and `MotionLayers` on the root.
//! Pattern reference: the old `ranger.rs` `spawn`/`ready`/`prepare` (see git history).
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
