//! Scenes "lie" (rung 0 gate: motors off, body flat, measure slide) and "stand" (T-pose torque writer).
//! One torque writer per joint, Avian joint motors unused (SphericalJoint has none enabled):
//!   tau = I*alpha* + Kp*theta_err + Kd*(omega* - (omega_c - omega_p)) + gravity_ff
//! with alpha* = omega* = 0 (hold rest pose), so the I*alpha* term is zero here.
//! +tau on child, -tau on parent (or child only with --unpaired). One clamp per joint.
use crate::{Args, DT};
use avian3d::prelude::*;
use bevy::{prelude::*, time::TimeUpdateStrategy};
use std::f32::consts::FRAC_PI_2;
use std::io::Write;
use std::time::Duration;

const G: f32 = 9.81;

pub fn make_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugins::default()))
        .insert_resource(Time::<Fixed>::from_seconds(DT as f64))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            DT as f64,
        )))
        .insert_resource(Gravity(Vec3::NEG_Y * G));
    app.finish();
    app.cleanup();
    app
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Leg,
    Arm,
    Spine,
}

struct SegDef {
    name: &'static str,
    parent: Option<usize>,
    mass: f32,
    pos: Vec3,
    rot: Quat,
    shape: Shape,
    anchor: Vec3,
    kind: Kind,
}
#[derive(Clone, Copy)]
enum Shape {
    Box(Vec3),
    Cap(f32, f32),
    Ball(f32),
}

fn defs(rest_offset: bool) -> Vec<SegDef> {
    // rest offset: feet start 5 mm above the floor (on) or exactly touching it (off)
    let lift = if rest_offset { 0.005 } else { 0.0 };
    let side = Quat::from_rotation_z(FRAC_PI_2);
    let mut v = vec![
        SegDef {
            name: "pelvis",
            parent: None,
            mass: 10.0,
            pos: Vec3::new(0.0, 1.0, 0.0),
            rot: Quat::IDENTITY,
            shape: Shape::Box(Vec3::new(0.3, 0.15, 0.2)),
            anchor: Vec3::ZERO,
            kind: Kind::Spine,
        },
        SegDef {
            name: "torso",
            parent: Some(0),
            mass: 20.0,
            pos: Vec3::new(0.0, 1.3, 0.0),
            rot: Quat::IDENTITY,
            shape: Shape::Box(Vec3::new(0.3, 0.4, 0.2)),
            anchor: Vec3::new(0.0, 1.08, 0.0),
            kind: Kind::Spine,
        },
        SegDef {
            name: "head",
            parent: Some(1),
            mass: 5.0,
            pos: Vec3::new(0.0, 1.62, 0.0),
            rot: Quat::IDENTITY,
            shape: Shape::Ball(0.11),
            anchor: Vec3::new(0.0, 1.51, 0.0),
            kind: Kind::Spine,
        },
    ];
    for s in [-1.0f32, 1.0] {
        let x = 0.1 * s;
        let base = v.len();
        v.push(SegDef {
            name: if s < 0.0 { "thigh_r" } else { "thigh_l" },
            parent: Some(0),
            mass: 8.0,
            pos: Vec3::new(x, 0.72, 0.0),
            rot: Quat::IDENTITY,
            shape: Shape::Cap(0.06, 0.28),
            anchor: Vec3::new(x, 0.92, 0.0),
            kind: Kind::Leg,
        });
        v.push(SegDef {
            name: if s < 0.0 { "shin_r" } else { "shin_l" },
            parent: Some(base),
            mass: 4.0,
            pos: Vec3::new(x, 0.30, 0.0),
            rot: Quat::IDENTITY,
            shape: Shape::Cap(0.05, 0.32),
            anchor: Vec3::new(x, 0.52, 0.0),
            kind: Kind::Leg,
        });
        v.push(SegDef {
            name: if s < 0.0 { "foot_r" } else { "foot_l" },
            parent: Some(base + 1),
            mass: 1.2,
            pos: Vec3::new(x, 0.03, 0.04),
            rot: Quat::IDENTITY,
            shape: Shape::Box(Vec3::new(0.1, 0.06, 0.24)),
            anchor: Vec3::new(x, 0.08, 0.0),
            kind: Kind::Leg,
        });
        v.push(SegDef {
            name: if s < 0.0 { "uarm_r" } else { "uarm_l" },
            parent: Some(1),
            mass: 2.5,
            pos: Vec3::new(0.32 * s, 1.45, 0.0),
            rot: side,
            shape: Shape::Cap(0.05, 0.16),
            anchor: Vec3::new(0.17 * s, 1.45, 0.0),
            kind: Kind::Arm,
        });
        v.push(SegDef {
            name: if s < 0.0 { "farm_r" } else { "farm_l" },
            parent: Some(base + 3),
            mass: 1.5,
            pos: Vec3::new(0.6 * s, 1.45, 0.0),
            rot: side,
            shape: Shape::Cap(0.04, 0.18),
            anchor: Vec3::new(0.47 * s, 1.45, 0.0),
            kind: Kind::Arm,
        });
    }
    for d in v.iter_mut() {
        d.pos.y += lift;
        d.anchor.y += lift;
    }
    v
}

