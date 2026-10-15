use phys_check::phys_mass::*;

const DT: f32 = 1.0 / 120.0;

#[test]
fn rod_limit_and_parallel_axis() {
    // Thin cylinder about end must approach m l^2 / 3.
    let p = segment_props(Segment { mass: 2.0, length: 0.3, radius: 0.001 }, &[]);
    let rod = 2.0 * 0.09 / 3.0;
    let e = (p.inertia_joint[0][0] - rod).abs() / rod;
    println!("rod about end: {:.6} vs {:.6}, rel err {:.2e}", p.inertia_joint[0][0], rod, e);
    assert!(e < 1e-4);
    // Weapon at tip: +m l^2.
    let w = segment_props(Segment { mass: 2.0, length: 0.3, radius: 0.001 }, &[Load { mass: 1.5, at: [0.0, 0.3, 0.0] }]);
    let d = w.inertia_joint[0][0] - p.inertia_joint[0][0];
    println!("1.5 kg at 0.3 m adds {:.5} kg m^2 (expect 0.135), com {:.4} m", d, w.com[1]);
    assert!((d - 0.135).abs() < 1e-5);
    assert!((w.com[1] - (2.0 * 0.15 + 1.5 * 0.3) / 3.5).abs() < 1e-6);
}

#[test]
fn tensor_is_symmetric_positive_and_triangle() {
    let p = segment_props(Segment { mass: 3.0, length: 0.4, radius: 0.05 }, &[Load { mass: 0.8, at: [0.03, 0.25, -0.02] }]);
    let i = p.inertia_joint;
    let mut asym: f32 = 0.0;
    for a in 0..3 { for b in 0..3 { asym = asym.max((i[a][b] - i[b][a]).abs()); } }
    // Positive definite via leading minors.
    let m1 = i[0][0];
    let m2 = i[0][0] * i[1][1] - i[0][1] * i[1][0];
    let m3 = i[0][0] * (i[1][1] * i[2][2] - i[1][2] * i[2][1]) - i[0][1] * (i[1][0] * i[2][2] - i[1][2] * i[2][0]) + i[0][2] * (i[1][0] * i[2][1] - i[1][1] * i[2][0]);
    println!("asym {:.1e}, minors {:.4e} {:.4e} {:.4e}", asym, m1, m2, m3);
    assert!(asym < 1e-7 && m1 > 0.0 && m2 > 0.0 && m3 > 0.0);
    // Triangle inequality on diagonal (true for any physical body).
    assert!(i[0][0] + i[1][1] >= i[2][2] && i[1][1] + i[2][2] >= i[0][0] && i[0][0] + i[2][2] >= i[1][1]);
}

/// 1-DOF forearm driven by motor_torque following a sine swing; compare tracking with/without
/// feed-forward, and holding level under gravity at the torque limit.
fn track(ff: bool, load: f32) -> (f32, f32) {
    let seg = Segment { mass: 1.2, length: 0.28, radius: 0.04 };
    let p = segment_props(seg, &[Load { mass: load, at: [0.0, 0.28, 0.0] }]);
    let iz = p.inertia_joint[2][2];
    let lim = arm_torque_limit(p.mass, p.com[1], iz, 120.0, 1.5, 0.0);
    let (mut th, mut w) = (0.0f32, 0.0f32);
    let (a, f) = (0.8f32, 1.5f32);
    let gains = Gains { kp: 40.0, kd: 4.0 };
    let mut max_err: f32 = 0.0;
    let mut max_dtau: f32 = 0.0;
    let mut prev = 0.0f32;
    for k in 0..(3.0 / DT) as usize {
        let t = k as f32 * DT;
        let wn = 2.0 * std::f32::consts::PI * f;
        let (tt, tw, ta) = (a * (wn * t).sin(), a * wn * (wn * t).cos(), -a * wn * wn * (wn * t).sin());
        // Segment points along +x rotated by th about z; gravity -y.
        let com = [p.com[1] * th.cos(), p.com[1] * th.sin(), 0.0];
        let gt = gravity_torque(p.mass, com)[2];
        let mt = MotorTarget { err: [0.0, 0.0, tt - th], omega: [0.0, 0.0, tw], alpha: [0.0, 0.0, if ff { ta } else { 0.0 }] };
        let gc = if ff { [0.0, 0.0, -gt] } else { [0.0; 3] };
        let mut ii = [[0.0; 3]; 3]; ii[2][2] = iz;
        let tau = motor_torque(&ii, [0.0, 0.0, w], mt, gc, gains)[2].clamp(-lim, lim);
        if t > 1.0 { max_err = max_err.max((tt - th).abs()); }
        if k > 0 { max_dtau = max_dtau.max((tau - prev).abs()); }
        prev = tau;
        // Semi-implicit Euler at 1/120.
        w += (tau + gt) / iz * DT;
        th += w * DT;
    }
    (max_err, max_dtau)
}

