//! Combo-cut decisions for the EXISTING inertializer in crates/game/src/anim.rs (pending_inertia,
//! off_t/off_r, inertia_total = transition_s). No blending here: only WHEN to cut (cancel window,
//! best tick), HOW LONG anim.rs fades (seconds for `rig.transition_s`), and WHETHER a cut may be
//! smoothed (never inside authored strike frames). Ticks are 1/120 s; hand positions in metres.

pub const DT: f32 = 1.0 / 120.0;
pub type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn len(a: V3) -> f32 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }

/// windup [0, strike), active [strike, recovery), recovery [recovery, len), in ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrikePhases { pub strike: u64, pub recovery: u64, pub len: u64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CancelWindow { pub open: u64, pub close: u64 }

/// Opens at recovery start on hit, 35% into recovery on whiff; closes at 80%. Never inside active frames.
pub fn cancel_window(ph: StrikePhases, hit_confirmed: bool) -> CancelWindow {
    let rec = ph.len.saturating_sub(ph.recovery).max(1);
    let open = ph.recovery + (rec as f32 * if hit_confirmed { 0.0 } else { 0.35 }) as u64;
    let close = ph.recovery + (rec as f32 * 0.8) as u64;
    CancelWindow { open: open.min(close.saturating_sub(1)).max(ph.recovery), close: close.max(open + 1) }
}

/// Authored active frames: anim.rs must not inertialize a cut here (keeps impact pops).
pub fn inside_strike_window(ph: StrikePhases, tick: u64) -> bool { tick >= ph.strike && tick < ph.recovery }

fn vel(p: &dyn Fn(u64) -> V3, t: u64) -> V3 { let d = sub(p(t), p(t.saturating_sub(1))); [d[0] / DT, d[1] / DT, d[2] / DT] }

/// Tick in the window best matching the incoming windup start; earliest wins ties.
pub fn best_cut_tick(out_pos: &dyn Fn(u64) -> V3, inc_pos: V3, inc_vel: V3, w: CancelWindow) -> u64 {
    let mut best = (f32::INFINITY, w.open);
    for t in w.open..w.close {
        let cost = len(sub(out_pos(t), inc_pos)) + 0.05 * len(sub(vel(out_pos, t), inc_vel));
        if cost < best.0 { best = (cost, t); }
    }
    best.1
}

/// Seconds for `rig.transition_s` (becomes inertia_total): longer for bigger mismatch.
pub fn blend_time_for(dp: f32, dv: f32, min_s: f32, max_s: f32) -> f32 { (min_s + 0.25 * dp + 0.02 * dv).clamp(min_s, max_s) }
