//! Movement maths carried from gb_live/src/locomotion.rs; no live-game UI or systems.
use bevy::prelude::*;
use guildbound_sim::{STEP, Tuning};

#[derive(Clone, Default)]
pub struct Mover {
    pub velocity: Vec3,
    pub yaw: f32,
}
impl Mover {
    pub fn new(yaw: f32) -> Self {
        Self { yaw, ..default() }
    }
    fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }
}

pub fn step(
    m: &mut Mover,
    position: &mut Vec3,
    want: Option<(Vec3, f32)>,
    t: &Tuning,
    dt: f32,
) -> f32 {
    let mut travelled = 0.0;
    let mut left = dt.min(0.1);
    while left > 1e-6 {
        let h = left.min(STEP);
        left -= h;
        let mut input = Vec3::ZERO;
        let mut speed = 0.0;
        if let Some((dir, s)) = want.filter(|w| w.0.length_squared() > 1e-6) {
            let goal = dir.x.atan2(dir.z);
            let d = (goal - m.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let turn = t.turn_rate.to_radians() * h;
            m.yaw += d.clamp(-turn, turn);
            input = m.forward() * d.cos().max(0.0);
            speed = s;
        }
        if input.length_squared() > 1e-8 && speed > 0.0 {
            let push = t.drag_accel * h;
            m.velocity = (m.velocity + input * push) * (speed / (speed + push));
        } else {
            m.velocity *= 0.5f32.powf(h / t.coast_half_life.max(1e-3));
        }
        *position += m.velocity * h;
        travelled += m.velocity.length() * h;
    }
    travelled
}

/// gb_live/controller.rs: phase advances by measured travel, not frame count.
pub fn advance_phase(phase: f32, distance: f32, stride: f32) -> f32 {
    (phase + distance.max(0.0) / stride.max(0.01)).rem_euclid(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_drag_reaches_target_and_coasts_to_rest() {
        let t = Tuning::default();
        let mut m = Mover::new(0.0);
        let mut p = Vec3::ZERO;
        for _ in 0..240 {
            step(&mut m, &mut p, Some((Vec3::Z, 1.4)), &t, 1.0 / 60.0);
        }
        assert!((m.velocity.length() - 1.4).abs() < 0.02 && p.z > 4.0);
        for _ in 0..240 {
            step(&mut m, &mut p, None, &t, 1.0 / 60.0);
        }
        assert!(m.velocity.length() < 0.01);
    }
    #[test]
    fn turn_is_bounded_and_phase_is_distance_based() {
        let t = Tuning::default();
        let mut m = Mover::new(0.0);
        let mut p = Vec3::ZERO;
        step(&mut m, &mut p, Some((Vec3::NEG_Z, 2.0)), &t, 0.05);
        assert!(m.velocity.length() < 1e-3);
        assert!(m.yaw.abs() <= t.turn_rate.to_radians() * 0.05 + 1e-4);
        for fps in [30, 60, 120] {
            let mut phase = 0.2;
            for _ in 0..fps {
                phase = advance_phase(phase, 3.0 / fps as f32, 2.0);
            }
            assert!((phase - 0.7).abs() < 1e-4);
        }
    }
}
