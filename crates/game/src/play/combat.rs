//! Melee combat (`Update / PlaySet::Act`, `CombatSet`): the melee state machine of every
//! character with `Fighter` + `Loadout` + `Action` + `Intent`, the ONLY `Hit` resolver, the only
//! writer of `Fighter` (hp, poise, stamina, death) and the weapon held in the hand.
//!
//! Moves come from the equipped `Moveset` (kinds `unarmed` / `melee`):
//! - light chain: a primary tap starts the next light attack. From free, a press waits until
//!   release (light) or `HEAVY_HOLD` (heavy) when the set has a heavy; during an attack presses
//!   are buffered for `BUFFER` s and chain once it passes its `cancel` fraction. When the chain
//!   stops, the attack's `recover` clip carries the body back to the stance;
//! - heavy: holds its windup pose while primary stays down (up to `charge` s, damage
//!   ×(1 + held), stamina drains), strikes on release, or by itself once fully charged (a clank
//!   and a small camera kick mark the moment);
//! - sprint attack (pressed while sprinting) and air attack: a plunge that hangs through the
//!   windup, dives, holds its strike pose down to the ground and lands with a shockwave;
//! - guard (secondary held, the set's `guard` clip): blows from the front `arc` are blocked for
//!   stamina (`absorb` of the damage stopped); within `parry_window` of raising the guard they
//!   are parried, which staggers the attacker; out of stamina the guard breaks;
//! - dodge (`dodge_pressed`): a roll toward the stick, or a sidestep / backstep while locked on,
//!   with i-frames, for stamina.
//!
//! Hit tests (`hit_test`) sweep the hitbox between frames: the blade of the held weapon, or the
//! attack's `hitbox` bone segment, as a capsule interpolated from last frame's to this frame's
//! pose, against the body capsules of other teams (and the training dummies), once per target per
//! swing, only inside the active window. They emit `Hit`; `resolve` (also fed by projectiles)
//! applies block / parry, damage scaled by closing speed, poise → flinch / stagger / knockdown,
//! knockback (`Mover.external`), spring impulses (`MotionLayers.impulses`), an eased hitstop
//! (`Time<Virtual>` relative speed), camera shake when the player is involved, sounds and sparks,
//! death (the body stays down until `Revive`), and a `RagdollRequest` above
//! `Feel.ragdoll_threshold` (the ragdoll lane takes over from there; until then a knockdown plays).
//!
//! Stamina: attacks, dodges, sprinting, charging and blocking spend it, `StaminaSpend` messages
//! spend it for other modules, and it regenerates after `REGEN_DELAY`. Emptying it exhausts the
//! fighter until it refills: no dodge, heavy or sprint drain, and the body slows to a tired jog.
//!
//! While the `Action` is free (Ground / Air) combat also steers `Mover.face` toward
//! `Intent.target` (lock-on strafing) and sets `Mover.move_scale` while exhausted, and resets
//! both when that ends. Reactions are `Owner::Hit` actions and may interrupt any owner; nothing
//! starts on a character in Swim / Climb / Ledge / Ragdoll or holding a tool (`tools::Holding`).
//!
//! The weapon (`WeaponDef`: a `sword` primitive built here, a basic primitive or a glTF scene)
//! is a child of the hand bone, placed like `tools` places the axe: the grip transform under the
//! bone, compensating the bone's global scale. `Equip` messages (the weapon chest) switch
//! `Loadout.current` while free. Monitor log lines: `hit_*`, `parry`, `guard_break`, `knockdown`, `death`, `revive`.
use super::camera::CameraShake;
use super::controller::{Teleport, yaw_of};
use super::fx::{Burst, BurstKind, Sfx, SfxKind};
use super::deeds::Deed;
use super::magic::Ward;
use super::monitor::Monitor;
use super::moveset::{Attack, ClipRef, Moveset, Movesets, Primitive, WeaponDef};
use super::ragdoll::RagdollRequest;
use super::stats::Derived;
use super::tools::Holding;
use super::world::TrainingDummy;
use super::{
    Action, ActionKind, ActionLayer, Feel, Fighter, HALF_HEIGHT, Hit, HitKind, Intent, Loadout, MoveMode, Mover,
    Owner, PlaySet, Player, StaminaSpend,
};
use crate::motion::bank::PoseBank;
use crate::motion::{Mask, MotionLayers, Rig};
use bevy::asset::RenderAssetUsages;
use bevy::ecs::query::QueryItem;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;

/// Bank tracks of the reactions and dodges (shared by every moveset).
mod tracks {
    pub const FLINCH_BODY: &str = "ual1/Hit_Chest";
    pub const FLINCH_HEAD: &str = "ual1/Hit_Head";
    /// Arms flung up and out, a step back: poise broken, guard broken, parried.
    pub const STAGGER: &str = "ual2/Idle_Shield_Break";
    /// Thrown onto the back; `GET_UP` starts from its last pose (and from `DEATH`'s).
    pub const KNOCKDOWN: &str = "ual2/Hit_Knockback";
    pub const GET_UP: &str = "ual2/LayToIdle";
    pub const DEATH: &str = "ual1/Death01";
    /// Dive roll with root travel (5 m over its first second).
    pub const ROLL: &str = "ual1/Roll_RM";
    pub const STEP_LEFT: &str = "sekiro/Jog Strafe Left";
    pub const STEP_RIGHT: &str = "sekiro/Jog Strafe Right";
    pub const STEP_BACK: &str = "sekiro/Jog Backward";
    pub const ALL: [&str; 10] = [
        FLINCH_BODY,
        FLINCH_HEAD,
        STAGGER,
        KNOCKDOWN,
        GET_UP,
        DEATH,
        ROLL,
        STEP_LEFT,
        STEP_RIGHT,
        STEP_BACK,
    ];
}
use tracks::*;

/// Seconds a primary / dodge press stays buffered.
const BUFFER: f32 = 0.35;
/// From free, holding primary this long starts the heavy instead of a light attack.
const HEAVY_HOLD: f32 = 0.18;
/// Sprint attacks need this horizontal speed (m/s); air attacks this long airborne (s).
const SPRINT_ATTACK_SPEED: f32 = 4.5;
const AIR_ATTACK_AFTER: f32 = 0.12;
/// Stick share while guarding, and in an attack's recovery once it may be cancelled.
const GUARD_MOVE: f32 = 0.45;
const RECOVER_MOVE: f32 = 0.6;
/// Stick tilt that walks out of a recovery clip.
const RECOVER_BREAK: f32 = 0.5;
/// Attack assist: windups turn toward an enemy within `ASSIST_RANGE` (m) inside this cone
/// (cos) around the stick / facing; lunges stop `LUNGE_STOP` (m) from the target's centre.
const ASSIST_RANGE: f32 = 4.0;
const ASSIST_CONE: f32 = 0.5;
const LUNGE_STOP: f32 = 1.1;
/// Reach of the arm from the body centre (m), plus a fist; weapons add their `length`.
const ARM_REACH: f32 = 0.75;
const FIST_REACH: f32 = 0.15;

/// Plunge: rise speed at the start (m/s), dive speed after the windup (m/s), the clip fraction
/// held while diving (share of `active`), shockwave radius (m), longest dive (s).
const PLUNGE_HOP: f32 = 2.0;
const PLUNGE_SPEED: f32 = 16.0;
const PLUNGE_HOLD: f32 = 0.92;
const PLUNGE_RADIUS: f32 = 1.6;
const PLUNGE_TIMEOUT: f32 = 2.5;

/// Roll: clip window (s), rate, share of the clip's root travel, i-frames (action s), attack
/// cancel from this share of the roll.
const ROLL_WINDOW: (f32, f32) = (0.05, 1.1);
const ROLL_RATE: f32 = 1.5;
const ROLL_TRAVEL: f32 = 0.6;
const ROLL_IFRAMES: (f32, f32) = (0.02, 0.42);
const DODGE_CANCEL: f32 = 0.7;
/// Sidestep / backstep: length (s), clip rate, distance (m), i-frames until (s).
const STEP_TIME: f32 = 0.36;
const STEP_RATE: f32 = 1.8;
const STEP_DISTANCE: f32 = 1.5;
const STEP_IFRAMES: f32 = 0.24;
/// Stick share across the facing that picks a sidestep over a backstep while locked on.
const STEP_SIDE: f32 = 0.35;

/// Stamina: regen delay (s) and rates (/s), sprint and charge drains (/s), dodge costs, block
/// cost (flat + per point of damage stopped).
const REGEN_DELAY: f32 = 0.8;
const REGEN_RATE: f32 = 32.0;
const EXHAUSTED_REGEN: f32 = 22.0;
const SPRINT_COST: f32 = 12.0;
const CHARGE_COST: f32 = 15.0;
const ROLL_COST: f32 = 18.0;
const STEP_COST: f32 = 12.0;
const BLOCK_COST: f32 = 6.0;
const BLOCK_COST_PER_DAMAGE: f32 = 1.2;
/// Stick share while exhausted (a tired jog).
const EXHAUSTED_MOVE: f32 = 0.45;
/// Poise comes back after this long without a blow (s), at this rate (/s).
const POISE_DELAY: f32 = 2.0;
const POISE_RATE: f32 = 25.0;

/// Body capsule for hit tests: heights over the feet (m) and radius.
const BODY_LOW: f32 = 0.25;
const BODY_HIGH: f32 = 1.55;
const BODY_RADIUS: f32 = 0.26;
/// Training dummy capsule around its centre: half length (m) and radius.
const DUMMY_HALF: f32 = 0.6;
const DUMMY_RADIUS: f32 = 0.28;
/// Blade capsule radius, and a bare hand's hit sphere (m).
const BLADE_RADIUS: f32 = 0.06;
const FIST_RADIUS: f32 = 0.1;
/// Sweep sub-steps: one per this much travel of the faster hitbox end (m), at most this many.
const SUBSTEP: f32 = 0.08;
const MAX_SUBSTEPS: usize = 12;
/// Hitbox points slower than this (m/s) never hit.
const MIN_HIT_SPEED: f32 = 1.0;
/// Closing speed → damage factor: `SPEED_FACTOR.0` at ≤ `SPEED_SLOW` m/s, `.1` at ≥ `SPEED_FAST`.
const SPEED_SLOW: f32 = 2.0;
const SPEED_FAST: f32 = 14.0;
const SPEED_FACTOR: (f32, f32) = (0.75, 1.3);
/// Blows landing higher than this over the feet (m) are head hits.
const HEAD_HEIGHT: f32 = 1.42;

/// Flinch (poise holds): length (s), knockback cap (m/s, planted feet barely slide), stick share.
const FLINCH_TIME: f32 = 0.33;
const FLINCH_PUSH: f32 = 0.5;
const FLINCH_MOVE: f32 = 0.25;
/// Stagger: clip rate for a broken poise / guard and for a parried attacker, knockback cap.
const STAGGER_RATE: f32 = 1.15;
const PARRIED_RATE: f32 = 0.95;
const STAGGER_PUSH: f32 = 1.5;
/// Knockdown from this knockback (m/s), or a broken poise under at least `KNOCKDOWN_POISE_KB`;
/// it lifts the body (m/s) so it leaves the ground, starts at `KNOCKDOWN_OFFSET` in the clip,
/// lies `DOWN_TIME` (s) and gets up at `GET_UP_RATE`.
const KNOCKDOWN_SPEED: f32 = 5.5;
const KNOCKDOWN_POISE_KB: f32 = 3.5;
const KNOCKDOWN_LIFT: f32 = 2.4;
const KNOCKDOWN_OFFSET: f32 = 0.08;
const DOWN_TIME: f32 = 0.6;
const GET_UP_RATE: f32 = 1.3;
/// The body lands this far into the death / knockdown clip (s): the fall sound.
const DEATH_FALL_AT: f32 = 0.95;
const KNOCKDOWN_FALL_AT: f32 = 0.3;
/// Sparks where a blow meets an Aura Ward.
const SPARKS_WARD: u32 = 10;
/// Blocked blows: knockback cap (m/s).
const BLOCK_PUSH: f32 = 1.0;

