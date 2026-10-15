//! Overhaul §API (GUILDBOUND_BRIEF.md): the one set of callable sim functions. Pure, no Bevy.
//! The game crate owns rigid-body dynamics (Avian); this layer owns bodies, targets, the single
//! motor writer and the stand state. `step` runs a minimal joint-space integrator so the sim
//! can run headless; when the game is wired, it feeds measured joint states back via `set_pose`.

pub mod motor;

use crate::math::{Mat3, Quat, Vec3};
use crate::phys_balance::Contact;

pub const TICK_HZ: f64 = 120.0;
pub const DT: f64 = 1.0 / TICK_HZ;
pub const G: f64 = 9.81;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyId(pub u32);
/// Index into rig.json joints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JointId(pub u16);

/// Rigid transform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Iso {
    pub pos: Vec3,
    pub rot: Quat,
}

/// What a joint does in the stand logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointRole {
    Pelvis,
    Hip(u8),
    Knee(u8),
    Ankle(u8),
    Arm(u8),
    Other,
}
impl JointRole {
    pub fn is_arm(self) -> bool {
        matches!(self, JointRole::Arm(_))
    }
    pub fn is_leg(self) -> bool {
        matches!(
            self,
            JointRole::Hip(_) | JointRole::Knee(_) | JointRole::Ankle(_)
        )
    }
}

/// Foot box half extents (m), foot-segment frame centred on the foot COM: x = width, y = height, z = length.
/// Ankle levers come from this box plus the foot COM offset from the ankle (d_toe = com.z + half_length,
/// d_heel = half_length − com.z, d_side = half_width ± com.x). Default = gb_lab stand rig (0.10×0.06×0.24).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoleBox {
    pub half_length: f64,
    pub half_width: f64,
    pub half_height: f64,
}
impl Default for SoleBox {
    fn default() -> Self {
        SoleBox {
            half_length: 0.12,
            half_width: 0.05,
            half_height: 0.03,
        }
    }
}

/// One joint/segment of the rig (rig.json row). `offset` is in the parent's frame at rest.
#[derive(Clone, Debug)]
pub struct RigJoint {
    pub name: String,
    pub parent: Option<u16>,
    pub offset: Vec3,
    pub mass: f64,
    /// Segment centre of mass, child frame.
    pub com: Vec3,
    /// Principal inertia about the segment com.
    pub inertia: Mat3,
    pub role: JointRole,
    /// Torque clamp (N·m); thighs are raised to >= M·g·0.12 at spawn.
    pub torque_cap: f64,
}

/// Rig profile: joints in parent-before-child order. Index 0 is the pelvis (root).
#[derive(Clone, Debug, Default)]
pub struct RigProfile {
    pub joints: Vec<RigJoint>,
    pub sole: SoleBox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Physical,
    Animated,
}

#[derive(Clone, Debug)]
pub struct BodySpec {
    pub rig: RigProfile,
    pub root: Iso,
    pub mode: Mode,
}

/// Motor target, parent frame (B_p).
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub joint: JointId,
    pub q: Quat,
    pub w: Vec3,
    pub alpha_ff: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct JointState {
    pub q_local: Quat,
    pub q_world: Quat,
    pub w: Vec3,
    pub pos: Vec3,
    pub torque: Vec3,
    pub sat: f64,
}

