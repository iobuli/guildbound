//! One presentation vocabulary for resolved combat outcomes. Profiles feed
//! both the camera and contact marks; they never alter the simulation.
use bevy::prelude::*;
use guildbound_sim::Outcome;

#[derive(Resource)]
pub struct FeedbackSettings {
    pub shake_scale: f32,
}

impl Default for FeedbackSettings {
    fn default() -> Self { Self { shake_scale: 0.65 } }
}

pub struct ImpactProfile {
    pub color: Color,
    pub shake_m: f32,
    pub radius_m: f32,
    pub duration_s: f32,
    pub label: &'static str,
}

pub fn profile(outcome: Outcome) -> ImpactProfile {
    let (color, shake_m, radius_m, duration_s, label) = match outcome {
        Outcome::Hit => (Color::srgb(1.0, 0.91, 0.72), 0.045, 0.24, 0.20, "HIT"),
        Outcome::Blocked => (Color::srgb(0.45, 0.76, 1.0), 0.022, 0.18, 0.18, "BLOCKED"),
        Outcome::CounterHit => (Color::srgb(1.0, 0.77, 0.28), 0.085, 0.34, 0.28, "COUNTER HIT"),
        Outcome::GuardBroken => (Color::srgb(1.0, 0.40, 0.22), 0.10, 0.40, 0.32, "GUARD BREAK"),
        Outcome::Parried => (Color::srgb(0.50, 1.0, 0.78), 0.075, 0.32, 0.26, "PARRY"),
    };
    ImpactProfile { color, shake_m, radius_m, duration_s, label }
}
