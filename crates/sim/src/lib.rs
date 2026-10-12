//! Deterministic combat core, motor slice. Ported from tag `unity-final`, `Assets/Game/Combat`
//! (CombatWorld.Move / Separate / TurnToward, BoxArena.Constrain, Tuning).
//! Positions are on the ground plane as `V2 { x, z }`. The motor is the only mover (MASTER L1).
//! Nothing here reads a clock, a random source or a hash-map order: same inputs, same state.

pub mod combat;
pub mod magic;
pub mod mobs;
mod math;
pub use combat::{Action, ActionKind, CombatTuning, Contact, MOVES, Outcome, Phase, Source};
pub use magic::{MagicTuning, Projectile, SPELLS};
pub use math::V2;

/// Fixed simulation step in seconds.
pub const STEP: f32 = 1.0 / 120.0;

/// How the motor turns input into velocity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotorModel {
    /// Velocity chases the target at bounded rates (accel / decel / reverse_brake).
    Linear,
    /// Push plus speed-scaled drag (reimagined from Overgrowth's ground movement): input pushes the body at
    /// `drag_accel`, drag bleeds momentum so terminal speed is exactly the target; coasting decays by
    /// `coast_half_life`. Fast starts that ease into top speed, momentum that carries, natural plant-and-turn.
    Drag,
}

/// Starting hypotheses from GDD v5.1 section 4 until measured (same values as the Unity `Tuning`).
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning {
    pub model: MotorModel,
    /// Drag model: push (m/s^2) toward the input.
    pub drag_accel: f32,
    /// Drag model: seconds for speed to halve with no input.
    pub coast_half_life: f32,
    pub body_radius: f32,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub arena_half_size: f32,
    /// Degrees per second a free fighter turns toward its movement.
    pub turn_rate: f32,
    /// m/s^2 gaining speed toward the input.
    pub accel: f32,
    /// m/s^2 losing speed with no input or a slower target (the momentum you carry into a stop).
    pub decel: f32,
    /// m/s^2 when the input opposes the current velocity (plant and turn).
    pub reverse_brake: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            model: MotorModel::Drag,
            drag_accel: 30.0,
            coast_half_life: 0.15,
            body_radius: 0.30,
            walk_speed: 2.1,
            run_speed: 4.0,
            arena_half_size: 10.0,
            turn_rate: 720.0,
            accel: 12.0,
            decel: 9.0,
            reverse_brake: 20.0,
        }
    }
}

/// One tick of player or AI intent. `move_dir` is world XZ with magnitude <= 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intent {
    pub move_dir: V2,
    pub run: bool,
    /// Press edges (true for exactly one tick).
    pub light: bool,
    pub heavy: bool,
    /// Held heavy (Mouse4/F): wind up a smash; releasing commits it.
    pub heavy_held: bool,
    pub guard_pressed: bool,
    pub step: bool,
    pub jump: bool,
    pub roll: bool,
    /// Spell slot pressed this tick (0-3 for keys 1-4), and whether that slot is still held.
    pub cast: Option<u8>,
    pub cast_held: bool,
    /// Q held: charge ki.
    pub ki_charge: bool,
    /// Held.
    pub guard: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fighter {
    pub position: V2,
    pub velocity: V2,
    /// Velocity change over the last tick (m/s^2); presentation reads it for lean and weight shift.
    pub accel: V2,
    /// Height above the ground (m) and vertical speed (m/s). 0 = grounded.
    pub height: f32,
    pub vertical: f32,
    /// Pre-jump crouch left (s); the take-off fires when it reaches 0.
    pub crouch_left: f32,
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// Index of the fighter this one faces while free (strafe), if any.
    pub lock_target: Option<usize>,
    pub action: Action,
    pub guard: f32,
    /// Light-string depth of the current string (0 = first hit).
    pub chain: u8,
    pub last_move: Option<combat::MoveId>,
    /// The fighter the current strike tracks, if any.
    pub strike_target: Option<usize>,
    /// Seconds left: hit-stop freeze, parry window, counter window, block stun.
    pub hit_stop: f32,
    pub parry_left: f32,
    pub counter_left: f32,
    pub block_stun: f32,
    pub ki: f32,
    /// 0..1 charge of the spell being held or just released.
    pub charge: f32,
    pub spell_cooldown: f32,
    /// 0..1 strength prepared by a held smash; presentation never determines it.
    pub heavy_charge: f32,
    pub(crate) parry_recharge: f32,
    pub(crate) step_cooldown: f32,
    pub(crate) guard_regen_wait: f32,
    pub(crate) landed: bool,
    pub(crate) hit_mask: u64,
    pub(crate) fist_prev: Option<V2>,
    pub(crate) start_facing: V2,
    pub(crate) buffered: Option<combat::Press>,
    move_from: V2,
}

