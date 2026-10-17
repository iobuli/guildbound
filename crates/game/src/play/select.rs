//! The motion selector (`Update / PlaySet::Select`): the ONLY writer of `MotionLayers.layers`.
//! Builds per character: locomotion base (idle / walk / jog / run / sprint / crouch / strafe /
//! back / turn, phase-synced by `Mover.phase` and blended by speed; air start/loop/fall/land;
//! swim; climb), the equipped moveset's `Stance` overlay, then `Action.layer` and
//! `Action.overlay`. Fills `ContactRequest` (foot IK off in air/swim/ragdoll, accel, squash from
//! landing/jump, look_at from Intent.target, and the `MotionFeel.bounce` character: a pelvis dip
//! after every foot plant with its vertical acceleration in `accel.y`, a squash/stretch rhythm
//! on the same beat, idle breathing). Hand and foot targets (`HandTargets`, `FootTargets`) pass
//! through with smoothed weights; a `Posture` adds its balance lean to `accel` and fades the
//! narrow stance in and out. Never touches `MotionLayers.impulses` / `raw`.
//!
//! Layer stack per frame (bottom → top):
//! 1. locomotion base: smoothed per-track weights stacked by `gait_layers` (Full mask),
//! 2. turn-in-place (Lower), 3. landing (Full, weight `Mover.landing`),
//! 4. stance (Upper), 5. `Action.layer`, 6. `Action.overlay`.
//!
//! Every weight is critically damped per track (`SelectState`), so mode, gait, direction and
//! action changes cross-fade instead of popping. Ending actions fade out frozen on their last
//! frame; a restarted clip (time jumps backwards) cross-fades from its old copy.
use super::moveset::{Movesets, Stance};
use super::{Action, ActionKind, Feel, Intent, Loadout, MoveMode, Mover, PlaySet, HALF_HEIGHT};
use crate::motion::bank::PoseBank;
use crate::motion::compose::{MotionFeel, StyleScale};
use crate::motion::timing::Style;
use crate::motion::{ContactRequest, Layer, Mask, MotionLayers, Source};
use bevy::prelude::*;
use std::collections::HashSet;
use std::f32::consts::{FRAC_PI_2, TAU};

pub fn plugin(app: &mut App) {
    app.add_systems(Update, select.in_set(PlaySet::Select));
}

/// Bank names of the locomotion tracks.
mod tracks {
    pub const IDLE: &str = "ual1/Idle_Loop";
    pub const WALK: &str = "ual1/Walk_Loop";
    pub const JOG: &str = "ual1/Jog_Fwd_Loop";
    pub const RUN: &str = "sekiro/Run-Forwards";
    pub const SPRINT: &str = "ual1/Sprint_Loop";
    pub const CROUCH_IDLE: &str = "ual1/Crouch_Idle_Loop";
    pub const CROUCH_MOVE: &str = "ual1/Crouch_Fwd_Loop";
    pub const WALK_BACK: &str = "sekiro/Walk-Backwards";
    pub const JOG_BACK: &str = "sekiro/Jog Backward";
    pub const WALK_LEFT: &str = "sekiro/Walk-Left";
    pub const WALK_RIGHT: &str = "sekiro/Walk-Right";
    pub const JOG_LEFT: &str = "sekiro/Jog Strafe Left";
    pub const JOG_RIGHT: &str = "sekiro/Jog Strafe Right";
    pub const TURN_LEFT: &str = "sekiro/Turn-Left-90";
    pub const TURN_RIGHT: &str = "sekiro/Turn-Right-90";
    pub const JUMP_START: &str = "ual1/Jump_Start";
    pub const JUMP_LOOP: &str = "ual1/Jump_Loop";
    pub const JUMP_LAND: &str = "ual1/Jump_Land";
    pub const FALL: &str = "sekiro/Falling Idle";
    /// Running (parkour) jump: athletic leap and airborne hold.
    pub const LEAP_START: &str = "ual2/NinjaJump_Start";
    pub const LEAP_AIR: &str = "ual2/NinjaJump_Idle_Loop";
    pub const SWIM_IDLE: &str = "ual1/Swim_Idle_Loop";
    pub const SWIM_FWD: &str = "ual1/Swim_Fwd_Loop";
    pub const CLIMB: &str = "ual2/ClimbUp_1m";
}
use tracks::*;

/// Critically damped smoothing time for locomotion / overlay weights (s).
const BLEND_TIME: f32 = 0.12;
/// Action layers fade in fast (responsiveness) and out a little slower (no pop).
const ACTION_IN: f32 = 0.05;
const ACTION_OUT: f32 = 0.12;
/// Foot IK comes on quickly at touchdown so feet never sink, and releases fast at takeoff.
const FOOT_IK_IN: f32 = 0.05;
const FOOT_IK_OUT: f32 = 0.06;
const HAND_TIME: f32 = 0.1;
/// Largest frame step the selector integrates (s).
const MAX_DT: f32 = 0.1;
/// Weights below this are treated as inactive.
const WEIGHT_EPS: f32 = 1e-3;
/// Free-running clock wraps here (keeps f32 precision over long sessions).
const CLOCK_WRAP: f32 = 3600.0;

/// Gait anchors: jog sits at this fraction of `Feel.run_speed`.
const JOG_FRACTION: f32 = 0.65;
/// Minimum spacing between gait anchors (m/s), keeps the partition valid for any `Feel`.
const MIN_ANCHOR_GAP: f32 = 0.1;
/// Track strides below this are unreliable; use the speed-based fallback instead.
const MIN_STRIDE: f32 = 0.3;
/// Crouch move reaches full weight at this fraction of walk speed.
const CROUCH_FULL_FRACTION: f32 = 0.6;
/// Below this horizontal speed the strafe direction is too noisy to use.
const STRAFE_MIN_SPEED: f32 = 0.15;

/// Air: Jump_Start covers the first part of upward flight, played from its push-off.
const JUMP_UP_SPEED: f32 = 0.5;
const JUMP_START_WINDOW: f32 = 0.15;
const JUMP_START_OFFSET: f32 = 0.1;
const JUMP_START_RATE: f32 = 1.5;
const FALL_SPEED: f32 = -4.0;
const FALL_AFTER: f32 = 0.6;
/// Running jump: takeoff speed (m/s) from which the leap poses replace the standing jump, and
/// the leap start's window, offset (just after its crouch) and rate. A leap hangs longer than a
/// standing hop before it reads as a fall.
const LEAP_SPEED: f32 = 3.0;
const LEAP_START_WINDOW: f32 = 0.25;
const LEAP_START_OFFSET: f32 = 0.2;
const LEAP_START_RATE: f32 = 1.2;
const LEAP_FALL_AFTER: f32 = 0.9;
/// Landing overlay: window (s), clip offset to just before the deepest crouch, playback rate.
const LAND_WINDOW: f32 = 0.28;
const LAND_OFFSET: f32 = 0.08;
const LAND_RATE: f32 = 1.2;
/// Contact squash / stretch (scaled by 0.5 + 0.5·bounce).
const LAND_SQUASH: f32 = 0.25;
const JUMP_STRETCH: f32 = 0.12;
const STRETCH_WINDOW: f32 = 0.1;
/// Step bounce at `MotionFeel.bounce` = 1: pelvis dip (m) per gait [idle, walk, jog, run,
/// sprint], halved while crouched; its lowest point trails each plant by `BOB_LAG` of a cycle
/// (weight acceptance). Squash at each plant and stretch at passing ride the same rhythm.
const BOB: [f32; 5] = [0.0, 0.03, 0.045, 0.05, 0.04];
const BOB_LAG: f32 = 0.04;
const CROUCH_BOB: f32 = 0.5;
const STEP_SQUASH: f32 = 0.06;
const STEP_STRETCH: f32 = 0.03;
/// Idle breathing: squash amplitude and period (s).
const BREATH: f32 = 0.035;
const BREATH_PERIOD: f32 = 3.4;

/// Turn in place: yaw rate threshold (rad/s), ramp to full, max overlay weight, idle speed.
const TURN_RATE_MIN: f32 = 2.0;
const TURN_RATE_RAMP: f32 = 1.5;
const TURN_WEIGHT: f32 = 0.6;
const TURN_IDLE_SPEED: f32 = 0.5;

