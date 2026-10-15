//! Character gait: WASD intent + Physics `phys_gait` (lean, foot placement, step events) -> pose.
//! Built on `proc` (same pose struct, PROC_BONES, `build` = ±1 tick Log derivatives, parent frame).
//! Frames as in `proc`: pelvis frame y up, z forward, +x = left; yaw + = left turn (about +Y).
//!
//! Step events are LIFT-OFF commands (phys_gait `StepEvent`): `step_event = Some((f, p))` = lift foot
//! `f` now and swing it to `p` (world, floor height). The phase is re-synced (critically damped) so f's
//! swing starts now (s_f = beta). Touchdown = `feet_down[f]` rising while `swing == Some(f)`; it also
//! re-syncs f's stance start (s_f = 0). Placement offsets are held through stance so a planted foot
//! never jumps; they change only during swing.

use super::DT;
use super::acro::{ANKLE_H, STAND_Y};
use super::proc::{
    GAIT_DEPTH, P, REACH, SOLE_TO_ANKLE, build, herm, idle_p, idle_pelvis_y, mix, proc_to_joints,
    qx, qy, qz, reachable, run_period, side_x, smooth,
};
use super::target::BoneTarget;
use crate::math::{Quat, Vec3};
use std::f64::consts::PI;

/// Speed (m/s) below which the gait settles to idle; walk ≈ 1.4, run ≥ 3.
pub const IDLE_SPEED: f64 = 0.1;
/// Turn above this |Δheading| (rad) uses a crossover step, below it a pivot step.
pub const CROSSOVER_RAD: f64 = PI / 3.0;
/// Pelvis/spine yaw lead time constant (ticks).
pub const YAW_LEAD_TICKS: f64 = 4.0;
const SPEED_TAU: f64 = 0.25;
const PHASE_TAU: f64 = 0.1;
const MAX_PLACE: f64 = 0.4;
/// Swing clearance: swing hip / knee flexion (rad) reached at mid-swing, blended in from lift-off and
/// out to touchdown with sin²(πu) (zero rate at both ends). Not scaled by GAIT_DEPTH.
pub const SWING_HIP_CLEAR: f64 = 0.5;
pub const SWING_KNEE_CLEAR: f64 = 0.9;
/// Max speed (m/s) at which a swing foot's placement aim can move.
const AIM_RATE: f64 = 0.8;

/// Per-body gait state (owned by the game).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GaitState {
    /// Cycle phase 0..1 (L touchdown at 0, R at 0.5).
    pub phase: f64,
    /// Remaining phase correction from the last step event.
    pub phase_err: f64,
    /// Filtered commanded speed (m/s) and idle->move blend weight (0..1).
    pub speed: f64,
    pub w_move: f64,
    /// Pelvis yaw lead relative to the body heading (rad).
    pub yaw: f64,
    /// Last touchdown (tick, foot, world foot position from `feet_world`).
    pub last_touchdown: Option<(u64, usize, Vec3)>,
    /// Foot currently swinging (set by a lift-off event, cleared at touchdown).
    pub swing: Option<usize>,
    /// The swinging foot has actually been seen off the ground (feet_down false) since its lift-off.
    pub left_ground: [bool; 2],
    /// World landing target per foot from Physics.
    pub target: [Option<Vec3>; 2],
    /// Placement offset (pelvis frame, xz) held in stance, and the offset being aimed in swing.
    pub off_land: [Vec3; 2],
    pub off_aim: [Vec3; 2],
    /// Foot twist held in stance / aimed in swing (rad).
    pub tw_land: [f64; 2],
    pub tw_aim: [f64; 2],
    prev_s: [f64; 2],
    /// Swing thigh local target (parent = pelvis frame) from the last `gait_pose`.
    swing_thigh: Option<(usize, Quat)>,
    phase_v: f64,
    yaw_v: f64,
}

/// Per-tick gait inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GaitInput {
    /// Wanted horizontal velocity (world), from WASD.
    pub v_want: Vec3,
    /// Measured velocity (world).
    pub v: Vec3,
    pub heading_want: f64,
    pub heading: f64,
    /// Lean angle (rad) and direction (world horizontal unit vector) from phys_gait `GaitCmd { lean, lean_dir }`:
    /// the trunk tilts by `lean` toward `lean_dir`.
    pub lean: f64,
    pub lean_dir: Vec3,
    /// Lift-off command (phys_gait `StepEvent`): (foot 0=L 1=R, landing target world at floor height).
    pub step_event: Option<(usize, Vec3)>,
    pub pelvis_pos: Vec3,
    pub q_pelvis: Quat,
    pub feet_world: [Vec3; 2],
    pub feet_down: [bool; 2],
}

