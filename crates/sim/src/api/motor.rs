//! The single motor writer (§API rule): the only code that writes joint torque. Per tick it calls
//! `phys_balance` (stable_pd3, reduced_inertia3, gravity_ff, balance_kb, support_polygon/centre,
//! sat_ratio, Contact; the ankle/hip split is per-axis here) with the anim targets. Maths: PHYSICS_MATH.md (PM) §3–§11,
//! SYSTEM_MATH.md (SM) §0–§13. Torques are world frame, +τ on the child joint's body, −τ on its parent.
//! Rig model: segment frame == bone frame (O_p = O_c = I); the game supplies capsule offsets if needed.

use super::{Body, DT, G, JointRole, JointState, Mode, Tuning};
use crate::math::{Mat3, Quat, Vec3};
use crate::phys_balance as pb;

fn world_inertia(i_body: &Mat3, r: Quat) -> Mat3 {
    let cols: [Vec3; 3] = std::array::from_fn(|k| {
        let e = Vec3::new(
            (k == 0) as u8 as f64,
            (k == 1) as u8 as f64,
            (k == 2) as u8 as f64,
        );
        r.rotate(i_body.mul_vec(r.conj().rotate(e)))
    });
    Mat3([
        [cols[0].x, cols[1].x, cols[2].x],
        [cols[0].y, cols[1].y, cols[2].y],
        [cols[0].z, cols[1].z, cols[2].z],
    ])
}

struct Fk {
    q_world: Vec<Quat>,
    pos: Vec<Vec3>,
    com: Vec<Vec3>,
    w_world: Vec<Vec3>,
    v_com: Option<Vec<Vec3>>,
}

fn fk_measured(b: &Body, m: &[super::MeasuredJoint]) -> Fk {
    let n = b.rig.joints.len();
    let q: Vec<Quat> = m.iter().map(|x| x.q.normalize()).collect();
    let com = (0..n)
        .map(|i| m[i].pos + q[i].rotate(b.rig.joints[i].com))
        .collect();
    Fk {
        pos: m.iter().map(|x| x.pos).collect(),
        w_world: m.iter().map(|x| x.w).collect(),
        v_com: Some(m.iter().map(|x| x.v).collect()),
        com,
        q_world: q,
    }
}

/// Substep motor (engine-agnostic): joint PD from the live pose `m` with hold time `dt`, plus the tick's held
/// residual (gravity ff, balance, SIMBICON corrections), clamped and weighted as in write_torques step 5.
pub(crate) fn substep_torques(
    b: &Body,
    t: &Tuning,
    m: &[super::MeasuredJoint],
    dt: f64,
) -> Vec<Vec3> {
    let n = b.rig.joints.len();
    if m.len() < n || b.residual.len() < n {
        return vec![Vec3::ZERO; n];
    }
    let f = fk_measured(b, &m[..n]);
    let iw: Vec<Mat3> = (0..n)
        .map(|i| world_inertia(&b.rig.joints[i].inertia, f.q_world[i]))
        .collect();
    let pd = pd_part(b, &f, &iw, t, dt);
    let mg = b.rig.joints.iter().map(|j| j.mass).sum::<f64>() * G;
    let fault_cap = t.fault_lever_m * mg;
    (0..n)
        .map(|i| {
            let tau = pd[i] + b.residual[i];
            if !tau.is_finite() {
                return Vec3::ZERO;
            }
            let cap = b.rig.joints[i].torque_cap.max(1.0);
            let w = (b.motor_w_body * b.motor_w[i] * b.mode_w).clamp(0.0, 1.0);
            tau.clamp_length(cap.min(fault_cap.max(1.0))) * w
        })
        .collect()
}

