//! Physics: whole-body balance for the sandbox ragdoll (amendment s7, s9, s10).
//! Pure, fixed bone order, whole ticks. Bones are `u16` indices until `event_log::BoneIdx` lands.

use crate::phys_math::*;

pub const TICK_HZ: u64 = 120;
/// Balance torque ramps 0 -> 1 over this many ticks after ragdoll-on (0.5 s), smoothstep.
pub const RAMP_TICKS: u64 = 60;

pub fn ramp(ticks_since_ragdoll_on: u64) -> f32 {
    let x = (ticks_since_ragdoll_on.min(RAMP_TICKS) as f32) / RAMP_TICKS as f32;
    x * x * (3.0 - 2.0 * x)
}

/// Per-bone mass sample: world centre of mass and mass, in bone index order.
#[derive(Clone, Copy, Debug)]
pub struct BoneMass { pub bone: u16, pub mass: f32, pub com: V3 }

/// Whole-body centre of mass (y up).
pub fn body_com(bones: &[BoneMass]) -> (f32, V3) {
    let mut m = 0.0f32;
    let mut s = [0.0f32; 3];
    for b in bones { m += b.mass; for k in 0..3 { s[k] += b.mass * b.com[k]; } }
    let inv = 1.0 / m.max(1e-6);
    (m, [s[0] * inv, s[1] * inv, s[2] * inv])
}