#[derive(Clone, Debug, Default)]
pub struct BodyState {
    pub com: Vec3,
    pub com_v: Vec3,
    pub feet_down: [bool; 2],
    pub cop: Vec3,
    pub capture_point: Vec3,
    pub sat_peak: f64,
    pub stand_failed: bool,
    pub joints: Vec<JointState>,
    /// Foot damping force per side (world, N), PM §8: the game applies it as a force on the foot body.
    pub foot_force: [Vec3; 2],
    /// Fault-cap hits this tick (|τ| > 3·M·g N·m or non-finite), PM §11.
    pub faults: u32,
    /// Body mode (owned by api's state machine).
    pub mode: BodyMode,
    /// Predicted seconds to touchdown (Airborne/Ragdoll/Recovering), from phys_air::time_to_land.
    pub t_land: Option<f64>,
    /// Foot landing targets (world) for anim while Airborne/Recovering (phys_air::foot_targets).
    pub air_feet: Option<[Vec3; 2]>,
    /// Grab link force magnitude (N) while Grabbing/Grabbed.
    pub grab_tension: f64,
    /// Shared COM of a grab pair (phys_grab::spin_center).
    pub spin_center: Option<Vec3>,
    /// Synced bodies: launch velocity to apply to every segment this tick (set by `launch`, one tick).
    pub launch_impulse: Option<Vec3>,
    /// External forces for the game to apply this tick: (joint, world force at that joint's anchor).
    pub ext_forces: Vec<(JointId, Vec3)>,
    /// Current overall motor weight (body × mode).
    pub motor_weight: f64,
    /// Contact weight share per foot, c_i/Σc (0 when airborne).
    pub foot_share: [f64; 2],
}

/// SIMBICON drive input (see `Sim::set_gait`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GaitDrive {
    pub heading: f64,
    pub swing: Option<usize>,
    /// Swing thigh world target (anim::gait::swing_thigh_world); None = upright-pelvis frame × the hip target.
    pub swing_q: Option<Quat>,
}

/// Body mode state machine (api only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BodyMode {
    #[default]
    Grounded,
    Airborne,
    Ragdoll,
    Recovering,
    Grabbing,
    Grabbed,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GrabLink {
    pub other: BodyId,
    pub hand: JointId,
    pub ankle: JointId,
    pub prev: Option<(Vec3, Vec3)>,
}

/// One joint's measured state from the game's rigid bodies (Avian), world frame, per tick.
/// `q`: segment world rotation; `w`: angular velocity; `v`: linear velocity of the segment COM;
/// `pos`: joint anchor (segment origin). Index = JointId; joints[0] (pelvis) also gives the root Iso.
#[derive(Clone, Copy, Debug, Default)]
pub struct MeasuredJoint {
    pub q: Quat,
    pub w: Vec3,
    pub v: Vec3,
    pub pos: Vec3,
}