/// World pose: measured (synced tick) or forward kinematics of the internal state (lab fallback).
fn fk(b: &Body) -> Fk {
    let n = b.rig.joints.len();
    if let Some(m) = &b.measured {
        let q: Vec<Quat> = m.iter().map(|x| x.q.normalize()).collect();
        let com = (0..n)
            .map(|i| m[i].pos + q[i].rotate(b.rig.joints[i].com))
            .collect();
        return Fk {
            pos: m.iter().map(|x| x.pos).collect(),
            w_world: m.iter().map(|x| x.w).collect(),
            v_com: Some(m.iter().map(|x| x.v).collect()),
            com,
            q_world: q,
        };
    }
    let (mut qw, mut pos, mut com, mut ww) = (
        vec![Quat::IDENTITY; n],
        vec![Vec3::ZERO; n],
        vec![Vec3::ZERO; n],
        vec![Vec3::ZERO; n],
    );
    for i in 0..n {
        let j = &b.rig.joints[i];
        match j.parent {
            Some(p) => {
                let p = p as usize;
                qw[i] = (qw[p] * b.q_local[i]).normalize();
                pos[i] = pos[p] + qw[p].rotate(j.offset);
                ww[i] = ww[p] + qw[p].rotate(b.w[i]);
            }
            None => {
                qw[i] = (b.root.rot * b.q_local[i]).normalize();
                pos[i] = b.root.pos;
                ww[i] = b.root.rot.rotate(b.w[i]);
            }
        }
        com[i] = pos[i] + qw[i].rotate(j.com);
    }
    Fk {
        q_world: qw,
        pos,
        com,
        w_world: ww,
        v_com: None,
    }
}

/// Child-side subtree (joint and descendants): (mass, com). Parent-before-child order → one reverse pass.
fn subtrees(b: &Body, f: &Fk) -> Vec<(f64, Vec3)> {
    let n = b.rig.joints.len();
    let mut m: Vec<f64> = b.rig.joints.iter().map(|j| j.mass).collect();
    let mut mc: Vec<Vec3> = (0..n).map(|i| f.com[i] * b.rig.joints[i].mass).collect();
    for i in (1..n).rev() {
        if let Some(p) = b.rig.joints[i].parent {
            let p = p as usize;
            m[p] += m[i];
            mc[p] = mc[p] + mc[i];
        }
    }
    (0..n)
        .map(|i| {
            (
                m[i],
                if m[i] > 0.0 {
                    mc[i] * (1.0 / m[i])
                } else {
                    f.pos[i]
                },
            )
        })
        .collect()
}

fn feet(b: &Body) -> [Option<usize>; 2] {
    let mut out = [None, None];
    for (i, j) in b.rig.joints.iter().enumerate() {
        if let JointRole::Ankle(s) = j.role {
            if (s as usize) < 2 {
                out[s as usize] = Some(i);
            }
        }
    }
    out
}

fn side_ix(b: &Body, i: usize) -> Option<usize> {
    match b.rig.joints[i].role {
        JointRole::Hip(s) | JointRole::Knee(s) | JointRole::Ankle(s) => Some(s as usize),
        _ => None,
    }
}

/// Lowest sole corner y: foot box (rig sole, centred on the foot COM) rotated by the foot pose (PM §8).
fn sole_y(b: &Body, f: &Fk, i: usize) -> f64 {
    let sb = b.rig.sole;
    let mut lo = f64::INFINITY;
    for &x in &[-sb.half_width, sb.half_width] {
        for &z in &[-sb.half_length, sb.half_length] {
            lo = lo.min((f.com[i] + f.q_world[i].rotate(Vec3::new(x, -sb.half_height, z))).y);
        }
    }
    lo
}

/// Sole box corners (world), foot COM centre, bottom face.
fn sole_corners(b: &Body, f: &Fk, i: usize) -> [Vec3; 4] {
    let sb = b.rig.sole;
    let c = |x: f64, z: f64| f.com[i] + f.q_world[i].rotate(Vec3::new(x, -sb.half_height, z));
    [
        c(-sb.half_width, -sb.half_length),
        c(sb.half_width, -sb.half_length),
        c(sb.half_width, sb.half_length),
        c(-sb.half_width, sb.half_length),
    ]
}

/// Ground point of a foot: mean of its sole corners projected to the rest sole height (not the ankle anchor).
fn sole_centre(b: &Body, f: &Fk, i: usize, rest_y: f64) -> Vec3 {
    let k = sole_corners(b, f, i);
    let m = (k[0] + k[1] + k[2] + k[3]) * 0.25;
    Vec3::new(m.x, rest_y, m.z)
}

/// Ankle edge levers from the ankle anchor to the sole box edges, foot frame (x lateral, z forward),
/// derived from SoleBox and the foot COM offset (rig joint `com`): (d_toe, d_heel, d_+x, d_−x).
fn ankle_levers(b: &Body, i: usize) -> (f64, f64, f64, f64) {
    let sb = b.rig.sole;
    let o = b.rig.joints[i].com;
    (
        (o.z + sb.half_length).max(0.0),
        (sb.half_length - o.z).max(0.0),
        (o.x + sb.half_width).max(0.0),
        (sb.half_width - o.x).max(0.0),
    )
}

