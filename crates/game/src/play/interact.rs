//! World interaction: nearest `Interactable` in range sets `InteractPrompt`; E walks/turns
//! the character into place, plays the interactable's track (looping or one-shot) as an
//! `ActionKind::Interact` action with hand IK targets, any movement/attack input cancels
//! looping ones. Small world reactions (chest lid opens, crop grows, tree shakes, torch toggles).
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