/// Hitstop: slowest relative speed, share of the duration held there before easing back, and
/// multipliers of `Feel.hitstop` per outcome.
const HITSTOP_SPEED: f32 = 0.05;
const HITSTOP_HOLD: f32 = 0.55;
const HITSTOP_BLOCK: f32 = 0.8;
const HITSTOP_HEAVY: f32 = 1.6;
const HITSTOP_PARRY: f32 = 2.4;
const HITSTOP_KILL: f32 = 3.0;
/// Camera trauma per outcome (player involved).
const SHAKE_LIGHT: f32 = 0.22;
const SHAKE_HEAVY: f32 = 0.38;
const SHAKE_DOWN: f32 = 0.5;
const SHAKE_BLOCK: f32 = 0.16;
const SHAKE_PARRY: f32 = 0.3;
const SHAKE_PLUNGE: f32 = 0.5;
/// A heavy reaching full charge.
const SHAKE_PRIMED: f32 = 0.12;
/// Spring impulse (rad/s) on the spine and head: base + per m/s of knockback, capped; shares
/// for spine_02, spine_03, neck, head.
const IMPULSE_BASE: f32 = 2.0;
const IMPULSE_PER: f32 = 1.2;
const IMPULSE_MAX: f32 = 12.0;
const IMPULSE_SHARE: [f32; 4] = [0.4, 0.7, 0.6, 1.0];
/// Attacker's recoil impulse share when the blade is stopped by a guard.
const RECOIL_SHARE: f32 = 0.6;
/// Sparks per block / parry.
const SPARKS_BLOCK: u32 = 10;
const SPARKS_PARRY: u32 = 24;
/// The player gets up this long after dying (s).
const PLAYER_REVIVE: f32 = 4.0;

/// Sword proportions (m) around its grip point (the mesh origin, held in the fist): grip, guard
/// and blade start heights, grip and pommel radius, guard span and depth over the blade width /
/// thickness, the point's share of the blade and its width there.
const GRIP_FROM: f32 = -0.11;
const GUARD_AT: f32 = 0.085;
const BLADE_FROM: f32 = 0.1;
const GRIP_RADIUS: f32 = 0.016;
const POMMEL_RADIUS: f32 = 0.028;
const GUARD_SPAN: f32 = 4.4;
const GUARD_DEPTH: f32 = 3.0;
const POINT_SHARE: f32 = 0.13;
const SHOULDER_WIDTH: f32 = 0.8;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Fighter, Melee>()
        .init_resource::<Hitstop>()
        .add_message::<Equip>()
        .add_systems(Startup, validate)
        .add_systems(PreUpdate, apply_hitstop)
        .add_systems(
            Update,
            (enroll, equip, arm, stamina, drive, hit_test, resolve)
                .chain()
                .in_set(PlaySet::Act)
                .in_set(CombatSet),
        );
}

/// Combat's systems inside `PlaySet::Act` (order other systems against this, not the fns).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CombatSet;

/// Damage multiplier for the blows this fighter deals (`ai` sets it from its difficulty).
#[derive(Component, Clone, Copy, Debug)]
pub struct DamageScale(pub f32);

/// Brings a dead fighter back after `after` seconds with full hp, poise and stamina, getting up
/// from where it lies, or at `home` (feet, yaw) when set. `combat` inserts it on the player,
/// `ai` on its NPCs.
#[derive(Component, Clone, Copy, Debug)]
pub struct Revive {
    pub after: f32,
    pub home: Option<(Vec3, f32)>,
}

/// On a character: the weapon entity under its hand bone, the moveset it belongs to and the
/// blade length along the weapon's local +Y (from `BLADE_FROM`).
#[derive(Component, Clone, Debug)]
pub struct WeaponSlot {
    pub weapon: Entity,
    pub set: String,
    pub length: f32,
}

/// Marker on a weapon entity (child of a hand bone).
#[derive(Component)]
pub struct Weapon;

/// A fighter's attack as an opponent reads it (`Melee::tell`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tell {
    /// Seconds since the attack started.
    pub elapsed: f32,
    /// Seconds until its active frames (0 while they run; infinite while a heavy charges).
    pub until_active: f32,
    pub active: bool,
    pub heavy: bool,
    /// Reach of the blow from the attacker's centre (m).
    pub reach: f32,
}

/// Combat state of one fighter (required by `Fighter`; written by `combat` only). Other modules
/// read it through the getters (`ai`: the opponent's `tell`; `hud`: exhaustion).
#[derive(Component, Clone, Debug)]
pub struct Melee {
    state: State,
    /// Next light attack of the chain.
    chain: usize,
    /// Age (s) of a primary / dodge press waiting to be used.
    primary: Option<f32>,
    dodge: Option<f32>,
    /// Seconds primary has been held down.
    held: f32,
    /// Seconds of invulnerability left.
    iframes: f32,
    /// Seconds since stamina was spent, poise was lost, damage was taken.
    rested: f32,
    steady: f32,
    hurt: f32,
    exhausted: bool,
    /// Combat set `Mover.face` (lock-on) / `Mover.move_scale` (exhaustion) while free.
    facing: bool,
    slowed: bool,
}

impl Default for Melee {
    fn default() -> Self {
        Self {
            state: State::Free,
            chain: 0,
            primary: None,
            dodge: None,
            held: 0.0,
            iframes: 0.0,
            rested: REGEN_DELAY,
            steady: POISE_DELAY,
            hurt: f32::INFINITY,
            exhausted: false,
            facing: false,
            slowed: false,
        }
    }
}

impl Melee {
    /// The attack in progress, as an opponent sees it coming.
    pub fn tell(&self) -> Option<Tell> {
        match &self.state {
            State::Swing(sw) if sw.recover.is_none() => Some(sw.tell()),
            _ => None,
        }
    }
    /// Out of stamina until it refills.
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }
    /// Seconds since this fighter last took damage.
    pub fn since_hurt(&self) -> f32 {
        self.hurt
    }
    /// Guard up (blocking or parrying).
    pub fn guarding(&self) -> bool {
        matches!(self.state, State::Guard { .. } | State::Parry { .. })
    }
    /// In a swing (attacking or recovering from one).
    pub fn attacking(&self) -> bool {
        matches!(self.state, State::Swing(_))
    }
    /// Knocked down, getting up or dead.
    pub fn down(&self) -> bool {
        match &self.state {
            State::Dead { .. } => true,
            State::React(r) => matches!(r.kind, ReactKind::Knockdown | ReactKind::GetUp),
            _ => false,
        }
    }

    fn free(&self) -> bool {
        matches!(self.state, State::Free)
    }

    fn spend(&mut self, fighter: &mut Fighter, amount: f32) {
        if !(amount > 0.0) {
            return;
        }
        fighter.stamina = (fighter.stamina - amount).max(0.0);
        self.rested = 0.0;
        if fighter.stamina <= 0.0 {
            self.exhausted = true;
        }
    }
}