#[test]
fn feed_forward_tracks_swing() {
    for load in [0.0f32, 2.0] {
        let (e_ff, d_ff) = track(true, load);
        let (e_pd, _) = track(false, load);
        println!("load {load} kg: max err with FF {:.4} rad, PD only {:.4} rad, max dtau/tick {:.3} N m", e_ff, e_pd, d_ff);
        assert!(e_ff < 0.03 && e_ff < e_pd / 2.0);
    }
}

#[test]
fn torque_limit_holds_level_with_headroom() {
    let p = segment_props(Segment { mass: 3.0, length: 0.3, radius: 0.05 }, &[Load { mass: 2.5, at: [0.0, 0.3, 0.0] }]);
    let lim = arm_torque_limit(p.mass, p.com[1], p.inertia_joint[2][2], 60.0, 1.5, 0.0);
    let hold = p.mass * 9.81 * p.com[1];
    println!("hold level {:.2} N m, limit {:.2} N m, ratio {:.2}", hold, lim, lim / hold);
    assert!(lim > 1.5 * hold - 1e-4);
    // Strike window raises only the inertia term, by at most 1.5x.
    let ls = arm_torque_limit(p.mass, p.com[1], p.inertia_joint[2][2], 60.0, 1.5, 1.0);
    assert!((ls - lim - 1.5 * p.inertia_joint[2][2] * 30.0).abs() < 1e-3);
    // Leg clamp blends between swing (c=0) and stance (c=1) mass.
    assert!((leg_torque_limit(LEG_K, 0.0, 60.0, 10.0, 0.2) - 1.2 * 9.81 * 10.0 * 0.2).abs() < 1e-4);
    assert!((leg_torque_limit(LEG_K, 1.0, 60.0, 10.0, 0.2) - 1.2 * 9.81 * 60.0 * 0.2).abs() < 1e-3);
}

#[test]
fn gyro_term_matches_euler_equation() {
    // Free body, no target: motor_torque with zero gains and zero alpha returns w x (I w).
    let i: I3 = [[0.02, 0.0, 0.0], [0.0, 0.005, 0.0], [0.0, 0.0, 0.018]];
    let w = [3.0, -1.0, 2.0];
    let tau = motor_torque(&i, w, MotorTarget { err: [0.0; 3], omega: w, alpha: [0.0; 3] }, [0.0; 3], Gains { kp: 0.0, kd: 0.0 });
    let exp = [(0.018 - 0.005) * -1.0 * 2.0, (0.02 - 0.018) * 2.0 * 3.0, (0.005 - 0.02) * 3.0 * -1.0];
    let err = (0..3).map(|k| (tau[k] - exp[k]).abs()).fold(0.0f32, f32::max);
    println!("gyro err {:.1e}", err);
    assert!(err < 1e-6);
}

#[test]
fn deterministic_replay() {
    assert_eq!(track(true, 2.0).0.to_bits(), track(true, 2.0).0.to_bits());
}
