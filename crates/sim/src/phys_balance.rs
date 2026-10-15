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
pub struct BoneMass {
    pub bone: u16,
    pub mass: f32,
    pub com: V3,
}

/// Whole-body centre of mass (y up).
pub fn body_com(bones: &[BoneMass]) -> (f32, V3) {
    let mut m = 0.0f32;
    let mut s = [0.0f32; 3];
    for b in bones {
        m += b.mass;
        for k in 0..3 {
            s[k] += b.mass * b.com[k];
        }
    }
    let inv = 1.0 / m.max(1e-6);
    (m, [s[0] * inv, s[1] * inv, s[2] * inv])
}

/// Convex hull (x, z ground plane) of planted contact points, counter-clockwise. Deterministic:
/// points are sorted with total_cmp before the monotone-chain pass.
pub fn support_polygon(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut p: Vec<[f32; 2]> = points.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let cr = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut h: Vec<[f32; 2]> = Vec::with_capacity(p.len() * 2);
    for &q in p.iter() {
        while h.len() >= 2 && cr(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    let lower = h.len() + 1;
    for &q in p.iter().rev().skip(1) {
        while h.len() >= lower && cr(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
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
    if n == 0 {
        return None;
    }
    let mean = || {
        let k = n as f32;
        [
            poly.iter().map(|p| p[0]).sum::<f32>() / k,
            poly.iter().map(|p| p[1]).sum::<f32>() / k,
        ]
    };
    if n < 3 {
        return Some(mean());
    }
    // Shoelace about poly[0] to keep precision far from the origin.
    let o = poly[0];
    let (mut a2, mut cx, mut cz) = (0.0f32, 0.0f32, 0.0f32);
    for i in 1..n - 1 {
        let (p, q) = (
            [poly[i][0] - o[0], poly[i][1] - o[1]],
            [poly[i + 1][0] - o[0], poly[i + 1][1] - o[1]],
        );
        let c = p[0] * q[1] - q[0] * p[1];
        a2 += c;
        cx += (p[0] + q[0]) * c;
        cz += (p[1] + q[1]) * c;
    }
    if a2.abs() < 1e-9 {
        return Some(mean());
    }
    Some([o[0] + cx / (3.0 * a2), o[1] + cz / (3.0 * a2)])
}

/// Signed distance from `c` to the polygon edge: positive inside (stable), negative outside.
/// For fewer than 3 points it is minus the distance to the nearest point or segment.
pub fn stability_margin(poly: &[[f32; 2]], c: [f32; 2]) -> f32 {
    let seg = |a: [f32; 2], b: [f32; 2]| {
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dz * dz;
        let t = if l2 > 0.0 {
            (((c[0] - a[0]) * dx + (c[1] - a[1]) * dz) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
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
                if (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) < 0.0 {
                    inside = false;
                }
                d = d.min(seg(a, b));
            }
            if inside { d } else { -d }
        }
    }
}

/// Lab balance (gb_lab, kb/kbd form): desired torque on the body above the feet,
/// τ = ((kb·e + kbd·ė) × up)·M·g, e = COM − support centre (x, z). kb is 1/m, kbd s/m. Ramped by `ramp`.
pub fn balance_kb(
    mass: f32,
    com: V3,
    com_vel: V3,
    centre: [f32; 2],
    kb: f32,
    kbd: f32,
    ticks_since_on: u64,
) -> V3 {
    let e = [com[0] - centre[0], 0.0, com[2] - centre[1]];
    let v = [com_vel[0], 0.0, com_vel[2]];
    scale(
        cross(add(scale(e, kb), scale(v, kbd)), [0.0, 1.0, 0.0]),
        mass * G * ramp(ticks_since_on),
    )
}

/// Gravity feed-forward for one joint, world frame. Apply +τ on child, -τ on parent, inside the one clamp.
/// Reaction form (lab-verified): τ = (com_sub − p) × (m_sub·g·up) − c·(cop − p) × (share·M·g·up).
/// `com_sub`/`m_sub`: subtree on the child (foot-ward) side. `cop`: this foot's centre of pressure.
/// `share`: this foot's share of body weight (sums to 1 over planted feet). `c`: filtered planted value.
/// Swing limb (c = 0) reduces to plain subtree gravity compensation.
pub fn gravity_ff(
    p_joint: V3,
    com_sub: V3,
    m_sub: f32,
    cop: V3,
    share: f32,
    m_total: f32,
    c: f32,
) -> V3 {
    let up = |m: f32| [0.0, m * G, 0.0];
    let own = cross(sub(com_sub, p_joint), up(m_sub));
    let react = cross(
        sub(cop, p_joint),
        up(c.clamp(0.0, 1.0) * share.max(0.0) * m_total),
    );
    sub(own, react)
}

pub type M3 = [[f32; 3]; 3];

/// Inverse of a symmetric 3x3 in f64 via the adjugate. None if |det| <= 1e-12 * scale^3 or non-finite.
fn inv3_sym(m: M3) -> Option<[[f64; 3]; 3]> {
    let a: [[f64; 3]; 3] =
        std::array::from_fn(|i| std::array::from_fn(|j| 0.5 * (m[i][j] as f64 + m[j][i] as f64)));
    let c00 = a[1][1] * a[2][2] - a[1][2] * a[2][1];
    let c01 = a[1][2] * a[2][0] - a[1][0] * a[2][2];
    let c02 = a[1][0] * a[2][1] - a[1][1] * a[2][0];
    let det = a[0][0] * c00 + a[0][1] * c01 + a[0][2] * c02;
    let sc = a.iter().flatten().fold(0.0f64, |x, v| x.max(v.abs()));
    if !det.is_finite() || sc == 0.0 || det.abs() <= 1e-12 * sc * sc * sc {
        return None;
    }
    let c11 = a[0][0] * a[2][2] - a[0][2] * a[2][0];
    let c12 = a[0][1] * a[2][0] - a[0][0] * a[2][1];
    let c22 = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    let r = [[c00, c01, c02], [c01, c11, c12], [c02, c12, c22]];
    Some(std::array::from_fn(|i| {
        std::array::from_fn(|j| r[i][j] / det)
    }))
}

/// Reduced inertia (row-major, world frame) of two bodies joined at a joint: (I_c^-1 + I_p^-1)^-1.
/// Pass each body's OWN world inertia (Avian applies the torque per body). Computed in f64, result
/// symmetrised. Guard: if exactly one input is singular (e.g. a static or massless parent reported as zero),
/// the other is returned, matching the lab's static-parent rule; if both, or the sum, are singular: zero.
pub fn reduced_inertia3(i_parent: M3, i_child: M3) -> M3 {
    let to32 = |r: [[f64; 3]; 3]| -> M3 {
        std::array::from_fn(|i| std::array::from_fn(|j| (0.5 * (r[i][j] + r[j][i])) as f32))
    };
    match (inv3_sym(i_parent), inv3_sym(i_child)) {
        (Some(ip), Some(ic)) => {
            let s: M3 =
                std::array::from_fn(|i| std::array::from_fn(|j| (ip[i][j] + ic[i][j]) as f32));
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
    feet.iter()
        .map(|&(c, share, ankle_cap)| {
            let want = scale(tau_bal, c.clamp(0.0, 1.0) * share.max(0.0));
            let l = len(want);
            let ankle = if l > ankle_cap && l > 0.0 {
                scale(want, ankle_cap.max(0.0) / l)
            } else {
                want
            };
            (ankle, sub(want, ankle))
        })
        .collect()
}

/// Saturation of one joint against its own clamp: |τ| / max(cap, 1 N·m). None for leg joints whose foot
/// is mostly off the ground (c < 0.2): their cap is tiny by design and the ratio says nothing.
pub fn sat_ratio(tau: V3, cap: f32, c: f32) -> Option<f32> {
    (c >= 0.2).then(|| len(tau) / cap.max(1.0))
}

/// Foot contact on the sole height ABOVE ITS CAPSULE'S LOWEST POINT (m): hysteresis (on below 1 cm,
/// off above 2.5 cm, then a 6-tick hold before it can flip back) feeds a one-pole filter (0.25 per tick)
/// that gives the planted factor `c` in [0, 1]. Caps, gravity_ff and the balance split all read `c`.
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub on: bool,
    pub held: u32,
    pub c: f32,
}
pub const CONTACT_ON_M: f32 = 0.01;
pub const CONTACT_OFF_M: f32 = 0.025;
pub const CONTACT_HOLD_TICKS: u32 = 6;
pub const CONTACT_ALPHA: f32 = 0.25;

impl Contact {
    /// Starts planted (c = 1): the stand test spawns on its feet.
    pub fn planted() -> Self {
        Contact {
            on: true,
            held: CONTACT_HOLD_TICKS,
            c: 1.0,
        }
    }
    /// One tick. Returns true when the on/off state flipped (for the flip counter).
    pub fn step(&mut self, sole_height: f32) -> bool {
        self.held = self.held.saturating_add(1);
        let want = if self.on {
            sole_height <= CONTACT_OFF_M
        } else {
            sole_height < CONTACT_ON_M
        };
        let flip = want != self.on && self.held >= CONTACT_HOLD_TICKS;
        if flip {
            self.on = want;
            self.held = 0;
        }
        let raw = if self.on { 1.0 } else { 0.0 };
        self.c += (raw - self.c) * CONTACT_ALPHA;
        flip
    }
}

#[cfg(test)]
mod tests3 {
    use super::*;
    fn diag(a: f32, b: f32, c: f32) -> M3 {
        [[a, 0.0, 0.0], [0.0, b, 0.0], [0.0, 0.0, c]]
    }
    #[test]
    fn equal_diag_halves() {
        let r = reduced_inertia3(diag(0.2, 0.3, 0.05), diag(0.2, 0.3, 0.05));
        let e = diag(0.1, 0.15, 0.025);
        for i in 0..3 {
            for j in 0..3 {
                assert!((r[i][j] - e[i][j]).abs() < 1e-6);
            }
        }
    }
    #[test]
    fn symmetric_and_bounded() {
        let p = [[0.30, 0.02, -0.01], [0.02, 0.25, 0.03], [-0.01, 0.03, 0.10]];
        let c = [
            [0.05, 0.004, 0.0],
            [0.004, 0.06, -0.002],
            [0.0, -0.002, 0.01],
        ];
        let r = reduced_inertia3(p, c);
        for i in 0..3 {
            for j in 0..3 {
                assert_eq!(r[i][j], r[j][i]);
            }
        }
        for i in 0..3 {
            assert!(r[i][i] > 0.0 && r[i][i] < c[i][i] + 1e-6);
        }
    }
    #[test]
    fn singular_guard() {
        let z = [[0.0; 3]; 3];
        assert_eq!(reduced_inertia3(z, z), z);
        assert_eq!(
            reduced_inertia3(diag(1.0, 1.0, 1.0), z),
            diag(1.0, 1.0, 1.0)
        );
    }
    #[test]
    fn stable_pd3_matches_formula() {
        let t = stable_pd3(
            diag(2.0, 2.0, 2.0),
            [0.1, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            4000.0,
            126.0,
            1.0 / 720.0,
        );
        let den = 1.0 + 126.0 / 720.0 + 4000.0 / (720.0f32 * 720.0);
        assert!(
            (t[0] - 2.0 * 400.0 / den).abs() < 1e-3
                && (t[1] + 2.0 * 126.0 / den).abs() < 1e-3
                && t[2] == 0.0
        );
    }
}