/// Direction-dependent ankle cap (PM §7 per axis). `want` = torque wanted on the body above (world).
/// In the foot frame, τ = (e×ŷ)·Mg: pitch τ_x < 0 ⇔ COM ahead ⇔ CoP toward toe; roll τ_z > 0 ⇔ CoP toward +x.
/// Each axis is clamped by n·d (n = s·min(Σc,1)·M·g); yaw is not an ankle-balance channel (goes to the hip).
fn ankle_clamp(b: &Body, f: &Fk, i: usize, want: Vec3, n: f64) -> Vec3 {
    let (d_toe, d_heel, d_px, d_nx) = ankle_levers(b, i);
    let q = f.q_world[i];
    let l = q.conj().rotate(want);
    let x = if l.x < 0.0 {
        l.x.max(-n * d_toe)
    } else {
        l.x.min(n * d_heel)
    };
    let z = if l.z > 0.0 {
        l.z.min(n * d_px)
    } else {
        l.z.max(-n * d_nx)
    };
    q.rotate(Vec3::new(x, 0.0, z))
}

/// Per-foot contact share s_i = c_i/Σc (PM §8, lab and pass 6).
fn shares(b: &Body) -> ([f64; 2], f64) {
    let c = [b.contact[0].c as f64, b.contact[1].c as f64];
    let sum = c[0] + c[1];
    (
        if sum > 0.0 {
            [c[0] / sum, c[1] / sum]
        } else {
            [0.0; 2]
        },
        sum,
    )
}

/// Contact, rest heights, COM, capture point, stand_failed. Advances the contact filter once per tick.
pub(crate) fn refresh_state(b: &mut Body, t: &Tuning) {
    let f = fk(b);
    let st = subtrees(b, &f);
    let (m_total, com) = st.first().copied().unwrap_or((0.0, b.root.pos));
    // COM velocity: mass-weighted body velocities when measured (PM §7), else finite difference.
    let com_v = match &f.v_com {
        Some(v) if m_total > 0.0 => {
            (0..v.len()).fold(Vec3::ZERO, |a, i| a + v[i] * b.rig.joints[i].mass) * (1.0 / m_total)
        }
        _ => {
            if b.ticks == 0 {
                Vec3::ZERO
            } else {
                (com - b.last_com) * (1.0 / DT)
            }
        }
    };
    let ft = feet(b);
    let since_on = b.ticks.saturating_sub(b.on_tick);
    // Rest sole height: first value at ragdoll-on, then the mean over ticks 1..=60 (SM §8).
    if since_on == 0 {
        for s in 0..2 {
            if let Some(i) = ft[s] {
                b.rest_sole_h[s] = sole_y(b, &f, i);
            }
        }
        b.rest_acc = [0.0; 2];
        b.rest_n = 0;
    } else if since_on <= t.rest_lock_tick {
        for s in 0..2 {
            if let Some(i) = ft[s] {
                b.rest_acc[s] += sole_y(b, &f, i);
            }
        }
        b.rest_n += 1;
        if since_on == t.rest_lock_tick {
            for s in 0..2 {
                b.rest_sole_h[s] = b.rest_acc[s] / b.rest_n.max(1) as f64;
            }
        }
    }
    let mut down = [false; 2];
    for s in 0..2 {
        if let Some(i) = ft[s] {
            if since_on > 0 || b.support_heights.is_some() {
                let clearance = match b.support_heights {
                    Some(h) => h[s]
                        .map(|y| (sole_y(b, &f, i) - y) as f32)
                        .unwrap_or(f32::INFINITY),
                    None => (sole_y(b, &f, i) - b.rest_sole_h[s]) as f32,
                };
                b.contact[s].step(clearance);
            }
            down[s] = b.contact[s].on;
        }
    }
    // Support centre s = Σ c_i cop_i / Σ c_i, cop = foot projected to its rest height (SM §6).
    let (_, csum) = shares(b);
    let mut cop = Vec3::ZERO;
    if csum > 0.0 {
        for s in 0..2 {
            if let Some(i) = ft[s] {
                cop = cop + sole_centre(b, &f, i, support_y(b, s)) * (b.contact[s].c as f64 / csum);
            }
        }
    }
    if down == [false, false] {
        b.air_ticks = b.air_ticks.saturating_add(1);
    } else {
        b.air_ticks = 0;
    }
    // Capture point (PM §7): z0 = max(x_y − s_y, 0.3).
    let z0 = (com.y - cop.y).max(0.3);
    let k = (z0 / G).sqrt();
    let capture_point = Vec3::new(com.x + com_v.x * k, cop.y, com.z + com_v.z * k);
    let ground = b
        .support_heights
        .map(|h| {
            let hits: Vec<_> = h.into_iter().flatten().collect();
            hits.iter().sum::<f64>() / hits.len().max(1) as f64
        })
        .unwrap_or(0.0);
    if since_on == t.rest_lock_tick {
        b.rest_com_h = Some(com.y - ground);
    }
    // stand_failed (brief §API): COM below 0.6·rest_com_h OR both feet up > 30 ticks. Latched.
    // Before the rest height locks (tick 60): absolute COM y < 0.5 m. Air rule from tick 0.
    // Only judged while Grounded (airborne/ragdoll/grab phases are intentional).
    let failed = b.state.stand_failed
        || (b.mode == Mode::Physical
            && b.body_mode == super::BodyMode::Grounded
            && (b.air_ticks > t.fail_air_ticks
                || match b.rest_com_h {
                    Some(r) => com.y - ground < t.fail_com_frac * r,
                    None => com.y - ground < t.fail_com_abs_m,
                }));
    let joints = (0..b.rig.joints.len())
        .map(|i| JointState {
            q_local: b.q_local[i],
            q_world: f.q_world[i],
            w: f.w_world[i],
            pos: f.pos[i],
            torque: b.torque[i],
            sat: b.sat[i],
        })
        .collect();
    let prev = std::mem::take(&mut b.state);
    b.state = super::BodyState {
        com,
        com_v,
        feet_down: down,
        cop,
        capture_point,
        stand_failed: failed,
        joints,
        ..prev
    };
}