/// Tuning defaults: PHYSICS_MATH.md §3–§8.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    // kp/kd (per unit inertia), kb/kbd: plain fields; Physics drops refit values from gb_lab/hold120/ here.
    // Stable PD dt is always DT = 1/120 (torque held across Avian's substeps).
    pub kp: f64,
    pub kd: f64,
    pub kb: f64,
    pub kbd: f64,
    pub substeps: u32,
    pub foot_damp: f64,
    pub mu: f64,
    pub k_com: f64,
    pub thigh_lever_m: f64,
    pub fail_com_frac: f64,
    pub fail_air_ticks: u32,
    pub rest_lock_tick: u64,
    pub w_cap: f64,
    /// kp and kd ramp in over this many ticks from ragdoll-on (smoothstep).
    pub gain_ramp_ticks: u64,
    /// Before rest_com_h locks: stand_failed if COM y < this (m).
    pub fail_com_abs_m: f64,
    /// Fault cap lever (m): |τ| > fault_lever·M·g → clamp + count (PM §11).
    pub fault_lever_m: f64,
    /// Full Tan numerator (−kp·dt·ω_rel); off (ruling 07:23).
    pub tan_full: bool,
    /// Grounded → Airborne after both feet are up for more than this many ticks.
    pub airborne_ticks: u32,
    /// Recovery: motor weight reaches 1 this many seconds before predicted touchdown.
    pub recover_margin_s: f64,
    /// SIMBICON heading PD on the pelvis yaw (N·m/rad, N·m·s/rad): Kd_h ≈ 30, Kp_h = Kd_h²/(4·1.5) (critical, I_yaw ≈ 1.5).
    pub heading_kp: f64,
    pub heading_kd: f64,
}
impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            kp: 1500.0,
            kd: 55.0,
            kb: 5.0,
            kbd: 0.3,
            substeps: 12,
            foot_damp: 40.0,
            mu: 0.8,
            k_com: 0.11,
            thigh_lever_m: 0.12,
            fail_com_frac: 0.6,
            fail_air_ticks: 30,
            rest_lock_tick: 60,
            w_cap: 50.0,
            gain_ramp_ticks: 12,
            fail_com_abs_m: 0.5,
            fault_lever_m: 3.0,
            tan_full: false,
            airborne_ticks: 6,
            recover_margin_s: 0.05,
            heading_kp: 150.0,
            heading_kd: 30.0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Body {
    pub rig: RigProfile,
    pub root: Iso,
    pub root_v: Vec3,
    pub mode: Mode,
    pub possessed: bool,
    pub q_local: Vec<Quat>,
    pub w: Vec<Vec3>,
    pub targets: Vec<Option<Target>>,
    pub impulses: Vec<Vec3>,
    pub torque: Vec<Vec3>,
    pub sat: Vec<f64>,
    pub contact: [Contact; 2],
    pub rest_sole_h: [f64; 2],
    pub support_heights: Option<[Option<f64>; 2]>,
    pub rest_com_h: Option<f64>,
    pub air_ticks: u32,
    pub ticks: u64,
    pub last_com: Vec3,
    pub state: BodyState,
    /// Tick the body went Physical (ramp origin, COM-pull anchor reset).
    pub on_tick: u64,
    pub com_anchor: Option<Vec3>,
    /// Running sum / count for the per-foot rest sole height over ticks 1..=60 (SM §8).
    pub rest_acc: [f64; 2],
    pub rest_n: u32,
    /// Measured state for this tick (set by sync_measured, cleared by step).
    pub measured: Option<Vec<MeasuredJoint>>,
    /// Previous tick's world angular velocity per joint (for ω̇_p in the feed-forward, PM §5).
    pub prev_w_world: Vec<Vec3>,
    /// Fade inputs for the COM pull: strike boost b(t) and acro weight a(t) in [0,1] (PM §7). Set by the game/anim.
    pub strike_b: f64,
    pub acro_a: f64,
    pub prev_fade: f64,
    /// Previous foot COM per side (unsynced foot velocity for damping).
    pub prev_foot_com: [Option<Vec3>; 2],
    pub body_mode: BodyMode,
    pub mode_w: f64,
    pub motor_w_body: f64,
    pub motor_w: Vec<f64>,
    pub pending_launch: Option<Vec3>,
    /// Set on the grabber (a): link to b.
    pub grab: Option<GrabLink>,
    /// Set on the grabbed body (b): who holds it.
    pub grabbed_by: Option<BodyId>,
    pub air_count: u32,
    /// Measured state used on the last synced tick (grab point velocities); None for unsynced ticks.
    pub last_measured: Option<Vec<MeasuredJoint>>,
    /// Joint weights saved at grab time, restored on release.
    pub saved_w: Option<Vec<f64>>,
    /// Gait drive for SIMBICON hips (heading + swing foot); None = plain per-joint PD hips.
    pub gait: Option<GaitDrive>,
    /// Tick torque minus its PD part (held over the substeps when the PD runs per substep).
    pub residual: Vec<Vec3>,
    /// Tick a recover/get-up started (mode weight ramp origin).
    pub ramp_from: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct Sim {
    pub tuning: Tuning,
    pub grab_params: crate::phys_grab::GrabParams,
    bodies: Vec<Option<Body>>,
}

impl Sim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn_body(&mut self, spec: BodySpec) -> BodyId {
        let mut rig = spec.rig;
        let m_total: f64 = rig.joints.iter().map(|j| j.mass).sum();
        let thigh_min = m_total * G * self.tuning.thigh_lever_m;
        for j in rig.joints.iter_mut() {
            if matches!(j.role, JointRole::Hip(_)) {
                j.torque_cap = j.torque_cap.max(thigh_min);
            }
        }
        let n = rig.joints.len();
        let mut b = Body {
            rig,
            root: spec.root,
            root_v: Vec3::ZERO,
            mode: spec.mode,
            possessed: false,
            q_local: vec![Quat::IDENTITY; n],
            w: vec![Vec3::ZERO; n],
            targets: vec![None; n],
            impulses: vec![Vec3::ZERO; n],
            torque: vec![Vec3::ZERO; n],
            sat: vec![0.0; n],
            contact: [Contact::planted(); 2],
            rest_sole_h: [0.0; 2],
            support_heights: None,
            rest_com_h: None,
            air_ticks: 0,
            ticks: 0,
            last_com: Vec3::ZERO,
            state: BodyState::default(),
            on_tick: 0,
            com_anchor: None,
            rest_acc: [0.0; 2],
            rest_n: 0,
            measured: None,
            prev_w_world: vec![Vec3::ZERO; n],
            strike_b: 0.0,
            acro_a: 0.0,
            prev_fade: 0.0,
            prev_foot_com: [None; 2],
            body_mode: BodyMode::Grounded,
            mode_w: 1.0,
            motor_w_body: 1.0,
            motor_w: vec![1.0; n],
            pending_launch: None,
            grab: None,
            grabbed_by: None,
            air_count: 0,
            last_measured: None,
            saved_w: None,
            gait: None,
            residual: Vec::new(),
            ramp_from: None,
        };
        motor::refresh_state(&mut b, &self.tuning);
        b.last_com = b.state.com;
        self.bodies.push(Some(b));
        BodyId(self.bodies.len() as u32 - 1)
    }

    /// Teleport to a pose (local joint rotations), zero velocities.
    pub fn set_pose(&mut self, b: BodyId, pose: &[Quat]) {
        let tuning = self.tuning;
        let Some(body) = self.body_mut(b) else { return };
        for (q, p) in body.q_local.iter_mut().zip(pose) {
            *q = p.normalize();
        }
        body.w.iter_mut().for_each(|w| *w = Vec3::ZERO);
        body.prev_w_world.iter_mut().for_each(|w| *w = Vec3::ZERO);
        body.prev_foot_com = [None; 2];
        body.root_v = Vec3::ZERO;
        motor::refresh_state(body, &tuning);
        body.last_com = body.state.com;
    }

    /// Anim -> motor writer. Replaces targets for the listed joints.
    pub fn set_target(&mut self, b: BodyId, t: &[Target]) {
        let Some(body) = self.body_mut(b) else { return };
        for x in t {
            if let Some(s) = body.targets.get_mut(x.joint.0 as usize) {
                *s = Some(*x);
            }
        }
    }

    /// Angular impulse p applied at world point `at` on joint j's segment (queued for next step).
    pub fn apply_impulse(&mut self, b: BodyId, j: JointId, p: Vec3, at: Vec3) {
        let Some(body) = self.body_mut(b) else { return };
        let i = j.0 as usize;
        if i >= body.impulses.len() {
            return;
        }
        let pivot = body
            .state
            .joints
            .get(i)
            .map(|s| s.pos)
            .unwrap_or(Vec3::ZERO);
        body.impulses[i] = body.impulses[i] + (at - pivot).cross(p);
        body.root_v = body.root_v
            + p * (1.0
                / body
                    .rig
                    .joints
                    .iter()
                    .map(|x| x.mass)
                    .sum::<f64>()
                    .max(1e-6));
    }

    pub fn possess(&mut self, b: BodyId) {
        if let Some(x) = self.body_mut(b) {
            if x.mode != Mode::Physical {
                x.on_tick = x.ticks;
                x.com_anchor = None;
            }
            x.state.stand_failed = false;
            x.possessed = true;
            x.mode = Mode::Physical;
        }
    }

    /// Feed the game's measured rigid-body state for this tick (call before `step`). A synced body
    /// skips the internal stepper in `step`: only the motor writer runs, and the game applies
    /// `read_state().joints[i].torque` as +τ on joint i's body and −τ on its parent, plus `foot_force`.
    pub fn sync_measured(&mut self, b: BodyId, joints: &[MeasuredJoint]) {
        let tuning = self.tuning;
        let Some(body) = self.body_mut(b) else { return };
        let n = body.rig.joints.len();
        if joints.len() < n {
            return;
        }
        let m: Vec<MeasuredJoint> = joints[..n].to_vec();
        body.root = Iso {
            pos: m[0].pos,
            rot: m[0].q.normalize(),
        };
        body.root_v = m[0].v;
        for i in 0..n {
            let (qp, wp) = match body.rig.joints[i].parent {
                Some(p) => (m[p as usize].q, m[p as usize].w),
                None => (Quat::IDENTITY, Vec3::ZERO),
            };
            body.q_local[i] = if body.rig.joints[i].parent.is_some() {
                (qp.conj() * m[i].q).normalize()
            } else {
                Quat::IDENTITY
            };
            body.w[i] = qp.conj().rotate(m[i].w - wp);
        }
        body.measured = Some(m);
        let _ = tuning;
    }

    /// COM-pull fade inputs (PM §7): strike boost b(t) and acro weight a(t), both in [0,1].
    pub fn set_fade(&mut self, b: BodyId, strike_b: f64, acro_a: f64) {
        if let Some(x) = self.body_mut(b) {
            x.strike_b = strike_b.clamp(0.0, 1.0);
            x.acro_a = acro_a.clamp(0.0, 1.0);
        }
    }
    /// Substep motor: joint torques for body `b` from its live measured state (world frame, as for
    /// `sync_measured`) with Stable PD hold time `dt` (the engine substep). Joint PD is recomputed; the rest
    /// of this tick's torque (gravity ff, balance, SIMBICON) is held from `step`. Apply +τ_i on body i and
    /// −τ_i on its parent for this substep only. Engine-agnostic: the game calls it from its substep loop.
    pub fn motor_substep(&self, b: BodyId, measured: &[MeasuredJoint], dt: f64) -> Vec<Vec3> {
        let t = self.tuning;
        self.body_ref(b)
            .map(|x| motor::substep_torques(x, &t, measured, dt))
            .unwrap_or_default()
    }

    /// Gait drive for this tick's motor: pelvis heading (yaw, rad) and the swinging foot (None = double
    /// support). While set and Grounded, the hips run SIMBICON (phys_gait::simbicon_hips). None turns it off.
    pub fn set_gait(&mut self, b: BodyId, g: Option<GaitDrive>) {
        if let Some(x) = self.body_mut(b) {
            x.gait = g;
        }
    }
    /// Exact terrain height below each foot. A missed trace means no support.
    /// None restores the original flat-stage calibration.
    pub fn set_support_heights(&mut self, b: BodyId, heights: Option<[Option<f64>; 2]>) {
        if let Some(x) = self.body_mut(b) {
            if x.support_heights.is_none() && heights.is_some() {
                x.state.stand_failed = false;
                x.rest_com_h = None;
                x.on_tick = x.ticks;
            }
            x.support_heights = heights;
        }
    }
    /// Remove a body from the sim completely (model swap). Breaks any grab it is part of first. Its id is
    /// never reused; every call on it afterwards is a no-op and read_state returns the default.
    pub fn despawn(&mut self, b: BodyId) {
        self.release_grab(b);
        if let Some(g) = self.body_ref(b).and_then(|x| x.grabbed_by) {
            self.release_grab(g);
        }
        if let Some(slot) = self.bodies.get_mut(b.0 as usize) {
            *slot = None;
        }
    }
    pub fn release(&mut self, b: BodyId) {
        if let Some(x) = self.body_mut(b) {
            x.possessed = false;
        }
    }

    /// One 120 Hz tick.
    pub fn step(&mut self) {
        let t = self.tuning;
        for body in self.bodies.iter_mut().flatten() {
            motor::tick(body, &t);
            body.state.ext_forces.clear();
        }
        self.grab_tick();
    }

    /// Ragdoll launch: motor weight 0, velocity v on root and segments (unsynced), or reported in
    /// `launch_impulse` for the game to apply (synced).
    pub fn launch(&mut self, b: BodyId, v: Vec3) {
        if self.body_ref(b).and_then(|x| x.grab).is_some() {
            self.release_grab(b);
        }
        if let Some(g) = self.body_ref(b).and_then(|x| x.grabbed_by) {
            self.release_grab(g);
        }
        let Some(x) = self.body_mut(b) else { return };
        x.body_mode = BodyMode::Ragdoll;
        x.mode_w = 0.0;
        x.mode = Mode::Physical;
        x.root_v = v;
        x.pending_launch = Some(v);
        x.air_count = 0;
        x.contact = [crate::phys_balance::Contact {
            on: false,
            held: 0,
            c: 0.0,
        }; 2];
        x.state.feet_down = [false, false];
        x.state.stand_failed = false;
    }

    /// Start air recovery: motor fades in by phys_air::recover_weight so it reaches 1 before touchdown.
    /// Also the get-up entry: the body goes Recovering, its body motor weight is restored, and mode weight
    /// ramps 0 -> 1 over RECOVER_RAMP_TICKS (smoothstep) whether or not a foot is down.
    pub fn recover(&mut self, b: BodyId) {
        if let Some(x) = self.body_mut(b) {
            x.body_mode = BodyMode::Recovering;
            x.on_tick = x.ticks;
            x.ramp_from = Some(x.ticks);
            x.motor_w_body = 1.0;
            x.mode_w = 0.0;
            x.com_anchor = None;
            x.state.stand_failed = false;
        }
    }

    /// Body a's hand grabs body b's ankle. a → Grabbing, b → Grabbed (b's leg motors default to 0.2).
    pub fn grab(&mut self, a: BodyId, hand: JointId, b: BodyId, ankle: JointId) {
        if a == b || self.body_ref(a).is_none() || self.body_ref(b).is_none() {
            return;
        }
        self.release_grab(a);
        if let Some(g) = self.body_ref(b).and_then(|x| x.grabbed_by) {
            self.release_grab(g);
        }
        let x = self.bodies[a.0 as usize].as_mut().unwrap();
        x.grab = Some(GrabLink {
            other: b,
            hand,
            ankle,
            prev: None,
        });
        x.body_mode = BodyMode::Grabbing;
        let y = self.bodies[b.0 as usize].as_mut().unwrap();
        y.grabbed_by = Some(a);
        y.body_mode = BodyMode::Grabbed;
        y.saved_w = Some(y.motor_w.clone());
        for (i, j) in y.rig.joints.iter().enumerate() {
            if j.role.is_leg() {
                y.motor_w[i] = 0.2;
            }
        }
    }

    /// Release body a's grab (no-op if none). Both bodies return to Grounded/Airborne by contact.
    pub fn release_grab(&mut self, a: BodyId) {
        let Some(link) = self.body_mut(a).and_then(|x| x.grab.take()) else {
            return;
        };
        let x = self.bodies[a.0 as usize].as_mut().unwrap();
        x.body_mode = if x.state.feet_down == [false, false] {
            BodyMode::Airborne
        } else {
            BodyMode::Grounded
        };
        x.state.grab_tension = 0.0;
        x.state.spin_center = None;
        if let Some(y) = self.body_mut(link.other) {
            y.grabbed_by = None;
            if let Some(w) = y.saved_w.take() {
                y.motor_w = w;
            }
            y.body_mode = if y.state.feet_down == [false, false] {
                BodyMode::Airborne
            } else {
                BodyMode::Grounded
            };
            y.state.grab_tension = 0.0;
            y.state.spin_center = None;
        }
    }

    /// Scale motor torque for one joint (Some) or the whole body (None); w in [0,1].
    pub fn set_motor_weight(&mut self, b: BodyId, joint: Option<JointId>, w: f64) {
        let Some(x) = self.body_mut(b) else { return };
        let w = w.clamp(0.0, 1.0);
        match joint {
            Some(j) => {
                if let Some(s) = x.motor_w.get_mut(j.0 as usize) {
                    *s = w;
                }
            }
            None => x.motor_w_body = w,
        }
    }

    /// Grab links: phys_grab::grab_force between a's hand and b's ankle; +F on b ankle, −F on a hand. Auto-release on break.
    fn grab_tick(&mut self) {
        let p = self.grab_params;
        let mut broken = Vec::new();
        for ai in 0..self.bodies.len() {
            let Some(link) = self.bodies[ai].as_ref().and_then(|x| x.grab) else {
                continue;
            };
            let bi = link.other.0 as usize;
            if self.bodies.get(bi).and_then(|x| x.as_ref()).is_none() {
                broken.push(BodyId(ai as u32));
                continue;
            }
            let (h, k) = (link.hand.0 as usize, link.ankle.0 as usize);
            let (Some(ja), Some(jb)) = (
                self.bodies[ai]
                    .as_ref()
                    .unwrap()
                    .state
                    .joints
                    .get(h)
                    .copied(),
                self.bodies[bi]
                    .as_ref()
                    .unwrap()
                    .state
                    .joints
                    .get(k)
                    .copied(),
            ) else {
                broken.push(BodyId(ai as u32));
                continue;
            };
            let (pa, pb) = (ja.pos, jb.pos);
            // Synced: point velocity v_com + w × (p − com_seg) from MeasuredJoint; unsynced (lab): position difference.
            let fd = link
                .prev
                .map(|(qa, qb)| ((pa - qa) * (1.0 / DT), (pb - qb) * (1.0 / DT)))
                .unwrap_or((Vec3::ZERO, Vec3::ZERO));
            let va = point_vel(self.bodies[ai].as_ref().unwrap(), h, pa).unwrap_or(fd.0);
            let vb = point_vel(self.bodies[bi].as_ref().unwrap(), k, pb).unwrap_or(fd.1);
            let ma: f64 = self.bodies[ai]
                .as_ref()
                .unwrap()
                .rig
                .joints
                .iter()
                .map(|j| j.mass)
                .sum();
            let mb: f64 = self.bodies[bi]
                .as_ref()
                .unwrap()
                .rig
                .joints
                .iter()
                .map(|j| j.mass)
                .sum();
            let (f, brk) = crate::phys_grab::grab_force(
                pa,
                va,
                pb,
                vb,
                crate::phys_grab::reduced_mass(ma, mb),
                p,
            );
            if brk {
                broken.push(BodyId(ai as u32));
                continue;
            }
            let sc = crate::phys_grab::spin_center(
                self.bodies[ai].as_ref().unwrap().state.com,
                ma,
                self.bodies[bi].as_ref().unwrap().state.com,
                mb,
            );
            if let Some(l) = self.bodies[ai].as_mut().unwrap().grab.as_mut() {
                l.prev = Some((pa, pb));
            }
            let a = &mut self.bodies[ai].as_mut().unwrap().state;
            a.ext_forces.push((link.hand, -f));
            a.grab_tension = f.length();
            a.spin_center = Some(sc);
            let b = &mut self.bodies[bi].as_mut().unwrap().state;
            b.ext_forces.push((link.ankle, f));
            b.grab_tension = f.length();
            b.spin_center = Some(sc);
        }
        for a in broken {
            self.release_grab(a);
        }
    }

    fn body_ref(&self, b: BodyId) -> Option<&Body> {
        self.bodies.get(b.0 as usize).and_then(|x| x.as_ref())
    }

    pub fn read_state(&self, b: BodyId) -> BodyState {
        self.body_ref(b)
            .map(|x| x.state.clone())
            .unwrap_or_default()
    }

    fn body_mut(&mut self, b: BodyId) -> Option<&mut Body> {
        self.bodies.get_mut(b.0 as usize).and_then(|x| x.as_mut())
    }
}