struct Seg {
    e: Entity,
    parent: Option<usize>,
    mass: f32,
    rest_rel: Quat,
    la_child: Vec3,
    kind: Kind,
    subtree: Vec<usize>,
    is_foot: bool,
    foot_half_h: f32,
    side: usize,
    name_thigh: bool,
}

#[derive(Resource)]
struct Lab {
    segs: Vec<Seg>,
    motors: bool,
    paired: bool,
    kp: f32,
    kd: f32,
    k: f32,
    leg_floor: f32,
    gff: bool,
    clamp: bool,
    scaled: bool,
    cp: bool,
    tan_full: bool,
    com_pull: f32,
    mu: f32,
    fault_ticks: u32,
    ired_scalar: bool,
    damp_rel: bool,
    ired_diff_max: f32,
    foot_damp: f32,
    anchor_k: f32,
    bal_rate: f32,
    bal_lp: f32,
    bal_prev: Vec3,
    anchors: [Option<Vec3>; 2],
    pending_f: Vec<(Entity, Vec3)>,
    flat_feet: bool,
    old_gff: bool,
    geom_edge: bool,
    bal_sign: f32,
    kb: f32,
    kbd: f32,
    bal_log: String,
    sole_now: [f32; 2],
    foot_tilt: [f32; 2],
    hip_err: f32,
    filter: bool,
    balance: bool,
    rest_offset: bool,
    first_below: Option<u32>,
    tick: u32,
    total_mass: f32,
    // contact
    c: [f32; 2],
    on: [bool; 2],
    flips: u32,
    // metrics
    com0: Vec3,
    com_min: f32,
    com_final: f32,
    drift_max: f32,
    drift_final: f32,
    peak_sat: f32,
    peak_sat_foot_down: f32,
    low_limit_ticks_foot_down: u32,
    sum_tau_max: f32,
    sum_tau_mean: f64,
    sole_rest: Vec<f32>,
    fell_through: bool,
    com_samples: Vec<(u32, f32)>,
    last: String,
    log: Option<std::fs::File>,
    pending: Vec<(Entity, Vec3)>,
}

/// Second half of the single writer: applies the torques computed this tick (split only for borrow rules).
fn apply_torques(mut lab: ResMut<Lab>, mut forces: Query<Forces>) {
    for (e, t) in lab.pending.drain(..) {
        if let Ok(mut f) = forces.get_mut(e) {
            f.apply_torque(t);
        }
    }
    for (e, v) in lab.pending_f.drain(..) {
        if let Ok(mut f) = forces.get_mut(e) {
            f.apply_force(v);
        }
    }
}

fn world_com(
    segs: &[Seg],
    idx: &[usize],
    q: &Query<(&Position, &Rotation, &LinearVelocity, &AngularVelocity)>,
) -> (Vec3, f32) {
    let (mut s, mut m) = (Vec3::ZERO, 0.0);
    for &i in idx {
        if let Ok((p, ..)) = q.get(segs[i].e) {
            s += p.0 * segs[i].mass;
            m += segs[i].mass;
        }
    }
    (s / m.max(1e-6), m)
}