/// Joint PD + feed-forward torques from pose `f` with hold time `dt` (step 1 of write_torques).
fn pd_part(b: &Body, f: &Fk, iw: &[Mat3], t: &Tuning, dt: f64) -> Vec<Vec3> {
    // 1. Stable PD (PM §4) + feed-forward (PM §5). θ = log(R_c* R_c^-1), R_c* = B_p Q_L (O = I).
    //    ω* = B_p ω_L; α* = B_p α_L + ω_p × ω*; τ_ff = I_red(α* + ω̇_p). Tan term optional (scaled rate term).
    let n = b.rig.joints.len();
    let mut tau = vec![Vec3::ZERO; n];
    for i in 0..n {
        let Some(p) = b.rig.joints[i].parent else {
            continue;
        };
        let p = p as usize;
        let (q_t, w_l, a_l) = match b.targets[i] {
            Some(x) => (x.q, x.w, x.alpha_ff),
            None => (Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO),
        };
        let bp = f.q_world[p];
        let theta = ((bp * q_t).normalize() * f.q_world[i].conj()).to_rotvec();
        let w_star = bp.rotate(w_l);
        let a_star = bp.rotate(a_l) + f.w_world[p].cross(w_star);
        let mut w_err = f.w_world[i] - f.w_world[p] - w_star;
        if t.tan_full {
            w_err = w_err * ((t.kd + t.kp * dt) / t.kd);
        } // full Tan term, base kp
        // Each body's inertia is shared by every motored joint touching it (Physics' rule: pelvis 3, thigh 2,
        // shin 2, upper arm 2, end segments 1, from the joint graph), so the summed per-tick Δω on any body
        // stays inside the Stable PD bound.
        let deg = |k: usize| {
            (b.rig
                .joints
                .iter()
                .filter(|j| j.parent == Some(k as u16))
                .count()
                + b.rig.joints[k].parent.is_some() as usize)
                .max(1) as f64
        };
        let scale = |m: &Mat3, d: f64| Mat3(m.0.map(|r| r.map(|x| x / d)));
        let ip = scale(&iw[p], deg(p));
        let ic = scale(&iw[i], deg(i));
        let i_red = Mat3::from_f32(pb::reduced_inertia3(ip.to_f32(), ic.to_f32()));
        // Strike boost (PM §10): Kp·(1 + 0.5·strike) on Arm joints only; strike weight from set_fade.
        let since_on = b.ticks.saturating_sub(b.on_tick);
        let gr = if t.gain_ramp_ticks == 0 {
            1.0
        } else {
            let u = (since_on as f64 / t.gain_ramp_ticks as f64).min(1.0);
            u * u * (3.0 - 2.0 * u)
        };
        let kp = gr
            * if b.rig.joints[i].role.is_arm() {
                t.kp * (1.0 + 0.5 * b.strike_b)
            } else {
                t.kp
            };
        let kd = gr * t.kd;
        // Stable PD dt = torque hold time: DT per tick, or the engine substep when the game runs the PD per substep.
        let pd = Vec3::from_f32(pb::stable_pd3(
            i_red.to_f32(),
            theta.to_f32(),
            w_err.to_f32(),
            kp as f32,
            kd as f32,
            dt as f32,
        ));
        let ff = i_red.mul_vec(a_star); // ω̇_p term dropped (Physics: it amplifies measured noise)
        tau[i] = pd + ff;
    }

    tau
}

