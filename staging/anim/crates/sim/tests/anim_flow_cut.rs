#[path = "../src/anim_flow_cut.rs"]
mod anim_flow_cut;
use anim_flow_cut::*;

#[test]
fn windows_and_strike_guard() {
    let ph = StrikePhases { strike: 20, recovery: 35, len: 80 };
    assert_eq!(cancel_window(ph, true), CancelWindow { open: 35, close: 71 });
    assert_eq!(cancel_window(ph, false), CancelWindow { open: 50, close: 71 });
    let w = cancel_window(ph, true);
    assert!((w.open..w.close).all(|t| !inside_strike_window(ph, t)));
    assert!(inside_strike_window(ph, 20) && inside_strike_window(ph, 34) && !inside_strike_window(ph, 35));
}

#[test]
fn best_cut_matches_incoming() {
    let p = |t: u64| { let a = t as f32 * 0.05; [a.cos() * 0.4, 1.2, a.sin() * 0.4] };
    let (a, b) = (p(60), p(59));
    let v = [(a[0] - b[0]) * 120.0, 0.0, (a[2] - b[2]) * 120.0];
    assert_eq!(best_cut_tick(&p, a, v, CancelWindow { open: 40, close: 75 }), 60);
    assert_eq!(blend_time_for(0.0, 0.0, 0.12, 0.3), 0.12);
    assert_eq!(blend_time_for(5.0, 0.0, 0.12, 0.3), 0.3);
}