impl GaitState {
    /// Foot that touches down now: it is swinging, has left the ground since lift-off, and is down again.
    /// Call before `phys_gait::gait_cmd`; `gait_pose` uses the same test.
    pub fn just_landed(&self, feet_down: [bool; 2]) -> Option<usize> {
        self.swing
            .filter(|&f| f < 2 && self.left_ground[f] && feet_down[f])
    }
}

fn wrap(a: f64) -> f64 {
    (a + PI).rem_euclid(2.0 * PI) - PI
}
fn wrap_half(a: f64) -> f64 {
    (a + 0.5).rem_euclid(1.0) - 0.5
}
fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}
/// Stance fraction of the cycle at `speed` (same formula as `layout`).
fn beta_of(speed: f64) -> f64 {
    let v = speed.max(0.3);
    let stride = GAIT_DEPTH * (0.6 + 0.37 * v).min(3.2);
    let base_y = STAND_Y - GAIT_DEPTH * (0.03 + 0.008 * v.min(8.0));
    let bob = GAIT_DEPTH * (0.012 + 0.006 * v.min(6.0)).min(0.05);
    let dy_hi = base_y + bob - ANKLE_H;
    let zmax = (REACH * REACH - dy_hi * dy_hi).max(0.0).sqrt();
    (0.62 - 0.035 * v)
        .clamp(0.35, 0.62)
        .min(2.0 * zmax / stride)
}

struct Ctx {
    w: f64,
    yaw: f64,
    pitch_add: f64,
    roll_add: f64,
    target_rel: [Option<Vec3>; 2],
    turn: f64,
    vrel: Vec3,
}

/// Pose at `phase` plus the placement offsets the swing feet are aiming for.
fn layout(
    gs: &GaitState,
    phase: f64,
    speed: f64,
    c: &Ctx,
) -> (P, [Vec3; 2], [f64; 2], Option<usize>) {
    let v = speed.max(0.3);
    let stride = GAIT_DEPTH * (0.6 + 0.37 * v).min(3.2);
    let base_y = STAND_Y - GAIT_DEPTH * (0.03 + 0.008 * v.min(8.0));
    let bob = GAIT_DEPTH * (0.012 + 0.006 * v.min(6.0)).min(0.05);
    let dy_hi = base_y + bob - ANKLE_H;
    let plan = REACH;
    let zmax = (plan * plan - dy_hi * dy_hi).max(0.0).sqrt();
    let beta = (0.62 - 0.035 * v)
        .clamp(0.35, 0.62)
        .min(2.0 * zmax / stride);
    let idle_y = idle_pelvis_y();
    let move_y = base_y - bob * (4.0 * PI * (phase - 0.5 * beta)).cos();
    let pelvis_y = mix(idle_y, move_y, c.w);
    let lift = GAIT_DEPTH * (0.06 + 0.04 * v).min(0.35);
    let half = 0.5 * beta * stride;
    let mut aim = gs.off_aim;
    let mut tw_aim = gs.tw_aim;
    let mut swing = None;
    let mut ank = [Vec3::ZERO; 2];
    let mut tw = [0.0; 2];
    let mut clear = [0.0; 2];
    for i in 0..2 {
        let s = (phase + 0.5 * i as f64).rem_euclid(1.0);
        let (mut z, mut y, off, t);
        if s < beta {
            z = half - s * stride;
            y = ANKLE_H;
            off = gs.off_land[i];
            t = gs.tw_land[i];
        } else {
            swing = Some(i);
            let u = (s - beta) / (1.0 - beta);
            clear[i] = (PI * u).sin().powi(2) * c.w;
            z = herm(-half, -stride, half, -stride, 1.0 - beta, s - beta);
            y = ANKLE_H + lift * (PI * u).sin().powi(3);
            // Aim: Physics landing target relative to the nominal landing spot, plus crossover.
            // Target predicted at touchdown (pelvis keeps moving), so the aim is ~constant over the swing.
            let t_rem = (1.0 - s) * stride / v;
            let mut a = match c.target_rel[i] {
                Some(tr) => flat(tr - c.vrel * t_rem - Vec3::new(side_x(i), 0.0, half))
                    .clamp_length(MAX_PLACE),
                None => Vec3::ZERO,
            };
            if c.turn.abs() > CROSSOVER_RAD
                && ((c.turn > 0.0 && i == 1) || (c.turn < 0.0 && i == 0))
            {
                a = a + Vec3::new(c.turn.signum() * 0.18, 0.0, 0.0);
            }
            // Rate-limited so a target arriving mid-swing eases in (no foot pop).
            aim[i] = gs.off_aim[i] + (a * c.w - gs.off_aim[i]).clamp_length(AIM_RATE * DT);
            tw_aim[i] = 0.5 * c.turn.clamp(-1.0, 1.0) * c.w;
            let k = smooth(u);
            off = gs.off_land[i] + (aim[i] - gs.off_land[i]) * k;
            t = mix(gs.tw_land[i], tw_aim[i], k);
        }
        // Idle: feet under the hips.
        z = mix(0.0, z, c.w);
        y = mix(ANKLE_H, y, c.w);
        let a = Vec3::new(side_x(i), y - pelvis_y, z) + off;
        ank[i] = qy(-c.yaw).rotate(a);
        tw[i] = t - c.yaw;
    }
    let mut p = idle_p();
    p.pitch = mix(0.0, (0.04 + 0.035 * v).min(0.3), c.w) + c.pitch_add;
    p.roll = c.roll_add;
    p.spine = 0.3 * mix(0.0, (0.04 + 0.035 * v).min(0.3), c.w);
    p.neck = -0.5 * p.pitch;
    p.yaw = c.yaw;
    p.spine_yaw = 0.5 * c.yaw;
    for i in 0..2 {
        p.leg(i, reachable(i, ank[i]), tw[i]);
        p.hip[i] = mix(p.hip[i], SWING_HIP_CLEAR, clear[i]);
        p.knee[i] = mix(p.knee[i], SWING_KNEE_CLEAR, clear[i]);
    }
    let a = (0.25 + 0.08 * v).min(0.9) * c.w;
    let cs = (2.0 * PI * phase).cos();
    p.sh = [-a * cs, a * cs];
    p.el = [mix(0.2, (0.4 + 0.12 * v).min(1.6), c.w); 2];
    p.sh_abd = [mix(0.1, 0.08, c.w); 2];
    // Exact soft idle at move weight 0 (keeps turning yaw and lean).
    let mut id = idle_p();
    id.yaw = p.yaw;
    id.spine_yaw = p.spine_yaw;
    id.pitch = c.pitch_add;
    id.roll = c.roll_add;
    id.neck = -0.5 * c.pitch_add;
    id.twist = tw;
    let p = P::lerp(&id, &p, c.w);
    (p, aim, tw_aim, if c.w > 0.0 { swing } else { None })
}

