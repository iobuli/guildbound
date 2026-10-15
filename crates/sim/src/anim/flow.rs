//! Combo chain flow: inertialized handovers between strike clips and recovery->windup cancel windows.
//! Pure, deterministic, tick based (1/120 s). No deps. The quintic matches `anim_gait::Inertializer`
//! (copied here so this file stands alone).

pub const DT: f32 = 1.0 / 120.0;
pub type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}

/// Position, velocity, acceleration of a track at one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kin {
    pub p: V3,
    pub v: V3,
    pub a: V3,
}

/// Kinematics at the END of the outgoing samples (backward differences over the last 3 ticks).
pub fn kin_backward(p2: V3, p1: V3, p0: V3, dt: f32) -> Kin {
    let v = scale(sub(p0, p1), 1.0 / dt);
    let a = scale(add(sub(p0, scale(p1, 2.0)), p2), 1.0 / (dt * dt));
    Kin { p: p0, v, a }
}
/// Kinematics at the START of the incoming samples (forward differences over the first 3 ticks).
pub fn kin_forward(q0: V3, q1: V3, q2: V3, dt: f32) -> Kin {
    let v = scale(sub(q1, q0), 1.0 / dt);
    let a = scale(add(sub(q2, scale(q1, 2.0)), q0), 1.0 / (dt * dt));
    Kin { p: q0, v, a }
}

/// Per-axis quintic that takes the outgoing-minus-incoming offset (x0, v0, a0) to rest at t_blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inertializer {
    c: [[f32; 6]; 3],
    t_blend: f32,
}

impl Inertializer {
    pub fn new(x0: V3, v0: V3, a0: V3, t_blend: f32) -> Self {
        let t = t_blend.max(1e-4);
        let mut c = [[0.0; 6]; 3];
        for i in 0..3 {
            let x = x0[i];
            // never let the offset start moving away from zero (avoids overshoot of the fade)
            let v = if x * v0[i] > 0.0 { 0.0 } else { v0[i] };
            let a = a0[i];
            let t2 = t * t;
            let t3 = t2 * t;
            let t4 = t3 * t;
            let t5 = t4 * t;
            c[i] = [
                x,
                v,
                0.5 * a,
                -(3.0 * a * t2 + 12.0 * v * t + 20.0 * x) / (2.0 * t3),
                (3.0 * a * t2 + 16.0 * v * t + 30.0 * x) / (2.0 * t4),
                -(a * t2 + 6.0 * v * t + 12.0 * x) / (2.0 * t5),
            ];
        }
        Self { c, t_blend: t }
    }
    pub fn from_kin(out: Kin, inc: Kin, t_blend: f32) -> Self {
        Self::new(
            sub(out.p, inc.p),
            sub(out.v, inc.v),
            sub(out.a, inc.a),
            t_blend,
        )
    }
    pub fn offset(&self, t: f32) -> V3 {
        if t >= self.t_blend {
            return [0.0; 3];
        }
        let t = t.max(0.0);
        let mut o = [0.0; 3];
        for i in 0..3 {
            let c = &self.c[i];
            o[i] = c[0] + t * (c[1] + t * (c[2] + t * (c[3] + t * (c[4] + t * c[5]))));
        }
        o
    }
    /// Blended position `ticks` after the handover, given the incoming clip's raw position.
    pub fn apply(&self, incoming: V3, ticks: u64) -> V3 {
        add(incoming, self.offset(ticks as f32 * DT))
    }
}

/// Blend time for a handover: bigger mismatch -> longer fade, clamped to [min, max] seconds.
/// Gives fast strings a snappy feel while big pose jumps still settle smoothly.
pub fn blend_time_for(out: Kin, inc: Kin, min_s: f32, max_s: f32) -> f32 {
    let dp = len(sub(out.p, inc.p));
    let dv = len(sub(out.v, inc.v));
    (min_s + 0.25 * dp + 0.02 * dv).clamp(min_s, max_s)
}

/// Phases of a strike clip in ticks: windup [0, strike), active [strike, recovery), recovery [recovery, len).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrikePhases {
    pub strike: u64,
    pub recovery: u64,
    pub len: u64,
}

/// Window (inclusive start, exclusive end tick in the outgoing clip) where the next strike may cancel in.
/// Opens a few ticks after the active frames end (so the hit reads), closes before the clip settles to idle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CancelWindow {
    pub open: u64,
    pub close: u64,
}

pub fn cancel_window(ph: StrikePhases, hit_confirmed: bool) -> CancelWindow {
    let rec = ph.len.saturating_sub(ph.recovery).max(1);
    // on hit, open earlier (combo reward); on whiff, make the player eat more recovery
    let open_frac = if hit_confirmed { 0.0 } else { 0.35 };
    let open = ph.recovery + (rec as f32 * open_frac) as u64;
    let close = ph.recovery + (rec as f32 * 0.8) as u64;
    CancelWindow {
        open: open.min(close.saturating_sub(1)),
        close: close.max(open + 1),
    }
}

/// Pick the cut tick inside the window that best matches the incoming windup: hand moving the
/// same way, close in position. `out_pos(t)` samples the outgoing hand path at tick t.
/// Ties resolve to the earliest tick (deterministic).
pub fn best_cut_tick(out_pos: &dyn Fn(u64) -> V3, inc: Kin, w: CancelWindow) -> u64 {
    let mut best = (f32::INFINITY, w.open);
    for t in w.open..w.close {
        let k = kin_backward(
            out_pos(t.saturating_sub(2)),
            out_pos(t.saturating_sub(1)),
            out_pos(t),
            DT,
        );
        let cost = len(sub(k.p, inc.p)) + 0.05 * len(sub(k.v, inc.v));
        if cost < best.0 {
            best = (cost, t);
        }
    }
    best.1
}

/// Largest per-tick change in acceleration (m/s^2 per tick) of a sampled path, over ticks [from, to).
pub fn max_accel_step(path: &[V3], from: usize, to: usize) -> f32 {
    let acc = |i: usize| {
        scale(
            add(sub(path[i + 1], scale(path[i], 2.0)), path[i - 1]),
            1.0 / (DT * DT),
        )
    };
    let mut m: f32 = 0.0;
    for i in from.max(1)..to.min(path.len().saturating_sub(2)) {
        m = m.max(len(sub(acc(i + 1), acc(i))));
    }
    m
}
