//! Closed-form critically damped motion, stable through long or paused frames.

/// Current hierarchy pose, before Bevy's end-of-frame global propagation.
/// Using last frame's globals to solve root locks or IK causes visible lag.
pub fn world_transform(world: &bevy::prelude::World, entity: bevy::prelude::Entity) -> Option<bevy::prelude::GlobalTransform> {
    fn resolve(world: &bevy::prelude::World, entity: bevy::prelude::Entity, depth: u8) -> Option<bevy::prelude::GlobalTransform> {
        use bevy::prelude::*;
        if depth > 64 { return None; }
        let local = world.get::<Transform>(entity)?;
        match world.get::<ChildOf>(entity) {
            Some(parent) => Some(resolve(world, parent.parent(), depth + 1)?.mul_transform(*local)),
            None => Some(GlobalTransform::from(*local)),
        }
    }
    resolve(world, entity, 0)
}

pub fn spring(position: f32, velocity: f32, target: f32, frequency: f32, dt: f32) -> (f32, f32) {
    if dt <= 0.0 { return (position, velocity); }
    let dt = dt.max(0.0);
    let omega = frequency.max(0.0);
    let offset = position - target;
    let coupled = velocity + omega * offset;
    let decay = (-omega * dt).exp();
    (target + (offset + coupled * dt) * decay, (velocity - omega * coupled * dt) * decay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_response_matches_across_frame_rates_and_preserves_pause() {
        let reference = spring(0.0, 0.0, 1.0, 12.0, 1.0);
        for frames in [30, 60, 120] {
            let (mut p, mut v) = (0.0, 0.0);
            for _ in 0..frames {
                (p, v) = spring(p, v, 1.0, 12.0, 1.0 / frames as f32);
                assert!((0.0..=1.0).contains(&p));
            }
            assert!((p - reference.0).abs() < 1e-5);
            assert!((v - reference.1).abs() < 1e-5);
            assert_eq!(spring(p, v, 1.0, 12.0, 0.0), (p, v));
        }
        let settled = spring(-10.0, 100.0, 1.0, 18.0, 10.0);
        assert_eq!(settled, (1.0, 0.0));
        assert_eq!(spring(0.123456, 0.345, 12345.0, 12.0, 0.0), (0.123456, 0.345));
    }
}