/// Runs in FixedUpdate, i.e. before the physics step in FixedPostUpdate.
fn torque_writer(
    mut lab: ResMut<Lab>,
    q: Query<(&Position, &Rotation, &LinearVelocity, &AngularVelocity)>,
    inertia: Query<&ComputedAngularInertia>,
) {
    // contact filter per foot: raw on < 1 cm, off > 2.5 cm (hysteresis), low-pass 0.25 per tick
    let feet: Vec<usize> = lab
        .segs
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_foot)
        .map(|(i, _)| i)
        .collect();
    for (fi, &i) in feet.iter().enumerate() {
        let s = &lab.segs[i];
        let Ok((p, r, ..)) = q.get(s.e) else { continue };
        // lowest of 4 sole corners
        let he = Vec3::new(0.05, s.foot_half_h, 0.12);
        let mut sole = f32::MAX;
        for cx in [-1.0, 1.0] {
            for cz in [-1.0, 1.0] {
                sole = sole.min((p.0 + r.0 * Vec3::new(he.x * cx, -he.y, he.z * cz)).y);
            }
        }
        lab.sole_now[fi] = sole;
        lab.foot_tilt[fi] = (r.0 * Vec3::Y).angle_between(Vec3::Y).to_degrees();
        let was = lab.on[fi];
        let now = if !lab.filter {
            sole < 0.01
        } else if was {
            sole <= 0.025
        } else {
            sole < 0.01
        };
        if now != was {
            lab.flips += 1;
        }
        lab.on[fi] = now;
        lab.c[fi] = if lab.filter {
            lab.c[fi] + 0.25 * ((now as u8 as f32) - lab.c[fi])
        } else {
            now as u8 as f32
        };
        if now {
            lab.sole_rest.push(sole);
        }
    }
    // (a) tangential damping and (b) touchdown anchor spring on stance feet. Modelled as static ground
    // friction: the reaction acts on the static floor, capped at mu * (contact share of body weight).
    let mut ff = Vec::new();
    let csum0 = (lab.c[0] + lab.c[1]).max(1e-3);
    let mut vcom = Vec3::ZERO;
    for sg in &lab.segs {
        if let Ok((_, _, lv, _)) = q.get(sg.e) {
            vcom += lv.0 * sg.mass;
        }
    }
    let vcom = vcom / lab.total_mass;
    for (fi, &i) in feet.iter().enumerate() {
        let Ok((p, _, v, _)) = q.get(lab.segs[i].e) else {
            continue;
        };
        let c = lab.c[fi];
        if lab.on[fi] && c > 0.5 {
            if lab.anchors[fi].is_none() {
                lab.anchors[fi] = Some(p.0);
            }
        } else if c < 0.2 {
            lab.anchors[fi] = None;
        }
        let mut f = Vec3::ZERO;
        if lab.foot_damp > 0.0 {
            let rel = if lab.damp_rel { v.0 - vcom } else { v.0 };
            f -= c * lab.foot_damp * rel.with_y(0.0);
        }
        if lab.anchor_k > 0.0 {
            if let Some(a) = lab.anchors[fi] {
                f -= c * lab.anchor_k * (p.0 - a).with_y(0.0);
            }
        }
        // fault cap (log only): a requested foot force above 3*M*g means something is wrong
        if f.length() > 3.0 * lab.total_mass * G {
            lab.fault_ticks += 1;
        }
        let cap = lab.mu * (c / csum0) * csum0.min(1.0) * lab.total_mass * G;
        if f.length() > cap {
            f *= cap / f.length();
        }
        if f != Vec3::ZERO {
            ff.push((lab.segs[i].e, f));
        }
    }
    lab.pending_f = ff;
    if !lab.motors {
        lab.tick += 1;
        return;
    }
    let all: Vec<usize> = (0..lab.segs.len()).collect();
    let (com_all, _) = world_com(&lab.segs, &all, &q);
    let csum = (lab.c[0] + lab.c[1]).max(1e-3);
    let foot_down = lab.on[0] || lab.on[1];
    let mut torques: Vec<(Entity, Vec3)> = Vec::new();
    // balance (ankles first, then stance hips): torque to bring the COM back over the support centre
    let (mut bal_ankle, mut bal_hip) = (Vec3::ZERO, Vec3::ZERO);
    if lab.balance && csum > 0.05 {
        let mut sup = Vec3::ZERO;
        let mut w = 0.0;
        for (fi, &i) in feet.iter().enumerate() {
            if let Ok((p, ..)) = q.get(lab.segs[i].e) {
                sup += p.0 * lab.c[fi];
                w += lab.c[fi];
            }
        }
        let sup = sup / w.max(1e-6);
        let mut v = Vec3::ZERO;
        for s in &lab.segs {
            if let Ok((_, _, lv, _)) = q.get(s.e) {
                v += lv.0 * s.mass;
            }
        }
        let v = v / lab.total_mass;
        let e = Vec3::new(com_all.x - sup.x, 0.0, com_all.z - sup.z);
        let ve = Vec3::new(v.x, 0.0, v.z);
        let h = (com_all.y - sup.y).max(0.3);
        let w0 = (G / h).sqrt();
        // (2) capture point: xi = x + v/omega0; drive xi back over the support centre
        let err = if lab.cp { e + ve / w0 } else { e };
        let want_raw = lab.bal_sign
            * (if lab.cp {
                err * lab.kb
            } else {
                e * lab.kb + ve * lab.kbd
            })
            .cross(Vec3::Y)
            * lab.total_mass
            * G;
        // (c) smooth the balance torque: low-pass then per-tick rate limit (N*m per tick)
        // weak COM-return pull toward the start point (xz), same units as kb (multiplies M*g)
        let want_raw = if lab.com_pull > 0.0 && lab.tick > 1 {
            let d = Vec3::new(com_all.x - lab.com0.x, 0.0, com_all.z - lab.com0.z);
            want_raw + lab.bal_sign * (d * lab.com_pull).cross(Vec3::Y) * lab.total_mass * G
        } else {
            want_raw
        };
        let mut want = lab.bal_prev + (want_raw - lab.bal_prev) * lab.bal_lp;
        if lab.bal_rate > 0.0 {
            let d = want - lab.bal_prev;
            if d.length() > lab.bal_rate {
                want = lab.bal_prev + d * (lab.bal_rate / d.length());
            }
        }
        lab.bal_prev = want;
        // (3) ankle limit: distance from support centre to the toe or heel edge along the push direction
        let d_edge = if lab.geom_edge {
            let mut d = 0.0f32;
            let mut wsum = 0.0;
            for (fi, &i) in feet.iter().enumerate() {
                if lab.c[fi] < 0.05 {
                    continue;
                }
                if let Ok((p, r, ..)) = q.get(lab.segs[i].e) {
                    let fwd = (r.0 * Vec3::Z).with_y(0.0).normalize_or_zero();
                    let side = (r.0 * Vec3::X).with_y(0.0).normalize_or_zero();
                    let off = Vec2::new((p.0 - sup).dot(side), (p.0 - sup).dot(fwd));
                    // CoP must move toward the error side; half extents 0.05 x 0.12 around the foot centre
                    let dir = Vec2::new(err.dot(side), err.dot(fwd)).normalize_or_zero();
                    let ext = Vec2::new(0.05, 0.12);
                    let reach = if dir == Vec2::ZERO {
                        0.0
                    } else {
                        ((ext.x + off.x * dir.x.signum()) / dir.x.abs().max(1e-3))
                            .min((ext.y + off.y * dir.y.signum()) / dir.y.abs().max(1e-3))
                    };
                    d += lab.c[fi] * reach.max(0.0);
                    wsum += lab.c[fi];
                }
            }
            d / wsum.max(1e-6)
        } else {
            0.12
        };
        let cap = csum.min(1.0) * lab.total_mass * G * d_edge;
        let n = want.length();
        bal_ankle = if n > cap { want * (cap / n) } else { want };
        // sign check: torque on the body above, rotating about the ankle, moves the COM by (want x up) direction
        let push = want.cross(Vec3::Y).with_y(0.0);
        let sign = if push.length() > 1e-6 && err.length() > 1e-6 {
            -push.normalize().dot(err.normalize())
        } else {
            0.0
        };
        lab.bal_log = format!(
            ",\"com_xz\":[{:.4},{:.4}],\"sup\":[{:.4},{:.4}],\"err\":[{:.4},{:.4}],\"v\":[{:.3},{:.3}],\"bal_ankle\":{:.2},\"bal_hip\":{:.2},\"d_edge\":{d_edge:.3},\"push_toward_support\":{sign:.2}",
            com_all.x,
            com_all.z,
            sup.x,
            sup.z,
            err.x,
            err.z,
            ve.x,
            ve.z,
            bal_ankle.length(),
            (want - bal_ankle).length()
        );
        bal_hip = want - bal_ankle;
    }
    let mut max_sat = 0.0f32;
    let mut ired_dmax = lab.ired_diff_max;
    let (mut hip_err_now, mut hip_axis, mut hip_tau_dir) = (0.0f32, Vec3::ZERO, 0.0f32);
    let _ = hip_axis;
    let mut low_limit = false;
    for ci in 0..lab.segs.len() {
        let s = &lab.segs[ci];
        let Some(pi) = s.parent else { continue };
        let (Ok((pc, rc, _, wc)), Ok((_, rp, _, wp))) = (q.get(s.e), q.get(lab.segs[pi].e)) else {
            continue;
        };
        let anchor = pc.0 + rc.0 * s.la_child;
        // theta_err: rotation taking child to its target (parent * rest_rel), world frame
        let mut target = rp.0 * s.rest_rel;
        // stance foot: hold the foot flat in the world (its spawn orientation) instead of relative to the shin
        if lab.flat_feet && s.is_foot {
            target = target.slerp(Quat::IDENTITY, lab.c[s.side]);
        }
        let mut qe = target * rc.0.inverse();
        if qe.w < 0.0 {
            qe = -qe;
        }
        let (axis, ang) = qe.to_axis_angle();
        let theta = if ang.is_finite() && ang > 1e-9 {
            axis * ang
        } else {
            Vec3::ZERO
        };
        let rel_w = wc.0 - wp.0;
        let (com_sub, m_sub) = world_com(&lab.segs, &s.subtree, &q);
        let r_sub = com_sub - anchor;
        let swing_ff = r_sub.cross(Vec3::Y * m_sub * G); // hold child subtree up
        let m_above = lab.total_mass - m_sub;
        let com_above = (com_all * lab.total_mass - com_sub * m_sub) / m_above.max(1e-6);
        let r_above = com_above - anchor;
        let (c, share) = if s.kind == Kind::Leg {
            let side = s.side;
            (lab.c[side], lab.c[side] / csum)
        } else {
            (0.0, 0.0)
        };
        let stance_ff = r_above.cross(Vec3::NEG_Y * m_above * G * share);
        // FIX (4): stance gravity_ff from the ground reaction at this leg's foot, not from the mass above.
        // The distal chain is held by tau + ground force (share of M*g at the foot centre) + its own weight:
        //   tau = r_sub x (m_sub*g up) - (cop - anchor) x (share*M*g up)
        let new_ff = if s.kind == Kind::Leg {
            let foot = lab
                .segs
                .iter()
                .find(|f| f.is_foot && f.side == s.side)
                .map(|f| f.e);
            let cop = foot
                .and_then(|e| q.get(e).ok())
                .map(|(p, ..)| p.0.with_y(0.0))
                .unwrap_or(anchor);
            swing_ff - (cop - anchor).cross(Vec3::Y * share * csum.min(1.0) * lab.total_mass * G)
        } else {
            swing_ff
        };
        let gff = if !lab.gff {
            Vec3::ZERO
        } else if lab.old_gff {
            (1.0 - c) * swing_ff + c * stance_ff
        } else {
            new_ff
        };
        // I*alpha* with Avian's world inertia; alpha* = 0 when holding a static pose
        let alpha_star = Vec3::ZERO;
        let i_alpha = inertia
            .get(s.e)
            .map(|ia| ia.rotated(rc.0).tensor() * alpha_star)
            .unwrap_or(Vec3::ZERO);
        // balance: torque on the body above = -tau, so tau = -want, split by stance share
        let bal = if s.kind == Kind::Leg && lab.balance {
            if s.is_foot {
                -bal_ankle * share
            } else if pi == 0 {
                -bal_hip * share
            } else {
                Vec3::ZERO
            }
        } else {
            Vec3::ZERO
        };
        let pd = if lab.scaled {
            // FIX: inertia-scaled stable PD. kp = w^2 (1/s^2), kd = 2*zeta*w (1/s). Subtree inertia about the joint
            // (parallel axis), implicit in dt: tau = I_joint (kp*theta - kd*w_rel) / (1 + kd*dt + kp*dt^2).
            let mut ij = Mat3::ZERO;
            // Avian divides a body torque by that body's own inertia, so size the PD by the child body alone
            for &b in &s.subtree[..1] {
                let sb = &lab.segs[b];
                if let (Ok(ia), Ok((pb, rb, ..))) = (inertia.get(sb.e), q.get(sb.e)) {
                    let t = ia.rotated(rb.0).tensor();
                    ij += Mat3::from_cols(t * Vec3::X, t * Vec3::Y, t * Vec3::Z);
                    let r = Vec3::ZERO;
                    let _ = pb;
                    ij += sb.mass
                        * (Mat3::from_diagonal(Vec3::splat(r.length_squared()))
                            - Mat3::from_cols(r * r.x, r * r.y, r * r.z));
                }
            }
            // parent side: the parent body alone about the anchor (spherical joints pass no torque further up)
            let mut ip = Mat3::ZERO;
            let pb = &lab.segs[pi];
            if let (Ok(ia), Ok((ppos, prot, ..))) = (inertia.get(pb.e), q.get(pb.e)) {
                let t = ia.rotated(prot.0).tensor();
                if t * Vec3::X != Vec3::ZERO {
                    ip = Mat3::from_cols(t * Vec3::X, t * Vec3::Y, t * Vec3::Z);
                    let r = Vec3::ZERO;
                    let _ = ppos;
                    ip += pb.mass
                        * (Mat3::from_diagonal(Vec3::splat(r.length_squared()))
                            - Mat3::from_cols(r * r.x, r * r.y, r * r.z));
                }
            }
            // reduced (joint-space) inertia; a static parent counts as infinite inertia
            let ired = if ip.determinant().abs() > 1e-12 {
                (ij.inverse() + ip.inverse()).inverse()
            } else {
                ij
            };
            let den = 1.0 + lab.kd * DT + lab.kp * DT * DT;
            // Tan, Liu & Turk 2011 s3.1: full numerator -(kd + kp*dt)*w_rel (switch); shipped form drops kp*dt
            let kdn = if lab.tan_full {
                lab.kd + lab.kp * DT
            } else {
                lab.kd
            };
            let u = (lab.kp * theta - kdn * rel_w) / den;
            // scalar check about the error axis: I_red = I_p*I_c/(I_p+I_c), static parent -> I_c
            let n = if lab.ired_scalar {
                u.normalize_or_zero()
            } else if theta.length() > 1e-6 {
                theta.normalize()
            } else {
                u.normalize_or_zero()
            };
            let (sc, sp) = (n.dot(ij * n), n.dot(ip * n));
            let ired_s = if sp > 1e-9 { sp * sc / (sp + sc) } else { sc };
            if n != Vec3::ZERO && ired_s > 1e-9 {
                let d = (n.dot(ired * n) - ired_s).abs() / ired_s;
                if d > ired_dmax {
                    ired_dmax = d;
                }
            }
            if lab.ired_scalar {
                u * ired_s
            } else {
                ired * u
            }
        } else {
            lab.kp * theta + lab.kd * (Vec3::ZERO - rel_w)
        };
        let mut tau = i_alpha + pd + gff + bal;
        // one clamp per joint; lever arms are full 3D joint-to-COM distances
        let limit = match s.kind {
            Kind::Leg => (lab.k
                * G
                * (c * share * m_above * r_above.length() + (1.0 - c) * m_sub * r_sub.length()))
            .max(lab.leg_floor),
            _ => 1.5 * m_sub * G * r_sub.length() + 5.0,
        };
        let n = tau.length();
        let sat = n / limit.max(1.0);
        if foot_down && s.kind == Kind::Leg && limit < 1.0 {
            low_limit = true;
        }
        max_sat = max_sat.max(sat);
        if lab.segs[ci].name_thigh && lab.segs[ci].side == 0 {
            hip_err_now = ang.to_degrees();
            hip_axis = theta.normalize_or_zero();
            hip_tau_dir = tau.normalize_or_zero().dot(hip_axis);
        }
        if std::env::var("GB_DEBUG").is_ok() && lab.tick < 40 && lab.tick % 4 == 0 {
            eprintln!(
                "t{} j{} ang{:.1} relw{:.2} tau{:.1} gff{:.1}",
                lab.tick,
                ci,
                ang.to_degrees(),
                rel_w.length(),
                n,
                gff.length()
            );
        }
        if lab.clamp && n > limit {
            tau *= limit / n;
        }
        torques.push((s.e, tau));
        if lab.paired {
            torques.push((lab.segs[pi].e, -tau));
        }
    }
    let mut sum = Vec3::ZERO;
    for (_, t) in &torques {
        sum += *t;
    }
    lab.pending = torques;
    let st = sum.length();
    lab.ired_diff_max = ired_dmax;
    lab.peak_sat = lab.peak_sat.max(max_sat);
    if foot_down {
        lab.peak_sat_foot_down = lab.peak_sat_foot_down.max(max_sat);
    }
    if low_limit {
        lab.low_limit_ticks_foot_down += 1;
    }
    lab.sum_tau_max = lab.sum_tau_max.max(st);
    lab.sum_tau_mean += st as f64;
    lab.hip_err = hip_err_now;
    lab.last = format!(
        "{}\"hip_err_deg\":{hip_err_now:.3},\"hip_tau_dot_err\":{hip_tau_dir:.3},\"sat\":{max_sat:.4},\"sum_tau\":{st:.6},\"low_limit\":{low_limit},\"sole\":[{:.4},{:.4}],\"tilt\":[{:.1},{:.1}]{}",
        "",
        lab.sole_now[0],
        lab.sole_now[1],
        lab.foot_tilt[0],
        lab.foot_tilt[1],
        lab.bal_log.clone()
    );
    lab.tick += 1;
}

