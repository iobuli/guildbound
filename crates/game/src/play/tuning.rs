//! Live tuning page (F1 toggles): edits `Feel` and `motion::compose::MotionFeel` (≤12 knobs:
//! run speed, accel, turn rate, jump, exaggeration, hold, stepped fps on/off, spring hz,
//! spring damping, lean, hitstop, ragdoll threshold). F5 saves `profiles/feel.json`, loaded
//! at startup if present.
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    let _ = app;
}