/// Swim forward reaches full weight at this speed (m/s).
const SWIM_FULL_SPEED: f32 = 1.2;
/// Wall climb: metres per ping-pong cycle and the share of `ClimbUp_1m` used as the reach.
const CLIMB_STEP: f32 = 0.6;
const CLIMB_SPAN: f32 = 0.35;

/// Look-at height above the target's feet (m). Character roots (anything with a `Mover`) sit
/// at the capsule centre, so their feet are `HALF_HEIGHT` below the Transform; other targets
/// use their Transform as the base.
const LOOK_HEIGHT: f32 = 1.5;

/// Optional world-space hand IK targets `[left, right]` as (point, weight 0..1). `select`
/// forwards them into `ContactRequest.hands` with smoothed weights. Inserted / updated by
/// `ranged` (bow string), `traverse` (climb holds) and `interact` (levers, tools); remove it
/// or set `None` to release (the hand fades back to the animation).
#[derive(Component, Clone, Debug, Default)]
pub struct HandTargets {
    pub hands: [Option<(Vec3, f32)>; 2],
}

/// Optional world-space ankle IK targets `[left, right]` as (point, weight 0..1): climb holds.
/// `select` forwards them into `ContactRequest.feet` with smoothed weights, like `HandTargets`.
/// Written by `traverse`; remove it to release (the feet fade back to the animation).
#[derive(Component, Clone, Debug, Default)]
pub struct FootTargets {
    pub feet: [Option<(Vec3, f32)>; 2],
}

/// Posture requests beyond the limbs, written by `traverse` on the balance beam. `lean` (m/s²,
/// world, horizontal) is added to the acceleration the torso leans into and the arms trail;
/// `stance` (m) is the farthest a foot may stand from the body's mid-plane. `select` fades the
/// stance in when this appears and out when it is removed.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Posture {
    pub lean: Vec3,
    pub stance: f32,
}

/// Stance fade time (s), and a stance width (m) wider than any animated one (the fade starts there).
const STANCE_TIME: f32 = 0.15;
const FREE_STANCE: f32 = 0.5;

/// How a layer's clip time is driven.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Clock {
    /// Locomotion loop locked to `Mover.phase`: t = fract(phase + left_contact) · duration.
    Synced,
    /// Free-running loop on the character clock (idle, swim idle, jump loop, falling).
    Free,
    /// Explicit clip time set by the selector or an action (clamped to the clip).
    Fixed(f32),
}

/// Per-track loop data `gait_layers` needs (from `extract::Track`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoopInfo {
    pub duration: f32,
    /// Normalized phase where the left foot plants (0 when unknown).
    pub left_contact: f32,
    /// Ground distance per cycle (m), 0 when unknown.
    pub stride: f32,
}

/// Gait anchor speeds (m/s) for [idle, walk, jog, run, sprint], strictly increasing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GaitAnchors(pub [f32; 5]);

impl GaitAnchors {
    /// Anchors at 0, walk, ~0.65·run, run and sprint speed, sanitized to stay increasing.
    pub fn from_feel(feel: &Feel) -> Self {
        let walk = finite_or(feel.walk_speed, 1.6).max(MIN_ANCHOR_GAP);
        let run = finite_or(feel.run_speed, 4.2);
        let sprint = finite_or(feel.sprint_speed, 6.8);
        let mut a = [0.0, walk, JOG_FRACTION * run, run, sprint];
        for i in 1..a.len() {
            a[i] = a[i].max(a[i - 1] + MIN_ANCHOR_GAP);
        }
        Self(a)
    }
}

/// Smooth partition of unity over [idle, walk, jog, run, sprint] for horizontal `speed`:
/// between two neighbouring anchors the weights are (1 − t, t) with t = smoothstep, so they
/// always sum to 1 and are C¹-continuous in speed. Below 0 → idle, above sprint → sprint.
pub fn gait_weights(speed: f32, anchors: &GaitAnchors) -> [f32; 5] {
    let a = anchors.0;
    let s = sane(speed).max(0.0);
    let mut w = [0.0; 5];
    if s <= a[0] {
        w[0] = 1.0;
        return w;
    }
    for i in 0..4 {
        if s < a[i + 1] {
            let span = (a[i + 1] - a[i]).max(1e-4);
            let t = smooth01((s - a[i]) / span);
            w[i] = 1.0 - t;
            w[i + 1] = t;
            return w;
        }
    }
    w[4] = 1.0;
    w
}

/// Local move angle relative to facing `yaw`: atan2(local x, local z), so 0 = forward,
/// +π/2 = left (+X is the character's left), ±π = back, −π/2 = right. None when not moving.
pub fn local_move_angle(velocity: Vec3, yaw: f32) -> Option<f32> {
    let v = Vec2::new(velocity.x, velocity.z);
    if !v.is_finite() || !yaw.is_finite() || v.length_squared() < 1e-6 {
        return None;
    }
    let (s, c) = yaw.sin_cos();
    let local_x = c * v.x - s * v.y;
    let local_z = s * v.x + c * v.y;
    Some(local_x.atan2(local_z))
}

/// Weights of the [forward, left, back, right] locomotion sets for local angle `theta`
/// (see `local_move_angle`): smoothstep between the two neighbouring cardinal directions,
/// always summing to 1.
pub fn direction_weights(theta: f32) -> [f32; 4] {
    let mut w = [0.0; 4];
    if !theta.is_finite() {
        w[0] = 1.0;
        return w;
    }
    let q = theta.rem_euclid(TAU) / FRAC_PI_2;
    let base = q.floor();
    let i = (base as usize) % 4;
    let t = smooth01(q - base);
    w[i] += 1.0 - t;
    w[(i + 1) % 4] += t;
    w
}

/// Stride used for phase advance: the track's own stride, or `1.1 + 0.35·speed` when the
/// track has no reliable stride (< 0.3 m).
pub fn stride_for(track_stride: f32, speed: f32) -> f32 {
    if track_stride.is_finite() && track_stride >= MIN_STRIDE {
        track_stride
    } else {
        1.1 + 0.35 * sane(speed).max(0.0)
    }
}

/// Weighted mean stride of `(weight, track_stride)` pairs (fallback per entry, see
/// `stride_for`). With no weight at all it returns the speed fallback.
pub fn blended_stride(entries: &[(f32, f32)], speed: f32) -> f32 {
    let (mut sum, mut total) = (0.0, 0.0);
    for &(w, stride) in entries {
        let w = sane(w).max(0.0);
        sum += w * stride_for(stride, speed);
        total += w;
    }
    if total > 1e-4 {
        sum / total
    } else {
        stride_for(0.0, speed)
    }
}

/// Advance the locomotion phase by travelled distance / stride, wrapped into [0, 1).
pub fn advance_phase(phase: f32, speed: f32, dt: f32, stride: f32) -> f32 {
    let stride = if stride.is_finite() && stride > 1e-3 {
        stride
    } else {
        stride_for(0.0, speed)
    };
    let step = sane(speed).max(0.0) * sane(dt).max(0.0) / stride;
    wrap01(sane(phase) + sane(step))
}

/// Clip time for `clock` (always inside [0, duration]).
pub fn clip_time(clock: Clock, info: LoopInfo, phase: f32, free: f32) -> f32 {
    let d = sane(info.duration).max(0.0);
    if d <= 1e-4 {
        return 0.0;
    }
    match clock {
        Clock::Synced => wrap01(sane(phase) + sane(info.left_contact)) * d,
        Clock::Free => {
            let t = sane(free).rem_euclid(d);
            if t >= d { 0.0 } else { t }
        }
        Clock::Fixed(t) => sane(t).clamp(0.0, d),
    }
}