/// Runs after the physics step.
fn metrics(
    mut lab: ResMut<Lab>,
    q: Query<(&Position, &Rotation, &LinearVelocity, &AngularVelocity)>,
) {
    let all: Vec<usize> = (0..lab.segs.len()).collect();
    let (com, _) = world_com(&lab.segs, &all, &q);
    let t = lab.tick;
    if t == 1 {
        lab.com0 = com;
        lab.com_min = com.y;
    }
    let drift = Vec2::new(com.x - lab.com0.x, com.z - lab.com0.z).length();
    lab.com_min = lab.com_min.min(com.y);
    lab.com_final = com.y;
    lab.drift_max = lab.drift_max.max(drift);
    lab.drift_final = drift;
    if com.y < -1.0 {
        lab.fell_through = true;
    }
    if com.y < -0.1 && lab.first_below.is_none() {
        lab.first_below = Some(t);
    }
    if t % 120 == 0 {
        lab.com_samples.push((t, com.y));
    }
    let line = format!(
        "{{\"tick\":{t},\"com\":[{:.5},{:.5},{:.5}],\"drift\":{drift:.5},\"c\":[{:.3},{:.3}],\"on\":[{},{}],\"flips\":{},{}}}",
        com.x,
        com.y,
        com.z,
        lab.c[0],
        lab.c[1],
        lab.on[0],
        lab.on[1],
        lab.flips,
        if lab.last.is_empty() {
            "\"motors\":false".into()
        } else {
            lab.last.clone()
        }
    );
    if let Some(f) = lab.log.as_mut() {
        let _ = writeln!(f, "{line}");
    }
}

