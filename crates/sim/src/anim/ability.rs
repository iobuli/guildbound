//! Ability contract and implementations: gait (Effort), strike flow (marks authored strike frames),
//! flare (Additive, Style), acrobatics (Replace, Style), plus `acro_active` and the strike boost.
//! Pure: same ctx + tick -> same output. Rotations are model-space hinge deltas unless a rest rig is
//! attached to the `Frame` (then converted with `Rig::to_local_post`). ω/α: see `target`.

use super::target::{BoneTarget, Frame};
use super::{acro, flare, flow};
use crate::math::Vec3;

pub use crate::anim_acro_active::{ACRO_EASE_TICKS, acro_active};

pub type Tick = u64;
pub type BoneIdx = u16;
pub type AbilityId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RigId {
    Master,
    Mannequin,
    Hero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Effort = 0,
    Style = 1,
    Balance = 2,
    Limits = 3,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayerMode {
    Replace,
    Additive,
    Masked(Vec<BoneIdx>),
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RootMotion {
    pub dpos: Vec3,
    pub dyaw: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimOutput {
    pub bones: Vec<(BoneIdx, BoneTarget)>,
    pub root: Option<RootMotion>,
    pub mode: LayerMode,
    pub weight: f32,
    pub priority: Priority,
    /// Inside an authored strike window: the scheduler must not smooth/inertialize.
    pub strike_window: bool,
}

impl AnimOutput {
    fn empty(mode: LayerMode) -> Self {
        Self {
            bones: vec![],
            root: None,
            mode,
            weight: 0.0,
            priority: Priority::Style,
            strike_window: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Contacts {
    pub left_foot: bool,
    pub right_foot: bool,
    pub left_hand: bool,
    pub right_hand: bool,
}

pub struct AnimCtx<'a> {
    pub rig: RigId,
    pub bone_count: u16,
    pub bone_names: &'a [&'a str],
    /// m/s, m/s², rad/s.
    pub speed: f32,
    pub accel: [f32; 3],
    pub yaw_rate: f32,
    /// [0,1)
    pub gait_phase: f32,
    pub contacts: Contacts,
}

pub trait AnimAbility: Send + Sync {
    fn id(&self) -> AbilityId;
    fn name(&self) -> &'static str;
    fn targets(&self, ctx: &AnimCtx, tick: Tick) -> AnimOutput;
    fn cancel_window(&self, _tick: Tick) -> Option<(Tick, Tick)> {
        None
    }
    fn finished(&self, _tick: Tick) -> bool {
        false
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AbilityEvent {
    Start {
        id: AbilityId,
        rig: RigId,
        params: Vec<f32>,
    },
    Cancel {
        id: AbilityId,
    },
    Handover {
        from: AbilityId,
        to: AbilityId,
        cut_tick: Tick,
    },
}

#[derive(Default)]
pub struct AbilityRegistry {
    abilities: Vec<Box<dyn AnimAbility>>,
    active: Vec<AbilityId>,
}

impl AbilityRegistry {
    pub fn register(&mut self, a: Box<dyn AnimAbility>) {
        self.abilities.push(a);
    }
    pub fn apply_event(&mut self, ev: &AbilityEvent) {
        match ev {
            AbilityEvent::Start { id, .. } => {
                if !self.active.contains(id) {
                    self.active.push(*id)
                }
            }
            AbilityEvent::Cancel { id } => self.active.retain(|a| a != id),
            AbilityEvent::Handover { from, to, .. } => {
                self.active.retain(|a| a != from);
                if !self.active.contains(to) {
                    self.active.push(*to)
                }
            }
        }
    }
    /// Outputs of active abilities, highest priority first.
    pub fn collect(&self, ctx: &AnimCtx, tick: Tick) -> Vec<AnimOutput> {
        let mut out: Vec<AnimOutput> = self
            .abilities
            .iter()
            .filter(|a| self.active.contains(&a.id()))
            .map(|a| a.targets(ctx, tick))
            .collect();
        out.sort_by(|a, b| b.priority.cmp(&a.priority));
        out
    }
}

/// Model lateral axis: hinge flexion is about +X.
const X: [f32; 3] = [1.0, 0.0, 0.0];
const DT32: f32 = super::DT as f32;

// ---------------------------------------------------------------- gait (Effort)
/// Leg swing + root motion from ctx.gait_phase / speed / yaw_rate. `stride_fn(speed) -> (stride m, cadence Hz)`.
pub struct GaitAbility {
    pub id: AbilityId,
    pub stride_fn: fn(f32) -> (f32, f32),
    pub leg_len: f32,
    pub frame: Frame,
}
pub fn default_stride(v: f32) -> (f32, f32) {
    let s = (0.6 + 0.37 * v).min(3.2);
    (s, if s > 0.0 { v / s } else { 0.0 })
}
impl GaitAbility {
    fn legs(&self, speed: f32, phase: f32) -> Vec<(BoneIdx, [f32; 4])> {
        use acro::bone::*;
        let (stride, _) = (self.stride_fn)(speed.max(0.0));
        let amp = ((0.25 * stride) / self.leg_len).clamp(0.0, 0.95).asin();
        let s = (std::f32::consts::TAU * phase).sin();
        let knee = |u: f32| 1.2 * amp * (std::f32::consts::TAU * u).sin().max(0.0);
        vec![
            (L_THIGH, flare::axis_angle(X, amp * s)),
            (L_CALF, flare::axis_angle(X, -knee(phase))),
            (R_THIGH, flare::axis_angle(X, -amp * s)),
            (R_CALF, flare::axis_angle(X, -knee(phase + 0.5))),
        ]
    }
}
impl AnimAbility for GaitAbility {
    fn id(&self) -> AbilityId {
        self.id
    }
    fn name(&self) -> &'static str {
        "gait"
    }
    fn targets(&self, ctx: &AnimCtx, tick: Tick) -> AnimOutput {
        use acro::bone::*;
        let dphase = (self.stride_fn)(ctx.speed.max(0.0)).1 * DT32;
        let raw = |t: i64| {
            self.legs(
                ctx.speed,
                (ctx.gait_phase + dphase * (t - tick as i64) as f32).rem_euclid(1.0),
            )
        };
        AnimOutput {
            bones: self.frame.targets(&raw, tick),
            root: Some(RootMotion {
                dpos: Vec3::new(0.0, 0.0, (ctx.speed * DT32) as f64),
                dyaw: (ctx.yaw_rate * DT32) as f64,
            }),
            mode: LayerMode::Masked(vec![L_THIGH, L_CALF, R_THIGH, R_CALF]),
            weight: 1.0,
            priority: Priority::Effort,
            strike_window: false,
        }
    }
}

// ---------------------------------------------------------------- strike flow (clip-driven)
/// One authored strike clip in a combo: emits no bones, flags active frames, exposes the cancel window.
pub struct StrikeFlowAbility {
    pub id: AbilityId,
    pub phases: flow::StrikePhases,
    pub hit_confirmed: bool,
}
impl AnimAbility for StrikeFlowAbility {
    fn id(&self) -> AbilityId {
        self.id
    }
    fn name(&self) -> &'static str {
        "strike_flow"
    }
    fn targets(&self, _ctx: &AnimCtx, tick: Tick) -> AnimOutput {
        let p = self.phases;
        AnimOutput {
            weight: 1.0,
            strike_window: tick >= p.strike && tick < p.recovery,
            ..AnimOutput::empty(LayerMode::Additive)
        }
    }
    fn cancel_window(&self, _tick: Tick) -> Option<(Tick, Tick)> {
        let w = flow::cancel_window(self.phases, self.hit_confirmed);
        Some((w.open, w.close))
    }
    fn finished(&self, tick: Tick) -> bool {
        tick >= self.phases.len
    }
}

// ---------------------------------------------------------------- flare (Additive, Style)
/// Strike twist + follow-through + lean from ctx.accel[2] + run swing.
pub struct FlareAbility {
    pub id: AbilityId,
    pub strike: flare::StrikeFlare,
    pub active_ticks: u32,
    pub follow_swing: f32,
    pub overshoot: f32,
    pub settle_s: f32,
    pub lean_gain: f32,
    pub lean_max: f32,
    pub run_max_speed: f32,
    pub frame: Frame,
}
impl FlareAbility {
    pub fn new(id: AbilityId) -> Self {
        Self {
            id,
            strike: flare::StrikeFlare::default(),
            active_ticks: 8,
            follow_swing: 0.5,
            overshoot: 0.15,
            settle_s: 0.3,
            lean_gain: 0.6,
            lean_max: 0.35,
            run_max_speed: 8.0,
            frame: Frame::default(),
        }
    }
    pub fn raw(&self, ctx: &AnimCtx, t: i64) -> Vec<(BoneIdx, [f32; 4])> {
        use flare::bone::*;
        let since = (t - self.strike.strike_tick as i64).max(0) as u32;
        let mut layers = vec![
            flare::strike_layer(&self.strike, t.max(0) as u32),
            flare::lean_layer(ctx.accel[2], self.lean_gain, self.lean_max),
        ];
        if t >= self.strike.strike_tick as i64 {
            layers.push(flare::follow_through_layer(
                since,
                self.follow_swing,
                self.overshoot,
                self.settle_s,
                self.strike.side,
            ));
        }
        if ctx.speed > 0.0 {
            layers.push(flare::run_layer(
                ctx.gait_phase,
                ctx.speed,
                self.run_max_speed,
            ));
        }
        let mut out = flare::compose(&layers);
        // fixed bone set so derivatives line up across ticks
        for b in [SPINE2, R_UPPERARM, R_FOREARM, L_UPPERARM, L_FOREARM] {
            if let Err(i) = out.binary_search_by_key(&b, |e| e.0) {
                out.insert(i, (b, [0.0, 0.0, 0.0, 1.0]));
            }
        }
        out
    }
}
impl AnimAbility for FlareAbility {
    fn id(&self) -> AbilityId {
        self.id
    }
    fn name(&self) -> &'static str {
        "flare"
    }
    fn targets(&self, ctx: &AnimCtx, tick: Tick) -> AnimOutput {
        let s = self.strike.strike_tick as u64;
        AnimOutput {
            bones: self.frame.targets(&|t| self.raw(ctx, t), tick),
            root: None,
            mode: LayerMode::Additive,
            weight: 1.0,
            priority: Priority::Style,
            strike_window: tick >= s && tick < s + self.active_ticks as u64,
        }
    }
    fn finished(&self, tick: Tick) -> bool {
        let settle = (self.settle_s * 1.5 * 120.0).round() as u64;
        tick > self.strike.strike_tick as u64 + settle + self.strike.recover_ticks as u64
    }
}

// ---------------------------------------------------------------- acrobatics (Replace, Style)
/// HOLD BACK: handspring travels 2.7 m and lands ~5.4 m/s; scale it down before live use.
pub struct AcroAbility {
    pub id: AbilityId,
    pub clip: acro::AcroClip,
    pub frame: Frame,
}
impl AcroAbility {
    pub fn handspring(id: AbilityId, back_step: bool) -> Self {
        Self {
            id,
            clip: acro::back_handspring(back_step),
            frame: Frame::default(),
        }
    }
    pub fn backflip(id: AbilityId, back_step: bool) -> Self {
        Self {
            id,
            clip: acro::backflip(back_step),
            frame: Frame::default(),
        }
    }
    fn raw(&self, t: i64) -> Vec<(BoneIdx, [f32; 4])> {
        // out of range -> empty, so clip edges get one-sided derivatives
        if t < 0 || t >= self.clip.poses.len() as i64 {
            return vec![];
        }
        acro::AcroClip::bone_angles(&self.clip.poses[t as usize])
            .iter()
            .map(|&(b, a)| (b, flare::axis_angle(X, a as f32)))
            .collect()
    }
    /// COM-pull fade weight for this clip started at `start` (see `acro_active`).
    pub fn active(&self, tick: Tick, start: Tick) -> f32 {
        acro_active(tick, start, self.clip.poses.len() as u64, ACRO_EASE_TICKS)
    }
}
impl AnimAbility for AcroAbility {
    fn id(&self) -> AbilityId {
        self.id
    }
    fn name(&self) -> &'static str {
        self.clip.name
    }
    fn targets(&self, _ctx: &AnimCtx, tick: Tick) -> AnimOutput {
        let n = self.clip.poses.len();
        if n == 0 {
            return AnimOutput::empty(LayerMode::Replace);
        }
        let i = (tick as usize).min(n - 1);
        let (a, b) = (
            self.clip.poses[i.saturating_sub(1)].pelvis,
            self.clip.poses[i].pelvis,
        );
        let root = if i == 0 {
            RootMotion::default()
        } else {
            RootMotion {
                dpos: Vec3::new(0.0, b[1] - a[1], b[0] - a[0]),
                dyaw: 0.0,
            }
        };
        AnimOutput {
            bones: self.frame.targets(&|t| self.raw(t), tick),
            root: Some(root),
            mode: LayerMode::Replace,
            weight: 1.0,
            priority: Priority::Style,
            strike_window: false,
        }
    }
    fn finished(&self, tick: Tick) -> bool {
        tick as usize >= self.clip.poses.len()
    }
}

// ---------------------------------------------------------------- strike boost
pub const STRIKE_BOOST_TICKS: u64 = 6;
pub const STRIKE_BOOST_COOLDOWN: u64 = 12;

/// Strike boost window: 6 ticks on, then a 12-tick cooldown (from the end of the boost).
/// Anim carries no gains: `value(tick)` in {0,1} is the only output. It goes to
/// `Sim::set_fade(b, strike_b, acro_a)`; the motor applies the arm-only Kp boost and the k_com fade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StrikeBoost {
    start: Option<Tick>,
}
impl StrikeBoost {
    /// Try to start a boost at `tick`; false while boosted or cooling down.
    pub fn trigger(&mut self, tick: Tick) -> bool {
        if let Some(s) = self.start {
            if tick < s + STRIKE_BOOST_TICKS + STRIKE_BOOST_COOLDOWN {
                return false;
            }
        }
        self.start = Some(tick);
        true
    }
    pub fn value(&self, tick: Tick) -> f64 {
        match self.start {
            Some(s) if tick >= s && tick < s + STRIKE_BOOST_TICKS => 1.0,
            _ => 0.0,
        }
    }
}

/// Pure fade weights `(strike_b, acro_a)` in [0,1] for `Sim::set_fade(b, strike_b, acro_a)`.
/// `acro` is the running clip with its start tick, if any.
pub fn fade_weights(
    boost: &StrikeBoost,
    acro: Option<(&AcroAbility, Tick)>,
    tick: Tick,
) -> (f64, f64) {
    let a = acro.map_or(0.0, |(c, start)| c.active(tick, start) as f64);
    (boost.value(tick).clamp(0.0, 1.0), a.clamp(0.0, 1.0))
}

/// Thin wrapper: push this tick's fade weights to the motor writer.
pub fn apply_fade(
    sim: &mut crate::api::Sim,
    b: crate::api::BodyId,
    boost: &StrikeBoost,
    acro: Option<(&AcroAbility, Tick)>,
    tick: Tick,
) {
    let (s, a) = fade_weights(boost, acro, tick);
    sim.set_fade(b, s, a);
}