fn support_y(b: &Body, side: usize) -> f64 {
    b.support_heights
        .and_then(|h| h[side])
        .unwrap_or(b.rest_sole_h[side])
}

/// Compute this tick's joint torques into `b.torque` and foot forces into `state.foot_force`.
pub(crate) fn write_torques(b: &mut Body, t: &Tuning) {
    let n = b.rig.joints.len();
    let f = fk(b);
    let st = subtrees(b, &f);
    let m_total = st.first().map(|x| x.0).unwrap_or(0.0);
    let mg = m_total * G;
    let ft = feet(b);
    let com = b.state.com;
    let (s_i, csum) = shares(b);
    let mut tau = vec![Vec3::ZERO; n];
    let iw: Vec<Mat3> = (0..n)
        .map(|i| world_inertia(&b.rig.joints[i].inertia, f.q_world[i]))
        .collect();

    let pd0 = pd_part(b, &f, &iw, t, DT);
    tau.clone_from(&pd0);

    let mut foot_force = [Vec3::ZERO; 2];
    let mut grav = vec![Vec3::ZERO; n];
    // Ankle leftover each stance hip received from the balance block (kept out of SIMBICON's inputs).
    let mut hip_left = [Vec3::ZERO; 2];
    if b.mode == Mode::Physical {
        let foot_pt = |s: usize| ft[s].map(|k| sole_centre(b, &f, k, support_y(b, s)));
        // 2. gravity_ff reaction form (PM §6) on every joint; legs use their own foot's cop/share/c, others c = 0.
        for i in 1..n {
            let (m_sub, com_sub) = st[i];
            let (cop, share, c) = match side_ix(b, i).and_then(|s| foot_pt(s).map(|cp| (cp, s))) {
                Some((cp, s)) => (cp, s_i[s], b.contact[s].c),
                None => (f.pos[i], 0.0, 0.0),
            };
            let g_ff = Vec3::from_f32(pb::gravity_ff(
                f.pos[i].to_f32(),
                com_sub.to_f32(),
                m_sub as f32,
                cop.to_f32(),
                share as f32,
                m_total as f32,
                c,
            ));
            tau[i] = tau[i] + g_ff;
            grav[i] = g_ff;
        }
        // 3. Balance (PM §7, SM §6): off when Σc ≤ 0.05; s = contact-weighted cop; ramp from ragdoll-on.
        if csum > 0.05 {
            let since_on = b.ticks.saturating_sub(b.on_tick);
            let centre = [b.state.cop.x as f32, b.state.cop.z as f32];
            let mut bal = Vec3::from_f32(pb::balance_kb(
                m_total as f32,
                com.to_f32(),
                b.state.com_v.to_f32(),
                centre,
                t.kb as f32,
                t.kbd as f32,
                since_on,
            ));
            // COM pull k_com (SM §13), faded by max(strike, acro); re-anchor when the fade returns to 0.
            let fade = b.strike_b.max(b.acro_a);
            if b.com_anchor.is_none()
                || (b.acro_a == 0.0 && b.strike_b == 0.0 && fade_was_active(b))
            {
                b.com_anchor = Some(com);
            }
            if let Some(x0) = b.com_anchor {
                let e = Vec3::new(com.x - x0.x, 0.0, com.z - x0.z);
                bal = bal
                    + e.cross(Vec3::Y) * (t.k_com * mg * (1.0 - fade) * pb::ramp(since_on) as f64);
            }
            // Ankle first (PM §7, SM §6): want_i = c_i·s_i·τ_bal; ankle takes it up to the per-axis edge caps
            // n_i·d (n_i = s_i·min(Σc,1)·M·g, d = d_toe/d_heel/d_side from the sole box); remainder to the
            // stance hip. Both enter negated (sign table).
            for s in 0..2 {
                let want = bal * (b.contact[s].c as f64 * s_i[s]);
                let n_i = s_i[s] * csum.min(1.0) * mg;
                let a = match ft[s] {
                    Some(i) => ankle_clamp(b, &f, i, want, n_i),
                    None => Vec3::ZERO,
                };
                let h = want - a;
                if let Some(i) = ft[s] {
                    tau[i] = tau[i] - a;
                }
                hip_left[s] = -h;
                if let Some(hip) = (0..n).find(|&k| b.rig.joints[k].role == JointRole::Hip(s as u8))
                {
                    tau[hip] = tau[hip] - h;
                }
            }
        }
        // 3b. SIMBICON (phys_gait::simbicon_hips) while the game's gait drives the body: the pelvis has no
        //     motor, so the hips hold it upright and on heading. Swing hip: world-frame PD toward
        //     q_pelvis_want·q_target (upright pelvis frame). Stance hip(s): −τ_trunk − τ_swing, plus the
        //     ankle leftover from the balance block added once (split by contact share in double support).
        if let (Some(g), super::BodyMode::Grounded) = (b.gait, b.body_mode) {
            let hips: [Option<usize>; 2] =
                [0, 1].map(|s| (0..n).find(|&k| b.rig.joints[k].role == JointRole::Hip(s as u8)));
            if let [Some(hl), Some(hr)] = hips {
                let since_on = b.ticks.saturating_sub(b.on_tick);
                let gr = if t.gain_ramp_ticks == 0 {
                    1.0
                } else {
                    let u = (since_on as f64 / t.gain_ramp_ticks as f64).min(1.0);
                    u * u * (3.0 - 2.0 * u)
                };
                let q_pw = Quat::from_rotvec(Vec3::new(0.0, g.heading, 0.0));
                let th_p = (q_pw * f.q_world[0].conj()).normalize().to_rotvec();
                let pd = |i_m: &Mat3, th: Vec3, w: Vec3| {
                    Vec3::from_f32(pb::stable_pd3(
                        i_m.to_f32(),
                        th.to_f32(),
                        w.to_f32(),
                        (gr * t.kp) as f32,
                        (gr * t.kd) as f32,
                        DT as f32,
                    ))
                };
                // Upright: pelvis PD on pitch/roll. Heading: τ_y = −Kp_h·(ψ−ψ_want) − Kd_h·ω_y (Tuning heading_kp/kd).
                let fwd = f.q_world[0].rotate(Vec3::new(0.0, 0.0, 1.0));
                let mut dpsi = fwd.x.atan2(fwd.z) - g.heading;
                while dpsi > std::f64::consts::PI {
                    dpsi -= 2.0 * std::f64::consts::PI;
                }
                while dpsi < -std::f64::consts::PI {
                    dpsi += 2.0 * std::f64::consts::PI;
                }
                let up = pd(
                    &iw[0],
                    Vec3::new(th_p.x, 0.0, th_p.z),
                    Vec3::new(f.w_world[0].x, 0.0, f.w_world[0].z),
                );
                // Heading PD limited to 0.8 of the stance hips' friction yaw limit (μ·n·0.07 m) so it never saturates.
                let yaw_lim: f64 = (0..2)
                    .filter(|&k| g.swing != Some(k))
                    .map(|k| t.mu * s_i[k] * csum.min(1.0) * mg * 0.07)
                    .sum::<f64>()
                    * 0.8;
                let tau_y = (gr * (-t.heading_kp * dpsi - t.heading_kd * f.w_world[0].y))
                    .clamp(-yaw_lim, yaw_lim);
                let tau_pw = Vec3::new(up.x, tau_y, up.z);
                let spine: Vec3 = (0..n)
                    .filter(|&k| {
                        b.rig.joints[k].parent == Some(0)
                            && !matches!(b.rig.joints[k].role, JointRole::Hip(_))
                    })
                    .fold(Vec3::ZERO, |a, k| a + tau[k]);
                // Keep the hips' gravity feed-forward (upper-body weight) in the pelvis budget: with it the
                // pelvis net torque is τ_pw − (g_L + g_R), exactly what the plain PD hips gave it, plus the PD.
                let tau_trunk = tau_pw + spine - grav[hl] - grav[hr];
                let mut hip_in = [Vec3::ZERO; 2];
                if let Some(sw) = g.swing {
                    let h = [hl, hr][sw];
                    let q_t = g.swing_q.unwrap_or_else(|| {
                        (q_pw * b.targets[h].map(|x| x.q).unwrap_or(Quat::IDENTITY)).normalize()
                    });
                    let th = (q_t * f.q_world[h].conj()).normalize().to_rotvec();
                    hip_in[sw] = pd(&iw[h], th, f.w_world[h]) + grav[h];
                }
                let share = [b.contact[0].c as f64, b.contact[1].c as f64];
                let mut out = match g.swing {
                    Some(_) => crate::phys_gait::simbicon_hips(tau_trunk, hip_in, g.swing, share),
                    // Double support: keep the PD hips (they hold leg posture against the pelvis) and only
                    // correct their sum to −τ_trunk; the correction is split by contact share.
                    // Double support: keep the PD hips (they hold leg posture and pitch/roll, which a pelvis-only
                    // PD cannot) and correct only the yaw sum to −τ_trunk,y (heading PD), split by contact share.
                    None => {
                        let base = [tau[hl] - hip_left[0], tau[hr] - hip_left[1]];
                        let e = tau_trunk + base[0] + base[1];
                        let c = crate::phys_gait::simbicon_hips(
                            Vec3::new(0.0, e.y, 0.0),
                            [Vec3::ZERO; 2],
                            None,
                            share,
                        );
                        [base[0] + c[0], base[1] + c[1]]
                    }
                };
                match g.swing {
                    Some(sw) => {
                        out[1 - sw] = out[1 - sw] + hip_left[1 - sw] + hip_left[sw];
                    }
                    None => {
                        let (c0, c1) = (b.contact[0].c as f64, b.contact[1].c as f64);
                        let cs = c0 + c1;
                        let (a0, a1) = if cs > 1e-9 {
                            (c0 / cs, c1 / cs)
                        } else {
                            (0.5, 0.5)
                        };
                        let tot = hip_left[0] + hip_left[1];
                        out[0] = out[0] + tot * a0;
                        out[1] = out[1] + tot * a1;
                    }
                }
                // Stance hip yaw limited by what the foot can hold in friction: μ·n·0.07 m.
                for (k, o) in out.iter_mut().enumerate() {
                    if g.swing == Some(k) {
                        continue;
                    }
                    let lim = t.mu * s_i[k] * csum.min(1.0) * mg * 0.07;
                    o.y = o.y.clamp(-lim, lim);
                }
                tau[hl] = out[0];
                tau[hr] = out[1];
            }
        }
        // 4. Foot damping (PM §8): F = −c·k·v_foot,xz (absolute), |F| ≤ μ·s·min(Σc,1)·M·g. Output for the game.
        for s in 0..2 {
            let Some(i) = ft[s] else { continue };
            let v = match &f.v_com {
                Some(v) => v[i],
                None => b.prev_foot_com[s]
                    .map(|p| (f.com[i] - p) * (1.0 / DT))
                    .unwrap_or(Vec3::ZERO),
            };
            let fv = Vec3::new(v.x, 0.0, v.z) * (-(b.contact[s].c as f64) * t.foot_damp);
            foot_force[s] = fv.clamp_length(t.mu * s_i[s] * csum.min(1.0) * mg);
        }
        b.prev_fade = b.strike_b.max(b.acro_a);
        for s in 0..2 {
            b.prev_foot_com[s] = ft[s].map(|i| f.com[i]);
        }
    }

    // Everything but the PD part is held for the whole tick when the game re-runs the PD per substep.
    b.residual = (0..n).map(|i| tau[i] - pd0[i]).collect();
    // 5. One clamp per joint: rig cap (thighs ≥ M·g·0.12), then the fault cap 3·M·g; non-finite → 0 (PM §11).
    let fault_cap = t.fault_lever_m * mg;
    let mut faults = 0;
    let mut peak = 0.0f64;
    for i in 0..n {
        let cap = b.rig.joints[i].torque_cap.max(1.0);
        let c = side_ix(b, i).map(|s| b.contact[s].c).unwrap_or(1.0);
        b.sat[i] = pb::sat_ratio(tau[i].to_f32(), cap as f32, c).unwrap_or(0.0) as f64;
        peak = peak.max(b.sat[i]);
        if !tau[i].is_finite() {
            faults += 1;
            b.torque[i] = Vec3::ZERO;
            continue;
        }
        if tau[i].length() > fault_cap {
            faults += 1;
        }
        // Motor weights (api): body × joint × mode (ragdoll 0, recovering ramp).
        let w = (b.motor_w_body * b.motor_w[i] * b.mode_w).clamp(0.0, 1.0);
        b.torque[i] = tau[i].clamp_length(cap.min(fault_cap.max(1.0))) * w;
    }
    for s in 0..2 {
        foot_force[s] = foot_force[s] * b.mode_w.clamp(0.0, 1.0);
    }
    b.prev_w_world = f.w_world.clone();
    b.state.sat_peak = b.state.sat_peak.max(peak);
    b.state.faults = faults;
    b.state.foot_force = foot_force;
    b.state.foot_share = if csum > 0.05 { s_i } else { [0.0; 2] };
    for i in 0..n {
        if let Some(j) = b.state.joints.get_mut(i) {
            j.torque = b.torque[i];
            j.sat = b.sat[i];
        }
    }
}