/// Gait pose for this tick (advances `gs`). See module docs for the step-event contract.
pub fn gait_pose(gs: &mut GaitState, inp: &GaitInput, tick: u64) -> Vec<(u16, BoneTarget)> {
    // Speed blend: filtered |v_want|, idle below IDLE_SPEED.
    let want = flat(inp.v_want).length();
    let want = if want.is_finite() { want } else { 0.0 };
    gs.speed += (want - gs.speed) * (1.0 - (-DT / SPEED_TAU).exp());
    if want == 0.0 && gs.speed < 0.02 {
        gs.speed = 0.0;
    }
    gs.w_move = smooth((gs.speed - IDLE_SPEED) / 0.4);
    // Lift-off: f's swing starts now (s_f = beta). Touchdown: feet_down[f] rises while f swings (s_f = 0).
    for f in 0..2 {
        if gs.swing == Some(f) && !inp.feet_down[f] {
            gs.left_ground[f] = true;
        }
    }
    if let Some(f) = gs.just_landed(inp.feet_down) {
        gs.swing = None;
        gs.last_touchdown = Some((tick, f, inp.feet_world[f]));
        gs.phase_err = wrap_half(-0.5 * f as f64 - gs.phase);
    }
    if let Some((f, p)) = inp.step_event {
        let f = f.min(1);
        gs.phase_err = wrap_half(beta_of(gs.speed) - 0.5 * f as f64 - gs.phase);
        gs.swing = Some(f);
        gs.left_ground[f] = false;
        gs.target[f] = if p.is_finite() { Some(p) } else { None };
    }
    // Critically damped correction (no velocity step at the event).
    let wn = 1.0 / PHASE_TAU;
    gs.phase_v += (wn * wn * gs.phase_err - 2.0 * wn * gs.phase_v) * DT;
    let corr = gs.phase_v * DT;
    gs.phase_err -= corr;
    let dp = (DT / run_period(gs.speed.max(0.3))) * gs.w_move + corr;
    gs.phase = (gs.phase + dp).rem_euclid(1.0);
    if gs.w_move == 0.0 {
        gs.phase_err = 0.0;
        gs.phase_v = 0.0;
    }
    // Touchdown commits held offsets (stance never moves a planted foot).
    for i in 0..2 {
        let s = (gs.phase + 0.5 * i as f64).rem_euclid(1.0);
        if s < gs.prev_s[i] - 0.5 {
            gs.off_land[i] = gs.off_aim[i];
            gs.tw_land[i] = gs.tw_aim[i];
            if gs.swing != Some(i) {
                gs.target[i] = None;
            }
        }
        gs.prev_s[i] = s;
    }
    if gs.w_move == 0.0 {
        for i in 0..2 {
            gs.off_land[i] = gs.off_land[i] * 0.9;
            gs.tw_land[i] *= 0.9;
            gs.off_aim[i] = gs.off_land[i];
            gs.tw_aim[i] = gs.tw_land[i];
        }
    }
    // Turning lead.
    let turn = wrap(inp.heading_want - inp.heading);
    let turn = if turn.is_finite() { turn } else { 0.0 };
    let yaw_t = turn.clamp(-0.6, 0.6);
    // Critically damped yaw lead (time constant YAW_LEAD_TICKS), no rate step on a heading change.
    let wy = 1.0 / (YAW_LEAD_TICKS * DT);
    gs.yaw_v += (wy * wy * (yaw_t - gs.yaw) - 2.0 * wy * gs.yaw_v) * DT;
    let dyaw = gs.yaw_v * DT;
    gs.yaw += dyaw;
    // Lean about the motion direction, in the pelvis frame.
    let qp = inp.q_pelvis.normalize();
    let qi = qp.conj();
    let d = flat(qi.rotate(flat(inp.lean_dir)));
    let dl = d.length();
    let phi = if inp.lean.is_finite() && d.is_finite() {
        inp.lean.clamp(-0.6, 0.6)
    } else {
        0.0
    };
    let (pitch_add, roll_add) = if dl > 1e-6 {
        (phi * d.z / dl, -phi * d.x / dl)
    } else {
        (0.0, 0.0)
    };
    let target_rel = gs
        .target
        .map(|t| t.map(|p| qi.rotate(p + Vec3::new(0.0, SOLE_TO_ANKLE, 0.0) - inp.pelvis_pos)));
    let vrel = flat(qi.rotate(inp.v));
    let vrel = if vrel.is_finite() { vrel } else { Vec3::ZERO };
    let mk = |y: f64| Ctx {
        w: gs.w_move,
        yaw: y,
        pitch_add,
        roll_add,
        target_rel,
        turn,
        vrel,
    };
    let snap = *gs;
    let (c, aim, tw_aim, swing) = layout(&snap, gs.phase, gs.speed, &mk(gs.yaw));
    let (m, _, _, _) = layout(&snap, gs.phase - dp, gs.speed, &mk(gs.yaw - dyaw));
    let (n, _, _, _) = layout(&snap, gs.phase + dp, gs.speed, &mk(gs.yaw + dyaw));
    gs.off_aim = aim;
    gs.tw_aim = tw_aim;
    gs.swing_thigh = swing.map(|i| {
        (
            i,
            (qy(c.twist[i]) * qz(c.abd[i]) * qx(c.hip[i])).normalize(),
        )
    });
    if gs.w_move == 0.0 && swing.is_none() && inp.feet_down == [true; 2] {
        gs.swing = None;
    }
    build(&m, &c, &n)
}