/// Stack weighted locomotion tracks into layers `(track, time, layer weight)`.
/// `blends` = (track, clock, weight) in canonical order (idle first). Weights are normalized;
/// the first active entry is the full-weight base (idle when active) and every later entry i
/// gets w_i / Σ_{j≤i} w_j (sequential normalized blending), which reproduces the normalized
/// weights exactly for linear blending. A fixed order keeps the result continuous as weights
/// cross. Synced loops sample at fract(phase + left_contact)·duration so feet line up.
pub fn gait_layers(
    blends: &[(&str, Clock, f32)],
    phase: f32,
    clock: f32,
    info: &dyn Fn(&str) -> Option<LoopInfo>,
) -> Vec<(String, f32, f32)> {
    let total: f32 = blends.iter().map(|b| sane(b.2).max(0.0)).sum();
    if total <= WEIGHT_EPS {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(blends.len());
    let mut cumulative = 0.0;
    for &(track, kind, weight) in blends {
        let w = sane(weight).max(0.0) / total;
        if w <= WEIGHT_EPS * 0.1 {
            continue;
        }
        cumulative += w;
        let layer_weight = if out.is_empty() {
            1.0
        } else {
            (w / cumulative).clamp(0.0, 1.0)
        };
        let time = clip_time(kind, info(track).unwrap_or_default(), phase, clock);
        out.push((track.to_string(), time, layer_weight));
    }
    out
}

/// Step bounce at locomotion `phase` (plants at 0 and 0.5, lowest `BOB_LAG` after each) for dip
/// amplitude `amp` (m) and phase rate `rate` (cycles/s). Returns (dip ≤ 0 m, its vertical
/// acceleration m/s², rhythm c = 1 at the bottom of the dip, −1 at passing).
pub fn step_bounce(phase: f32, amp: f32, rate: f32) -> (f32, f32, f32) {
    let amp = sane(amp).max(0.0);
    let c = (2.0 * TAU * (sane(phase) - BOB_LAG)).cos();
    let w = 2.0 * TAU * sane(rate);
    // dip = −A/2·(1 + cos(4π(φ − lag))); d²/dt² with dφ/dt = rate gives A/2·(4π·rate)²·cos.
    (-amp * 0.5 * (1.0 + c), amp * 0.5 * w * w * c, c)
}

/// Critically damped approach of `current` toward `target` with smoothing time `tau` (s);
/// stable for any `dt` (Game Programming Gems 4, critically damped ease).
pub fn smooth_damp(current: f32, target: f32, velocity: &mut f32, tau: f32, dt: f32) -> f32 {
    let (current, target) = (sane(current), sane(target));
    let dt = sane(dt).max(0.0);
    if dt <= 0.0 {
        return current;
    }
    let omega = 2.0 / tau.max(1e-3);
    let x = omega * dt;
    let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (sane(*velocity) + omega * change) * dt;
    *velocity = sane((sane(*velocity) - omega * temp) * decay);
    let out = target + (change + temp) * decay;
    if out.is_finite() {
        out
    } else {
        *velocity = 0.0;
        target
    }
}

#[inline]
fn sane(x: f32) -> f32 {
    if x.is_finite() { x } else { 0.0 }
}

#[inline]
fn finite_or(x: f32, fallback: f32) -> f32 {
    if x.is_finite() { x } else { fallback }
}

#[inline]
fn sane_vec(v: Vec3) -> Vec3 {
    if v.is_finite() { v } else { Vec3::ZERO }
}

#[inline]
fn smooth01(t: f32) -> f32 {
    let t = sane(t).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn wrap01(x: f32) -> f32 {
    let r = sane(x).rem_euclid(1.0);
    if r >= 1.0 { 0.0 } else { r }
}

/// One smoothed layer candidate.
#[derive(Clone, Debug)]
struct Slot {
    track: String,
    mask: Mask,
    clock: Clock,
    looping: bool,
    style: Option<Style>,
    /// Canonical order inside the channel (base channel: idle first, slow → fast).
    rank: u16,
    target: f32,
    weight: f32,
    velocity: f32,
    /// A restarted copy took over; this one only fades out.
    retired: bool,
}

/// A set of smoothed layers. Untargeted slots fade out keeping their last clock (Fixed times
/// freeze), then drop.
#[derive(Clone, Debug, Default)]
struct Channel {
    slots: Vec<Slot>,
    /// Non-looping clips whose time jumps backwards start a fresh copy (cross-fade restart).
    restart_on_rewind: bool,
}

impl Channel {
    fn restarting() -> Self {
        Self {
            slots: Vec::new(),
            restart_on_rewind: true,
        }
    }

    fn begin(&mut self) {
        for s in &mut self.slots {
            s.target = 0.0;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn want(
        &mut self,
        track: &str,
        mask: Mask,
        clock: Clock,
        looping: bool,
        style: Option<Style>,
        weight: f32,
        rank: u16,
    ) {
        let weight = sane(weight).clamp(0.0, 1.0);
        let found = self
            .slots
            .iter()
            .position(|s| !s.retired && s.track == track && s.mask == mask);
        if let Some(i) = found {
            let rewound = match (self.slots[i].clock, clock) {
                (Clock::Fixed(old), Clock::Fixed(new)) => new < old - 0.05,
                _ => false,
            };
            let restart =
                self.restart_on_rewind && !looping && rewound && self.slots[i].weight > WEIGHT_EPS;
            if !restart {
                let s = &mut self.slots[i];
                s.target = (s.target + weight).min(1.0);
                s.clock = clock;
                s.looping = looping;
                s.style = style;
                return;
            }
            self.slots[i].retired = true;
        }
        let slot = Slot {
            track: track.to_string(),
            mask,
            clock,
            looping,
            style,
            rank,
            target: weight,
            weight: 0.0,
            velocity: 0.0,
            retired: false,
        };
        let at = self
            .slots
            .iter()
            .position(|s| s.rank > rank)
            .unwrap_or(self.slots.len());
        self.slots.insert(at, slot);
    }

    /// Smooth every slot toward its target (`rise` / `fall` smoothing times), drop dead ones.
    fn settle(&mut self, dt: f32, rise: f32, fall: f32, snap: bool) {
        for s in &mut self.slots {
            if snap {
                s.weight = s.target;
                s.velocity = 0.0;
            } else {
                let tau = if s.target > s.weight { rise } else { fall };
                s.weight = smooth_damp(s.weight, s.target, &mut s.velocity, tau, dt).clamp(0.0, 1.0);
            }
        }
        self.slots
            .retain(|s| s.target > 0.0 || s.weight > WEIGHT_EPS);
    }

    #[cfg(test)]
    fn weight_of(&self, track: &str) -> f32 {
        self.slots
            .iter()
            .filter(|s| s.track == track)
            .map(|s| s.weight)
            .sum()
    }

    /// Emit active slots as layers (time = their Fixed clock), bottom → top.
    fn emit(&self, out: &mut Vec<Layer>) {
        for s in &self.slots {
            if s.weight <= WEIGHT_EPS {
                continue;
            }
            let time = match s.clock {
                Clock::Fixed(t) => sane(t).max(0.0),
                _ => 0.0,
            };
            out.push(Layer {
                source: Source::Track {
                    name: s.track.clone(),
                    time,
                    looping: s.looping,
                },
                weight: s.weight,
                mask: s.mask,
                style: s.style,
            });
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct HandFade {
    point: Vec3,
    weight: f32,
    velocity: f32,
}

/// Per-character selector memory, inserted by `select` the first time it sees a character:
/// smoothed layer weights, the free-running loop clock and small timers.
#[derive(Component, Clone, Debug)]
pub struct SelectState {
    base: Channel,
    turn: Channel,
    stance: Channel,
    action: Channel,
    overlay: Channel,
    clock: f32,
    /// Seconds since the last touchdown.
    land_t: f32,
    prev_mode: MoveMode,
    prev_landing: f32,
    turn_progress: f32,
    climb_phase: f32,
    foot_ik: f32,
    foot_ik_velocity: f32,
    hands: [HandFade; 2],
    feet: [HandFade; 2],
    /// `Posture.stance` fade: weight 0..1, its rate, and the last requested width (m).
    narrow: f32,
    narrow_velocity: f32,
    narrow_width: f32,
    started: bool,
}

impl Default for SelectState {
    fn default() -> Self {
        Self {
            base: Channel::restarting(),
            turn: Channel::restarting(),
            stance: Channel::default(),
            action: Channel::restarting(),
            overlay: Channel::restarting(),
            clock: 0.0,
            land_t: LAND_WINDOW,
            prev_mode: MoveMode::Ground,
            prev_landing: 0.0,
            turn_progress: 0.0,
            climb_phase: 0.0,
            foot_ik: 1.0,
            foot_ik_velocity: 0.0,
            hands: [HandFade::default(); 2],
            feet: [HandFade::default(); 2],
            narrow: 0.0,
            narrow_velocity: 0.0,
            narrow_width: FREE_STANCE,
            started: false,
        }
    }
}

/// Resolved locomotion track names (missing tracks fall back to a neighbour, or `None`).
struct Loco<'a> {
    /// [forward, left, back, right] × [idle, walk, jog, run, sprint].
    sets: [[Option<&'a str>; 5]; 4],
    crouch: [Option<&'a str>; 2],
}

fn pick<'a>(bank: &PoseBank, options: &[Option<&'a str>]) -> Option<&'a str> {
    options.iter().flatten().copied().find(|n| bank.has(n))
}

impl<'a> Loco<'a> {
    fn resolve(bank: &PoseBank, stance: Option<&'a Stance>) -> Self {
        let stance_idle = stance.and_then(|s| s.idle.as_deref());
        let stance_walk = stance.and_then(|s| s.walk.as_deref());
        let idle = pick(bank, &[stance_idle, Some(IDLE)]);
        let walk = pick(bank, &[stance_walk, Some(WALK), Some(JOG)]);
        let jog = pick(bank, &[Some(JOG), Some(RUN), walk]);
        let run = pick(bank, &[Some(RUN), Some(JOG), Some(SPRINT)]);
        let sprint = pick(bank, &[Some(SPRINT), Some(RUN), Some(JOG)]);
        let fwd = [idle, walk, jog, run, sprint];
        let side = |walk_track: &'static str, jog_track: &'static str| {
            let w = pick(bank, &[Some(walk_track)]).or(fwd[1]);
            let j = pick(bank, &[Some(jog_track), Some(walk_track)]).or(fwd[2]);
            [idle, w, j, j, j]
        };
        Self {
            sets: [
                fwd,
                side(WALK_LEFT, JOG_LEFT),
                side(WALK_BACK, JOG_BACK),
                side(WALK_RIGHT, JOG_RIGHT),
            ],
            crouch: [
                pick(bank, &[Some(CROUCH_IDLE), idle]),
                pick(bank, &[Some(CROUCH_MOVE), walk]),
            ],
        }
    }
}

type CharacterQuery<'a> = (
    Entity,
    &'a mut Mover,
    &'a mut MotionLayers,
    Option<&'a Action>,
    Option<&'a Intent>,
    Option<&'a Loadout>,
    (Option<&'a HandTargets>, Option<&'a FootTargets>, Option<&'a Posture>),
    Option<&'a mut SelectState>,
    Option<&'a StyleScale>,
);

#[allow(clippy::too_many_arguments)]
fn select(
    mut commands: Commands,
    time: Res<Time>,
    feel: Res<Feel>,
    bank: Option<Res<PoseBank>>,
    movesets: Option<Res<Movesets>>,
    targets: Query<(&Transform, Has<Mover>)>,
    mut characters: Query<CharacterQuery>,
    motion: Option<Res<MotionFeel>>,
    mut missing: Local<HashSet<String>>,
) {
    let Some(bank) = bank else {
        return;
    };
    let frame = Frame {
        bank: &bank,
        anchors: GaitAnchors::from_feel(&feel),
        dt: sane(time.delta_secs()).clamp(0.0, MAX_DT),
        bounce: motion.map_or(1.0, |m| sane(m.bounce).clamp(0.0, 2.0)),
    };
    for (entity, mut mover, mut layers, action, intent, loadout, (hands, feet, posture), state, scale) in &mut characters {
        let frame = Frame {
            bounce: (frame.bounce * scale.map_or(1.0, |s| sane(s.bounce))).clamp(0.0, 2.5),
            ..frame
        };
        let stance = loadout
            .zip(movesets.as_deref())
            .and_then(|(l, m)| m.get(l.equipped()))
            .and_then(|m| m.stance.as_ref());
        let look_at = intent
            .and_then(|i| i.target)
            .filter(|t| *t != entity)
            .and_then(|t| targets.get(t).ok())
            .map(|(tf, is_character)| {
                let base = if is_character {
                    tf.translation - Vec3::Y * HALF_HEIGHT
                } else {
                    tf.translation
                };
                base + Vec3::Y * LOOK_HEIGHT
            })
            .filter(|p| p.is_finite());
        let strafing = mover.face.is_some() || intent.is_some_and(|i| i.target.is_some());
        let input = CharacterInput {
            stance,
            action,
            hands,
            feet,
            posture,
            look_at,
            strafing,
        };
        match state {
            Some(mut state) => {
                update_character(&frame, &input, &mut mover, &mut state, &mut layers, &mut missing)
            }
            None => {
                let mut state = SelectState::default();
                update_character(&frame, &input, &mut mover, &mut state, &mut layers, &mut missing);
                commands.entity(entity).try_insert(state);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Frame<'a> {
    bank: &'a PoseBank,
    anchors: GaitAnchors,
    dt: f32,
    /// `MotionFeel.bounce`.
    bounce: f32,
}

struct CharacterInput<'a> {
    stance: Option<&'a Stance>,
    action: Option<&'a Action>,
    hands: Option<&'a HandTargets>,
    feet: Option<&'a FootTargets>,
    posture: Option<&'a Posture>,
    look_at: Option<Vec3>,
    strafing: bool,
}

fn update_character(
    frame: &Frame,
    input: &CharacterInput,
    mover: &mut Mover,
    state: &mut SelectState,
    out: &mut MotionLayers,
    missing: &mut HashSet<String>,
) {
    let (bank, dt) = (frame.bank, frame.dt);
    let snap = !state.started;
    state.clock = (state.clock + dt) % CLOCK_WRAP;

    if mover.mode == MoveMode::Ragdoll {
        // `ragdoll` owns the layers; foot IK ramps back in once it hands control back. A state
        // first seen here stays un-started so the first selected frame snaps its weights.
        state.prev_mode = MoveMode::Ragdoll;
        state.foot_ik = 0.0;
        state.foot_ik_velocity = 0.0;
        return;
    }
    state.started = true;

    let velocity = sane_vec(mover.velocity);
    let speed = Vec2::new(velocity.x, velocity.z).length();
    let mode = mover.mode;

    // Touchdown bookkeeping for the landing overlay.
    let landing = sane(mover.landing).clamp(0.0, 1.0);
    let touched_down = state.prev_mode == MoveMode::Air && mode == MoveMode::Ground;
    if touched_down || landing > state.prev_landing + 0.05 {
        state.land_t = 0.0;
    } else {
        state.land_t = (state.land_t + dt).min(10.0);
    }
    state.prev_landing = landing;
    state.prev_mode = mode;

    // 1. Locomotion base.
    let loco = Loco::resolve(bank, input.stance);
    let gait = gait_weights(speed, &frame.anchors);
    state.base.begin();
    let mut phase_speed = speed;
    match mode {
        MoveMode::Ground => ground_targets(&mut state.base, &loco, mover, input, speed, gait, frame),
        MoveMode::Air => air_targets(&mut state.base, &loco, bank, mover, velocity),
        MoveMode::Swim => {
            phase_speed = velocity.length();
            let forward = smooth01(phase_speed / SWIM_FULL_SPEED);
            if let Some(t) = pick(bank, &[Some(SWIM_IDLE), loco.sets[0][0]]) {
                state.base.want(t, Mask::Full, Clock::Free, true, None, 1.0 - forward, 80);
            }
            if let Some(t) = pick(bank, &[Some(SWIM_FWD), loco.sets[0][1]]) {
                state.base.want(t, Mask::Full, Clock::Synced, true, None, forward, 81);
            }
        }
        MoveMode::Climb => {
            state.climb_phase = wrap01(state.climb_phase + velocity.length() * dt / CLIMB_STEP);
            if bank.has(CLIMB) {
                // Ping-pong over the reach part of the clip. Flagged looping so the backward
                // half is not mistaken for a restart (the time never reaches the clip end).
                let reach = 1.0 - (2.0 * state.climb_phase - 1.0).abs();
                let t = reach * CLIMB_SPAN * bank.duration(CLIMB);
                state.base.want(CLIMB, Mask::Full, Clock::Fixed(t), true, None, 1.0, 96);
            }
        }
        MoveMode::Ledge => {
            if bank.has(CLIMB) {
                state.base.want(CLIMB, Mask::Full, Clock::Fixed(0.0), false, None, 1.0, 96);
            }
        }
        MoveMode::Ragdoll => {}
    }
    state.base.settle(dt, BLEND_TIME, BLEND_TIME, snap);

    // Phase: travelled distance over the blended stride of the visible synced loops.
    let strides: Vec<(f32, f32)> = state
        .base
        .slots
        .iter()
        .filter(|s| s.clock == Clock::Synced)
        .map(|s| (s.weight, bank.tracks.get(&s.track).map_or(0.0, |t| t.stride)))
        .collect();
    let stride = blended_stride(&strides, phase_speed);
    mover.phase = advance_phase(mover.phase, phase_speed, dt, stride);

    let blends: Vec<(&str, Clock, f32)> = state
        .base
        .slots
        .iter()
        .map(|s| (s.track.as_str(), s.clock, s.weight))
        .collect();
    let info = |name: &str| {
        bank.tracks.get(name).map(|t| LoopInfo {
            duration: t.duration,
            left_contact: t.left_contact.unwrap_or(0.0),
            stride: t.stride,
        })
    };
    let stacked = gait_layers(&blends, mover.phase, state.clock, &info);
    out.layers.clear();
    for (track, time, weight) in stacked {
        let looping = state
            .base
            .slots
            .iter()
            .find(|s| s.track == track)
            .is_none_or(|s| s.looping);
        out.layers.push(Layer {
            source: Source::Track {
                name: track,
                time,
                looping,
            },
            weight,
            mask: Mask::Full,
            style: None,
        });
    }
    if out.layers.is_empty() {
        out.layers.push(Layer {
            source: Source::Pose(bank.rest.clone()),
            weight: 1.0,
            mask: Mask::Full,
            style: None,
        });
    }

    // 2. Turn in place (lower body).
    state.turn.begin();
    let yaw_rate = sane(mover.yaw_rate);
    if mode == MoveMode::Ground
        && !mover.crouch
        && speed < TURN_IDLE_SPEED
        && yaw_rate.abs() > TURN_RATE_MIN
    {
        let track = if yaw_rate > 0.0 { TURN_LEFT } else { TURN_RIGHT };
        if bank.has(track) {
            state.turn_progress = wrap01(state.turn_progress + yaw_rate.abs() * dt / FRAC_PI_2);
            let t = state.turn_progress * bank.duration(track);
            let w = TURN_WEIGHT * smooth01((yaw_rate.abs() - TURN_RATE_MIN) / TURN_RATE_RAMP);
            state.turn.want(track, Mask::Lower, Clock::Fixed(t), false, None, w, u16::MAX);
        }
    }
    state.turn.settle(dt, BLEND_TIME, BLEND_TIME, snap);
    if state.turn.slots.is_empty() {
        state.turn_progress = 0.0;
    }
    state.turn.emit(&mut out.layers);

    // 3. Landing (weighted full body; mostly legs, pelvis drop and spine).
    if mode == MoveMode::Ground && landing > 0.01 && state.land_t < LAND_WINDOW && bank.has(JUMP_LAND)
    {
        let weight = landing * (1.0 - smooth01(state.land_t / LAND_WINDOW));
        if weight > WEIGHT_EPS {
            out.layers.push(Layer {
                source: Source::Track {
                    name: JUMP_LAND.to_string(),
                    time: LAND_OFFSET + state.land_t * LAND_RATE,
                    looping: false,
                },
                weight,
                mask: Mask::Full,
                style: None,
            });
        }
    }

    // 4. Stance (upper body held pose of the equipped moveset).
    state.stance.begin();
    if let Some(stance) = input.stance
        && matches!(mode, MoveMode::Ground | MoveMode::Air)
        && bank.has(&stance.clip.track)
    {
        let moving = if mode == MoveMode::Air { 1.0 } else { 1.0 - gait[0] };
        let moving_weight = finite_or(stance.moving_weight, 1.0).clamp(0.0, 1.0);
        let weight = 1.0 + (moving_weight - 1.0) * moving;
        let mask = if stance.clip.mask == Mask::Full {
            Mask::Upper
        } else {
            stance.clip.mask
        };
        let hold = finite_or(stance.hold, 0.0).max(0.0);
        state.stance.want(
            &stance.clip.track,
            mask,
            Clock::Fixed(hold),
            false,
            stance.clip.style,
            weight,
            u16::MAX,
        );
    }
    state.stance.settle(dt, BLEND_TIME, BLEND_TIME, snap);
    state.stance.emit(&mut out.layers);

    // 5./6. Action layers.
    let action = input.action;
    let action_layers = [
        (&mut state.action, action.and_then(|a| a.layer.as_ref())),
        (&mut state.overlay, action.and_then(|a| a.overlay.as_ref())),
    ];
    for (channel, layer) in action_layers {
        channel.begin();
        if let Some(l) = layer {
            if bank.has(&l.track) {
                channel.want(
                    &l.track,
                    l.mask,
                    Clock::Fixed(finite_or(l.time, 0.0)),
                    l.looping,
                    l.style,
                    l.weight,
                    u16::MAX,
                );
            } else if missing.insert(l.track.clone()) {
                warn!("[select] action track '{}' is not in the pose bank", l.track);
            }
        }
        channel.settle(dt, ACTION_IN, ACTION_OUT, snap);
        channel.emit(&mut out.layers);
    }

    // Contact requests.
    let full_body_action = action.is_some_and(|a| {
        matches!(
            a.kind,
            ActionKind::Dodge
                | ActionKind::Knockdown
                | ActionKind::GetUp
                | ActionKind::Dead
                | ActionKind::Mantle
                | ActionKind::Roll
        )
    });
    let foot_target = if mode == MoveMode::Ground && !full_body_action { 1.0 } else { 0.0 };
    state.foot_ik = if snap {
        foot_target
    } else {
        let tau = if foot_target > state.foot_ik { FOOT_IK_IN } else { FOOT_IK_OUT };
        smooth_damp(state.foot_ik, foot_target, &mut state.foot_ik_velocity, tau, dt).clamp(0.0, 1.0)
    };

    let bounce = frame.bounce;
    let stretch = if mode == MoveMode::Air
        && velocity.y > JUMP_UP_SPEED
        && sane(mover.air_time) < STRETCH_WINDOW
    {
        JUMP_STRETCH
    } else {
        0.0
    };
    let contact_squash = (-LAND_SQUASH * landing + stretch) * (0.5 + 0.5 * bounce);

    // Step bounce and idle breath, on the ground outside full-body actions.
    let (mut bob, mut bob_accel, mut rhythm_squash) = (0.0, 0.0, 0.0);
    if mode == MoveMode::Ground && !full_body_action {
        let crouch = if mover.crouch { CROUCH_BOB } else { 1.0 };
        let amp = gait.iter().zip(BOB).map(|(w, a)| w * a).sum::<f32>() * bounce * crouch;
        let rate = phase_speed / stride.max(1e-3);
        let (dip, accel, c) = step_bounce(mover.phase, amp, rate);
        let moving = 1.0 - gait[0];
        bob = dip;
        bob_accel = accel;
        rhythm_squash = bounce * moving * (-STEP_SQUASH * 0.5 * (1.0 + c) + STEP_STRETCH * 0.5 * (1.0 - c))
            + bounce * gait[0] * BREATH * (TAU * state.clock / BREATH_PERIOD).sin();
    }
    let squash = (contact_squash + rhythm_squash).clamp(-0.3, 0.3);

    let hands = fade_limbs(&mut state.hands, input.hands.map(|h| h.hands), snap, dt);
    let feet = fade_limbs(&mut state.feet, input.feet.map(|f| f.feet), snap, dt);

    // Posture: the balance lean adds to the acceleration; the narrow stance fades in and out.
    let lean = input.posture.map_or(Vec3::ZERO, |p| sane_vec(p.lean).with_y(0.0));
    let wanted = input.posture.map(|p| sane(p.stance)).filter(|w| *w > 0.0);
    if let Some(width) = wanted {
        state.narrow_width = width;
    }
    let goal = if wanted.is_some() { 1.0 } else { 0.0 };
    state.narrow = if snap {
        goal
    } else {
        smooth_damp(state.narrow, goal, &mut state.narrow_velocity, STANCE_TIME, dt).clamp(0.0, 1.0)
    };
    let stance = if state.narrow > WEIGHT_EPS {
        FREE_STANCE + (state.narrow_width - FREE_STANCE) * state.narrow
    } else {
        f32::INFINITY
    };

    out.contact = ContactRequest {
        foot_ik: state.foot_ik,
        hands,
        accel: (sane_vec(mover.accel) + lean).with_y(sane(bob_accel)),
        bob: sane(bob),
        squash,
        look_at: input.look_at,
        root_ease: mover.step_ease,
        feet,
        stance,
    };
}

/// Smoothed limb targets: each weight eases toward the wanted one (`HAND_TIME`); a released
/// limb keeps its last point while it fades out.
fn fade_limbs(
    fades: &mut [HandFade; 2],
    wanted: Option<[Option<(Vec3, f32)>; 2]>,
    snap: bool,
    dt: f32,
) -> [Option<(Vec3, f32)>; 2] {
    let mut out = [None, None];
    for (i, limb) in out.iter_mut().enumerate() {
        let fade = &mut fades[i];
        let wanted = wanted
            .and_then(|w| w[i])
            .filter(|(p, w)| p.is_finite() && w.is_finite());
        let target = match wanted {
            Some((point, w)) => {
                fade.point = point;
                w.clamp(0.0, 1.0)
            }
            None => 0.0,
        };
        fade.weight = if snap {
            target
        } else {
            smooth_damp(fade.weight, target, &mut fade.velocity, HAND_TIME, dt).clamp(0.0, 1.0)
        };
        if fade.weight > WEIGHT_EPS {
            *limb = Some((fade.point, fade.weight));
        }
    }
    out
}

/// Ground locomotion targets: idle + phase-synced gaits, blended across strafe directions
/// while strafing, or the crouch pair.
fn ground_targets(
    base: &mut Channel,
    loco: &Loco,
    mover: &Mover,
    input: &CharacterInput,
    speed: f32,
    gait: [f32; 5],
    frame: &Frame,
) {
    if mover.crouch {
        let full = (frame.anchors.0[1] * CROUCH_FULL_FRACTION).max(0.3);
        let moving = smooth01(speed / full);
        if let Some(t) = loco.crouch[0] {
            base.want(t, Mask::Full, Clock::Free, true, None, 1.0 - moving, 0);
        }
        if let Some(t) = loco.crouch[1] {
            base.want(t, Mask::Full, Clock::Synced, true, None, moving, 8);
        }
        return;
    }
    let directions = match local_move_angle(mover.velocity, mover.yaw) {
        Some(theta) if input.strafing && speed > STRAFE_MIN_SPEED => direction_weights(theta),
        _ => [1.0, 0.0, 0.0, 0.0],
    };
    for (g, &gw) in gait.iter().enumerate() {
        if gw <= 0.0 {
            continue;
        }
        if g == 0 {
            if let Some(t) = loco.sets[0][0] {
                base.want(t, Mask::Full, Clock::Free, true, None, gw, 0);
            }
            continue;
        }
        for (d, &dw) in directions.iter().enumerate() {
            if dw <= 0.0 {
                continue;
            }
            if let Some(t) = loco.sets[d][g] {
                let rank = (g * 8 + d) as u16;
                base.want(t, Mask::Full, Clock::Synced, true, None, gw * dw, rank);
            }
        }
    }
}

/// Air targets: push-off (Jump_Start) early in upward flight, Jump_Loop while rising / at the
/// apex, Falling Idle once falling fast or airborne for long. A body that left the ground at
/// running speed (`Mover.takeoff_speed`) leaps instead: NinjaJump_Start, then its airborne hold.
fn air_targets(base: &mut Channel, loco: &Loco, bank: &PoseBank, mover: &Mover, velocity: Vec3) {
    let air_time = sane(mover.air_time).max(0.0);
    let leap = sane(mover.takeoff_speed) >= LEAP_SPEED && bank.has(LEAP_START) && bank.has(LEAP_AIR);
    let (start, window, offset, rate, hang, fall_after) = if leap {
        (LEAP_START, LEAP_START_WINDOW, LEAP_START_OFFSET, LEAP_START_RATE, LEAP_AIR, LEAP_FALL_AFTER)
    } else {
        (JUMP_START, JUMP_START_WINDOW, JUMP_START_OFFSET, JUMP_START_RATE, JUMP_LOOP, FALL_AFTER)
    };
    let falling = velocity.y < FALL_SPEED || air_time > fall_after;
    if velocity.y > JUMP_UP_SPEED && air_time < window && bank.has(start) {
        let t = offset + air_time * rate;
        base.want(start, Mask::Full, Clock::Fixed(t), false, None, 1.0, 64);
    } else if let Some(t) = if falling {
        pick(bank, &[Some(FALL), Some(hang), Some(JUMP_LOOP)])
    } else {
        pick(bank, &[Some(hang), Some(JUMP_LOOP), Some(FALL)])
    } {
        let rank = if t == FALL { 66 } else { 65 };
        base.want(t, Mask::Full, Clock::Free, true, None, 1.0, rank);
    } else if let Some(t) = loco.sets[0][0] {
        base.want(t, Mask::Full, Clock::Free, true, None, 1.0, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn anchors() -> GaitAnchors {
        GaitAnchors::from_feel(&Feel::default())
    }

    #[test]
    fn gait_weights_partition_of_unity() {
        let a = anchors();
        let mut s = -1.0;
        while s < 12.0 {
            let w = gait_weights(s, &a);
            let sum: f32 = w.iter().sum();
            assert!((sum - 1.0).abs() < 1e-5, "sum {sum} at speed {s}");
            assert!(w.iter().all(|x| (0.0..=1.0).contains(x)), "{w:?} at {s}");
            s += 0.005;
        }
        assert_eq!(gait_weights(f32::NAN, &a), [1.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn gait_weights_hit_anchors_and_are_continuous() {
        let a = anchors();
        for (i, &speed) in a.0.iter().enumerate() {
            let w = gait_weights(speed, &a);
            assert!((w[i] - 1.0).abs() < 1e-5, "anchor {i}: {w:?}");
        }
        let step = 1e-3;
        let mut s = 0.0;
        while s < 10.0 {
            let (w0, w1) = (gait_weights(s, &a), gait_weights(s + step, &a));
            for k in 0..5 {
                assert!((w0[k] - w1[k]).abs() < 0.01, "jump at {s}: {w0:?} -> {w1:?}");
            }
            s += step;
        }
    }

    #[test]
    fn anchors_stay_increasing_for_odd_feel() {
        let feel = Feel {
            walk_speed: 5.0,
            run_speed: 3.0,
            sprint_speed: f32::NAN,
            ..Feel::default()
        };
        let a = GaitAnchors::from_feel(&feel).0;
        for i in 1..5 {
            assert!(a[i] > a[i - 1], "{a:?}");
        }
    }

    #[test]
    fn phase_advance_wraps() {
        let p = advance_phase(0.9, 3.0, 0.1, 1.0);
        assert!((p - 0.2).abs() < 1e-5, "{p}");
        let p = advance_phase(0.5, 0.0, 0.1, 1.0);
        assert!((p - 0.5).abs() < 1e-6);
        // Many steps never leave [0, 1).
        let mut p = 0.0;
        for _ in 0..10_000 {
            p = advance_phase(p, 6.8, 1.0 / 60.0, 1.37);
            assert!((0.0..1.0).contains(&p), "{p}");
        }
        assert_eq!(advance_phase(f32::NAN, 1.0, 0.0, 1.0), 0.0);
        let p = advance_phase(0.25, 1.0, 0.1, f32::NAN);
        assert!(p.is_finite() && (0.0..1.0).contains(&p));
    }

    #[test]
    fn stride_fallback() {
        assert_eq!(stride_for(1.4, 3.0), 1.4);
        assert!((stride_for(0.1, 2.0) - 1.8).abs() < 1e-6);
        let s = blended_stride(&[(0.5, 1.0), (0.5, 2.0)], 1.0);
        assert!((s - 1.5).abs() < 1e-6);
        assert!((blended_stride(&[], 0.0) - 1.1).abs() < 1e-6);
    }

    #[test]
    fn strafe_quadrant_mapping() {
        let near = |w: [f32; 4], e: [f32; 4]| w.iter().zip(e).all(|(a, b)| (a - b).abs() < 1e-4);
        assert!(near(direction_weights(0.0), [1.0, 0.0, 0.0, 0.0]));
        assert!(near(direction_weights(FRAC_PI_2), [0.0, 1.0, 0.0, 0.0]));
        assert!(near(direction_weights(PI), [0.0, 0.0, 1.0, 0.0]));
        assert!(near(direction_weights(-PI), [0.0, 0.0, 1.0, 0.0]));
        assert!(near(direction_weights(-FRAC_PI_2), [0.0, 0.0, 0.0, 1.0]));
        assert!(near(direction_weights(PI / 4.0), [0.5, 0.5, 0.0, 0.0]));
        assert!(near(direction_weights(3.0 * PI / 4.0), [0.0, 0.5, 0.5, 0.0]));
        assert!(near(direction_weights(-3.0 * PI / 4.0), [0.0, 0.0, 0.5, 0.5]));
        assert!(near(direction_weights(-PI / 4.0), [0.5, 0.0, 0.0, 0.5]));
        let mut t = -7.0;
        while t < 7.0 {
            let sum: f32 = direction_weights(t).iter().sum();
            assert!((sum - 1.0).abs() < 1e-5);
            t += 0.01;
        }
    }

    #[test]
    fn local_angle_matches_character_frame() {
        // Facing +Z: moving +X is moving left.
        let a = local_move_angle(Vec3::X, 0.0).unwrap();
        assert!((a - FRAC_PI_2).abs() < 1e-5, "{a}");
        // Facing +X (yaw π/2): left is −Z, right is +Z, back is −X.
        let yaw = FRAC_PI_2;
        let left = direction_weights(local_move_angle(Vec3::NEG_Z, yaw).unwrap());
        assert!((left[1] - 1.0).abs() < 1e-4, "{left:?}");
        let right = direction_weights(local_move_angle(Vec3::Z, yaw).unwrap());
        assert!((right[3] - 1.0).abs() < 1e-4, "{right:?}");
        let back = direction_weights(local_move_angle(Vec3::NEG_X, yaw).unwrap());
        assert!((back[2] - 1.0).abs() < 1e-4, "{back:?}");
        let fwd = direction_weights(local_move_angle(Vec3::X * 3.0, yaw).unwrap());
        assert!((fwd[0] - 1.0).abs() < 1e-4, "{fwd:?}");
        assert!(local_move_angle(Vec3::Y, 0.0).is_none());
        assert!(local_move_angle(Vec3::NAN, 0.0).is_none());
    }

    #[test]
    fn gait_layers_stack_to_input_weights() {
        let info = |name: &str| {
            Some(match name {
                "idle" => LoopInfo { duration: 2.5, left_contact: 0.0, stride: 0.0 },
                "walk" => LoopInfo { duration: 1.2, left_contact: 0.25, stride: 1.4 },
                _ => LoopInfo { duration: 0.8, left_contact: 0.6, stride: 2.2 },
            })
        };
        let blends = [
            ("idle", Clock::Free, 0.2),
            ("walk", Clock::Synced, 0.5),
            ("jog", Clock::Synced, 0.3),
        ];
        let layers = gait_layers(&blends, 0.5, 3.0, &info);
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0].0, "idle");
        assert_eq!(layers[0].2, 1.0);
        // Effective contribution of layer i = w_i · Π_{j>i} (1 − w_j).
        let lw: Vec<f32> = layers.iter().map(|l| l.2).collect();
        let effective: Vec<f32> = (0..lw.len())
            .map(|i| lw[i] * lw[i + 1..].iter().map(|w| 1.0 - w).product::<f32>())
            .collect();
        for (e, b) in effective.iter().zip(blends) {
            assert!((e - b.2).abs() < 1e-5, "{effective:?}");
        }
        // Times: free loop wraps the clock, synced loops share phase + left contact.
        assert!((layers[0].1 - 0.5).abs() < 1e-5);
        assert!((layers[1].1 - 0.75 * 1.2).abs() < 1e-5);
        assert!((layers[2].1 - 0.1 * 0.8).abs() < 1e-4);
    }

    #[test]
    fn gait_layers_skip_inactive_and_normalize() {
        let info = |_: &str| Some(LoopInfo { duration: 1.0, left_contact: 0.0, stride: 1.0 });
        let blends = [("idle", Clock::Free, 0.0), ("walk", Clock::Synced, 2.0), ("jog", Clock::Synced, 2.0)];
        let layers = gait_layers(&blends, 0.0, 0.0, &info);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].0, "walk");
        assert_eq!(layers[0].2, 1.0);
        assert!((layers[1].2 - 0.5).abs() < 1e-6);
        assert!(gait_layers(&[("idle", Clock::Free, f32::NAN)], 0.0, 0.0, &info).is_empty());
        // Fixed clocks clamp into the clip.
        let fixed = gait_layers(&[("x", Clock::Fixed(5.0), 1.0)], 0.0, 0.0, &info);
        assert_eq!(fixed[0].1, 1.0);
    }

    #[test]
    fn step_bounce_dips_twice_per_cycle_and_never_rises() {
        let (amp, rate) = (0.05, 1.8);
        let mut minima = 0;
        let n = 2000;
        let dip = |i: i32| step_bounce(i as f32 / n as f32, amp, rate).0;
        for i in 0..n {
            let d = dip(i);
            assert!(d <= 0.0 && d >= -amp - 1e-6, "{d}");
            if d < dip(i - 1) && d <= dip(i + 1) {
                minima += 1;
            }
        }
        assert_eq!(minima, 2);
        // Lowest just after each plant.
        assert!((step_bounce(BOB_LAG, amp, rate).0 + amp).abs() < 1e-6);
        assert!((step_bounce(0.5 + BOB_LAG, amp, rate).0 + amp).abs() < 1e-6);
        // The reported acceleration is the dip's second time derivative.
        let (dt, phase) = (1e-3, 0.3);
        let at = |t: f32| step_bounce(phase + rate * t, amp, rate).0;
        let fd = (at(dt) - 2.0 * at(0.0) + at(-dt)) / (dt * dt);
        let accel = step_bounce(phase, amp, rate).1;
        assert!((fd - accel).abs() < 0.05 * accel.abs().max(1.0), "{fd} vs {accel}");
        assert_eq!(step_bounce(0.2, 0.0, rate).0, 0.0);
        assert!(step_bounce(f32::NAN, f32::NAN, f32::NAN).0.is_finite());
    }

    #[test]
    fn smooth_damp_is_stable_and_monotonic_from_rest() {
        let mut v = 0.0;
        let mut x = 0.0;
        let mut prev = 0.0;
        for _ in 0..120 {
            x = smooth_damp(x, 1.0, &mut v, BLEND_TIME, 1.0 / 60.0);
            assert!(x >= prev - 1e-6 && x <= 1.0 + 1e-4, "{x}");
            prev = x;
        }
        assert!((x - 1.0).abs() < 1e-3);
        // Huge dt does not explode.
        let mut v = 0.0;
        let y = smooth_damp(0.0, 1.0, &mut v, 0.01, 10.0);
        assert!(y.is_finite() && (y - 1.0).abs() < 0.05);
        assert_eq!(smooth_damp(0.3, 1.0, &mut v, 0.1, 0.0), 0.3);
    }

    #[test]
    fn channel_fades_out_frozen_and_restarts_on_rewind() {
        let mut c = Channel::restarting();
        c.begin();
        c.want("atk", Mask::Full, Clock::Fixed(0.4), false, None, 1.0, u16::MAX);
        c.settle(1.0 / 60.0, ACTION_IN, ACTION_OUT, true);
        assert_eq!(c.slots.len(), 1);
        assert_eq!(c.slots[0].weight, 1.0);
        // Same track restarted from 0 → old copy retires, new copy fades in on top.
        c.begin();
        c.want("atk", Mask::Full, Clock::Fixed(0.0), false, None, 1.0, u16::MAX);
        c.settle(1.0 / 60.0, ACTION_IN, ACTION_OUT, false);
        assert_eq!(c.slots.len(), 2);
        assert!(c.slots[0].retired && c.slots[0].clock == Clock::Fixed(0.4));
        assert!(c.slots[1].weight > 0.0 && c.slots[1].weight < 1.0);
        // Released: everything fades and is dropped, frozen times untouched.
        for _ in 0..120 {
            c.begin();
            c.settle(1.0 / 60.0, ACTION_IN, ACTION_OUT, false);
        }
        assert!(c.slots.is_empty());
        assert_eq!(c.weight_of("atk"), 0.0);
    }

    /// A one-bone bank holding every locomotion track (enough for `select`, no donor files).
    fn fake_bank() -> PoseBank {
        use crate::motion::extract::{Key, Tag, Track};
        use crate::motion::retarget::Clip;
        use crate::motion::{Bones, Pose, Skeleton};
        use std::collections::HashMap;
        let pose = Pose {
            rot: vec![Quat::IDENTITY],
            pelvis: Vec3::ZERO,
        };
        let skeleton = Skeleton {
            names: vec!["root".into()],
            parent: vec![None],
            rest: vec![Transform::IDENTITY],
            base: Transform::IDENTITY,
            index: HashMap::from([("root".to_string(), 0)]),
        };
        let bones = Bones {
            root: 0,
            pelvis: 0,
            spine: [0; 3],
            neck: 0,
            head: 0,
            clavicle: [0; 2],
            upperarm: [0; 2],
            lowerarm: [0; 2],
            hand: [0; 2],
            thigh: [0; 2],
            calf: [0; 2],
            foot: [0; 2],
            ball: [0; 2],
        };
        let names = [
            IDLE, WALK, JOG, RUN, SPRINT, CROUCH_IDLE, CROUCH_MOVE, WALK_BACK, JOG_BACK, WALK_LEFT,
            WALK_RIGHT, JOG_LEFT, JOG_RIGHT, TURN_LEFT, TURN_RIGHT, JUMP_START, JUMP_LOOP,
            JUMP_LAND, FALL, SWIM_IDLE, SWIM_FWD, CLIMB, "ual1/Punch_Jab",
        ];
        let tracks = names
            .iter()
            .map(|&n| {
                let track = Track {
                    name: n.into(),
                    duration: 1.0,
                    looping: n.contains("Loop"),
                    keys: vec![Key {
                        time: 0.0,
                        pose: pose.clone(),
                        tag: Tag::Start,
                    }],
                    left_contact: Some(0.3),
                    stride: 1.4,
                    dense: Clip {
                        track: n.into(),
                        duration: 1.0,
                        looping: false,
                        frames: vec![pose.clone()],
                        travel: vec![Vec3::ZERO],
                    },
                };
                (n.to_string(), track)
            })
            .collect();
        PoseBank {
            skeleton,
            bones,
            rest: pose,
            tracks,
            style: Style::default(),
            masks: HashMap::new(),
        }
    }

    fn app() -> (App, Entity) {
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_secs_f32(1.0 / 60.0),
            ))
            .init_resource::<Feel>()
            .insert_resource(Movesets::default())
            .insert_resource(fake_bank())
            .add_plugins(plugin);
        let e = app
            .world_mut()
            .spawn((
                Transform::default(),
                Mover::default(),
                MotionLayers::default(),
                Action::default(),
                Intent::default(),
                Loadout {
                    slots: vec!["unarmed".into()],
                    current: 0,
                },
            ))
            .id();
        (app, e)
    }

    fn check_layers(layers: &MotionLayers) {
        assert!(!layers.layers.is_empty());
        assert_eq!(layers.layers[0].weight, 1.0);
        assert_eq!(layers.layers[0].mask, Mask::Full);
        for l in &layers.layers {
            assert!(l.weight.is_finite() && (0.0..=1.0).contains(&l.weight), "{l:?}");
            if let Source::Track { time, .. } = &l.source {
                assert!(time.is_finite() && *time >= 0.0, "{l:?}");
            }
        }
        let c = &layers.contact;
        assert!((0.0..=1.0).contains(&c.foot_ik) && c.accel.is_finite() && c.squash.is_finite());
    }

    fn track_names(layers: &MotionLayers) -> Vec<String> {
        layers
            .layers
            .iter()
            .filter_map(|l| match &l.source {
                Source::Track { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn system_selects_idle_run_air_and_actions() {
        let (mut app, e) = app();
        app.update();
        app.update();
        assert!(app.world().get::<SelectState>(e).is_some());
        let layers = app.world().get::<MotionLayers>(e).unwrap().clone();
        check_layers(&layers);
        assert_eq!(track_names(&layers), [IDLE]);
        assert_eq!(layers.contact.foot_ik, 1.0);

        // Running: phase advances, idle fades out, run dominates.
        app.world_mut().get_mut::<Mover>(e).unwrap().velocity = Vec3::new(0.0, 0.0, 4.2);
        for _ in 0..60 {
            app.update();
            check_layers(app.world().get::<MotionLayers>(e).unwrap());
        }
        let layers = app.world().get::<MotionLayers>(e).unwrap().clone();
        assert_eq!(track_names(&layers), [RUN]);
        assert!(app.world().get::<Mover>(e).unwrap().phase > 0.0);

        // Jump: push-off clip, stretch, foot IK released.
        {
            let mut m = app.world_mut().get_mut::<Mover>(e).unwrap();
            m.mode = MoveMode::Air;
            m.velocity = Vec3::new(0.0, 5.0, 4.0);
            m.air_time = 0.02;
        }
        for _ in 0..4 {
            app.update();
        }
        let layers = app.world().get::<MotionLayers>(e).unwrap().clone();
        check_layers(&layers);
        assert!(track_names(&layers).iter().any(|n| n == JUMP_START));
        assert!(layers.contact.squash > 0.0);
        assert!(layers.contact.foot_ik < 0.5);

        // Land with an attack starting: landing overlay + action layer on top.
        {
            let mut m = app.world_mut().get_mut::<Mover>(e).unwrap();
            m.mode = MoveMode::Ground;
            m.velocity = Vec3::ZERO;
            m.landing = 0.8;
            let mut a = app.world_mut().get_mut::<Action>(e).unwrap();
            a.start(
                ActionKind::Attack,
                super::super::Owner::Combat,
                Some(super::super::ActionLayer::new("ual1/Punch_Jab", 0.1, Mask::Upper)),
            );
        }
        for _ in 0..3 {
            app.update();
        }
        let layers = app.world().get::<MotionLayers>(e).unwrap().clone();
        check_layers(&layers);
        let names = track_names(&layers);
        assert!(names.iter().any(|n| n == JUMP_LAND), "{names:?}");
        assert_eq!(names.last().map(String::as_str), Some("ual1/Punch_Jab"));
        assert!(layers.contact.squash < 0.0);

        // Ragdoll: layers are left exactly as they were.
        app.world_mut().get_mut::<Mover>(e).unwrap().mode = MoveMode::Ragdoll;
        app.world_mut().get_mut::<MotionLayers>(e).unwrap().layers.clear();
        app.update();
        assert!(app.world().get::<MotionLayers>(e).unwrap().layers.is_empty());
    }

    #[test]
    fn system_strafes_and_tracks_hands_and_look() {
        let (mut app, e) = app();
        let target = app
            .world_mut()
            .spawn((Transform::from_xyz(3.0, HALF_HEIGHT, 0.0), Mover::default()))
            .id();
        app.world_mut().get_mut::<Intent>(e).unwrap().target = Some(target);
        {
            let mut m = app.world_mut().get_mut::<Mover>(e).unwrap();
            m.face = Some(0.0);
            // Facing +Z, moving +X = strafing left at walk speed.
            m.velocity = Vec3::new(1.6, 0.0, 0.0);
        }
        app.world_mut().entity_mut(e).insert(HandTargets {
            hands: [None, Some((Vec3::new(0.2, 1.2, 0.4), 1.0))],
        });
        for _ in 0..60 {
            app.update();
        }
        let layers = app.world().get::<MotionLayers>(e).unwrap().clone();
        check_layers(&layers);
        assert_eq!(track_names(&layers), [WALK_LEFT]);
        let look = layers.contact.look_at.unwrap();
        assert!((look - Vec3::new(3.0, LOOK_HEIGHT, 0.0)).length() < 1e-4, "{look}");
        assert!(layers.contact.hands[0].is_none());
        let (p, w) = layers.contact.hands[1].unwrap();
        assert!((w - 1.0).abs() < 1e-2 && (p - Vec3::new(0.2, 1.2, 0.4)).length() < 1e-5);
        // Releasing the hand fades it out.
        app.world_mut().entity_mut(e).remove::<HandTargets>();
        app.update();
        let w = app.world().get::<MotionLayers>(e).unwrap().contact.hands[1].map_or(0.0, |h| h.1);
        assert!(w > 0.0 && w < 1.0, "{w}");
    }

    #[test]
    fn base_channel_keeps_canonical_order() {
        let mut c = Channel::default();
        c.begin();
        c.want("jog", Mask::Full, Clock::Synced, true, None, 0.5, 16);
        c.want("walk", Mask::Full, Clock::Synced, true, None, 0.25, 8);
        c.want("idle", Mask::Full, Clock::Free, true, None, 0.25, 0);
        c.want("walk", Mask::Full, Clock::Synced, true, None, 0.25, 8);
        c.settle(0.0, BLEND_TIME, BLEND_TIME, true);
        let names: Vec<&str> = c.slots.iter().map(|s| s.track.as_str()).collect();
        assert_eq!(names, ["idle", "walk", "jog"]);
        assert!((c.weight_of("walk") - 0.5).abs() < 1e-6);
    }
}