/// What combat is doing with a fighter's body (`Action` mirrors it for `select`).
#[derive(Clone, Debug, Default)]
enum State {
    #[default]
    Free,
    Swing(Box<Swing>),
    /// Guard up for `t` s (the parry window counts from the raise).
    Guard {
        t: f32,
    },
    /// A blow was just parried; the parry clip plays for `t` s.
    Parry {
        t: f32,
    },
    Dodge(Dodge),
    React(React),
    /// Dead for `t` s.
    Dead {
        t: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SwingKind {
    Light(usize),
    Heavy(usize),
    Sprint,
    Air,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Plunge {
    #[default]
    None,
    /// Rising through the windup.
    Hang,
    /// Falling with the strike pose held.
    Dive,
    /// Hit the ground; the shockwave is still to be dealt.
    Quake,
    Landed,
}

/// A clip window being played: track window (s), playback rate and the current clip time.
#[derive(Clone, Debug)]
struct Clip {
    clip: ClipRef,
    from: f32,
    to: f32,
    rate: f32,
    t: f32,
}

impl Clip {
    fn new(clip: &ClipRef, bank: &PoseBank) -> Self {
        let from = finite(clip.start, 0.0).max(0.0);
        let end = clip.end.filter(|e| e.is_finite()).unwrap_or_else(|| bank.duration(&clip.track));
        let style = clip.style.map_or(1.0, |s| finite(s.speed, 1.0));
        Self {
            clip: clip.clone(),
            from,
            to: end.max(from + 0.05),
            rate: (finite(clip.speed, 1.0) * style).clamp(0.05, 10.0),
            t: from,
        }
    }
    /// Progress through the window, 0..1 (may exceed 1 on the last frame).
    fn u(&self) -> f32 {
        (self.t - self.from) / (self.to - self.from)
    }
    /// Clip time at window fraction `u`.
    fn at(&self, u: f32) -> f32 {
        self.from + u * (self.to - self.from)
    }
    /// Real seconds the whole window takes.
    fn length(&self) -> f32 {
        (self.to - self.from) / self.rate
    }
    fn layer(&self, mask: Option<Mask>) -> ActionLayer {
        ActionLayer {
            track: self.clip.track.clone(),
            time: self.t,
            looping: false,
            mask: mask.unwrap_or(self.clip.mask),
            weight: 1.0,
            style: self.clip.style,
        }
    }
}

#[derive(Clone, Debug)]
struct Swing {
    kind: SwingKind,
    attack: Attack,
    clip: Clip,
    /// Recovery clip once the chain stopped here.
    recover: Option<Clip>,
    /// Seconds since the attack started; seconds charged (heavy).
    elapsed: f32,
    charged: f32,
    /// Attack heading (yaw) and lunge distance covered (m).
    yaw: f32,
    lunged: f32,
    /// Reach of the blow (m from the centre), for `Tell`.
    reach: f32,
    /// Targets already struck by this swing.
    struck: Vec<Entity>,
    /// Hitbox segment (world) last frame while active.
    prev: Option<[Vec3; 2]>,
    plunge: Plunge,
    whoosh: bool,
    /// The full-charge cue has played (heavy).
    primed: bool,
}

impl Swing {
    fn heavy(&self) -> bool {
        matches!(self.kind, SwingKind::Heavy(_))
    }
    fn active(&self) -> bool {
        let u = self.clip.u();
        self.recover.is_none() && u >= self.attack.windup && u < self.attack.active
    }
    fn tell(&self) -> Tell {
        let span = self.clip.to - self.clip.from;
        let charging = self.heavy() && self.clip.u() >= self.attack.windup && self.charged < self.attack.charge;
        let until = ((self.attack.windup - self.clip.u()) * span / self.clip.rate).max(0.0);
        Tell {
            elapsed: self.elapsed,
            until_active: if charging && !self.active() { f32::INFINITY } else { until },
            active: self.active(),
            heavy: self.heavy() || matches!(self.kind, SwingKind::Sprint | SwingKind::Air),
            reach: self.reach,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DodgeKind {
    Roll,
    Step,
}

#[derive(Clone, Copy, Debug)]
struct Dodge {
    kind: DodgeKind,
    track: &'static str,
    /// World direction of travel and the yaw faced meanwhile.
    dir: Vec3,
    face: f32,
    t: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReactKind {
    Flinch,
    Stagger,
    Knockdown,
    GetUp,
}

#[derive(Clone, Copy, Debug)]
struct React {
    kind: ReactKind,
    track: &'static str,
    offset: f32,
    rate: f32,
    mask: Mask,
    t: f32,
    duration: f32,
    /// Yaw faced meanwhile (toward the attacker), if any.
    face: Option<f32>,
    /// Whether the fall sound has played (knockdown).
    fell: bool,
}

impl React {
    fn new(kind: ReactKind, track: &'static str, offset: f32, rate: f32, duration: f32) -> Self {
        Self {
            kind,
            track,
            offset,
            rate,
            mask: Mask::Full,
            t: 0.0,
            duration,
            face: None,
            fell: false,
        }
    }
    fn action(&self) -> ActionKind {
        match self.kind {
            ReactKind::Flinch | ReactKind::Stagger => ActionKind::Stagger,
            ReactKind::Knockdown => ActionKind::Knockdown,
            ReactKind::GetUp => ActionKind::GetUp,
        }
    }
    fn layer(&self) -> ActionLayer {
        ActionLayer::new(self.track, self.offset + self.t * self.rate, self.mask)
    }
}

/// Eased global hitstop: `Time<Virtual>` runs at `hitstop_speed` while it lasts.
#[derive(Resource, Default, Debug)]
pub struct Hitstop {
    /// Real seconds since it began, and its length.
    t: f32,
    duration: f32,
}

impl Hitstop {
    /// Start a hitstop of `duration` real seconds unless a longer one is still running.
    pub fn trigger(&mut self, duration: f32) {
        if duration.is_finite() && duration > self.duration - self.t {
            self.t = 0.0;
            self.duration = duration.clamp(0.0, 0.5);
        }
    }
}

/// Relative clock speed `t` s into a hitstop of `duration` s: `HITSTOP_SPEED` for the first
/// `HITSTOP_HOLD` share, then eased back to 1.
pub fn hitstop_speed(t: f32, duration: f32) -> f32 {
    if !(duration > 0.0) || !t.is_finite() || t >= duration {
        return 1.0;
    }
    let u = (t / duration).max(0.0);
    if u < HITSTOP_HOLD {
        HITSTOP_SPEED
    } else {
        HITSTOP_SPEED + (1.0 - HITSTOP_SPEED) * smooth01((u - HITSTOP_HOLD) / (1.0 - HITSTOP_HOLD))
    }
}

/// Damage factor for a closing speed (m/s): `SPEED_FACTOR.0` when slow, `.1` when fast.
pub fn speed_factor(speed: f32) -> f32 {
    let s = smooth01((finite(speed, 0.0) - SPEED_SLOW) / (SPEED_FAST - SPEED_SLOW));
    SPEED_FACTOR.0 + (SPEED_FACTOR.1 - SPEED_FACTOR.0) * s
}

/// Reach of a moveset's blows from the body centre (m): arm plus weapon (or a fist).
pub fn reach(set: &Moveset) -> f32 {
    ARM_REACH + set.weapon.as_ref().map_or(FIST_REACH, |w| finite(w.length, 0.0).max(0.0) * finite(w.scale, 1.0))
}

/// Closest points of segments `p1 q1` and `p2 q2`: (distance, s on the first, t on the second),
/// s and t in 0..1 (Ericson, Real-Time Collision Detection 5.1.9).
pub fn segment_distance(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (f32, f32, f32) {
    const EPS: f32 = 1e-8;
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (s, t) = if a <= EPS && e <= EPS {
        (0.0, 0.0)
    } else if a <= EPS {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= EPS {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let s = if denom > EPS { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let t = (b * s + f) / e;
            if t < 0.0 {
                ((-c / a).clamp(0.0, 1.0), 0.0)
            } else if t > 1.0 {
                (((b - c) / a).clamp(0.0, 1.0), 1.0)
            } else {
                (s, t)
            }
        }
    };
    ((p1 + d1 * s - (p2 + d2 * t)).length(), s, t)
}

/// First contact of a capsule (radius `radius`) swept from segment `prev` to `now` with a static
/// capsule `body` (radius `body_radius`), sampled in sub-steps: (sweep fraction, fraction along
/// the moving segment, contact point on the body's surface).
pub fn sweep(prev: [Vec3; 2], now: [Vec3; 2], radius: f32, body: [Vec3; 2], body_radius: f32) -> Option<(f32, f32, Vec3)> {
    let travel = (now[0] - prev[0]).length().max((now[1] - prev[1]).length());
    if !travel.is_finite() {
        return None;
    }
    let steps = ((travel / SUBSTEP).ceil() as usize).clamp(1, MAX_SUBSTEPS);
    (0..=steps).find_map(|i| {
        let f = i as f32 / steps as f32;
        let (a, b) = (prev[0].lerp(now[0], f), prev[1].lerp(now[1], f));
        let (d, s, t) = segment_distance(a, b, body[0], body[1]);
        (d <= radius + body_radius).then(|| {
            let (on_blade, on_body) = (a.lerp(b, s), body[0].lerp(body[1], t));
            (f, s, on_body + (on_blade - on_body).normalize_or_zero() * body_radius)
        })
    })
}

/// What the target was doing when a blow arrived.
#[derive(Clone, Copy, Debug)]
pub struct Defense {
    /// Guard up: seconds since raised, parry window (s), share of damage stopped, cos of half
    /// the protected arc.
    pub guard: Option<(f32, f32, f32, f32)>,
    pub invulnerable: bool,
    pub stamina: f32,
    /// Flat unit facing of the target.
    pub facing: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    /// Dodged, invulnerable or already down.
    Missed,
    Parried,
    /// Guard held: damage taken and stamina spent.
    Blocked { damage: f32, stamina: f32 },
    /// Not enough stamina to hold the guard: damage taken, stamina emptied.
    GuardBroken { damage: f32 },
    Struck { damage: f32 },
}

/// Outcome of `damage` arriving from the flat direction `from` (target → attacker, unit).
pub fn judge(damage: f32, from: Vec3, defense: &Defense) -> Verdict {
    if defense.invulnerable {
        return Verdict::Missed;
    }
    let damage = finite(damage, 0.0).max(0.0);
    let Some((t, parry_window, absorb, arc_cos)) = defense.guard else {
        return Verdict::Struck { damage };
    };
    if defense.facing.dot(from) < arc_cos {
        return Verdict::Struck { damage };
    }
    if t <= parry_window {
        return Verdict::Parried;
    }
    let absorb = finite(absorb, 0.0).clamp(0.0, 1.0);
    let stopped = damage * absorb;
    let stamina = BLOCK_COST + stopped * BLOCK_COST_PER_DAMAGE;
    if defense.stamina < stamina {
        Verdict::GuardBroken { damage: damage * (0.5 + 0.5 * (1.0 - absorb)) }
    } else {
        Verdict::Blocked { damage: damage - stopped, stamina }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    Flinch,
    Stagger,
    Knockdown,
    Death,
}

/// Reaction to a blow that leaves `hp` and `poise`, with knockback `knockback` (m/s).
pub fn reaction(hp: f32, poise: f32, knockback: f32) -> Reaction {
    if !(hp > 0.0) {
        Reaction::Death
    } else if knockback >= KNOCKDOWN_SPEED || (poise <= 0.0 && knockback >= KNOCKDOWN_POISE_KB) {
        Reaction::Knockdown
    } else if poise <= 0.0 {
        Reaction::Stagger
    } else {
        Reaction::Flinch
    }
}

/// Lunge distance covered by window fraction `u` for an attack lunging `travel` m by `active`.
pub fn lunge(travel: f32, active: f32, u: f32) -> f32 {
    finite(travel, 0.0).max(0.0) * smooth01(u / finite(active, 1.0).max(1e-3))
}

#[inline]
fn finite(x: f32, fallback: f32) -> f32 {
    if x.is_finite() { x } else { fallback }
}

#[inline]
fn smooth01(x: f32) -> f32 {
    let x = finite(x, 0.0).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

fn forward(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

/// A body hit tests can strike: capsule axis (world), radius, team (dummies have none).
#[derive(Clone, Copy, Debug)]
struct Target {
    entity: Entity,
    center: Vec3,
    axis: [Vec3; 2],
    radius: f32,
    team: Option<u8>,
    velocity: Vec3,
    alive: bool,
    /// Down or dodging: hits pass.
    immune: bool,
}

fn body_axis(center: Vec3) -> [Vec3; 2] {
    let feet = center - Vec3::Y * HALF_HEIGHT;
    [feet + Vec3::Y * BODY_LOW, feet + Vec3::Y * BODY_HIGH]
}

/// Warn about moveset and reaction tracks the bank lacks.
fn validate(bank: Option<Res<PoseBank>>, movesets: Res<Movesets>) {
    let Some(bank) = bank else { return };
    for (set, track) in movesets.tracks() {
        if !bank.has(&track) {
            warn!("[combat] moveset '{set}': unknown track '{track}'");
        }
    }
    for track in tracks::ALL {
        if !bank.has(track) {
            warn!("[combat] reaction track '{track}' is not in the pose bank");
        }
    }
}

/// The player gets back up after dying.
fn enroll(mut commands: Commands, players: Query<Entity, (Added<Player>, With<Fighter>, Without<Revive>)>) {
    for player in &players {
        commands.entity(player).try_insert(Revive {
            after: PLAYER_REVIVE,
            home: None,
        });
    }
}

/// Wield a moveset (the weapon chest). Applied once the fighter is free; the request waits until
/// then.
#[derive(Message, Clone, Debug)]
pub struct Equip {
    pub who: Entity,
    pub set: String,
}

/// `Equip` messages switch the wielded moveset while the fighter is free (`Loadout` keeps the
/// sets it has wielded; `current` is the one in hand).
fn equip(
    mut requests: MessageReader<Equip>,
    mut pending: Local<Vec<Equip>>,
    mut fighters: Query<(&Action, &Melee, &mut Loadout)>,
) {
    pending.extend(requests.read().cloned());
    pending.retain(|req| {
        let Ok((action, melee, mut loadout)) = fighters.get_mut(req.who) else { return false };
        if !(action.is_free() && melee.free()) {
            return true;
        }
        let index = loadout.slots.iter().position(|s| *s == req.set).unwrap_or_else(|| {
            loadout.slots.push(req.set.clone());
            loadout.slots.len() - 1
        });
        loadout.current = index;
        false
    });
}

/// Grip transform of `def` under a hand bone with global scale `bone_scale`.
fn grip(def: &WeaponDef, bone_scale: f32) -> Transform {
    let s = if bone_scale.is_finite() && bone_scale > 1e-3 { bone_scale } else { 1.0 };
    let [x, y, z] = def.rotation_deg.map(f32::to_radians);
    Transform {
        translation: Vec3::from(def.offset) / s,
        rotation: Quat::from_euler(EulerRot::XYZ, x, y, z),
        scale: Vec3::splat(finite(def.scale, 1.0).max(0.01) / s),
    }
}

/// Keep the equipped moveset's weapon in its hand: spawn it under the hand bone, drop it when
/// the moveset changes, hide it while the hand holds a tool.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn arm(
    mut commands: Commands,
    movesets: Res<Movesets>,
    bank: Option<Res<PoseBank>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    characters: Query<(Entity, &Loadout, &Rig, Option<&WeaponSlot>, Has<Holding>)>,
    globals: Query<&GlobalTransform>,
    mut weapons: Query<(&mut Transform, &mut Visibility), With<Weapon>>,
) {
    let Some(bank) = bank else { return };
    for (entity, loadout, rig, slot, holding) in &characters {
        let def = movesets.get(loadout.equipped()).and_then(|m| m.weapon.as_ref());
        let hand = def.and_then(|d| {
            let side = usize::from(d.hand != "l");
            let bone = *bank.bones.hand.get(side)?;
            rig.bones.get(bone).copied().filter(|b| *b != Entity::PLACEHOLDER)
        });
        match (slot, def, hand) {
            (Some(slot), Some(def), Some(hand)) if slot.set == loadout.equipped() => {
                let Ok((mut tf, mut vis)) = weapons.get_mut(slot.weapon) else {
                    // The hand (and the weapon with it) went away: arm again next frame.
                    commands.entity(entity).try_remove::<WeaponSlot>();
                    continue;
                };
                let scale = globals.get(hand).map_or(1.0, |g| g.compute_transform().scale.abs().max_element());
                tf.set_if_neq(grip(def, scale));
                vis.set_if_neq(if holding { Visibility::Hidden } else { Visibility::Inherited });
            }
            (Some(slot), ..) => {
                commands.entity(slot.weapon).try_despawn();
                commands.entity(entity).try_remove::<WeaponSlot>();
            }
            (None, Some(def), Some(hand)) => {
                let weapon = commands
                    .spawn((
                        Name::new(format!("{} weapon", loadout.equipped())),
                        Weapon,
                        grip(def, 1.0),
                        Visibility::Hidden,
                        ChildOf(hand),
                    ))
                    .id();
                build_weapon(&mut commands, weapon, def, &assets, &mut meshes, &mut materials);
                commands.entity(entity).try_insert(WeaponSlot {
                    weapon,
                    set: loadout.equipped().to_string(),
                    length: finite(def.length, 0.0).max(0.0),
                });
            }
            _ => {}
        }
    }
}

/// Visual parts of a weapon under its root `weapon`.
fn build_weapon(
    commands: &mut Commands,
    weapon: Entity,
    def: &WeaponDef,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    if let Some(path) = &def.mesh {
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(path.clone()));
        let [x, y, z] = def.mesh_rotation_deg.map(f32::to_radians);
        let rotation = Quat::from_euler(EulerRot::XYZ, x, y, z);
        let at = Transform::from_translation(Vec3::from(def.mesh_offset)).with_rotation(rotation);
        commands.spawn((Name::new("Weapon mesh"), WorldAssetRoot(scene), at, ChildOf(weapon)));
        return;
    }
    let Some(prim) = &def.primitive else { return };
    let [r, g, b] = prim.color;
    let steel = materials.add(StandardMaterial {
        base_color: Color::srgb(r, g, b),
        metallic: 0.9,
        perceptual_roughness: 0.32,
        ..default()
    });
    let mut part = |mesh: Mesh, material: Handle<StandardMaterial>, at: Transform| {
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material), at, ChildOf(weapon)));
    };
    let [x, y, z] = prim.size.map(|v| finite(v, 0.1).abs().max(0.005));
    match prim.shape.as_str() {
        "sword" => {
            let brass = materials.add(StandardMaterial {
                base_color: Color::srgb(0.62, 0.48, 0.22),
                metallic: 0.8,
                perceptual_roughness: 0.4,
                ..default()
            });
            let leather = materials.add(StandardMaterial {
                base_color: Color::srgb(0.22, 0.13, 0.07),
                perceptual_roughness: 0.9,
                ..default()
            });
            part(blade_mesh(x, y, z), steel, Transform::default());
            let guard = Cuboid::new(x * GUARD_SPAN, 0.025, y * GUARD_DEPTH);
            part(guard.into(), brass.clone(), Transform::from_xyz(0.0, GUARD_AT, 0.0));
            let grip_len = GUARD_AT - GRIP_FROM;
            let handle = Cylinder::new(GRIP_RADIUS, grip_len);
            part(handle.into(), leather, Transform::from_xyz(0.0, GRIP_FROM + grip_len * 0.5, 0.0));
            part(Sphere::new(POMMEL_RADIUS).into(), brass, Transform::from_xyz(0.0, GRIP_FROM - 0.015, 0.0));
        }
        "cylinder" => part(Cylinder::new(x * 0.5, y).into(), steel, Transform::from_xyz(0.0, y * 0.5, 0.0)),
        "sphere" => part(Sphere::new(x * 0.5).into(), steel, Transform::default()),
        _ => part(Cuboid::new(x, y, z).into(), steel, Transform::from_xyz(0.0, y * 0.5, 0.0)),
    }
}

/// Primitive-free check that the shape names the moveset may use are known.
#[allow(dead_code)]
fn _shape_known(prim: &Primitive) -> bool {
    matches!(prim.shape.as_str(), "sword" | "box" | "cylinder" | "sphere")
}

/// A blade along +Y from `BLADE_FROM`: diamond cross-section (edges ±X, ridge ±Z), straight
/// sides narrowing to `SHOULDER_WIDTH` of the base, then a point. Flat shaded.
fn blade_mesh(width: f32, thickness: f32, length: f32) -> Mesh {
    let point = length * POINT_SHARE;
    let (y0, y1, y2) = (BLADE_FROM, BLADE_FROM + length - point, BLADE_FROM + length);
    let section = |y: f32, half_width: f32, half_thick: f32| {
        [
            Vec3::new(half_width, y, 0.0),
            Vec3::new(0.0, y, half_thick),
            Vec3::new(-half_width, y, 0.0),
            Vec3::new(0.0, y, -half_thick),
        ]
    };
    let base = section(y0, width * 0.5, thickness * 0.5);
    let shoulder = section(y1, width * 0.5 * SHOULDER_WIDTH, thickness * 0.42);
    let tip = Vec3::new(0.0, y2, 0.0);
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut tri = |a: Vec3, b: Vec3, c: Vec3, outward: Vec3| {
        let (b, c) = if (b - a).cross(c - a).dot(outward) >= 0.0 { (b, c) } else { (c, b) };
        positions.extend([a.to_array(), b.to_array(), c.to_array()]);
    };
    for i in 0..4 {
        let j = (i + 1) % 4;
        let out = flat(base[i] + base[j]);
        tri(base[i], base[j], shoulder[j], out);
        tri(base[i], shoulder[j], shoulder[i], out);
        tri(shoulder[i], shoulder[j], tip, flat(shoulder[i] + shoulder[j]));
    }
    tri(base[0], base[1], base[2], Vec3::NEG_Y);
    tri(base[0], base[2], base[3], Vec3::NEG_Y);
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.compute_flat_normals();
    mesh
}

/// Stamina and poise: other modules' `StaminaSpend`, sprint drain, regeneration, exhaustion
/// (and its slow-down while free).
fn stamina(
    time: Res<Time>,
    mut spends: MessageReader<StaminaSpend>,
    mut fighters: Query<(&mut Fighter, &mut Melee, &mut Mover, &Action, Option<&Derived>)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for spend in spends.read() {
        if let Ok((mut fighter, mut melee, ..)) = fighters.get_mut(spend.who) {
            melee.spend(&mut fighter, spend.amount);
        }
    }
    for (mut fighter, mut melee, mut mover, action, derived) in &mut fighters {
        if let Some(d) = derived {
            apply_derived(&mut fighter, d);
        }
        if fighter.dead {
            continue;
        }
        melee.steady += dt;
        if melee.steady > POISE_DELAY {
            fighter.poise = (fighter.poise + POISE_RATE * dt).min(fighter.max_poise);
        }
        let running = mover.sprinting && mover.mode == MoveMode::Ground && flat(mover.velocity).length() > 1.0;
        if running && !melee.exhausted {
            melee.spend(&mut fighter, SPRINT_COST * dt);
        }
        melee.rested += dt;
        if melee.rested > REGEN_DELAY {
            let rate = if melee.exhausted { EXHAUSTED_REGEN } else { REGEN_RATE } * derived.map_or(1.0, |d| d.regen);
            fighter.stamina = (fighter.stamina + rate * dt).min(fighter.max_stamina);
            if fighter.stamina >= fighter.max_stamina {
                melee.exhausted = false;
            }
        }
        let own_body = action.is_free() && matches!(mover.mode, MoveMode::Ground | MoveMode::Air);
        if own_body && melee.exhausted {
            mover.move_scale = EXHAUSTED_MOVE;
            melee.slowed = true;
        } else if own_body && melee.slowed {
            mover.move_scale = 1.0;
            melee.slowed = false;
        }
    }
}

/// Take the maxima from `stats`, keeping the current hp / stamina / poise shares.
fn apply_derived(f: &mut Fighter, d: &Derived) {
    let rescale = |now: &mut f32, max: &mut f32, new: f32| {
        if (*max - new).abs() > 1e-3 && new > 0.0 {
            let share = if *max > 0.0 { (*now / *max).clamp(0.0, 1.0) } else { 1.0 };
            *max = new;
            *now = share * new;
        }
    };
    rescale(&mut f.hp, &mut f.max_hp, d.max_hp);
    rescale(&mut f.stamina, &mut f.max_stamina, d.max_stamina);
    rescale(&mut f.poise, &mut f.max_poise, d.max_poise);
}

/// Everything a fighter's step reads besides its own components.
struct Ctx<'a> {
    dt: f32,
    bank: &'a PoseBank,
    set: Option<&'a Moveset>,
    targets: &'a [Target],
    me: Entity,
    team: u8,
    pos: Vec3,
    holding: bool,
}

/// A fighter's own components during its step.
struct Body<'a> {
    mover: &'a mut Mover,
    action: &'a mut Action,
    fighter: &'a mut Fighter,
    intent: &'a Intent,
}

impl Ctx<'_> {
    /// Combat may start something on this body now.
    fn can_act(&self, b: &Body) -> bool {
        !self.holding && b.action.is_free() && matches!(b.mover.mode, MoveMode::Ground | MoveMode::Air)
    }

    /// An enemy (alive, other team) by entity.
    fn enemy(&self, entity: Entity) -> Option<&Target> {
        self.targets
            .iter()
            .find(|t| t.entity == entity && t.alive && t.team.is_some_and(|team| team != self.team))
    }

    /// Where an attack goes: the locked target, else an enemy near the stick / facing direction,
    /// else the stick, else straight ahead.
    fn heading(&self, b: &Body) -> f32 {
        let toward = |t: &Target| yaw_of(t.center - self.pos);
        if let Some(yaw) = b.intent.target.and_then(|t| self.enemy(t)).and_then(toward) {
            return yaw;
        }
        let stick = flat(b.intent.move_dir);
        let aim = if stick.length() > 0.3 { stick.normalize() } else { b.mover.forward() };
        self.targets
            .iter()
            .filter(|t| t.alive && t.team.is_some_and(|team| team != self.team))
            .filter_map(|t| {
                let to = flat(t.center - self.pos);
                let d = to.length();
                (d < ASSIST_RANGE && d > 1e-3 && aim.dot(to / d) > ASSIST_CONE).then_some((d, to))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .and_then(|(_, to)| yaw_of(to))
            .or_else(|| (stick.length() > 0.3).then(|| yaw_of(stick)).flatten())
            .unwrap_or(b.mover.yaw)
    }

    /// Distance (m) to the nearest enemy along `dir` within the assist cone, if any.
    fn ahead(&self, dir: Vec3) -> Option<f32> {
        self.targets
            .iter()
            .filter(|t| t.alive && t.team.is_some_and(|team| team != self.team))
            .filter_map(|t| {
                let to = flat(t.center - self.pos);
                let d = to.length();
                (d > 1e-3 && dir.dot(to / d) > ASSIST_CONE).then_some(d)
            })
            .min_by(f32::total_cmp)
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn drive(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    movesets: Res<Movesets>,
    mut commands: Commands,
    mut sfx: MessageWriter<Sfx>,
    mut monitor: ResMut<Monitor>,
    mut fighters: Query<(
        Entity,
        &Transform,
        &mut Mover,
        &Intent,
        &mut Action,
        &mut Fighter,
        &mut Melee,
        &Loadout,
        Has<Holding>,
        Option<&Revive>,
        Option<&Name>,
    )>,
    dummies: Query<(Entity, &Transform), With<TrainingDummy>>,
) {
    let Some(bank) = bank else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let mut targets: Vec<Target> = fighters
        .iter()
        .map(|(entity, tf, mover, _, _, fighter, melee, ..)| Target {
            entity,
            center: tf.translation,
            axis: body_axis(tf.translation),
            radius: BODY_RADIUS,
            team: Some(fighter.team),
            velocity: mover.velocity,
            alive: !fighter.dead,
            immune: melee.iframes > 0.0 || melee.down(),
        })
        .collect();
    targets.extend(dummies.iter().map(|(entity, tf)| Target {
        entity,
        center: tf.translation,
        axis: [tf.translation - Vec3::Y * DUMMY_HALF, tf.translation + Vec3::Y * DUMMY_HALF],
        radius: DUMMY_RADIUS,
        team: None,
        velocity: Vec3::ZERO,
        alive: true,
        immune: false,
    }));
    let now = time.elapsed_secs();

    for (entity, tf, mut mover, intent, mut action, mut fighter, mut melee, loadout, holding, revive, name) in
        &mut fighters
    {
        let cx = Ctx {
            dt,
            bank: &bank,
            set: movesets.get(loadout.equipped()),
            targets: &targets,
            me: entity,
            team: fighter.team,
            pos: tf.translation,
            holding,
        };
        let mut b = Body {
            mover: &mut mover,
            action: &mut action,
            fighter: &mut fighter,
            intent,
        };
        let melee = &mut *melee;
        tick_inputs(melee, intent, dt);
        if !owned(&melee.state, b.action) {
            melee.state = State::Free;
        }
        if b.fighter.dead && !matches!(melee.state, State::Dead { .. }) {
            melee.state = die(&mut b);
        }
        let state = std::mem::take(&mut melee.state);
        melee.state = match state {
            State::Free => free(&cx, &mut b, melee),
            State::Swing(sw) => swing(*sw, &cx, &mut b, melee),
            State::Guard { t } => guard(t, &cx, &mut b, melee),
            State::Parry { t } => parry(t, &cx, &mut b, melee),
            State::Dodge(d) => dodge(d, &cx, &mut b, melee),
            State::React(r) => react(r, &cx, &mut b, melee, &mut sfx),
            State::Dead { t } => dead(t + dt, &cx, &mut b, melee, revive, &mut sfx),
        };
        if let (Some(revive), State::React(r)) = (revive, &melee.state)
            && r.kind == ReactKind::GetUp
            && r.t == 0.0
        {
            if let Some((feet, yaw)) = revive.home {
                commands.entity(entity).try_insert(Teleport { feet, yaw });
            }
            let who = name.map_or("?", |n| n.as_str());
            monitor.write(&format!("{now:.2} {who} revive - {:.3} {:?} GetUp 0.00", b.fighter.hp, b.mover.mode));
        }
    }
}

/// Age the input buffers and the timers.
fn tick_inputs(melee: &mut Melee, intent: &Intent, dt: f32) {
    if intent.primary_pressed {
        melee.primary = Some(0.0);
        melee.held = 0.0;
    } else if let Some(age) = melee.primary.as_mut() {
        *age += dt;
    }
    if intent.primary_held {
        melee.held += dt;
    } else if melee.primary.is_some_and(|age| age > BUFFER) {
        melee.primary = None;
    }
    if intent.dodge_pressed {
        melee.dodge = Some(0.0);
    } else if let Some(age) = melee.dodge.as_mut() {
        *age += dt;
        if *age > BUFFER {
            melee.dodge = None;
        }
    }
    melee.iframes = (melee.iframes - dt).max(0.0);
    melee.hurt += dt;
}

/// Whether the `Action` still belongs to the combat state (another owner may have taken it).
fn owned(state: &State, action: &Action) -> bool {
    match state {
        State::Free => true,
        State::Swing(_) => action.owner == Owner::Combat && action.kind == ActionKind::Attack,
        State::Guard { .. } => action.owner == Owner::Combat && action.kind == ActionKind::Block,
        State::Parry { .. } => action.owner == Owner::Combat && action.kind == ActionKind::Parry,
        State::Dodge(_) => action.owner == Owner::Combat && action.kind == ActionKind::Dodge,
        State::React(r) => action.owner == Owner::Hit && action.kind == r.action(),
        State::Dead { .. } => action.owner == Owner::Hit && action.kind == ActionKind::Dead,
    }
}

/// Hand the body back to the player / AI.
fn release(b: &mut Body) -> State {
    b.action.clear();
    b.mover.move_scale = 1.0;
    b.mover.face = None;
    b.mover.root_motion = Vec3::ZERO;
    State::Free
}

/// Free: lock-on facing, then dodge, guard or attack when asked.
fn free(cx: &Ctx, b: &mut Body, m: &mut Melee) -> State {
    face_target(cx, b, m);
    if !cx.can_act(b) {
        return State::Free;
    }
    let Some(set) = cx.set else { return State::Free };
    if m.dodge.is_some() && b.mover.mode == MoveMode::Ground && !m.exhausted {
        m.dodge = None;
        return start_dodge(cx, b, m);
    }
    if b.intent.secondary_held && b.mover.mode == MoveMode::Ground && set.guard.is_some() {
        return start_guard(cx, b, m);
    }
    let Some(age) = m.primary else { return State::Free };
    let kind = if b.mover.mode == MoveMode::Air {
        if set.air_attack.is_none() || b.mover.air_time < AIR_ATTACK_AFTER {
            return State::Free;
        }
        SwingKind::Air
    } else if b.mover.sprinting && flat(b.mover.velocity).length() > SPRINT_ATTACK_SPEED && set.sprint_attack.is_some() {
        SwingKind::Sprint
    } else if set.heavy.is_empty() || !b.intent.primary_held {
        if set.light.is_empty() {
            return State::Free;
        }
        SwingKind::Light(m.chain % set.light.len())
    } else if m.held >= HEAVY_HOLD.min(age + HEAVY_HOLD) {
        if m.exhausted { SwingKind::Light(m.chain % set.light.len().max(1)) } else { SwingKind::Heavy(0) }
    } else {
        // Still deciding between a tap (light) and a hold (heavy).
        return State::Free;
    };
    m.primary = None;
    start_swing(kind, cx, b, m).unwrap_or_default()
}

/// Lock-on strafing while free: face the locked target, and let go when the lock ends.
fn face_target(cx: &Ctx, b: &mut Body, m: &mut Melee) {
    let ours = b.action.is_free() && matches!(b.mover.mode, MoveMode::Ground | MoveMode::Air);
    let wanted = b.intent.target.and_then(|t| cx.enemy(t)).and_then(|t| yaw_of(t.center - cx.pos));
    match wanted {
        Some(yaw) if ours => {
            b.mover.face = Some(yaw);
            m.facing = true;
        }
        _ if m.facing => {
            if ours {
                b.mover.face = None;
            }
            m.facing = false;
        }
        _ => {}
    }
}

/// The moveset entry for a swing kind.
fn attack_of(set: &Moveset, kind: SwingKind) -> Option<&Attack> {
    match kind {
        SwingKind::Light(i) => set.light.get(i),
        SwingKind::Heavy(i) => set.heavy.get(i),
        SwingKind::Sprint => set.sprint_attack.as_ref(),
        SwingKind::Air => set.air_attack.as_ref(),
    }
}

fn start_swing(kind: SwingKind, cx: &Ctx, b: &mut Body, m: &mut Melee) -> Option<State> {
    let set = cx.set?;
    let attack = attack_of(set, kind)?.clone();
    let clip = Clip::new(&attack.clip, cx.bank);
    m.spend(b.fighter, attack.stamina);
    m.chain = match kind {
        SwingKind::Light(i) => i + 1,
        _ => 0,
    };
    let yaw = cx.heading(b);
    b.action.start(ActionKind::Attack, Owner::Combat, Some(clip.layer(None)));
    b.action.index = match kind {
        SwingKind::Light(i) | SwingKind::Heavy(i) => i,
        _ => 0,
    };
    b.mover.face = Some(yaw);
    b.mover.move_scale = 0.0;
    let plunge = if kind == SwingKind::Air {
        b.mover.external.y = PLUNGE_HOP - b.mover.velocity.y;
        Plunge::Hang
    } else {
        Plunge::None
    };
    Some(State::Swing(Box::new(Swing {
        kind,
        attack,
        clip,
        recover: None,
        elapsed: 0.0,
        charged: 0.0,
        yaw,
        lunged: 0.0,
        reach: reach(set),
        struck: Vec::new(),
        prev: None,
        plunge,
        whoosh: false,
        primed: false,
    })))
}

/// One frame of an attack: advance (charge hold, plunge), chain / cancel, recovery, root motion.
fn swing(mut sw: Swing, cx: &Ctx, b: &mut Body, m: &mut Melee) -> State {
    sw.elapsed += cx.dt;
    if sw.recover.is_some() {
        return recovering(sw, cx, b, m);
    }
    let t_before = sw.clip.t;
    let windup_t = sw.clip.at(sw.attack.windup);
    let charging = sw.heavy()
        && sw.clip.t >= windup_t
        && sw.charged < sw.attack.charge
        && b.intent.primary_held
        && !m.exhausted;
    if charging {
        sw.clip.t = windup_t;
        sw.charged += cx.dt;
        m.spend(b.fighter, CHARGE_COST * cx.dt);
    } else if !plunge(&mut sw, b) {
        sw.clip.t += cx.dt * sw.clip.rate;
    }
    let u = sw.clip.u();
    if u >= sw.attack.cancel {
        if let Some(next) = cancel(&sw, cx, b, m) {
            return next;
        }
    }
    if u >= 1.0 || (sw.plunge == Plunge::Dive && sw.elapsed > PLUNGE_TIMEOUT) {
        m.chain = 0;
        let Some(clip) = sw.attack.recover.as_ref().map(|r| Clip::new(r, cx.bank)) else {
            return release(b);
        };
        b.action.layer = Some(clip.layer(None));
        sw.recover = Some(clip);
        return State::Swing(Box::new(sw));
    }
    swing_body(&mut sw, t_before, cx, b);
    if let Some(layer) = b.action.layer.as_mut() {
        layer.time = sw.clip.t;
    }
    b.action.elapsed = sw.elapsed;
    State::Swing(Box::new(sw))
}

/// Plunge phases; true while the clip time is held (diving).
fn plunge(sw: &mut Swing, b: &mut Body) -> bool {
    let landed = b.mover.mode == MoveMode::Ground;
    match sw.plunge {
        Plunge::None | Plunge::Landed => false,
        Plunge::Hang if landed => {
            sw.plunge = Plunge::Quake;
            false
        }
        Plunge::Hang => {
            if sw.clip.u() >= sw.attack.windup {
                sw.plunge = Plunge::Dive;
                b.mover.external.y = -PLUNGE_SPEED - b.mover.velocity.y;
            }
            false
        }
        Plunge::Dive if landed => {
            sw.plunge = Plunge::Quake;
            false
        }
        Plunge::Dive => {
            let hold = sw.clip.at(sw.attack.active * PLUNGE_HOLD);
            if sw.clip.t >= hold {
                sw.clip.t = hold;
                return true;
            }
            false
        }
        // Dealt by `hit_test` this frame.
        Plunge::Quake => false,
    }
}

/// After `cancel`: dodge, chain the next light, or raise the guard.
fn cancel(sw: &Swing, cx: &Ctx, b: &mut Body, m: &mut Melee) -> Option<State> {
    if cx.holding || b.mover.mode != MoveMode::Ground {
        return None;
    }
    let set = cx.set?;
    if m.dodge.is_some() && !m.exhausted {
        m.dodge = None;
        b.action.clear();
        return Some(start_dodge(cx, b, m));
    }
    if m.primary.is_some()
        && let SwingKind::Light(i) = sw.kind
        && i + 1 < set.light.len()
    {
        m.primary = None;
        b.action.clear();
        return start_swing(SwingKind::Light(i + 1), cx, b, m);
    }
    if b.intent.secondary_held && set.guard.is_some() {
        b.action.clear();
        return Some(start_guard(cx, b, m));
    }
    None
}

/// Facing, lunge, clip root travel and stick share for this frame of the swing.
fn swing_body(sw: &mut Swing, t_before: f32, cx: &Ctx, b: &mut Body) {
    let u = sw.clip.u();
    if u < sw.attack.windup && sw.plunge == Plunge::None {
        sw.yaw = cx.heading(b);
    }
    b.mover.face = Some(sw.yaw);
    let after = ((u - sw.attack.cancel) / (1.0 - sw.attack.cancel).max(1e-3)).clamp(0.0, 1.0);
    b.mover.move_scale = if u >= sw.attack.cancel { RECOVER_MOVE * smooth01(after) } else { 0.0 };
    let dt = cx.dt.max(1e-4);
    if sw.kind == SwingKind::Air {
        b.mover.root_motion = Vec3::ZERO;
        return;
    }
    let travel = Quat::from_rotation_y(b.mover.yaw) * cx.bank.travel(&sw.clip.clip.track, t_before, sw.clip.t);
    let dir = forward(sw.yaw);
    let target = lunge(sw.attack.travel, sw.attack.active, u);
    let room = cx.ahead(dir).map_or(f32::INFINITY, |d| (d - LUNGE_STOP).max(0.0));
    let step = (target - sw.lunged).clamp(0.0, (room - sw.lunged).max(0.0));
    sw.lunged += step;
    b.mover.root_motion = flat(travel) / dt + dir * (step / dt);
}

/// The recovery clip after the chain stopped: any new input takes over.
fn recovering(mut sw: Swing, cx: &Ctx, b: &mut Body, m: &mut Melee) -> State {
    let Some(mut clip) = sw.recover.take() else {
        return release(b);
    };
    let t_before = clip.t;
    clip.t += cx.dt * clip.rate;
    let u = clip.u();
    let ground = b.mover.mode == MoveMode::Ground && !cx.holding;
    if ground && m.dodge.is_some() && !m.exhausted {
        m.dodge = None;
        b.action.clear();
        return start_dodge(cx, b, m);
    }
    if ground && m.primary.is_some() && !b.intent.primary_held {
        m.primary = None;
        b.action.clear();
        return start_swing(SwingKind::Light(0), cx, b, m).unwrap_or_default();
    }
    if ground && b.intent.secondary_held && cx.set.is_some_and(|s| s.guard.is_some()) {
        b.action.clear();
        return start_guard(cx, b, m);
    }
    if u >= 1.0 || (u > 0.3 && b.intent.move_dir.length() > RECOVER_BREAK) {
        return release(b);
    }
    let travel = Quat::from_rotation_y(b.mover.yaw) * cx.bank.travel(&clip.clip.track, t_before, clip.t);
    b.mover.root_motion = flat(travel) / cx.dt.max(1e-4);
    b.mover.move_scale = RECOVER_MOVE * smooth01(u);
    b.mover.face = Some(sw.yaw);
    if let Some(layer) = b.action.layer.as_mut() {
        layer.time = clip.t;
    }
    b.action.elapsed = sw.elapsed;
    sw.recover = Some(clip);
    State::Swing(Box::new(sw))
}

fn start_guard(cx: &Ctx, b: &mut Body, _m: &mut Melee) -> State {
    let Some(guard) = cx.set.and_then(|s| s.guard.as_ref()) else {
        return State::Free;
    };
    let clip = Clip::new(&guard.clip, cx.bank);
    b.action.start(ActionKind::Block, Owner::Combat, Some(clip.layer(Some(upper(guard.clip.mask)))));
    b.mover.move_scale = GUARD_MOVE;
    State::Guard { t: 0.0 }
}

/// A guard layer drives the upper body so the legs keep walking (a full mask becomes upper).
fn upper(mask: Mask) -> Mask {
    if mask == Mask::Full { Mask::Upper } else { mask }
}

/// Guard up while secondary is held: the clip rises to `hold` and stays; attack / dodge leave.
fn guard(t: f32, cx: &Ctx, b: &mut Body, m: &mut Melee) -> State {
    let Some(spec) = cx.set.and_then(|s| s.guard.as_ref()) else {
        return release(b);
    };
    let t = t + cx.dt;
    if !b.intent.secondary_held || b.mover.mode != MoveMode::Ground {
        return release(b);
    }
    if m.dodge.is_some() && !m.exhausted {
        m.dodge = None;
        b.action.clear();
        return start_dodge(cx, b, m);
    }
    if m.primary.is_some() && !b.intent.primary_held {
        m.primary = None;
        b.action.clear();
        let light = SwingKind::Light(0);
        return start_swing(light, cx, b, m).unwrap_or_default();
    }
    let clip = Clip::new(&spec.clip, cx.bank);
    let hold = finite(spec.hold, clip.from).clamp(clip.from, clip.to);
    if let Some(layer) = b.action.layer.as_mut() {
        layer.time = (clip.from + t * clip.rate).min(hold);
    }
    b.action.elapsed = t;
    b.mover.move_scale = GUARD_MOVE;
    b.mover.root_motion = Vec3::ZERO;
    let look = flat(b.intent.look - cx.pos);
    b.mover.face = b
        .intent
        .target
        .and_then(|e| cx.enemy(e))
        .and_then(|e| yaw_of(e.center - cx.pos))
        .or_else(|| yaw_of(look));
    State::Guard { t }
}

/// The parry flourish; back to the guard (still held) or free when it ends.
fn parry(t: f32, cx: &Ctx, b: &mut Body, m: &mut Melee) -> State {
    let Some(spec) = cx.set.and_then(|s| s.guard.as_ref()) else {
        return release(b);
    };
    let t = t + cx.dt;
    let clip = Clip::new(spec.parry.as_ref().unwrap_or(&spec.clip), cx.bank);
    if t >= clip.length() {
        b.action.clear();
        return if b.intent.secondary_held { start_guard(cx, b, m) } else { release(b) };
    }
    if let Some(layer) = b.action.layer.as_mut() {
        layer.time = clip.from + t * clip.rate;
    }
    b.action.elapsed = t;
    b.mover.root_motion = Vec3::ZERO;
    b.mover.move_scale = 0.0;
    State::Parry { t }
}

/// Start a parry on a guarding fighter (from `resolve`).
fn start_parry(set: Option<&Moveset>, bank: &PoseBank, action: &mut Action, m: &mut Melee) {
    let Some(spec) = set.and_then(|s| s.guard.as_ref()) else { return };
    let clip = Clip::new(spec.parry.as_ref().unwrap_or(&spec.clip), bank);
    action.start(ActionKind::Parry, Owner::Combat, Some(clip.layer(Some(upper(clip.clip.mask)))));
    m.state = State::Parry { t: 0.0 };
}

/// Roll toward the stick, or (locked on and not pushing forward) a sidestep / backstep.
fn start_dodge(cx: &Ctx, b: &mut Body, m: &mut Melee) -> State {
    let stick = flat(b.intent.move_dir);
    let facing = b.mover.forward();
    let locked = b.intent.target.and_then(|t| cx.enemy(t)).and_then(|t| yaw_of(t.center - cx.pos));
    let dir = if stick.length() > 0.2 { stick.normalize() } else { -facing };
    let (kind, track, face) = match locked {
        Some(yaw) if dir.dot(forward(yaw)) < 0.5 => {
            let side = dir.dot(Vec3::Y.cross(forward(yaw)));
            let track = if side > STEP_SIDE {
                STEP_LEFT
            } else if side < -STEP_SIDE {
                STEP_RIGHT
            } else {
                STEP_BACK
            };
            (DodgeKind::Step, track, yaw)
        }
        _ => (DodgeKind::Roll, ROLL, yaw_of(dir).unwrap_or(b.mover.yaw)),
    };
    let (cost, offset, iframes) = match kind {
        DodgeKind::Roll => (ROLL_COST, ROLL_WINDOW.0, ROLL_IFRAMES.1),
        DodgeKind::Step => (STEP_COST, 0.0, STEP_IFRAMES),
    };
    m.spend(b.fighter, cost);
    m.iframes = iframes;
    m.chain = 0;
    let layer = ActionLayer {
        looping: kind == DodgeKind::Step,
        ..ActionLayer::new(track, offset, Mask::Full)
    };
    b.action.start(ActionKind::Dodge, Owner::Combat, Some(layer));
    b.mover.move_scale = 0.0;
    b.mover.face = Some(face);
    State::Dodge(Dodge {
        kind,
        track,
        dir,
        face,
        t: 0.0,
    })
}

fn dodge(mut d: Dodge, cx: &Ctx, b: &mut Body, m: &mut Melee) -> State {
    let t_before = d.t;
    d.t += cx.dt;
    let (duration, rate, offset) = match d.kind {
        DodgeKind::Roll => ((ROLL_WINDOW.1 - ROLL_WINDOW.0) / ROLL_RATE, ROLL_RATE, ROLL_WINDOW.0),
        DodgeKind::Step => (STEP_TIME, STEP_RATE, 0.0),
    };
    let u = d.t / duration;
    if d.kind == DodgeKind::Roll && d.t < ROLL_IFRAMES.0 {
        m.iframes = m.iframes.max(ROLL_IFRAMES.1 - d.t);
    }
    if u >= DODGE_CANCEL && m.primary.is_some() && !b.intent.primary_held && !cx.holding {
        m.primary = None;
        b.action.clear();
        return start_swing(SwingKind::Light(0), cx, b, m).unwrap_or_default();
    }
    if u >= 1.0 || b.mover.mode != MoveMode::Ground {
        return release(b);
    }
    let dt = cx.dt.max(1e-4);
    b.mover.root_motion = match d.kind {
        DodgeKind::Roll => {
            let along = cx.bank.travel(ROLL, offset + t_before * rate, offset + d.t * rate).z;
            d.dir * (along * ROLL_TRAVEL / dt)
        }
        // Linear ease-out covering `STEP_DISTANCE` over the step.
        DodgeKind::Step => d.dir * (2.0 * STEP_DISTANCE / STEP_TIME * (1.0 - u).max(0.0)),
    };
    let track_time = offset + d.t * rate;
    if let Some(layer) = b.action.layer.as_mut() {
        layer.time = match d.kind {
            DodgeKind::Roll => track_time,
            DodgeKind::Step => track_time % cx.bank.duration(d.track).max(1e-3),
        };
    }
    b.action.elapsed = d.t;
    b.mover.face = Some(d.face);
    b.mover.move_scale = 0.0;
    State::Dodge(d)
}

/// A hit reaction (`Owner::Hit`).
fn react(mut r: React, _cx: &Ctx, b: &mut Body, _m: &mut Melee, sfx: &mut MessageWriter<Sfx>) -> State {
    r.t += _cx.dt;
    if r.kind == ReactKind::Knockdown && !r.fell && r.t >= KNOCKDOWN_FALL_AT {
        r.fell = true;
        sfx.write(Sfx {
            kind: SfxKind::BodyFall,
            at: _cx.pos,
            volume: 0.8,
        });
    }
    if r.t >= r.duration {
        if r.kind == ReactKind::Knockdown {
            return start_react(get_up(), b);
        }
        return release(b);
    }
    if let Some(layer) = b.action.layer.as_mut() {
        layer.time = r.layer().time;
    }
    b.action.elapsed = r.t;
    b.mover.root_motion = Vec3::ZERO;
    b.mover.move_scale = if r.kind == ReactKind::Flinch { FLINCH_MOVE } else { 0.0 };
    b.mover.face = r.face;
    State::React(r)
}

fn get_up() -> React {
    React::new(ReactKind::GetUp, GET_UP, 0.0, GET_UP_RATE, 0.0)
}

/// Start reaction `r` on a body; its duration defaults to the rest of the clip.
fn start_react(mut r: React, b: &mut Body) -> State {
    begin_react(&mut r, b.action, b.mover, None);
    State::React(r)
}

fn begin_react(r: &mut React, action: &mut Action, mover: &mut Mover, bank: Option<&PoseBank>) {
    if r.duration <= 0.0 {
        let clip = bank.map_or(1.2, |bank| bank.duration(r.track));
        r.duration = ((clip - r.offset) / r.rate).max(0.2);
    }
    action.start(r.action(), Owner::Hit, Some(r.layer()));
    mover.move_scale = 0.0;
    mover.root_motion = Vec3::ZERO;
    mover.face = r.face;
}

/// Death: the clip plays once and holds; nothing else moves the body.
fn die(b: &mut Body) -> State {
    b.action.start(ActionKind::Dead, Owner::Hit, Some(ActionLayer::new(DEATH, 0.0, Mask::Full)));
    b.mover.move_scale = 0.0;
    b.mover.face = None;
    b.mover.root_motion = Vec3::ZERO;
    State::Dead { t: 0.0 }
}

fn dead(
    t: f32,
    cx: &Ctx,
    b: &mut Body,
    m: &mut Melee,
    revive: Option<&Revive>,
    sfx: &mut MessageWriter<Sfx>,
) -> State {
    if t - cx.dt < DEATH_FALL_AT && t >= DEATH_FALL_AT {
        sfx.write(Sfx {
            kind: SfxKind::BodyFall,
            at: cx.pos,
            volume: 0.9,
        });
    }
    if let Some(layer) = b.action.layer.as_mut() {
        layer.time = t.min(cx.bank.duration(DEATH));
    }
    b.action.elapsed = t;
    b.mover.move_scale = 0.0;
    b.mover.root_motion = Vec3::ZERO;
    if revive.is_some_and(|r| t >= r.after) {
        let f = &mut *b.fighter;
        f.dead = false;
        f.hp = f.max_hp;
        f.poise = f.max_poise;
        f.stamina = f.max_stamina;
        m.exhausted = false;
        m.chain = 0;
        let mut r = get_up();
        begin_react(&mut r, b.action, b.mover, Some(cx.bank));
        m.iframes = r.duration;
        return State::React(r);
    }
    State::Dead { t }
}

/// The world segment and radius of a swing's hitbox: the attack's bone segment, the held blade,
/// or the right fist.
fn hitbox(
    attack: &Attack,
    rig: Option<&Rig>,
    slot: Option<&WeaponSlot>,
    bank: &PoseBank,
    globals: &Query<&GlobalTransform>,
) -> Option<([Vec3; 2], f32)> {
    let bone = |index: usize| {
        let entity = rig?.bones.get(index).copied().filter(|e| *e != Entity::PLACEHOLDER)?;
        let (_, rotation, translation) = globals.get(entity).ok()?.to_scale_rotation_translation();
        Some((translation, rotation))
    };
    let segment = if let Some(h) = &attack.hitbox {
        let (pos, rot) = bone(bank.skeleton.find(&h.bone)?)?;
        let tip = h.tip.map_or(pos, |t| pos + rot * Vec3::from(t));
        ([pos, tip], h.radius)
    } else if let Some(slot) = slot {
        let g = globals.get(slot.weapon).ok()?;
        let base = g.transform_point(Vec3::Y * BLADE_FROM);
        ([base, g.transform_point(Vec3::Y * (BLADE_FROM + slot.length))], BLADE_RADIUS)
    } else {
        let (pos, _) = bone(bank.bones.hand[1])?;
        ([pos, pos], FIST_RADIUS)
    };
    (segment.0[0].is_finite() && segment.0[1].is_finite()).then_some(segment)
}

/// Sweep every active hitbox from last frame's pose to this one and emit `Hit` for each new
/// target it meets; plunges that just landed hit everything around them.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn hit_test(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    mut hits: MessageWriter<Hit>,
    mut sfx: MessageWriter<Sfx>,
    mut shake: ResMut<CameraShake>,
    mut fighters: Query<(Entity, &Transform, &Mover, &Fighter, &mut Melee, Option<&Rig>, Option<&WeaponSlot>, Has<Player>)>,
    dummies: Query<(Entity, &Transform), With<TrainingDummy>>,
    globals: Query<&GlobalTransform>,
) {
    let Some(bank) = bank else { return };
    let dt = time.delta_secs().max(1e-4);
    let mut targets: Vec<Target> = fighters
        .iter()
        .map(|(entity, tf, mover, fighter, melee, ..)| Target {
            entity,
            center: tf.translation,
            axis: body_axis(tf.translation),
            radius: BODY_RADIUS,
            team: Some(fighter.team),
            velocity: mover.velocity,
            alive: !fighter.dead,
            immune: melee.iframes > 0.0 || melee.down(),
        })
        .collect();
    targets.extend(dummies.iter().map(|(entity, tf)| Target {
        entity,
        center: tf.translation,
        axis: [tf.translation - Vec3::Y * DUMMY_HALF, tf.translation + Vec3::Y * DUMMY_HALF],
        radius: DUMMY_RADIUS,
        team: None,
        velocity: Vec3::ZERO,
        alive: true,
        immune: false,
    }));
    for (entity, tf, _, fighter, mut melee, rig, slot, player) in &mut fighters {
        let State::Swing(sw) = &mut melee.state else { continue };
        // A heavy held to full charge strikes by itself; a ring and a kick say it is coming.
        if sw.heavy() && !sw.primed && sw.attack.charge > 0.0 && sw.charged >= sw.attack.charge {
            sw.primed = true;
            sfx.write(Sfx {
                kind: SfxKind::Clank,
                at: tf.translation + Vec3::Y * 0.4,
                volume: 0.8,
            });
            if player {
                shake.add(SHAKE_PRIMED);
            }
        }
        let enemies = targets
            .iter()
            .filter(|t| t.entity != entity && t.alive && !t.immune && t.team != Some(fighter.team));
        if sw.plunge == Plunge::Quake {
            sw.plunge = Plunge::Landed;
            let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
            for t in enemies.clone() {
                let away = flat(t.center - feet);
                if away.length() > PLUNGE_RADIUS + t.radius || sw.struck.contains(&t.entity) {
                    continue;
                }
                sw.struck.push(t.entity);
                hits.write(blow(entity, t, sw, feet + Vec3::Y * 0.4, away.normalize_or(Vec3::Z), PLUNGE_SPEED));
            }
            sfx.write(Sfx {
                kind: SfxKind::BodyFall,
                at: feet,
                volume: 1.0,
            });
            if player {
                shake.add(SHAKE_PLUNGE);
            }
        }
        if !sw.active() {
            sw.prev = None;
            continue;
        }
        let Some((now, radius)) = hitbox(&sw.attack, rig, slot, &bank, &globals) else { continue };
        if !sw.whoosh {
            sw.whoosh = true;
            sfx.write(Sfx {
                kind: SfxKind::Swing,
                at: now[1],
                volume: if sw.heavy() { 0.7 } else { 0.5 },
            });
        }
        let prev = sw.prev.replace(now).unwrap_or(now);
        for t in enemies {
            if sw.struck.contains(&t.entity) {
                continue;
            }
            let Some((_, s, point)) = sweep(prev, now, radius, t.axis, t.radius) else { continue };
            let velocity = (now[0].lerp(now[1], s) - prev[0].lerp(prev[1], s)) / dt;
            let closing = (velocity - t.velocity).length();
            if prev != now && velocity.length() < MIN_HIT_SPEED {
                continue;
            }
            sw.struck.push(t.entity);
            let away = flat(t.center - tf.translation).normalize_or(forward(sw.yaw));
            let swept = flat(velocity).normalize_or_zero();
            let dir = (away * 0.65 + swept * 0.35).normalize_or(away);
            hits.write(blow(entity, t, sw, point, dir, closing));
        }
    }
}

/// The `Hit` a swing deals to `target` at `point`, blowing along `dir` at closing speed `speed`.
fn blow(attacker: Entity, target: &Target, sw: &Swing, point: Vec3, dir: Vec3, speed: f32) -> Hit {
    let charge = sw.charged.clamp(0.0, sw.attack.charge.max(0.0));
    Hit {
        attacker,
        target: target.entity,
        point,
        direction: dir,
        damage: sw.attack.damage * (1.0 + charge),
        poise: sw.attack.poise * (1.0 + charge),
        knockback: sw.attack.knockback * (1.0 + 0.5 * charge),
        speed,
        kind: HitKind::Melee,
    }
}

/// What `resolve` knows about the attacker of a blow.
#[derive(Clone, Debug)]
struct Attacker {
    pos: Vec3,
    scale: f32,
    player: bool,
    /// The wielded moveset (the `hit` deed's tag).
    weapon: String,
}

#[allow(clippy::type_complexity)]
type Victim<'a> = (
    &'a Transform,
    &'a mut Mover,
    &'a mut Action,
    &'a mut Fighter,
    &'a mut Melee,
    &'a Loadout,
    Option<&'a mut MotionLayers>,
    Option<&'a DamageScale>,
    Has<Player>,
    Option<&'a Name>,
    Option<&'a Ward>,
);

/// Feedback shared by every outcome: hitstop, shake, sound, sparks.
// One lifetime per writer: they are separate system params and `&mut` is invariant.
struct Feedback<'sfx, 'bursts, 'deeds, 'a> {
    feel: &'a Feel,
    stop: &'a mut Hitstop,
    shake: &'a mut CameraShake,
    sfx: &'a mut MessageWriter<'sfx, Sfx>,
    bursts: &'a mut MessageWriter<'bursts, Burst>,
    deeds: &'a mut MessageWriter<'deeds, Deed>,
    monitor: &'a mut Monitor,
    now: f32,
}

impl Feedback<'_, '_, '_, '_> {
    fn deed(&mut self, who: Entity, kind: &'static str, tag: &str) {
        self.deeds.write(Deed::new(who, kind, tag, 1.0));
    }
    fn strike(&mut self, involved: bool, hitstop: f32, trauma: f32, sound: SfxKind, at: Vec3, volume: f32) {
        if involved {
            self.stop.trigger(self.feel.hitstop.max(0.0) * hitstop);
            self.shake.add(trauma);
        }
        self.sfx.write(Sfx { kind: sound, at, volume });
    }
    fn sparks(&mut self, at: Vec3, dir: Vec3, count: u32) {
        self.bursts.write(Burst {
            kind: BurstKind::Sparks,
            at,
            dir,
            count,
        });
    }
}

/// The only `Hit` resolver: guard / parry / dodge, damage, poise, reactions, death, knockback,
/// impulses, ragdoll requests and all the feedback.
#[allow(clippy::too_many_arguments)]
fn resolve(
    time: Res<Time>,
    feel: Res<Feel>,
    bank: Option<Res<PoseBank>>,
    movesets: Res<Movesets>,
    mut hits: MessageReader<Hit>,
    mut stop: ResMut<Hitstop>,
    mut shake: ResMut<CameraShake>,
    mut sfx: MessageWriter<Sfx>,
    mut bursts: MessageWriter<Burst>,
    mut deeds: MessageWriter<Deed>,
    mut monitor: ResMut<Monitor>,
    mut commands: Commands,
    mut fighters: Query<Victim>,
) {
    let Some(bank) = bank else {
        hits.clear();
        return;
    };
    let mut fx = Feedback {
        feel: &feel,
        stop: &mut stop,
        shake: &mut shake,
        sfx: &mut sfx,
        bursts: &mut bursts,
        deeds: &mut deeds,
        monitor: &mut monitor,
        now: time.elapsed_secs(),
    };
    for hit in hits.read() {
        let attacker = fighters.get(hit.attacker).ok().map(|(tf, _, _, _, _, loadout, _, scale, player, _, _)| Attacker {
            pos: tf.translation,
            scale: scale.map_or(1.0, |s| finite(s.0, 1.0).max(0.0)),
            player,
            weapon: loadout.equipped().to_string(),
        });
        let attacker = attacker.unwrap_or(Attacker {
            pos: hit.point - hit.direction,
            scale: 1.0,
            player: false,
            weapon: String::new(),
        });
        let Ok(victim) = fighters.get_mut(hit.target) else {
            // A training dummy (or anything else that can't be hurt): feedback only.
            let heavy = hit.knockback >= KNOCKDOWN_POISE_KB;
            let sound = if heavy { SfxKind::HitHeavy } else { SfxKind::Hit };
            fx.strike(attacker.player, 1.0, SHAKE_LIGHT, sound, hit.point, 0.8);
            if hit.kind == HitKind::Melee {
                fx.deed(hit.attacker, "hit", &attacker.weapon);
            }
            continue;
        };
        let parried = strike(hit, &attacker, victim, &bank, &movesets, &feel, &mut fx, &mut commands);
        if parried && let Ok((_, mut mover, mut action, _, mut melee, ..)) = fighters.get_mut(hit.attacker) {
            let mut r = React::new(ReactKind::Stagger, STAGGER, 0.0, PARRIED_RATE, 0.0);
            r.face = Some(mover.yaw);
            begin_react(&mut r, &mut action, &mut mover, Some(&bank));
            melee.state = State::React(r);
        }
    }
}

/// Apply one blow to its target; true when it was parried (the attacker staggers).
#[allow(clippy::too_many_arguments)]
fn strike(
    hit: &Hit,
    attacker: &Attacker,
    victim: QueryItem<'_, '_, Victim>,
    bank: &PoseBank,
    movesets: &Movesets,
    feel: &Feel,
    fx: &mut Feedback,
    commands: &mut Commands,
) -> bool {
    let (tf, mut mover, mut action, mut fighter, mut melee, loadout, layers, _, player, name, ward) = victim;
    if fighter.dead {
        return false;
    }
    let set = movesets.get(loadout.equipped());
    let facing = mover.forward();
    let from = flat(attacker.pos - tf.translation).normalize_or(-hit.direction);
    let guard = match melee.state {
        State::Guard { t } => set.and_then(|s| s.guard.as_ref()).map(|g| {
            let arc = (finite(g.arc, 120.0).clamp(0.0, 360.0) * 0.5).to_radians().cos();
            (t, g.parry_window, g.absorb, arc)
        }),
        _ => None,
    };
    let defense = Defense {
        guard,
        invulnerable: melee.iframes > 0.0 || melee.down(),
        stamina: fighter.stamina,
        facing,
    };
    let factor = speed_factor(hit.speed);
    // Spells arrive already scaled by the caster's spell power; `might` is for weapons.
    let scale = if hit.kind == HitKind::Melee { attacker.scale } else { 1.0 };
    // An Aura Ward (`magic`) turns aside its share of damage and poise.
    let kept = 1.0 - ward.map_or(0.0, |w| finite(w.share, 0.0).clamp(0.0, 0.9));
    if kept < 1.0 && !matches!(melee.state, State::Guard { .. }) {
        // The shell flares where the blow lands.
        fx.sparks(hit.point, flat(hit.point - tf.translation).normalize_or(-hit.direction), SPARKS_WARD);
        fx.sfx.write(Sfx { kind: SfxKind::Block, at: hit.point, volume: 0.5 });
    }
    let damage = hit.damage * factor * scale * kept;
    let poise = hit.poise * factor * scale * kept;
    let knockback = hit.knockback * factor * feel.knockback_scale.max(0.0);
    let involved = attacker.player || player;
    let who = name.map_or("?", |n| n.as_str()).to_string();
    let verdict = judge(damage, from, &defense);
    let log = |fx: &mut Feedback, what: &str, value: f32, action: &Action, mover: &Mover| {
        let line = format!(
            "{:.2} {who} {what} - {value:.3} {:?} {:?} {:.2}",
            fx.now, mover.mode, action.kind, hit.speed
        );
        fx.monitor.write(&line);
    };
    let impulses = |layers: Option<Mut<MotionLayers>>, dir: Vec3, kb: f32, head: bool| {
        let Some(mut layers) = layers else { return };
        let axis = Vec3::Y.cross(flat(dir)).normalize_or_zero();
        let mag = (IMPULSE_BASE + IMPULSE_PER * kb).min(IMPULSE_MAX);
        let b = &bank.bones;
        let bones = [b.spine[1], b.spine[2], b.neck, b.head];
        for (i, (bone, share)) in bones.into_iter().zip(IMPULSE_SHARE).enumerate() {
            let boost = if head && i >= 2 { 1.5 } else { 1.0 };
            layers.impulses.push((bone, axis * mag * share * boost));
        }
    };
    match verdict {
        Verdict::Missed => false,
        Verdict::Parried => {
            start_parry(set, bank, &mut action, &mut melee);
            fx.strike(involved, HITSTOP_PARRY, SHAKE_PARRY, SfxKind::Parry, hit.point, 1.0);
            fx.sparks(hit.point, from, SPARKS_PARRY);
            log(fx, "parry", 0.0, &action, &mover);
            fx.deed(hit.target, "parry", "");
            true
        }
        Verdict::Blocked { damage, stamina } => {
            melee.spend(&mut fighter, stamina);
            fighter.hp = (fighter.hp - damage).max(0.0);
            push(&mut mover, hit.direction, knockback.min(BLOCK_PUSH), 0.0);
            impulses(layers, hit.direction, knockback * 0.4, false);
            fx.strike(involved, HITSTOP_BLOCK, SHAKE_BLOCK, SfxKind::Block, hit.point, 0.9);
            fx.sparks(hit.point, from, SPARKS_BLOCK);
            log(fx, "hit_blocked", damage, &action, &mover);
            fx.deed(hit.target, "block", "");
            if fighter.hp <= 0.0 {
                fx.deed(hit.attacker, "kill", "");
                kill(&mut fighter, &mut action, &mut mover, &mut melee, fx, hit.point, involved);
            }
            false
        }
        Verdict::GuardBroken { damage } => {
            let (stamina, max_poise) = (fighter.stamina.max(1.0), fighter.max_poise);
            melee.spend(&mut fighter, stamina);
            hurt(&mut fighter, &mut melee, damage, max_poise);
            impulses(layers, hit.direction, knockback, false);
            fx.strike(involved, HITSTOP_HEAVY, SHAKE_HEAVY, SfxKind::Block, hit.point, 1.0);
            fx.sparks(hit.point, from, SPARKS_BLOCK);
            log(fx, "guard_break", damage, &action, &mover);
            if hit.kind == HitKind::Melee {
                fx.deed(hit.attacker, "hit", &attacker.weapon);
            }
            if fighter.hp <= 0.0 {
                fx.deed(hit.attacker, "kill", "");
                kill(&mut fighter, &mut action, &mut mover, &mut melee, fx, hit.point, involved);
            } else {
                stagger(&mut melee, &mut action, &mut mover, bank, from, hit.direction, knockback);
            }
            false
        }
        Verdict::Struck { damage } => {
            hurt(&mut fighter, &mut melee, damage, poise);
            let feet = tf.translation.y - HALF_HEIGHT;
            let head = hit.point.y - feet > HEAD_HEIGHT;
            impulses(layers, hit.direction, knockback, head);
            let outcome = reaction(fighter.hp, fighter.poise, knockback);
            let heavy = outcome != Reaction::Flinch || knockback >= KNOCKDOWN_POISE_KB;
            let sound = if heavy { SfxKind::HitHeavy } else { SfxKind::Hit };
            let (stop, trauma) = match outcome {
                Reaction::Death => (HITSTOP_KILL, SHAKE_DOWN),
                Reaction::Knockdown => (HITSTOP_HEAVY, SHAKE_DOWN),
                _ if heavy => (HITSTOP_HEAVY, SHAKE_HEAVY),
                _ => (1.0, SHAKE_LIGHT),
            };
            fx.strike(involved, stop, trauma, sound, hit.point, 1.0);
            if knockback > feel.ragdoll_threshold {
                commands.entity(hit.target).try_insert(RagdollRequest {
                    velocity: hit.direction * knockback + Vec3::Y * KNOCKDOWN_LIFT,
                    point: hit.point,
                });
            }
            log(fx, &format!("hit_{outcome:?}").to_lowercase(), damage, &action, &mover);
            if hit.kind == HitKind::Melee {
                fx.deed(hit.attacker, "hit", &attacker.weapon);
            }
            fx.deed(hit.target, "struck", &format!("{:?}", hit.kind));
            if outcome == Reaction::Death {
                fx.deed(hit.attacker, "kill", "");
            }
            let swimming = !matches!(mover.mode, MoveMode::Ground | MoveMode::Air);
            match outcome {
                Reaction::Death => {
                    push(&mut mover, hit.direction, knockback.min(STAGGER_PUSH * 2.0), KNOCKDOWN_LIFT * 0.5);
                    kill(&mut fighter, &mut action, &mut mover, &mut melee, fx, hit.point, involved);
                }
                _ if swimming => {}
                Reaction::Knockdown => {
                    fighter.poise = fighter.max_poise;
                    let mut r = React::new(ReactKind::Knockdown, KNOCKDOWN, KNOCKDOWN_OFFSET, 1.0, 0.0);
                    r.face = yaw_of(from);
                    begin_react(&mut r, &mut action, &mut mover, Some(bank));
                    r.duration += DOWN_TIME;
                    melee.state = State::React(r);
                    melee.iframes = 0.0;
                    push(&mut mover, hit.direction, knockback, KNOCKDOWN_LIFT);
                    log(fx, "knockdown", knockback, &action, &mover);
                }
                Reaction::Stagger => {
                    fighter.poise = fighter.max_poise;
                    stagger(&mut melee, &mut action, &mut mover, bank, from, hit.direction, knockback);
                }
                Reaction::Flinch => {
                    flinch(&mut melee, &mut action, &mut mover, head);
                    push(&mut mover, hit.direction, knockback.min(FLINCH_PUSH), 0.0);
                }
            }
            false
        }
    }
}

/// Take damage and poise damage.
fn hurt(fighter: &mut Fighter, melee: &mut Melee, damage: f32, poise: f32) {
    fighter.hp = (fighter.hp - damage.max(0.0)).max(0.0);
    fighter.poise -= poise.max(0.0);
    melee.hurt = 0.0;
    melee.steady = 0.0;
}

/// Knockback through `Mover.external` (horizontal along `dir`, plus `lift` up).
fn push(mover: &mut Mover, dir: Vec3, speed: f32, lift: f32) {
    let horizontal = flat(dir).normalize_or_zero() * finite(speed, 0.0).max(0.0);
    if horizontal.length_squared() > flat(mover.external).length_squared() {
        mover.external.x = horizontal.x;
        mover.external.z = horizontal.z;
    }
    if lift > 0.0 {
        mover.external.y = lift;
    }
}

fn stagger(melee: &mut Melee, action: &mut Action, mover: &mut Mover, bank: &PoseBank, from: Vec3, dir: Vec3, kb: f32) {
    let mut r = React::new(ReactKind::Stagger, STAGGER, 0.0, STAGGER_RATE, 0.0);
    r.face = yaw_of(from);
    begin_react(&mut r, action, mover, Some(bank));
    melee.state = State::React(r);
    push(mover, dir, kb.min(STAGGER_PUSH), 0.0);
}

/// A short upper-body flinch; it never cuts into a stagger, knockdown or guard.
fn flinch(melee: &mut Melee, action: &mut Action, mover: &mut Mover, head: bool) {
    let busy = match &melee.state {
        State::React(r) => r.kind != ReactKind::Flinch,
        State::Guard { .. } | State::Parry { .. } | State::Dead { .. } => true,
        _ => false,
    };
    if busy {
        return;
    }
    let track = if head { FLINCH_HEAD } else { FLINCH_BODY };
    let mut r = React::new(ReactKind::Flinch, track, 0.0, 1.0, FLINCH_TIME);
    r.mask = Mask::Upper;
    begin_react(&mut r, action, mover, None);
    mover.move_scale = FLINCH_MOVE;
    melee.state = State::React(r);
}

fn kill(
    fighter: &mut Fighter,
    action: &mut Action,
    mover: &mut Mover,
    melee: &mut Melee,
    fx: &mut Feedback,
    at: Vec3,
    involved: bool,
) {
    fighter.hp = 0.0;
    fighter.dead = true;
    let mut body = Body {
        mover,
        action,
        fighter,
        intent: &Intent::default(),
    };
    melee.state = die(&mut body);
    melee.iframes = 0.0;
    if involved {
        fx.stop.trigger(fx.feel.hitstop.max(0.0) * HITSTOP_KILL);
    }
    let line = format!("{:.2} - death - 0.000 {:?} Dead 0.00", fx.now, body.mover.mode);
    fx.monitor.write(&line);
    let _ = at;
}

/// Run the hitstop clock in real time and set the virtual clock's speed from it.
fn apply_hitstop(real: Res<Time<Real>>, mut stop: ResMut<Hitstop>, mut virt: ResMut<Time<Virtual>>) {
    if stop.duration > 0.0 {
        stop.t += real.delta_secs().clamp(0.0, 0.1);
        if stop.t >= stop.duration {
            *stop = Hitstop::default();
        }
    }
    let speed = hitstop_speed(stop.t, stop.duration);
    if (virt.relative_speed() - speed).abs() > 1e-4 {
        virt.set_relative_speed(speed.clamp(HITSTOP_SPEED, 1.0));
    }
}