/// Pelvis bone offset O_p (rest rotation of the pelvis bone in its capsule frame). Identity until the
/// rig confirms otherwise; use `swing_thigh_world_with` to pass the real one.
pub const PELVIS_O_P: Quat = Quat::IDENTITY;

/// World-frame target of the swing thigh for `phys_gait::simbicon_hips`: q_pelvis·O_p·q_local, the same
/// B_p = R_p·O_p composition `target::rates_to_world_rel` uses (`q_pelvis` = measured pelvis capsule
/// world rotation, O_p = `PELVIS_O_P`). None when no foot is swinging (idle / between steps).
pub fn swing_thigh_world(gs: &GaitState, q_pelvis: Quat) -> Option<(usize, Quat)> {
    swing_thigh_world_with(gs, q_pelvis, PELVIS_O_P)
}
/// `swing_thigh_world` with an explicit pelvis bone offset O_p.
pub fn swing_thigh_world_with(gs: &GaitState, q_pelvis: Quat, o_p: Quat) -> Option<(usize, Quat)> {
    gs.swing_thigh
        .map(|(i, ql)| (i, (q_pelvis.normalize() * o_p * ql).normalize()))
}

/// `gait_pose` mapped to the 13-joint physics rig (`proc_to_joints`), ready for `Sim::set_target`.
pub fn gait_joints(gs: &mut GaitState, inp: &GaitInput, tick: u64) -> Vec<crate::api::Target> {
    proc_to_joints(&gait_pose(gs, inp, tick))
}