/// Mode state machine, run once per tick right after the state refresh (before torques on synced bodies).
/// Recover / get-up motor weight ramp (ticks, ~0.3 s).
pub const RECOVER_RAMP_TICKS: u64 = 36;

pub(crate) fn update_mode(b: &mut Body, t: &Tuning) {
    use crate::phys_air;
    let down = b.state.feet_down != [false, false];
    b.air_count = if down {
        0
    } else {
        b.air_count.saturating_add(1)
    };
    let rest = b.rest_com_h.unwrap_or(b.state.com.y.max(0.3));
    let (com, v) = (b.state.com, b.state.com_v);
    let t_land = if down {
        None
    } else {
        phys_air::time_to_land(com, v, rest)
    };
    let feet = |b: &Body| -> Option<[Vec3; 2]> {
        let tl = t_land?;
        let land = phys_air::landing_com(com, v, tl, rest);
        let side = b.root.rot.rotate(Vec3::new(1.0, 0.0, 0.0));
        let side = Vec3::new(side.x, 0.0, side.z);
        let side = if side.length() > 1e-6 {
            side * (1.0 / side.length())
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        Some(phys_air::foot_targets(
            land,
            v,
            rest,
            side,
            b.rig.sole.half_width * 2.0,
            0.0,
        ))
    };
    let touchdown = |b: &mut Body| {
        b.body_mode = BodyMode::Grounded;
        b.mode_w = 1.0;
        b.on_tick = b.ticks;
        b.com_anchor = None;
        b.state.stand_failed = false;
    };
    match b.body_mode {
        BodyMode::Grounded => {
            if b.mode == Mode::Physical && b.air_count > t.airborne_ticks {
                b.body_mode = BodyMode::Airborne;
            }
            b.mode_w = 1.0;
        }
        BodyMode::Airborne => {
            b.mode_w = 1.0;
            if down {
                touchdown(b);
            }
        }
        BodyMode::Ragdoll => {
            b.mode_w = 0.0;
        }
        BodyMode::Recovering => {
            let u = b
                .ramp_from
                .map(|r0| (b.ticks.saturating_sub(r0) as f64 / RECOVER_RAMP_TICKS as f64).min(1.0))
                .unwrap_or(1.0);
            let ramp = u * u * (3.0 - 2.0 * u);
            if down && ramp >= 1.0 {
                b.ramp_from = None;
                touchdown(b);
            } else if down {
                b.mode_w = ramp;
            } else {
                b.mode_w = t_land
                    .map(|tl| phys_air::recover_weight(tl, 0.1, t.recover_margin_s))
                    .unwrap_or(ramp);
            }
        }
        BodyMode::Grabbing | BodyMode::Grabbed => {
            b.mode_w = 1.0;
        }
    }
    let airish = matches!(
        b.body_mode,
        BodyMode::Airborne | BodyMode::Recovering | BodyMode::Ragdoll
    );
    b.state.mode = b.body_mode;
    b.state.t_land = if airish { t_land } else { None };
    b.state.air_feet = if matches!(b.body_mode, BodyMode::Airborne | BodyMode::Recovering) {
        feet(b)
    } else {
        None
    };
    b.state.launch_impulse = b.pending_launch.take();
    b.state.motor_weight = b.motor_w_body * b.mode_w;
}

/// Velocity of world point `p` on joint i's segment from the last synced measurement: v_com + w × (p − com).
fn point_vel(b: &Body, i: usize, p: Vec3) -> Option<Vec3> {
    let m = b.last_measured.as_ref()?.get(i)?;
    let com = m.pos + m.q.normalize().rotate(b.rig.joints.get(i)?.com);
    Some(m.v + m.w.cross(p - com))
}

#[cfg(test)]
mod terrain_tests {
    use super::*;
    #[test]
    fn contacts_follow_traced_floor_at_any_world_height_and_misses_remove_support() {
        for floor in [-3.0, 2.5] {
            let rig = crate::phys_rig::load_rig(crate::phys_rig::RIG_JSON, "master", 3.0)
                .unwrap()
                .profile;
            let mut sim = Sim::new();
            let id = sim.spawn_body(BodySpec {
                rig,
                root: Iso {
                    pos: Vec3::new(0.0, floor + 1.0, 0.0),
                    rot: Quat::IDENTITY,
                },
                mode: Mode::Physical,
            });
            let measured: Vec<_> = sim
                .read_state(id)
                .joints
                .iter()
                .map(|j| MeasuredJoint {
                    pos: j.pos,
                    q: j.q_world,
                    ..Default::default()
                })
                .collect();
            sim.set_support_heights(id, Some([Some(floor); 2]));
            for _ in 0..80 {
                sim.sync_measured(id, &measured);
                sim.step();
            }
            let state = sim.read_state(id);
            assert_eq!(state.feet_down, [true, true]);
            assert!(!state.stand_failed);
            assert!((state.cop.y - floor).abs() < 1e-8);
            sim.set_support_heights(id, Some([None, None]));
            for _ in 0..40 {
                sim.sync_measured(id, &measured);
                sim.step();
            }
            assert_eq!(sim.read_state(id).feet_down, [false, false]);
            assert_eq!(sim.read_state(id).foot_share, [0.0, 0.0]);
        }
    }
}
