//! Movement any presentation character can call (NPCs today): the hero's motor and locomotion in one place.
//! `step` moves a body with the sim's live tuning (the Overgrowth-style drag motor: push toward the wanted
//! speed, coast to a stop by half-life; turning at the tuned rate, slowing while it turns hard). `animate`
//! plays the hero library's relaxed blend space on any rig with the hero's bone names: walk, run and sprint
//! weighted by ground speed, every clip at one gait phase advanced by the distance actually travelled (feet
//! match the ground), fading into the relaxed idle below 0.6 m/s. Tune it live in the dev panel.
use bevy::prelude::*;
use guildbound_sim::{STEP, Tuning};

use crate::anim::{AnimLibrary, AnimSettings};
use crate::controller::LocomotionMap;

#[derive(Clone, Default)]
pub struct Mover {
    pub velocity: Vec3,
    pub yaw: f32,
    pub gait_phase: f32,
    loco: Vec<AnimationNodeIndex>,
}

impl Mover {
    pub fn new(yaw: f32) -> Self {
        Self { yaw, ..default() }
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }
}

/// Move toward `want` = (direction, speed) or coast to a stop on None; returns the distance travelled.
pub fn step(m: &mut Mover, position: &mut Vec3, want: Option<(Vec3, f32)>, t: &Tuning, dt: f32) -> f32 {
    let mut travelled = 0.0;
    let mut left = dt.min(0.1);
    while left > 1e-6 {
        let h = left.min(STEP);
        left -= h;
        let mut input = Vec3::ZERO;
        let mut speed = 0.0;
        if let Some((dir, s)) = want.filter(|w| w.0.length_squared() > 1e-6) {
            let goal = dir.x.atan2(dir.z);
            let d = (goal - m.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            let turn = t.turn_rate.to_radians() * h;
            m.yaw += d.clamp(-turn, turn);
            // push along the facing, less while still turning hard (a body plants and turns before it runs)
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

/// Play the relaxed blend space for a mover's speed on its player (which must use the hero library graph).
pub fn animate(player: &mut AnimationPlayer, lib: &AnimLibrary, map: &LocomotionMap, settings: &AnimSettings, t: &Tuning, m: &mut Mover, travelled: f32, idle: &str) {
    let speed = m.velocity.length();
    let gait = if speed <= t.run_speed {
        ((speed - t.walk_speed) / (t.run_speed - t.walk_speed).max(0.1)).clamp(0.0, 1.0)
    } else {
        1.0 + ((speed - t.run_speed) / (t.sprint_speed - t.run_speed).max(0.1)).clamp(0.0, 1.0)
    };
    let k = (speed / 0.6).clamp(0.0, 1.0);
    let idle_w = 1.0 - k * k * (3.0 - 2.0 * k);
    let mut set: Vec<(String, f32, bool)> = Vec::new();
    if idle_w < 0.999 {
        for (clip, w) in map.blend(lib, 0.0, gait, true) {
            set.push((clip.to_owned(), w * (1.0 - idle_w), true));
        }
    }
    if idle_w > 0.001 || set.is_empty() {
        set.push((idle.to_owned(), if set.is_empty() { 1.0 } else { idle_w }, false));
    }
    let nodes: Vec<(AnimationNodeIndex, String, f32, bool)> = set.into_iter().filter_map(|(c, w, g)| lib.node(&c).map(|n| (n, c, w, g))).collect();
    let synced: f32 = nodes.iter().filter(|x| x.3).map(|x| x.2).sum();
    if synced > 1e-4 {
        let stride: f32 = nodes.iter().filter(|x| x.3).map(|x| {
            let reference = if x.1.contains("sprint") { t.sprint_speed } else if x.1.contains("run") { settings.run_reference_speed } else { settings.reference_speed };
            lib.stride(&x.1, reference) * x.2
        }).sum::<f32>() / synced;
        m.gait_phase = crate::controller::advance_phase(m.gait_phase, travelled, stride);
    }
    for &old in &m.loco {
        if !nodes.iter().any(|x| x.0 == old) {
            player.stop(old);
        }
    }
    for (node, clip, w, gait_synced) in &nodes {
        let active = player.play(*node).repeat();
        active.set_weight(*w);
        if *gait_synced {
            let phase = (m.gait_phase + lib.left_strike(clip)).rem_euclid(1.0);
            active.set_speed(0.0).set_seek_time(phase * lib.duration(clip));
        } else {
            active.set_speed(1.0);
        }
    }
    m.loco = nodes.iter().map(|x| x.0).collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drag_motor_settles_at_the_wanted_speed_and_coasts_to_a_stop() {
        let t = Tuning::default();
        let mut m = Mover::new(0.0);
        let mut p = Vec3::ZERO;
        for _ in 0..240 {
            step(&mut m, &mut p, Some((Vec3::Z, 1.4)), &t, 1.0 / 60.0);
        }
        assert!((m.velocity.length() - 1.4).abs() < 0.02, "{}", m.velocity.length());
        assert!(p.z > 4.0 && p.x.abs() < 1e-3);
        for _ in 0..240 {
            step(&mut m, &mut p, None, &t, 1.0 / 60.0);
        }
        assert!(m.velocity.length() < 0.01);
    }

    #[test]
    fn a_mover_turns_at_the_tuned_rate_before_it_runs_off() {
        let t = Tuning::default();
        let mut m = Mover::new(0.0);
        let mut p = Vec3::ZERO;
        step(&mut m, &mut p, Some((Vec3::NEG_Z, 2.0)), &t, 0.05);
        // facing straight away from the goal: it turns but does not push yet
        assert!(m.velocity.length() < 1e-3);
        assert!(m.yaw.abs() > 0.0 && m.yaw.abs() <= t.turn_rate.to_radians() * 0.05 + 1e-4);
    }
}