pub fn run(args: &Args, lie: bool) -> String {
    let mut app = make_app();
    app.insert_resource(SubstepCount(args.substeps));
    app.insert_resource(avian3d::dynamics::solver::SolverConfig {
        contact_frequency_factor: args.cff,
        contact_damping_ratio: args.cdr,
        ..default()
    });
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 1.0, 40.0),
        Transform::from_xyz(0.0, -0.5, 0.0),
        Friction::new(args.friction),
        CollisionLayers::new(LayerMask(1), LayerMask(2)),
    ));
    let mut d = defs(args.rest_offset);
    if args.one_leg {
        // pelvis + right thigh + right shin only (single-chain joint test)
        let keep = ["pelvis", "thigh_r", "shin_r"];
        let names: Vec<&str> = d.iter().map(|s| s.name).collect();
        let mut nd = Vec::new();
        for s in d.into_iter() {
            if keep.contains(&s.name) {
                nd.push(s);
            }
        }
        for s in nd.iter_mut() {
            s.parent = s
                .parent
                .map(|p| keep.iter().position(|k| *k == names[p]).unwrap());
        }
        d = nd;
    }
    // rung 0: rotate the whole T-pose to lie face-up on the floor
    let (root_r, root_t) = if lie {
        (Quat::from_rotation_x(-FRAC_PI_2), Vec3::new(0.0, 0.13, 0.0))
    } else {
        (Quat::IDENTITY, Vec3::ZERO)
    };
    let xf = |p: Vec3| root_r * p + root_t;
    let mut ents = Vec::new();
    for s in &d {
        let col = match s.shape {
            Shape::Box(e) => Collider::cuboid(e.x, e.y, e.z),
            Shape::Cap(r, l) => Collider::capsule(r, l),
            Shape::Ball(r) => Collider::sphere(r),
        };
        let e = app
            .world_mut()
            .spawn((
                if args.pin_pelvis && s.parent.is_none() {
                    RigidBody::Static
                } else {
                    RigidBody::Dynamic
                },
                col,
                Mass(s.mass),
                Transform::from_translation(xf(s.pos)).with_rotation(root_r * s.rot),
                CollisionLayers::new(LayerMask(2), LayerMask(1)),
                SleepingDisabled,
                Friction::new(args.friction),
            ))
            .id();
        ents.push(e);
    }
    let mut segs = Vec::new();
    for (i, s) in d.iter().enumerate() {
        if let Some(p) = s.parent {
            app.world_mut()
                .spawn(SphericalJoint::new(ents[p], ents[i]).with_anchor(xf(s.anchor))); // compliance 0 by default (point/swing/twist)
        }
        let mut rest_rel = (root_r * d[s.parent.unwrap_or(i)].rot).inverse() * (root_r * s.rot);
        if s.name.starts_with("thigh") {
            rest_rel = Quat::from_rotation_x(args.hip_offset_deg.to_radians()) * rest_rel;
        }
        let la_child = s.rot.inverse() * (s.anchor - s.pos);
        let mut subtree = vec![i];
        let mut k = 0;
        while k < subtree.len() {
            let cur = subtree[k];
            for (j, dj) in d.iter().enumerate() {
                if dj.parent == Some(cur) {
                    subtree.push(j);
                }
            }
            k += 1;
        }
        let fh = if let Shape::Box(e) = s.shape {
            e.y * 0.5
        } else {
            0.0
        };
        segs.push(Seg {
            e: ents[i],
            parent: s.parent,
            mass: s.mass,
            rest_rel,
            la_child,
            kind: s.kind,
            subtree,
            is_foot: s.name.starts_with("foot"),
            foot_half_h: fh,
            side: if s.pos.x < 0.0 { 0 } else { 1 },
            name_thigh: s.name.starts_with("thigh"),
        });
    }
    let total_mass: f32 = d.iter().map(|s| s.mass).sum();
    let mut log = args
        .out
        .as_ref()
        .map(|p| std::fs::File::create(p).expect("open --out"));
    // tick-0 header: no clips, one fixed model; a change would fail the run
    if let Some(f) = log.as_mut() {
        let _ = writeln!(
            f,
            "{{\"tick\":0,\"clip\":\"none\",\"model\":\"capsule_ragdoll_v1\"}}"
        );
    }
    app.insert_resource(Lab {
        segs,
        motors: !lie,
        paired: !args.unpaired,
        kp: args.kp,
        kd: args.kd,
        k: args.k,
        leg_floor: args.leg_floor,
        gff: !args.no_gff,
        clamp: !args.no_clamp,
        scaled: args.scaled,
        cp: args.cp,
        tan_full: args.tan_full,
        com_pull: args.com_pull,
        mu: args.mu,
        fault_ticks: 0,
        ired_scalar: args.ired_scalar,
        damp_rel: args.damp_rel,
        ired_diff_max: 0.0,
        foot_damp: args.foot_damp,
        anchor_k: args.anchor_k,
        bal_rate: args.bal_rate,
        bal_lp: args.bal_lp,
        bal_prev: Vec3::ZERO,
        anchors: [None; 2],
        pending_f: Vec::new(),
        flat_feet: args.flat_feet,
        old_gff: args.old_gff,
        geom_edge: args.geom_edge,
        bal_sign: args.bal_sign,
        kb: args.kb,
        kbd: args.kbd,
        bal_log: String::new(),
        sole_now: [0.0; 2],
        foot_tilt: [0.0; 2],
        hip_err: 0.0,
        filter: !args.no_filter,
        balance: args.balance,
        rest_offset: args.rest_offset,
        first_below: None,
        tick: 0,
        total_mass,
        c: [0.0; 2],
        on: [false; 2],
        flips: 0,
        com0: Vec3::ZERO,
        com_min: f32::MAX,
        com_final: 0.0,
        drift_max: 0.0,
        drift_final: 0.0,
        peak_sat: 0.0,
        peak_sat_foot_down: 0.0,
        low_limit_ticks_foot_down: 0,
        sum_tau_max: 0.0,
        sum_tau_mean: 0.0,
        sole_rest: Vec::new(),
        fell_through: false,
        com_samples: Vec::new(),
        last: String::new(),
        log,
        pending: Vec::new(),
    });
    app.add_systems(FixedUpdate, (torque_writer, apply_torques).chain());
    app.add_systems(FixedPostUpdate, metrics.after(PhysicsSystems::Last));
    let mut guard = 0;
    while app.world().resource::<Lab>().tick < args.ticks && guard < args.ticks * 4 + 10 {
        app.update();
        guard += 1;
    }
    let lab = app.world().resource::<Lab>();
    let secs = lab.tick as f32 * DT;
    let n = lab.sole_rest.len();
    let sole = if n > 0 {
        lab.sole_rest[n.saturating_sub(240)..].iter().sum::<f32>() / n.min(240) as f32
    } else {
        f32::NAN
    };
    let samples: Vec<String> = lab
        .com_samples
        .iter()
        .map(|(t, y)| format!("[{:.0},{y:.3}]", *t as f32 * DT))
        .collect();
    format!(
        "{{\"scene\":\"{}\",\"ticks\":{},\"seconds\":{secs:.2},\"motors\":{},\"paired\":{},\"kp\":{},\"kd\":{},\"k\":{},\"leg_floor\":{},\"scaled\":{},\"clamp\":{},\"gff\":{},\"filter\":{},\"balance\":{},\"rest_offset\":{},\"clip\":\"none\",\"model_changes\":0,\"first_tick_below_-0.1\":{},\"com_start\":{:.4},\"com_min\":{:.4},\"com_final\":{:.4},\"drift_max\":{:.4},\"drift_final\":{:.4},\"contact_flips\":{},\"flips_per_sec\":{:.3},\"sole_rest_height\":{sole:.4},\"peak_sat\":{:.3},\"peak_sat_foot_down\":{:.3},\"ticks_leg_limit_below_1Nm_foot_down\":{},\"sum_tau_max\":{:.4e},\"sum_tau_mean\":{:.4e},\"ired_rel_diff_max\":{:.4},\"mu\":{},\"tan_full\":{},\"k_com\":{},\"fault_3Mg_ticks\":{},\"substeps\":{},\"kb\":{},\"kbd\":{},\"contact_freq\":{},\"contact_damping\":{},\"foot_damp\":{},\"fell_through_floor\":{},\"com_per_sec\":[{}]}}",
        if lie { "lie" } else { "stand" },
        lab.tick,
        lab.motors,
        lab.paired,
        lab.kp,
        lab.kd,
        lab.k,
        lab.leg_floor,
        lab.scaled,
        lab.clamp,
        lab.gff,
        lab.filter,
        lab.balance,
        lab.rest_offset,
        lab.first_below.map(|t| t as i64).unwrap_or(-1),
        lab.com0.y,
        lab.com_min,
        lab.com_final,
        lab.drift_max,
        lab.drift_final,
        lab.flips,
        lab.flips as f32 / secs.max(1e-6),
        lab.peak_sat,
        lab.peak_sat_foot_down,
        lab.low_limit_ticks_foot_down,
        lab.sum_tau_max,
        lab.sum_tau_mean / lab.tick.max(1) as f64,
        lab.ired_diff_max,
        lab.mu,
        lab.tan_full,
        lab.com_pull,
        lab.fault_ticks,
        args.substeps,
        args.kb,
        args.kbd,
        args.cff,
        args.cdr,
        args.foot_damp,
        lab.fell_through,
        samples.join(",")
    )
}