impl Fighter {
    pub fn new(position: V2, facing: V2, hp: f32) -> Self {
        let facing = facing.normalized();
        Self {
            position,
            velocity: V2::ZERO,
            accel: V2::ZERO,
            height: 0.0,
            vertical: 0.0,
            crouch_left: 0.0,
            facing,
            hp,
            max_hp: hp,
            lock_target: None,
            action: Action::IDLE,
            guard: CombatTuning::default().guard_max,
            chain: 0,
            last_move: None,
            strike_target: None,
            hit_stop: 0.0,
            parry_left: 0.0,
            counter_left: 0.0,
            block_stun: 0.0,
            ki: 60.0,
            charge: 0.0,
            spell_cooldown: 0.0,
            heavy_charge: 0.0,
            parry_recharge: 0.0,
            step_cooldown: 0.0,
            guard_regen_wait: 0.0,
            landed: false,
            hit_mask: 0,
            fist_prev: None,
            start_facing: facing,
            buffered: None,
            move_from: position,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct World {
    pub tuning: Tuning,
    pub combat: CombatTuning,
    pub magic: MagicTuning,
    pub fighters: Vec<Fighter>,
    pub projectiles: Vec<Projectile>,
    pub tick: u64,
    /// Contacts resolved since the presentation last drained them.
    pub contacts: Vec<Contact>,
    pub(crate) seq: u32,
}

impl World {
    /// The sparring floor: two fighters 3 m apart, facing each other.
    pub fn sparring(tuning: Tuning) -> Self {
        let fighters = vec![
            Fighter::new(V2::new(0.0, -1.5), V2::new(0.0, 1.0), 100.0),
            Fighter::new(V2::new(0.0, 1.5), V2::new(0.0, -1.0), 100.0),
        ];
        Self { tuning, combat: CombatTuning::default(), magic: MagicTuning::default(), fighters, projectiles: Vec::new(), tick: 0, contacts: Vec::new(), seq: 0 }
    }

    /// Advance one fixed step. Missing intents count as idle.
    pub fn step(&mut self, intents: &[Intent]) {
        self.tick += 1;
        for f in &mut self.fighters {
            f.move_from = f.position;
        }
        for i in 0..self.fighters.len() {
            let intent = intents.get(i).copied().unwrap_or_default();
            self.think(i, &intent);
        }
        for i in 0..self.fighters.len() {
            let intent = intents.get(i).copied().unwrap_or_default();
            self.move_fighter(i, intent);
        }
        self.separate();
        for i in 0..self.fighters.len() {
            self.sweep(i);
        }
        self.advance_projectiles();
    }

    /// Hand the contacts resolved since the last call to the caller.
    pub fn drain_contacts(&mut self) -> Vec<Contact> {
        std::mem::take(&mut self.contacts)
    }

    fn move_fighter(&mut self, i: usize, intent: Intent) {
        let t = &self.tuning;
        let f = &self.fighters[i];
        if f.hit_stop > 0.0 {
            // frozen in hit-stop: keeps its velocity for when the clock resumes
            self.fighters[i].accel = V2::ZERO;
            return;
        }
        let action = f.action;
        // strikes, stuns and knockdowns coast on momentum (run carry, knockback); guard walks slowly
        let steering = action.free() && f.hp > 0.0;
        let raw = if steering { intent.move_dir } else { V2::ZERO };
        let m = if raw.length_squared() > 1.0 { raw.normalized() } else { raw };
        let speed = if action.kind == ActionKind::Guard {
            self.combat.guard_walk_speed
        } else if intent.run {
            t.run_speed
        } else {
            t.walk_speed
        };
        let target = m * speed;
        let lock = self.fighters[i].lock_target.filter(|&j| j != i && j < self.fighters.len()).map(|j| self.fighters[j].position);
        let max_turn = t.turn_rate * STEP;
        let (accel, decel, reverse) = (t.accel, t.decel, t.reverse_brake);
        let (model, drag_accel, coast_half_life) = (t.model, t.drag_accel, t.coast_half_life);
        let f = &mut self.fighters[i];

        let v = f.velocity;
        let next = match model {
            // momentum: velocity chases the input target at a bounded rate; which rate depends on what the body is doing
            MotorModel::Linear => {
                let rate = if target.length_squared() < 1e-8 {
                    decel
                } else if v.dot(target) < 0.0 {
                    reverse
                } else if target.length() >= v.length() {
                    accel
                } else {
                    decel
                };
                let mut dv = target - v;
                let max = rate * STEP;
                if dv.length() > max {
                    dv = dv.normalized() * max;
                }
                v + dv
            }
            // push, then drag chosen so the steady state of v' = (v + a dt m) * d is exactly speed * m
            MotorModel::Drag => {
                if m.length_squared() > 1e-8 {
                    let push = drag_accel * STEP;
                    (v + m * push) * (speed / (speed + push))
                } else {
                    v * 0.5f32.powf(STEP / coast_half_life.max(1e-3))
                }
            }
        };
        // a coasting body comes to rest instead of creeping forever
        f.velocity = if m.length_squared() <= 1e-8 && next.length_squared() < 1e-6 { V2::ZERO } else { next };
        f.accel = (f.velocity - v) / STEP;
        f.position = f.position + f.velocity * STEP;
        // vertical: pre-jump crouch, then a ballistic arc; air keeps its momentum
        let (g, jump_speed) = (self.combat.gravity, self.combat.jump_speed);
        if f.crouch_left > 0.0 {
            f.crouch_left -= STEP;
            if f.crouch_left <= 0.0 {
                f.crouch_left = 0.0;
                // a running take-off goes a little higher (Overgrowth: jump scales with speed)
                f.vertical = jump_speed * (1.0 + 0.15 * (f.velocity.length() / 4.0).min(1.0));
            }
        }
        if f.height > 0.0 || f.vertical > 0.0 {
            f.vertical -= g * STEP;
            f.height += f.vertical * STEP;
            if f.height <= 0.0 {
                f.height = 0.0;
                f.vertical = 0.0;
            }
        }

        if !steering {
            // committed actions keep their facing (strike retarget is bounded in combat::think)
        } else if let Some(target) = lock {
            let to = target - f.position;
            if to.length_squared() > 1e-6 {
                f.facing = turn_toward(f.facing, to.normalized(), max_turn * 0.75);
            }
        } else if m.length_squared() > 1e-4 {
            // face where the player is steering, so a reversal visibly plants and turns
            f.facing = turn_toward(f.facing, m.normalized(), max_turn);
        } else if f.velocity.length_squared() > 1e-2 {
            f.facing = turn_toward(f.facing, f.velocity.normalized(), max_turn);
        }
    }

    fn separate(&mut self) {
        let min = 2.0 * self.tuning.body_radius;
        let n = self.fighters.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let d = self.fighters[j].position - self.fighters[i].position;
                let len = d.length();
                if len >= min {
                    continue;
                }
                let dir = if len > 1e-5 { d / len } else { V2::new(1.0, 0.0) };
                let push = dir * ((min - len) * 0.5);
                self.fighters[i].position = self.fighters[i].position - push;
                self.fighters[j].position = self.fighters[j].position + push;
            }
        }
        // one world constraint per tick, from the pre-motion position, so separation never pushes through a wall
        let half = self.tuning.arena_half_size - self.tuning.body_radius;
        for f in &mut self.fighters {
            let clamped = V2::new(f.position.x.clamp(-half, half), f.position.z.clamp(-half, half));
            // a wall absorbs the momentum into it
            if clamped.x != f.position.x {
                f.velocity.x = 0.0;
            }
            if clamped.z != f.position.z {
                f.velocity.z = 0.0;
            }
            f.position = clamped;
        }
    }

    /// FNV-1a over every state field in index order. Equal hashes for equal input scripts prove determinism.
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bits: u64| {
            for b in bits.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        eat(self.tick);
        for f in &self.fighters {
            for v in [f.position.x, f.position.z, f.velocity.x, f.velocity.z, f.accel.x, f.accel.z, f.facing.x, f.facing.z, f.hp, f.max_hp] {
                eat(v.to_bits() as u64);
            }
            eat(f.lock_target.map_or(u64::MAX, |j| j as u64));
            for v in [f.guard, f.hit_stop, f.parry_left, f.counter_left, f.action.elapsed, f.action.length, f.ki, f.charge, f.heavy_charge, f.height, f.vertical, f.crouch_left] {
                eat(v.to_bits() as u64);
            }
            let kind = match f.action.kind {
                ActionKind::None => 0,
                ActionKind::Guard => 1,
                ActionKind::Strike(m) => 0x1000 + m as u64,
                ActionKind::HeavyCharge(m) => 0x2000 + m as u64,
                ActionKind::HitStun => 2, ActionKind::BlockStun => 3, ActionKind::GuardBreak => 4,
                ActionKind::Staggered => 5, ActionKind::Knockdown => 6, ActionKind::GetUp => 7,
                ActionKind::Step => 8, ActionKind::Roll => 9, ActionKind::KiCharge => 10,
                ActionKind::Charge(s) => 0x3000 + s as u64, ActionKind::Cast(s) => 0x4000 + s as u64,
            };
            eat(kind);
            eat(f.buffered.map_or(u64::MAX, |p| u64::from(p.heavy) | ((p.dir as u64) << 1) | (u64::from(p.held) << 3)));
            eat(f.action.seq as u64);
            eat(f.chain as u64);
        }
        h
    }
}

/// Planar turn toward `to`, at most `max_degrees`. An exact reversal still turns, never flips.
pub fn turn_toward(from: V2, to: V2, max_degrees: f32) -> V2 {
    let angle = from.signed_angle_to(to);
    let max = max_degrees.to_radians();
    if angle.abs() <= max { to } else { from.rotated(max.copysign(angle)) }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_combat;