/// Convex hull (x, z ground plane) of planted contact points, counter-clockwise. Deterministic:
/// points are sorted with total_cmp before the monotone-chain pass.
pub fn support_polygon(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut p: Vec<[f32; 2]> = points.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup();
    if p.len() < 3 { return p; }
    let cr = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut h: Vec<[f32; 2]> = Vec::with_capacity(p.len() * 2);
    for &q in p.iter() {
        while h.len() >= 2 && cr(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 { h.pop(); }
        h.push(q);
    }
    let lower = h.len() + 1;
    for &q in p.iter().rev().skip(1) {
        while h.len() >= lower && cr(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 { h.pop(); }
        h.push(q);
    }
    h.pop();
    h
}

/// Centre of the support polygon (x, z): the true area centroid of the hull, so an uneven
/// footprint does not pull the centre towards the side with more corners.
/// Fallback for 1-2 points (one foot point / degenerate hull) or zero area: the mean of the points.
pub fn support_centre(poly: &[[f32; 2]]) -> Option<[f32; 2]> {
    let n = poly.len();
    if n == 0 { return None; }
    let mean = || {
        let k = n as f32;
        [poly.iter().map(|p| p[0]).sum::<f32>() / k, poly.iter().map(|p| p[1]).sum::<f32>() / k]
    };
    if n < 3 { return Some(mean()); }
    // Shoelace about poly[0] to keep precision far from the origin.
    let o = poly[0];
    let (mut a2, mut cx, mut cz) = (0.0f32, 0.0f32, 0.0f32);
    for i in 1..n - 1 {
        let (p, q) = ([poly[i][0] - o[0], poly[i][1] - o[1]], [poly[i + 1][0] - o[0], poly[i + 1][1] - o[1]]);
        let c = p[0] * q[1] - q[0] * p[1];
        a2 += c;
        cx += (p[0] + q[0]) * c;
        cz += (p[1] + q[1]) * c;
    }
    if a2.abs() < 1e-9 { return Some(mean()); }
    Some([o[0] + cx / (3.0 * a2), o[1] + cz / (3.0 * a2)])
}

/// Signed distance from `c` to the polygon edge: positive inside (stable), negative outside.
/// For fewer than 3 points it is minus the distance to the nearest point or segment.
pub fn stability_margin(poly: &[[f32; 2]], c: [f32; 2]) -> f32 {
    let seg = |a: [f32; 2], b: [f32; 2]| {
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dz * dz;
        let t = if l2 > 0.0 { (((c[0] - a[0]) * dx + (c[1] - a[1]) * dz) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let (px, pz) = (a[0] + t * dx - c[0], a[1] + t * dz - c[1]);
        (px * px + pz * pz).sqrt()
    };
    match poly.len() {
        0 => f32::NEG_INFINITY,
        1 => -seg(poly[0], poly[0]),
        2 => -seg(poly[0], poly[1]),
        n => {
            let mut inside = true;
            let mut d = f32::INFINITY;
            for i in 0..n {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                if (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) < 0.0 { inside = false; }
                d = d.min(seg(a, b));
            }
            if inside { d } else { -d }
        }
    }
}

/// Balance gains: a PD on the horizontal COM offset from the polygon centroid, turned into a
/// corrective torque about the ankle/hip pivot (inverted pendulum, s9).
#[derive(Clone, Copy, Debug)]
pub struct BalanceGains { pub kp: f32, pub kd: f32 }

/// Corrective torque (world, about the support centroid) to bring the COM back over the feet.
/// x-offset form: tau = m h (w^2 x + 2 z w xdot) + m g x, units N m. `h` is the COM height above the
/// mean ankle (passed in, measured per rig at load). x = h sin(theta), xdot = h thetadot, so this
/// equals m h^2 (w^2 theta + 2 z w thetadot) + m g h sin(theta) at small angle. `com_vel` is the COM velocity. Ramped by `ramp(ticks_since_on)`.
/// Returns RAW torque: the only clamp is the per-joint clamp (`phys_mass::leg_torque_limit`).
pub fn balance_torque(mass: f32, h: f32, com: V3, com_vel: V3, poly: &[[f32; 2]], g: BalanceGains, ticks_since_on: u64) -> V3 {
    let Some([cx, cz]) = support_centre(poly) else { return [0.0; 3]; };
    let (ex, ez) = (com[0] - cx, com[2] - cz);
    // Desired horizontal restoring force, then torque = r x F about the pivot at ground level.
    let fx = -mass * (g.kp * ex + g.kd * com_vel[0]);
    let fz = -mass * (g.kp * ez + g.kd * com_vel[2]);
    let h = h.max(0.05);
    // Gravity cancellation m g x (small-angle m g h sin(theta)), as an equivalent force at height h.
    let fx = fx - mass * G * ex / h;
    let fz = fz - mass * G * ez / h;
    let w = ramp(ticks_since_on);
    // Restoring torque about the ground pivot: +x offset needs +tau_z, +z offset needs -tau_x.
    [h * fz * w, 0.0, -h * fx * w]
}

/// Per-bone torque saturation |tau| / limit, in bone order, for the Physics panel.
pub fn saturation(torques: &[(u16, V3, f32)]) -> Vec<(u16, f32)> {
    torques.iter().map(|&(b, t, lim)| (b, (len(t) / lim.max(1e-6)).min(1.0))).collect()
}

/// Horizontal slip of a planted foot this tick (m): ground-plane displacement while in contact.
pub fn foot_slip(prev: V3, now: V3, planted: bool) -> f32 {
    if !planted { return 0.0; }
    let (dx, dz) = (now[0] - prev[0], now[2] - prev[2]);
    (dx * dx + dz * dz).sqrt()
}

/// What the Physics panel and `metrics.jsonl` read each tick.
#[derive(Clone, Debug, Default)]
pub struct BalanceReport {
    pub tick: u64,
    pub com: V3,
    pub support: Vec<[f32; 2]>,
    pub margin: f32,
    pub saturation: Vec<(u16, f32)>,
    pub slip: [f32; 2],
    pub ramp: f32,
}

impl BalanceReport {
    /// Bit-exact hash input for replay checks (feed into `SimStep::hash_into`).
    pub fn hash_words(&self, out: &mut Vec<u32>) {
        out.push(self.tick as u32);
        out.push((self.tick >> 32) as u32);
        for v in self.com { out.push(v.to_bits()); }
        for p in &self.support { out.push(p[0].to_bits()); out.push(p[1].to_bits()); }
        out.push(self.margin.to_bits());
        for &(b, s) in &self.saturation { out.push(b as u32); out.push(s.to_bits()); }
        for s in self.slip { out.push(s.to_bits()); }
    }

    /// One `metrics.jsonl` line.
    pub fn json_line(&self) -> String {
        let peak = self.saturation.iter().fold((0u16, 0.0f32), |a, &(b, s)| if s > a.1 { (b, s) } else { a });
        format!("{{\"tick\":{},\"com\":[{:.4},{:.4},{:.4}],\"margin\":{:.4},\"peak_sat_bone\":{},\"peak_sat\":{:.3},\"slip\":[{:.5},{:.5}],\"ramp\":{:.3}}}",
            self.tick, self.com[0], self.com[1], self.com[2], self.margin, peak.0, peak.1, self.slip[0], self.slip[1], self.ramp)
    }
}

/// Helper only (no longer applied inside `balance_torque`). Whole-body balance torque cap, scaled to the body: m * g * 0.4 m
/// (about 294 N m on master at 75 kg, 233 N m on the mannequin at 59.3 kg).
pub fn balance_cap(body_mass: f32) -> f32 { body_mass * G * 0.4 }

/// PD gains scaled by effective inertia so the same natural frequency and damping ratio hold on
/// both rigs: kp = omega^2, kd = 2 zeta omega (per unit mass, as `balance_torque` multiplies by mass).
pub fn balance_gains(omega: f32, zeta: f32) -> BalanceGains { BalanceGains { kp: omega * omega, kd: 2.0 * zeta * omega } }

/// Gravity feed-forward for one joint, world frame. Apply +τ on child, -τ on parent, inside the one clamp.
/// Reaction form (lab-verified): τ = (com_sub − p) × (m_sub·g·up) − c·(cop − p) × (share·M·g·up).
/// `com_sub`/`m_sub`: subtree on the child (foot-ward) side. `cop`: this foot's centre of pressure.
/// `share`: this foot's share of body weight (sums to 1 over planted feet). `c`: filtered planted value.
/// Swing limb (c = 0) reduces to plain subtree gravity compensation.
pub fn gravity_ff(p_joint: V3, com_sub: V3, m_sub: f32, cop: V3, share: f32, m_total: f32, c: f32) -> V3 {
    let up = |m: f32| [0.0, m * G, 0.0];
    let own = cross(sub(com_sub, p_joint), up(m_sub));
    let react = cross(sub(cop, p_joint), up(c.clamp(0.0, 1.0) * share.max(0.0) * m_total));
    sub(own, react)
}

/// DEPRECATED: scalar form; the lab gate showed it differs 42-59% per tick from the 3x3 form and
/// blows up even at 12 substeps. Use `stable_pd3` with `reduced_inertia3`. Kept for comparison only.
/// Stable-PD joint torque (Tan et al. 2011 style): implicit in dt, scaled by the
/// reduced inertia of the two bodies about the error axis. kp = ω², kd = 2ζω.
/// `i_red` = 1 / (1/I_parent + 1/I_child) about the axis, each body's OWN world inertia.
pub fn stable_pd(i_red: f32, theta_err: V3, omega_rel: V3, kp: f32, kd: f32, dt: f32) -> V3 {
    let den = 1.0 + kd * dt + kp * dt * dt;
    scale(sub(scale(theta_err, kp), scale(omega_rel, kd)), i_red / den)
}

/// DEPRECATED: scalar projection; use `reduced_inertia3` (full 3x3, lab gate winner).
/// Reduced inertia about unit axis `n` from two world inertia tensors (row-major 3x3).
pub fn reduced_inertia(i_parent: [[f32; 3]; 3], i_child: [[f32; 3]; 3], n: V3) -> f32 {
    let q = |m: [[f32; 3]; 3]| dot(n, [dot(m[0], n), dot(m[1], n), dot(m[2], n)]).max(1e-9);
    let (a, b) = (q(i_parent), q(i_child));
    a * b / (a + b)
}

pub type M3 = [[f32; 3]; 3];

/// Inverse of a symmetric 3x3 in f64 via the adjugate. None if |det| <= 1e-12 * scale^3 or non-finite.
fn inv3_sym(m: M3) -> Option<[[f64; 3]; 3]> {
    let a: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| 0.5 * (m[i][j] as f64 + m[j][i] as f64)));
    let c00 = a[1][1] * a[2][2] - a[1][2] * a[2][1];
    let c01 = a[1][2] * a[2][0] - a[1][0] * a[2][2];
    let c02 = a[1][0] * a[2][1] - a[1][1] * a[2][0];
    let det = a[0][0] * c00 + a[0][1] * c01 + a[0][2] * c02;
    let sc = a.iter().flatten().fold(0.0f64, |x, v| x.max(v.abs()));
    if !det.is_finite() || sc == 0.0 || det.abs() <= 1e-12 * sc * sc * sc { return None; }
    let c11 = a[0][0] * a[2][2] - a[0][2] * a[2][0];
    let c12 = a[0][1] * a[2][0] - a[0][0] * a[2][1];
    let c22 = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    let r = [[c00, c01, c02], [c01, c11, c12], [c02, c12, c22]];
    Some(std::array::from_fn(|i| std::array::from_fn(|j| r[i][j] / det)))
}

/// Reduced inertia (row-major, world frame) of two bodies joined at a joint: (I_c^-1 + I_p^-1)^-1.
/// Pass each body's OWN world inertia (Avian applies the torque per body). Computed in f64, result
/// symmetrised. Guard: if exactly one input is singular (e.g. a static or massless parent reported as zero),
/// the other is returned, matching the lab's static-parent rule; if both, or the sum, are singular: zero.
pub fn reduced_inertia3(i_parent: M3, i_child: M3) -> M3 {
    let to32 = |r: [[f64; 3]; 3]| -> M3 { std::array::from_fn(|i| std::array::from_fn(|j| (0.5 * (r[i][j] + r[j][i])) as f32)) };
    match (inv3_sym(i_parent), inv3_sym(i_child)) {
        (Some(ip), Some(ic)) => {
            let s: M3 = std::array::from_fn(|i| std::array::from_fn(|j| (ip[i][j] + ic[i][j]) as f32));
            inv3_sym(s).map(to32).unwrap_or([[0.0; 3]; 3])
        }
        (Some(_), None) => i_parent,
        (None, Some(_)) => i_child,
        (None, None) => [[0.0; 3]; 3],
    }
}

/// Stable-PD joint torque, 3x3 form (lab gate winner): τ = I_red·(kp·θ − kd·ω_rel)/(1 + kd·dt + kp·dt²).
/// World frame; apply +τ on child, −τ on parent. kp = ω², kd = 2ζω (per unit inertia).
pub fn stable_pd3(i_red: M3, theta_err: V3, omega_rel: V3, kp: f32, kd: f32, dt: f32) -> V3 {
    let den = 1.0 + kd * dt + kp * dt * dt;
    let u = scale(sub(scale(theta_err, kp), scale(omega_rel, kd)), 1.0 / den);
    [dot(i_red[0], u), dot(i_red[1], u), dot(i_red[2], u)]
}

/// Split a whole-body balance torque across stance feet: ankles first (each capped by its edge limit),
/// remainder to the matching stance hip (applied −τ on pelvis, +τ on thigh). Shares should sum to 1.
/// Input per foot: (c, share, ankle_cap). Output per foot: (ankle_τ, hip_τ). Both go into each joint's sum before its single clamp.
pub fn split_balance(tau_bal: V3, feet: &[(f32, f32, f32)]) -> Vec<(V3, V3)> {
    feet.iter().map(|&(c, share, ankle_cap)| {
        let want = scale(tau_bal, c.clamp(0.0, 1.0) * share.max(0.0));
        let l = len(want);
        let ankle = if l > ankle_cap && l > 0.0 { scale(want, ankle_cap.max(0.0) / l) } else { want };
        (ankle, sub(want, ankle))
    }).collect()
}

#[cfg(test)]
mod tests3 {
    use super::*;
    fn diag(a: f32, b: f32, c: f32) -> M3 { [[a, 0.0, 0.0], [0.0, b, 0.0], [0.0, 0.0, c]] }
    #[test]
    fn equal_diag_halves() {
        let r = reduced_inertia3(diag(0.2, 0.3, 0.05), diag(0.2, 0.3, 0.05));
        let e = diag(0.1, 0.15, 0.025);
        for i in 0..3 { for j in 0..3 { assert!((r[i][j] - e[i][j]).abs() < 1e-6); } }
    }
    #[test]
    fn symmetric_and_bounded() {
        let p = [[0.30, 0.02, -0.01], [0.02, 0.25, 0.03], [-0.01, 0.03, 0.10]];
        let c = [[0.05, 0.004, 0.0], [0.004, 0.06, -0.002], [0.0, -0.002, 0.01]];
        let r = reduced_inertia3(p, c);
        for i in 0..3 { for j in 0..3 { assert_eq!(r[i][j], r[j][i]); } }
        for i in 0..3 { assert!(r[i][i] > 0.0 && r[i][i] < c[i][i] + 1e-6); }
    }
    #[test]
    fn singular_guard() {
        let z = [[0.0; 3]; 3];
        assert_eq!(reduced_inertia3(z, z), z);
        assert_eq!(reduced_inertia3(diag(1.0, 1.0, 1.0), z), diag(1.0, 1.0, 1.0));
    }
    #[test]
    fn stable_pd3_matches_formula() {
        let t = stable_pd3(diag(2.0, 2.0, 2.0), [0.1, 0.0, 0.0], [0.0, 1.0, 0.0], 4000.0, 126.0, 1.0 / 720.0);
        let den = 1.0 + 126.0 / 720.0 + 4000.0 / (720.0f32 * 720.0);
        assert!((t[0] - 2.0 * 400.0 / den).abs() < 1e-3 && (t[1] + 2.0 * 126.0 / den).abs() < 1e-3 && t[2] == 0.0);
    }
}
