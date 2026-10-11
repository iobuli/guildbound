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
    [Intent { move_dir: dir, run: phase % 2 == 0, ..Default::default() }, Intent { move_dir: V2::new(-dir.z, dir.x), run: false, ..Default::default() }]
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
    w.fighters[0].position = V2::new(-5.0, 0.0);
    let walk = [Intent { move_dir: V2::new(1.0, 0.0), run: false, ..Default::default() }];
    for _ in 0..60 {
        w.step(&walk); // accelerate to cruise first (momentum)
    }
    let x0 = w.fighters[0].position.x;
    for _ in 0..120 {
        w.step(&walk);
    }
    assert!((w.fighters[0].position.x - x0 - 2.1).abs() < 1e-3, "dx = {}", w.fighters[0].position.x - x0);
}

#[test]
fn oversized_input_is_clamped_to_unit() {
    let mut w = World::sparring(Tuning::default());
    w.fighters.truncate(1);
    w.fighters[0].position = V2::new(-5.0, 0.0);
    for _ in 0..240 {
        w.step(&[Intent { move_dir: V2::new(5.0, 0.0), run: true, ..Default::default() }]);
    }
    assert!((w.fighters[0].velocity.length() - 4.0).abs() < 1e-4);
}

#[test]
fn linear_speed_ramps_up_and_momentum_carries_into_a_stop() {
    let mut w = World::sparring(Tuning { model: MotorModel::Linear, ..Tuning::default() });
    w.fighters.truncate(1);
    w.fighters[0].position = V2::new(-5.0, 0.0);
    w.step(&[Intent { move_dir: V2::new(1.0, 0.0), run: true, ..Default::default() }]);
    assert!((w.fighters[0].velocity.x - 12.0 * STEP).abs() < 1e-5, "first tick only gains accel * dt");
    for _ in 0..60 {
        w.step(&[Intent { move_dir: V2::new(1.0, 0.0), run: true, ..Default::default() }]);
    }
    let x0 = w.fighters[0].position.x;
    let mut ticks = 0;
    while w.fighters[0].velocity.length() > 0.0 {
        w.step(&[Intent::default()]);
        ticks += 1;
    }
    let slide = w.fighters[0].position.x - x0;
    let expect = 4.0 * 4.0 / (2.0 * 9.0); // v^2 / 2a
    assert!((slide - expect).abs() < 4.0 * STEP, "slide {slide} vs {expect}");
    assert!(ticks > 40, "a run does not stop instantly");
}

#[test]
fn arena_holds_bodies_inside() {
    let mut w = World::sparring(Tuning::default());
    let half = w.tuning.arena_half_size - w.tuning.body_radius;
    for _ in 0..1200 {
        w.step(&[Intent { move_dir: V2::new(1.0, 1.0), run: true, ..Default::default() }, Intent { move_dir: V2::new(-1.0, -1.0), run: true, ..Default::default() }]);
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
        w.step(&[Intent { move_dir: V2::new(0.0, 1.0), run: true, ..Default::default() }, Intent { move_dir: V2::new(0.0, -1.0), run: true, ..Default::default() }]);
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
        w.step(&[Intent { move_dir: V2::new(-1.0, 0.0), run: false, ..Default::default() }]);
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
    w.step(&[Intent { move_dir: V2::new(1.0, 0.0), run: false, ..Default::default() }]);
    let to = (w.fighters[1].position - w.fighters[0].position).normalized();
    assert!(w.fighters[0].facing.dot(to) > 0.9999);
}

fn run_to_cruise(model: MotorModel) -> World {
    let mut w = World::sparring(Tuning { model, ..Tuning::default() });
    w.fighters.truncate(1);
    w.fighters[0].position = V2::new(-5.5, 0.0);
    for _ in 0..240 {
        w.step(&[Intent { move_dir: V2::new(1.0, 0.0), run: true, ..Default::default() }]);
    }
    w
}

#[test]
fn drag_terminal_speed_is_exactly_the_target() {
    let w = run_to_cruise(MotorModel::Drag);
    assert!((w.fighters[0].velocity.x - 4.0).abs() < 1e-4, "v = {}", w.fighters[0].velocity.x);
}

#[test]
fn drag_starts_fast_then_eases_into_top_speed() {
    let mut w = World::sparring(Tuning::default());
    w.fighters.truncate(1);
    w.fighters[0].position = V2::new(-5.5, 0.0);
    let run = [Intent { move_dir: V2::new(1.0, 0.0), run: true, ..Default::default() }];
    let mut speeds = Vec::new();
    for _ in 0..60 {
        w.step(&run);
        speeds.push(w.fighters[0].velocity.x);
    }
    let gain = |a: usize, b: usize| speeds[b] - speeds[a];
    assert!(gain(0, 10) > gain(30, 40) * 2.0, "early ticks gain more speed than late ones");
    // discrete closed form: v_n = s (1 - d^n), d = s / (s + a dt)
    let (s, push) = (4.0f32, 30.0 * STEP);
    let expect = s * (1.0 - (s / (s + push)).powi(60));
    assert!((speeds[59] - expect).abs() < 1e-4, "v60 {} vs {expect}", speeds[59]);
}

#[test]
fn drag_coast_halves_speed_each_half_life() {
    let mut w = run_to_cruise(MotorModel::Drag);
    let half = w.tuning.coast_half_life;
    let ticks = (half / STEP).round() as usize;
    for _ in 0..ticks {
        w.step(&[Intent::default()]);
    }
    assert!((w.fighters[0].velocity.x - 2.0).abs() < 0.05, "v = {}", w.fighters[0].velocity.x);
    for _ in 0..600 {
        w.step(&[Intent::default()]);
    }
    assert_eq!(w.fighters[0].velocity, V2::ZERO, "coasting comes to rest");
}

#[test]
fn drag_reversal_plants_before_moving_back() {
    let mut w = run_to_cruise(MotorModel::Drag);
    let x0 = w.fighters[0].position.x;
    let mut furthest = x0;
    for _ in 0..120 {
        w.step(&[Intent { move_dir: V2::new(-1.0, 0.0), run: true, ..Default::default() }]);
        furthest = furthest.max(w.fighters[0].position.x);
    }
    assert!(furthest > x0 + 0.05, "momentum carries the body on before it turns");
    assert!(w.fighters[0].velocity.x < -3.9, "then it runs back at full speed");
}
