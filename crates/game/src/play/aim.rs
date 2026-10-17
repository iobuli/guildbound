//! Aiming helpers shared by `ranged` (arrows, bolts) and `magic` (spells, arts).
//! - `ballistic`: the launch direction that lands a projectile of `speed` under `gravity` on a
//!   point (the low arc; out of range → the 45° throw that gets closest).
//! - `lead`: aim at where a moving target will be when the shot arrives (a few fixed-point
//!   iterations of flight time, capped at `MAX_LEAD` s), arc included.
//! - `Velocities`: every mover's velocity this frame (written here only, `PlaySet::Think`), so
//!   shooters can lead a target without borrowing its `Mover`.
use super::{Mover, PlaySet};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

/// Longest flight time (s) a lead looks ahead.
const MAX_LEAD: f32 = 1.5;

#[derive(Resource, Default)]
pub struct Velocities(pub HashMap<Entity, Vec3>);

impl Velocities {
    pub fn of(&self, e: Entity) -> Vec3 {
        self.0.get(&e).copied().unwrap_or(Vec3::ZERO)
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Velocities>().add_systems(Update, record.in_set(PlaySet::Think));
}

fn record(mut v: ResMut<Velocities>, movers: Query<(Entity, &Mover)>) {
    v.0.clear();
    for (e, m) in &movers {
        if m.velocity.is_finite() {
            v.0.insert(e, m.velocity);
        }
    }
}

/// Unit launch direction from `from` that lands on `to` at `speed` with `gravity` (m/s², down).
pub fn ballistic(from: Vec3, to: Vec3, speed: f32, gravity: f32) -> Vec3 {
    let d = to - from;
    let flat = Vec2::new(d.x, d.z);
    let h = flat.length();
    if gravity <= 1e-4 || h < 1e-3 || speed <= 1e-3 {
        return d.normalize_or(Vec3::Z);
    }
    let (v2, y) = (speed * speed, d.y);
    let disc = v2 * v2 - gravity * (gravity * h * h + 2.0 * y * v2);
    let angle = if disc >= 0.0 {
        ((v2 - disc.sqrt()) / (gravity * h)).atan()
    } else {
        std::f32::consts::FRAC_PI_4
    };
    let across = flat / h;
    Vec3::new(across.x * angle.cos(), angle.sin(), across.y * angle.cos())
}

/// Direction (and the predicted aim point) to hit a target at `target` moving at `velocity`.
pub fn lead(from: Vec3, target: Vec3, velocity: Vec3, speed: f32, gravity: f32) -> (Vec3, Vec3) {
    let vel = if velocity.is_finite() { velocity } else { Vec3::ZERO };
    let mut point = target;
    let mut dir = ballistic(from, point, speed, gravity);
    for _ in 0..4 {
        let flat = Vec2::new(point.x - from.x, point.z - from.z).length();
        let along = (speed * Vec2::new(dir.x, dir.z).length()).max(1e-3);
        let t = if flat > 1e-3 { flat / along } else { (point - from).length() / speed.max(1e-3) };
        point = target + vel * t.min(MAX_LEAD);
        dir = ballistic(from, point, speed, gravity);
    }
    (dir, point)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fly a shot by simple integration; return how close it passes `to`.
    fn miss(from: Vec3, dir: Vec3, speed: f32, gravity: f32, to: impl Fn(f32) -> Vec3) -> f32 {
        let (mut p, mut v, mut best) = (from, dir * speed, f32::MAX);
        let dt = 1.0 / 600.0;
        for i in 0..3000 {
            let t = i as f32 * dt;
            best = best.min(p.distance(to(t)));
            v.y -= gravity * dt;
            p += v * dt;
        }
        best
    }

    #[test]
    fn ballistic_lands_on_the_point() {
        let (from, to) = (Vec3::new(0.0, 1.4, 0.0), Vec3::new(30.0, 0.5, 12.0));
        let dir = ballistic(from, to, 30.0, 9.8);
        assert!(dir.y > 0.0, "arcs up");
        assert!(miss(from, dir, 30.0, 9.8, |_| to) < 0.1);
        // No gravity: straight at it.
        assert!(ballistic(from, to, 30.0, 0.0).dot((to - from).normalize()) > 0.9999);
    }

    #[test]
    fn lead_hits_a_runner() {
        let from = Vec3::new(0.0, 1.4, 0.0);
        let (start, vel) = (Vec3::new(20.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 5.0));
        let (dir, _) = lead(from, start, vel, 25.0, 6.0);
        assert!(miss(from, dir, 25.0, 6.0, |t| start + vel * t) < 0.25);
        // Aiming straight at it misses by metres.
        let naive = ballistic(from, start, 25.0, 6.0);
        assert!(miss(from, naive, 25.0, 6.0, |t| start + vel * t) > 1.5);
    }
}
