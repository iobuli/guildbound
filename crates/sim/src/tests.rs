use super::*;

fn script(tick: u64) -> [Intent; 2] {
    // deterministic, varied input: walk, run, reversals, diagonals, idle
    let phase = (tick / 37) % 6;
    let dir = match phase {
        0 => V2::new(1.0, 0.0),
        1 => V2::new(-1.0, 0.0),
        2 => V2::new(0.7, 0.7),
        3 => V2::new(0.0, -1.0),
        4 => V2::ZERO,
        _ => V2::new(-0.6, 0.8),
    };
    [Intent { move_dir: dir, run: phase % 2 == 0 }, Intent { move_dir: V2::new(-dir.z, dir.x), run: false }]
}

#[test]
fn replay_is_identical() {
    let mut a = World::sparring(Tuning::default());
    let mut b = World::sparring(Tuning::default());
    for t in 0..2400 {
        a.step(&script(t));
        b.step(&script(t));
        if t % 64 == 0 {
            assert_eq!(a.state_hash(), b.state_hash(), "diverged at tick {t}");
        }
    }
    assert_eq!(a, b);
}

#[test]
fn walk_covers_walk_speed_per_second() {
    let mut w = World::sparring(Tuning::default());
    w.fighters.truncate(1);
    w.fighters[0].position = V2::ZERO;
    for _ in 0..120 {
        w.step(&[Intent { move_dir: V2::new(1.0, 0.0), run: false }]);
    }
    assert!((w.fighters[0].position.x - 2.1).abs() < 1e-4, "x = {}", w.fighters[0].position.x);
}

#[test]
fn oversized_input_is_clamped_to_unit() {
    let mut w = World::sparring(Tuning::default());
    w.fighters.truncate(1);
    w.fighters[0].position = V2::ZERO;
    w.step(&[Intent { move_dir: V2::new(5.0, 0.0), run: true }]);
    assert!((w.fighters[0].velocity.length() - 4.0).abs() < 1e-5);
}

#[test]
fn arena_holds_bodies_inside() {
    let mut w = World::sparring(Tuning::default());
    let half = w.tuning.arena_half_size - w.tuning.body_radius;
    for _ in 0..1200 {
        w.step(&[Intent { move_dir: V2::new(1.0, 1.0), run: true }, Intent { move_dir: V2::new(-1.0, -1.0), run: true }]);
        for f in &w.fighters {
            assert!(f.position.x.abs() <= half + 1e-6 && f.position.z.abs() <= half + 1e-6);
        }
    }
}

#[test]
fn bodies_never_overlap_after_a_step() {
    let mut w = World::sparring(Tuning::default());
    let min = 2.0 * w.tuning.body_radius;
    for _ in 0..600 {
        // walk straight into each other
        w.step(&[Intent { move_dir: V2::new(0.0, 1.0), run: true }, Intent { move_dir: V2::new(0.0, -1.0), run: true }]);
        let d = (w.fighters[1].position - w.fighters[0].position).length();
        assert!(d >= min - 1e-4, "overlap: {d}");
    }
}

#[test]
fn turning_is_bounded_and_reversal_does_not_flip() {
    let mut w = World::sparring(Tuning::default());
    w.fighters.truncate(1);
    w.fighters[0].facing = V2::new(1.0, 0.0);
    let max = w.tuning.turn_rate * STEP;
    let mut prev = w.fighters[0].facing;
    for _ in 0..30 {
        w.step(&[Intent { move_dir: V2::new(-1.0, 0.0), run: false }]);
        let f = w.fighters[0].facing;
        assert!(prev.signed_angle_to(f).abs().to_degrees() <= max + 1e-3);
        assert!((f.length() - 1.0).abs() < 1e-5);
        prev = f;
    }
    assert!(prev.dot(V2::new(-1.0, 0.0)) > 0.9999);
}

#[test]
fn lock_target_faces_the_target() {
    let mut w = World::sparring(Tuning::default());
    w.fighters[0].lock_target = Some(1);
    w.step(&[Intent { move_dir: V2::new(1.0, 0.0), run: false }]);
    let to = (w.fighters[1].position - w.fighters[0].position).normalized();
    assert!(w.fighters[0].facing.dot(to) > 0.9999);
}