fn fade_was_active(b: &Body) -> bool {
    b.prev_fade > 0.0
}

/// Lab fallback integrator (unsynced bodies only). Torques come from write_torques with dt = DT and are
/// held over n_s substeps of h = DT/n_s here, mirroring Avian.: own-segment gravity + motor torque, n_s substeps,
/// 50 rad/s cap, NaN reset. Root pinned while a foot is down; falls to the floor otherwise. Ignores the
/// −τ parent reaction and contact forces, so it cannot reproduce Avian stand results.
fn integrate(b: &mut Body, t: &Tuning) {
    let n = b.rig.joints.len();
    let h = DT / t.substeps.max(1) as f64;
    for _ in 0..t.substeps.max(1) {
        let f = fk(b);
        for i in 1..n {
            let j = &b.rig.joints[i];
            let p = j.parent.unwrap_or(0) as usize;
            let grav = if b.mode == Mode::Physical {
                (f.com[i] - f.pos[i]).cross(Vec3::new(0.0, -j.mass * G, 0.0))
            } else {
                Vec3::ZERO
            };
            let iw = world_inertia(&j.inertia, f.q_world[i]).inverse();
            let alpha_w = iw
                .map(|m| m.mul_vec(b.torque[i] + grav))
                .unwrap_or(Vec3::ZERO);
            let mut w = b.w[i] + f.q_world[p].conj().rotate(alpha_w * h);
            if b.impulses[i] != Vec3::ZERO {
                w = w + f.q_world[p]
                    .conj()
                    .rotate(iw.map(|m| m.mul_vec(b.impulses[i])).unwrap_or(Vec3::ZERO));
            }
            w = w.clamp_length(t.w_cap);
            if !w.is_finite() {
                w = Vec3::ZERO;
            }
            b.w[i] = w;
            b.q_local[i] = b.q_local[i].integrate(w, h);
        }
        b.impulses.iter_mut().for_each(|x| *x = Vec3::ZERO);
        if b.mode == Mode::Physical && b.state.feet_down == [false, false] {
            b.root_v = b.root_v + Vec3::new(0.0, -G * h, 0.0);
        } else {
            b.root_v = Vec3::new(b.root_v.x * 0.9, 0.0, b.root_v.z * 0.9);
        }
        b.root.pos = b.root.pos + b.root_v * h;
        if b.root.pos.y < 0.0 {
            b.root.pos.y = 0.0;
            b.root_v = Vec3::ZERO;
        }
    }
}

/// One 120 Hz tick for one body. Synced: state from measurement → torques (no integration).
/// Unsynced (lab fallback): torques → internal stepper → state.
pub(crate) fn tick(b: &mut Body, t: &Tuning) {
    if b.measured.is_some() {
        refresh_state(b, t);
        super::update_mode(b, t);
        write_torques(b, t);
        b.last_com = b.state.com;
        b.ticks += 1;
        b.last_measured = b.measured.take();
        b.impulses.iter_mut().for_each(|x| *x = Vec3::ZERO); // the game applies impulses to its bodies directly
    } else {
        b.last_measured = None;
        write_torques(b, t);
        integrate(b, t);
        b.last_com = b.state.com;
        b.ticks += 1;
        refresh_state(b, t);
        super::update_mode(b, t);
    }
}
