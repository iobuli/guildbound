//! Combat rules: move table, action states, swept strikes, guard / parry / counter, momentum knockback,
//! per-actor hit-stop. Rebuilt from the alpha 2 rules (tag unity-final, CombatWorld) and improved with the
//! study notes (Documentation/reference/STUDY_NOTES.md): BT3-style branching rush strings, Overgrowth-style
//! parry recharge, momentum knockback through the motor, counter-hits.
//! Every value is a starting hypothesis until measured in play.

use crate::{STEP, V2, World};

/// Index into MOVES.
pub type MoveId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    /// Hit stun, stays on the feet.
    Stun,
    /// Knocked down, then gets up.
    Knockdown,
}

/// Where on the target a strike lands; drives tracking height and the reaction played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aim {
    Head,
    Body,
    Low,
}

#[derive(Clone, Copy, Debug)]
pub struct MoveDef {
    pub name: &'static str,
    /// Presentation cluster (anim::CLUSTER_SPECS name) whose clips play this move.
    pub cluster: &'static str,
    pub heavy: bool,
    pub startup: f32,
    pub active: f32,
    pub recovery: f32,
    /// Fist/foot tip distance from the body centre at full extension (m).
    pub reach: f32,
    pub damage: f32,
    pub guard_drain: f32,
    /// Knockback added to the target's velocity on a hit (m/s); a block takes a third.
    pub push: f32,
    /// Forward impulse added to the attacker at the start (m/s): the step into the strike.
    pub lunge: f32,
    pub hit_stun: f32,
    pub hit_stop: f32,
    pub reaction: Reaction,
    pub aim: Aim,
}

impl MoveDef {
    pub fn total(&self) -> f32 {
        self.startup + self.active + self.recovery
    }
}

pub const JAB: MoveId = 0;
pub const CROSS: MoveId = 1;
pub const HOOK: MoveId = 2;
pub const UPPERCUT: MoveId = 3;
pub const FRONT_KICK: MoveId = 4;
pub const LOW_KICK: MoveId = 5;
pub const ROUNDHOUSE: MoveId = 6;
pub const RISING: MoveId = 7;
pub const SWEEP: MoveId = 8;
pub const RUN_PUNCH: MoveId = 9;
pub const COUNTER_BLOW: MoveId = 10;
pub const WEAPON_SLASH: MoveId = 11;
pub const WEAPON_CROSS: MoveId = 12;
pub const WEAPON_HEAVY: MoveId = 13;
pub const SIDE_KICK: MoveId = 14;

/// Light string by depth (caps at four), heavy finishers by depth and direction (Decisions 15-17).
pub const LIGHT_STRING: [MoveId; 4] = [JAB, CROSS, HOOK, UPPERCUT];

pub const COMBO_GUIDE: &[(&str, &str, &str)] = &[
    (
        "Four-hit rush",
        "LMB > LMB > LMB > LMB",
        "Jab > cross > hook > uppercut",
    ),
    ("Quick front kick", "F / Mouse4", "Standalone neutral heavy"),
    ("Rush into front kick", "LMB > F", "Jab > front kick"),
    (
        "Rush into low kick",
        "LMB > LMB > F",
        "Jab > cross > low kick",
    ),
    (
        "Rush into roundhouse",
        "LMB > LMB > LMB > F",
        "Jab > cross > hook > roundhouse",
    ),
    (
        "Full rush finisher",
        "LMB > LMB > LMB > LMB > F",
        "Four hits into a roundhouse",
    ),
    (
        "Rising finisher",
        "Forward + F, standalone or after any rush hit",
        "Rising uppercut; knocks down",
    ),
    (
        "Sweep finisher",
        "Backward + F, standalone or after any rush hit",
        "Low sweep; knocks down",
    ),
    (
        "Side kick finisher",
        "Sideways + F, standalone or after any rush hit",
        "Side kick; breaks pressure",
    ),
    (
        "Running opener",
        "Run into LMB",
        "Run punch, then follow with the rush string",
    ),
    (
        "Perfect parry counter",
        "Tap RMB just before contact > LMB",
        "Fast counter blow in the counter window",
    ),
    (
        "Charged smash",
        "Hold F / Mouse4 > release",
        "Charge any directional finisher for more damage",
    ),
    (
        "Weapon rush",
        "Equip sword / axe > LMB four times",
        "Alternating slash and cross into a four-hit chain",
    ),
    (
        "Weapon finisher",
        "Equip sword / axe > F, or F after a rush hit",
        "Heavy weapon strike; hold to charge",
    ),
];

const fn mv(
    name: &'static str,
    cluster: &'static str,
    heavy: bool,
    startup: f32,
    active: f32,
    recovery: f32,
    reach: f32,
    damage: f32,
    guard_drain: f32,
    push: f32,
    lunge: f32,
    hit_stun: f32,
    hit_stop: f32,
    reaction: Reaction,
    aim: Aim,
) -> MoveDef {
    MoveDef {
        name,
        cluster,
        heavy,
        startup,
        active,
        recovery,
        reach,
        damage,
        guard_drain,
        push,
        lunge,
        hit_stun,
        hit_stop,
        reaction,
        aim,
    }
}

pub const MOVES: [MoveDef; 15] = [
    mv(
        "jab",
        "jab",
        false,
        0.10,
        0.05,
        0.18,
        0.80,
        4.0,
        8.0,
        1.5,
        1.0,
        0.28,
        0.05,
        Reaction::Stun,
        Aim::Head,
    ),
    mv(
        "cross",
        "cross",
        false,
        0.11,
        0.05,
        0.20,
        0.85,
        5.0,
        9.0,
        1.8,
        1.2,
        0.30,
        0.06,
        Reaction::Stun,
        Aim::Head,
    ),
    mv(
        "hook",
        "hook",
        false,
        0.13,
        0.06,
        0.22,
        0.80,
        6.0,
        10.0,
        2.2,
        1.0,
        0.32,
        0.07,
        Reaction::Stun,
        Aim::Head,
    ),
    mv(
        "uppercut",
        "uppercut",
        false,
        0.15,
        0.06,
        0.28,
        0.80,
        7.0,
        12.0,
        2.5,
        0.8,
        0.36,
        0.08,
        Reaction::Stun,
        Aim::Body,
    ),
    mv(
        "front kick",
        "front kick",
        true,
        0.22,
        0.07,
        0.36,
        0.95,
        9.0,
        25.0,
        4.5,
        1.6,
        0.45,
        0.10,
        Reaction::Stun,
        Aim::Body,
    ),
    mv(
        "low kick",
        "low kick",
        true,
        0.20,
        0.07,
        0.34,
        0.90,
        8.0,
        22.0,
        3.0,
        1.2,
        0.40,
        0.09,
        Reaction::Stun,
        Aim::Low,
    ),
    mv(
        "roundhouse",
        "roundhouse",
        true,
        0.26,
        0.08,
        0.40,
        1.00,
        12.0,
        35.0,
        6.0,
        1.4,
        0.50,
        0.12,
        Reaction::Knockdown,
        Aim::Head,
    ),
    mv(
        "rising",
        "uppercut",
        true,
        0.24,
        0.08,
        0.42,
        0.90,
        11.0,
        30.0,
        5.0,
        1.0,
        0.50,
        0.11,
        Reaction::Knockdown,
        Aim::Body,
    ),
    mv(
        "sweep",
        "low kick",
        true,
        0.22,
        0.08,
        0.40,
        1.00,
        9.0,
        25.0,
        2.0,
        0.8,
        0.45,
        0.10,
        Reaction::Knockdown,
        Aim::Low,
    ),
    mv(
        "run punch",
        "run punch",
        false,
        0.12,
        0.06,
        0.26,
        0.90,
        7.0,
        14.0,
        4.0,
        0.0,
        0.35,
        0.08,
        Reaction::Stun,
        Aim::Head,
    ),
    mv(
        "counter blow",
        "counter",
        true,
        0.11,
        0.06,
        0.32,
        0.90,
        8.0,
        24.0,
        5.0,
        1.2,
        0.45,
        0.10,
        Reaction::Knockdown,
        Aim::Body,
    ),
    mv(
        "weapon slash",
        "weapon light",
        false,
        0.20,
        0.10,
        0.30,
        0.80,
        6.0,
        12.0,
        2.0,
        0.7,
        0.30,
        0.06,
        Reaction::Stun,
        Aim::Body,
    ),
    mv(
        "weapon cross",
        "weapon light",
        false,
        0.22,
        0.10,
        0.32,
        0.80,
        7.0,
        14.0,
        2.4,
        0.7,
        0.32,
        0.07,
        Reaction::Stun,
        Aim::Body,
    ),
    mv(
        "weapon heavy",
        "weapon heavy",
        true,
        0.40,
        0.12,
        0.48,
        0.85,
        12.0,
        32.0,
        4.0,
        0.8,
        0.45,
        0.10,
        Reaction::Knockdown,
        Aim::Body,
    ),
    mv(
        "side kick",
        "side kick",
        true,
        0.24,
        0.08,
        0.36,
        1.00,
        10.0,
        28.0,
        4.5,
        1.0,
        0.45,
        0.10,
        Reaction::Stun,
        Aim::Body,
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    None,
    Guard,
    Strike(MoveId),
    /// Hold a heavy before committing its selected, directional finisher.
    HeavyCharge(MoveId),
    HitStun,
    BlockStun,
    GuardBreak,
    /// Parried: the attacker is open.
    Staggered,
    Knockdown,
    GetUp,
    /// Ctrl step/dash: an impulse the motor carries, invulnerable inside the evade window.
    Step,
    /// C roll: a longer, lower dodge with a longer evade window.
    Roll,
    /// Posture broken: open to a deathblow.
    Dizzy,
    /// Drinking a consumable; walks slowly, the item works at the commit point.
    Drink(crate::rpg::ItemId),
    /// A spell key held: charging a chargeable spell.
    Charge(crate::magic::SpellId),
    /// Casting a spell: wind-up, release, recovery.
    Cast(crate::magic::SpellId),
}

/// What caused a contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Move(MoveId),
    Spell(crate::magic::SpellId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Startup,
    Active,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Action {
    pub kind: ActionKind,
    pub elapsed: f32,
    /// Seconds for non-strike timed actions (stuns, knockdown, get-up).
    pub length: f32,
    /// Increments on every new action, so presentation can detect restarts.
    pub seq: u32,
}

impl Action {
    pub const IDLE: Action = Action {
        kind: ActionKind::None,
        elapsed: 0.0,
        length: 0.0,
        seq: 0,
    };

    pub fn phase(&self) -> Phase {
        match self.kind {
            ActionKind::Strike(m) => {
                let d = &MOVES[m];
                if self.elapsed < d.startup {
                    Phase::Startup
                } else if self.elapsed < d.startup + d.active {
                    Phase::Active
                } else {
                    Phase::Recovery
                }
            }
            _ => Phase::Recovery,
        }
    }

    /// Free to move, turn and start a new action.
    pub fn free(&self) -> bool {
        matches!(self.kind, ActionKind::None | ActionKind::Guard)
    }
}

/// Direction held at a heavy press, relative to facing (W = up, S = down).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StrikeDir {
    #[default]
    Neutral,
    Up,
    Down,
    Side,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Hit,
    CounterHit,
    Blocked,
    GuardBroken,
    Parried,
    /// Struck while dizzy, or from behind while unaware: down in one.
    Deathblow,
}

/// One resolved contact, for presentation (sparks, sound, camera). Never read back by the rules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub attacker: usize,
    pub defender: usize,
    pub source: Source,
    pub outcome: Outcome,
    pub point: V2,
    pub aim: Aim,
    pub tick: u64,
    /// Hp taken (after armor), and whether it was a critical hit.
    pub damage: f32,
    pub crit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatTuning {
    pub guard_max: f32,
    pub guard_regen_delay: f32,
    pub guard_regen_rate: f32,
    /// Half-angle (deg) of the frontal arc a guard covers.
    pub guard_arc: f32,
    pub guard_walk_speed: f32,
    pub parry_window: f32,
    /// After a guard press, another press cannot open a parry window for this long (no mashing).
    pub parry_recharge: f32,
    pub counter_window: f32,
    pub stagger_time: f32,
    pub block_stun: f32,
    pub guard_break_time: f32,
    pub knockdown_time: f32,
    pub get_up_time: f32,
    /// A press this close to the end of recovery is kept and fires when the action ends.
    pub buffer: f32,
    pub fist_radius: f32,
    pub fist_start: f32,
    /// Degrees an unlocked strike may turn toward the stick during its wind-up.
    pub retarget_max: f32,
    /// A strike with a target tracks it during the wind-up at this rate (deg/s), up to track_max from its start.
    pub track_rate: f32,
    pub track_max: f32,
    /// Range within which an unlocked strike picks the nearest fighter ahead as its target (m), and the cone (deg).
    pub track_range: f32,
    pub track_cone: f32,
    /// Largest step-in a strike takes to close on its target (m).
    pub max_step_in: f32,
    pub counter_hit_mult: f32,
    /// Step/dash: launch speed (the motor's coast decides the distance), duration, evade window, cooldown.
    pub step_speed: f32,
    pub step_time: f32,
    pub evade_from: f32,
    pub evade_to: f32,
    pub step_cooldown: f32,
    pub roll_speed: f32,
    pub roll_time: f32,
    pub gravity: f32,
    pub jump_speed: f32,
    pub pre_jump: f32,
    pub heavy_charge_time: f32,
    pub heavy_damage_gain: f32,
    pub heavy_push_gain: f32,
    pub homing_dash_cost: f32,
    /// Seconds a fighter stays in combat mode after fighting.
    pub combat_linger: f32,
}

impl Default for CombatTuning {
    fn default() -> Self {
        Self {
            guard_max: 100.0,
            guard_regen_delay: 0.8,
            guard_regen_rate: 25.0,
            guard_arc: 70.0,
            guard_walk_speed: 1.2,
            parry_window: 0.13,
            parry_recharge: 0.2,
            counter_window: 0.65,
            stagger_time: 0.5,
            block_stun: 0.15,
            guard_break_time: 1.0,
            knockdown_time: 1.0,
            get_up_time: 0.6,
            buffer: 0.25,
            fist_radius: 0.10,
            fist_start: 0.20,
            retarget_max: 15.0,
            track_rate: 540.0,
            track_max: 60.0,
            track_range: 2.4,
            track_cone: 75.0,
            max_step_in: 0.9,
            counter_hit_mult: 1.5,
            step_speed: 5.5,
            step_time: 0.32,
            evade_from: 0.03,
            evade_to: 0.20,
            roll_speed: 7.0,
            roll_time: 0.55,
            gravity: 22.0,
            jump_speed: 7.0,
            pre_jump: 0.05,
            step_cooldown: 0.35,
            heavy_charge_time: 0.7,
            heavy_damage_gain: 1.0,
            heavy_push_gain: 1.0,
            homing_dash_cost: 8.0,
            combat_linger: 5.0,
        }
    }
}

/// A press waiting for the current action to end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    pub heavy: bool,
    pub dir: StrikeDir,
    pub held: bool,
}

impl World {
    pub(crate) fn new_action(&mut self, i: usize, kind: ActionKind, length: f32) {
        self.seq += 1;
        let f = &mut self.fighters[i];
        f.action = Action {
            kind,
            elapsed: 0.0,
            length,
            seq: self.seq,
        };
        f.hit_mask = 0;
        f.landed = false;
        f.fist_prev = None;
        f.start_facing = f.facing;
        f.buffered = None;
    }

    /// The fighter a strike tracks: the lock target, else the nearest living fighter ahead within range.
    fn strike_target(&self, i: usize) -> Option<usize> {
        let f = &self.fighters[i];
        if let Some(t) = f
            .lock_target
            .filter(|&t| t < self.fighters.len() && self.fighters[t].hp > 0.0)
        {
            return Some(t);
        }
        let cone = self.combat.track_cone.to_radians();
        self.fighters
            .iter()
            .enumerate()
            .filter(|(j, o)| *j != i && o.hp > 0.0)
            .map(|(j, o)| (j, o.position - f.position))
            .filter(|(_, d)| {
                d.length() <= self.combat.track_range
                    && f.facing.signed_angle_to(d.normalized()).abs() <= cone
            })
            .min_by(|a, b| a.1.length().total_cmp(&b.1.length()))
            .map(|(j, _)| j)
    }

    fn start_strike(&mut self, i: usize, mv: MoveId) {
        self.new_action(i, ActionKind::Strike(mv), 0.0);
        let target = self.strike_target(i);
        let mut d = MOVES[mv];
        d.reach += self.fighters[i].kit.weapon().map_or(0.0, |w| w.reach());
        let c = self.combat;
        let (model, half_life, decel) = (
            self.tuning.model,
            self.tuning.coast_half_life,
            self.tuning.decel,
        );
        let tpos = target.map(|t| self.fighters[t].position);
        let f = &mut self.fighters[i];
        f.heavy_charge = 0.0;
        f.strike_target = target;
        f.last_move = Some(mv);
        match tpos {
            Some(p) => {
                // step in exactly far enough that the tip reaches the target (no stretching, no overshoot)
                let to = p - f.position;
                let close = (to.length() - d.reach * 0.85).clamp(0.0, c.max_step_in);
                let speed = match model {
                    crate::MotorModel::Drag => close * std::f32::consts::LN_2 / half_life.max(1e-3),
                    crate::MotorModel::Linear => (2.0 * decel * close).sqrt(),
                };
                let dir = to.normalized();
                // keep sideways momentum from a run, replace the forward part with the measured step
                let along = f.velocity.dot(dir).max(0.0);
                f.velocity = f.velocity - dir * along + dir * along.max(speed);
            }
            None => f.velocity = f.velocity + f.facing * d.lunge,
        }
    }

    /// Choose the move for a press from the current string, the run speed and the held direction.
    fn pick(&self, i: usize, press: Press, chained: bool) -> MoveId {
        let f = &self.fighters[i];
        let depth = if chained { f.chain + 1 } else { 0 } as usize;
        if f.kit.weapon().is_some() {
            return if press.heavy || (!chained && f.counter_left > 0.0) {
                WEAPON_HEAVY
            } else if depth % 2 == 0 {
                WEAPON_SLASH
            } else {
                WEAPON_CROSS
            };
        }
        let split = 0.5 * (self.tuning.walk_speed + self.tuning.run_speed);
        if press.heavy {
            return match press.dir {
                StrikeDir::Up => RISING,
                StrikeDir::Down => SWEEP,
                StrikeDir::Side => SIDE_KICK,
                StrikeDir::Neutral => match depth {
                    0 | 1 => FRONT_KICK,
                    2 => LOW_KICK,
                    _ => ROUNDHOUSE,
                },
            };
        }
        if !chained && f.counter_left > 0.0 {
            return COUNTER_BLOW;
        }
        if !chained && f.velocity.length() > split {
            return RUN_PUNCH;
        }
        LIGHT_STRING[depth.min(LIGHT_STRING.len() - 1)]
    }

    fn press(&mut self, i: usize, press: Press, chained: bool) {
        let mv = self.pick(i, press, chained);
        let chain = if chained {
            self.fighters[i].chain + 1
        } else {
            0
        };
        if press.heavy && press.held {
            let target = self.strike_target(i);
            self.new_action(
                i,
                ActionKind::HeavyCharge(mv),
                self.combat.heavy_charge_time,
            );
            self.fighters[i].strike_target = target;
            self.fighters[i].heavy_charge = 0.0;
        } else {
            self.start_strike(i, mv);
        }
        // heavies finish the string; the light string caps at four
        self.fighters[i].chain = if MOVES[mv].heavy { 3 } else { chain.min(3) };
    }

    /// Timers, intent and the action clock for one fighter. Called before the motor.
    pub(crate) fn think(&mut self, i: usize, intent: &crate::Intent) {
        let c = self.combat;
        self.recover_posture(i);
        {
            let f = &mut self.fighters[i];
            f.combat_timer = (f.combat_timer - STEP).max(0.0);
        }
        if intent.guard || intent.light || intent.heavy {
            self.engage(i);
        }
        {
            let f = &mut self.fighters[i];
            f.parry_left = (f.parry_left - STEP).max(0.0);
            f.parry_recharge = (f.parry_recharge - STEP).max(0.0);
            f.counter_left = (f.counter_left - STEP).max(0.0);
            f.block_stun = (f.block_stun - STEP).max(0.0);
            f.step_cooldown = (f.step_cooldown - STEP).max(0.0);
            if f.action.kind != ActionKind::Guard {
                if f.guard_regen_wait > 0.0 {
                    f.guard_regen_wait = (f.guard_regen_wait - STEP).max(0.0);
                } else if f.action.kind != ActionKind::GuardBreak {
                    f.guard = (f.guard + c.guard_regen_rate * STEP).min(c.guard_max);
                }
            }
            if f.hit_stop > 0.0 {
                // per-actor hit-stop: this fighter's clock and body freeze; everyone else runs on
                f.hit_stop = (f.hit_stop - STEP).max(0.0);
                return;
            }
        }
        if self.fighters[i].hp <= 0.0 {
            return;
        }
        if self.think_magic(i, intent) {
            return;
        }
        if self.fighters[i].block_stun > 0.0 && self.fighters[i].action.kind == ActionKind::Guard {
            // a blocked hit pins the guard for a moment: no strike, no dropping guard
            return;
        }
        // grounded strikes only (Decision 14: no aerial attacks)
        let airborne = self.fighters[i].height > 0.0 || self.fighters[i].crouch_left > 0.0;
        let press = if airborne {
            None
        } else if intent.heavy {
            Some(Press {
                heavy: true,
                dir: self.dir_of(i, intent),
                held: intent.heavy_held,
            })
        } else if intent.light {
            Some(Press {
                heavy: false,
                dir: StrikeDir::Neutral,
                held: false,
            })
        } else {
            None
        };

        // guard press: opens a parry window unless recharging; holding never renews it
        if intent.guard_pressed && self.fighters[i].action.free() {
            let f = &mut self.fighters[i];
            if f.parry_recharge <= 0.0 {
                f.parry_left = c.parry_window;
            }
            f.parry_recharge = c.parry_recharge;
        }

        let act = self.fighters[i].action;
        match act.kind {
            ActionKind::None | ActionKind::Guard => {
                let grounded =
                    self.fighters[i].height <= 0.0 && self.fighters[i].crouch_left <= 0.0;
                if intent.jump && grounded && act.kind == ActionKind::None {
                    self.fighters[i].crouch_left = c.pre_jump;
                } else if intent.roll && grounded && self.fighters[i].step_cooldown <= 0.0 {
                    let f = &self.fighters[i];
                    let dir = if intent.move_dir.length_squared() > 0.04 {
                        intent.move_dir.normalized()
                    } else {
                        f.facing
                    };
                    self.new_action(i, ActionKind::Roll, c.roll_time);
                    let f = &mut self.fighters[i];
                    f.velocity = dir * c.roll_speed;
                    f.facing = dir;
                    f.step_cooldown = c.roll_time + c.step_cooldown;
                } else if intent.step && self.fighters[i].step_cooldown <= 0.0 {
                    // step toward the stick, or back away from the facing with no stick
                    // Shift + Ctrl: homing dash at the lock target (or the nearest fighter ahead), BT3-style
                    let homing = if intent.run && self.fighters[i].mana >= c.homing_dash_cost {
                        self.fighters[i]
                            .lock_target
                            .or_else(|| self.strike_target(i))
                            .filter(|&t| {
                                t != i
                                    && t < self.fighters.len()
                                    && self.fighters[t].hp > 0.0
                                    && (self.fighters[t].position - self.fighters[i].position)
                                        .length()
                                        > 1.2
                            })
                    } else {
                        None
                    };
                    let f = &self.fighters[i];
                    let (dir, speed) = match homing.map(|t| self.fighters[t].position - f.position)
                    {
                        Some(to) if to.length() > 1.2 => (to.normalized(), c.step_speed * 1.7),
                        _ if intent.move_dir.length_squared() > 0.04 => {
                            (intent.move_dir.normalized(), c.step_speed)
                        }
                        _ => (f.facing * -1.0, c.step_speed),
                    };
                    self.new_action(i, ActionKind::Step, c.step_time);
                    let f = &mut self.fighters[i];
                    f.velocity = dir * speed;
                    if homing.is_some() {
                        f.facing = dir;
                        f.mana -= c.homing_dash_cost;
                    }
                    f.step_cooldown = c.step_time + c.step_cooldown;
                } else if let Some(p) = press {
                    // a counter window (after a parry) or a fresh press starts a new string
                    if act.kind == ActionKind::Guard {
                        self.fighters[i].guard_regen_wait = c.guard_regen_delay;
                    }
                    self.press(i, p, false);
                } else if intent.guard && self.fighters[i].guard > 0.0 {
                    if act.kind != ActionKind::Guard {
                        self.new_action(i, ActionKind::Guard, 0.0);
                    }
                } else if act.kind == ActionKind::Guard {
                    self.fighters[i].guard_regen_wait = c.guard_regen_delay;
                    self.new_action(i, ActionKind::None, 0.0);
                }
            }
            ActionKind::HeavyCharge(mv) => {
                let f = &mut self.fighters[i];
                f.action.elapsed += STEP;
                f.heavy_charge = (f.action.elapsed / c.heavy_charge_time.max(STEP)).clamp(0.0, 1.0);
                if !intent.heavy_held || f.heavy_charge >= 1.0 {
                    let charge = f.heavy_charge;
                    self.start_strike(i, mv);
                    self.fighters[i].heavy_charge = charge;
                    // The held anticipation already covered this part of wind-up.
                    self.fighters[i].action.elapsed = MOVES[mv].startup * 0.75 * charge;
                }
            }
            ActionKind::Strike(mv) => {
                let d = MOVES[mv];
                // hit-confirm cancel: a strike that landed may cancel its recovery into the next one
                if let Some(p) = press {
                    let f = &self.fighters[i];
                    let in_recovery = act.phase() == Phase::Recovery;
                    let string_open = p.heavy || f.chain < 3;
                    if in_recovery && f.landed && string_open && !d.heavy {
                        self.press(i, p, true);
                        return;
                    }
                    // Queue during the final anticipation/contact interval as
                    // well as recovery. A queue cannot bypass the hit-confirm gate.
                    if d.total() - act.elapsed <= c.buffer {
                        self.fighters[i].buffered = Some(p);
                    }
                }
                if act.phase() == Phase::Recovery
                    && self.fighters[i].landed
                    && !d.heavy
                    && let Some(p) = self.fighters[i].buffered
                    && (p.heavy || self.fighters[i].chain < 3)
                {
                    self.fighters[i].buffered = None;
                    self.press(i, p, true);
                    return;
                }
                // late recovery: moving cancels the rest of the strike (UE5 move-interrupt window)
                let cancel_from = d.startup + d.active + d.recovery * self.posture.move_cancel;
                if act.elapsed >= cancel_from
                    && press.is_none()
                    && self.fighters[i].buffered.is_none()
                    && intent.move_dir.length_squared() > 0.04
                {
                    self.finish(i, true);
                    return;
                }
                // wind-up tracking: turn toward the target's position (bounded), so the strike flows from
                // wherever the body was into the target
                if act.phase() == Phase::Startup
                    && let Some(t) = self.fighters[i].strike_target
                {
                    let tp = self.fighters[t].position;
                    let f = &mut self.fighters[i];
                    let to = tp - f.position;
                    if to.length_squared() > 1e-4 {
                        let turned =
                            crate::turn_toward(f.facing, to.normalized(), c.track_rate * STEP);
                        let off = f.start_facing.signed_angle_to(turned);
                        let max = c.track_max.to_radians();
                        f.facing = if off.abs() <= max {
                            turned
                        } else {
                            f.start_facing.rotated(max.copysign(off))
                        };
                    }
                } else if act.phase() == Phase::Startup
                    && self.fighters[i].lock_target.is_none()
                    && intent.move_dir.length_squared() > 0.04
                {
                    let f = &mut self.fighters[i];
                    let turned = crate::turn_toward(
                        f.facing,
                        intent.move_dir.normalized(),
                        self.tuning.turn_rate * STEP,
                    );
                    let off = f.start_facing.signed_angle_to(turned);
                    let max = c.retarget_max.to_radians();
                    f.facing = if off.abs() <= max {
                        turned
                    } else {
                        f.start_facing.rotated(max.copysign(off))
                    };
                }
                self.fighters[i].action.elapsed += STEP;
                if self.fighters[i].action.elapsed >= d.total() {
                    self.finish(i, true);
                }
            }
            ActionKind::Step => {
                if let Some(p) = press {
                    self.fighters[i].buffered = Some(p);
                }
                if act.elapsed >= c.evade_to
                    && let Some(p) = self.fighters[i].buffered.take()
                {
                    // A rush-in commits only after the evade window. It cannot
                    // carry the step's invulnerability into an attacking state.
                    self.press(i, p, false);
                    return;
                }
                self.fighters[i].action.elapsed += STEP;
                if self.fighters[i].action.elapsed >= act.length {
                    self.finish(i, false);
                }
            }
            ActionKind::Drink(id) => {
                self.advance_drink(i, id);
                if self.fighters[i].action.elapsed >= act.length {
                    self.finish(i, false);
                }
            }
            _ => {
                self.fighters[i].action.elapsed += STEP;
                if self.fighters[i].action.elapsed >= act.length {
                    if act.kind == ActionKind::Knockdown {
                        self.new_action(i, ActionKind::GetUp, c.get_up_time);
                    } else if act.kind == ActionKind::Dizzy {
                        self.fighters[i].posture = self.posture.max * self.posture.after_dizzy;
                        self.fighters[i].posture_wait = self.posture.wait;
                        self.finish(i, false);
                    } else {
                        self.finish(i, false);
                    }
                }
            }
        }
    }

    fn finish(&mut self, i: usize, was_strike: bool) {
        let buffered = self.fighters[i].buffered.take();
        let landed = self.fighters[i].landed;
        self.new_action(i, ActionKind::None, 0.0);
        if let Some(p) = buffered {
            // a buffered press continues the string only after a strike that landed
            self.press(i, p, was_strike && landed && self.fighters[i].chain < 3);
        } else {
            self.fighters[i].chain = 0;
        }
    }

    fn dir_of(&self, i: usize, intent: &crate::Intent) -> StrikeDir {
        let along = intent.move_dir.dot(self.fighters[i].facing);
        if along > 0.5 {
            StrikeDir::Up
        } else if along < -0.5 {
            StrikeDir::Down
        } else if intent.move_dir.length_squared() > 0.25 {
            StrikeDir::Side
        } else {
            StrikeDir::Neutral
        }
    }

    /// Swept strike contact for one attacker, after the motor. One outcome per target per action.
    pub(crate) fn sweep(&mut self, a: usize) {
        let f = &self.fighters[a];
        let ActionKind::Strike(mv) = f.action.kind else {
            return;
        };
        if f.hit_stop > 0.0 || f.hp <= 0.0 {
            return;
        }
        let mut d = MOVES[mv];
        d.reach += f.kit.weapon().map_or(0.0, |w| w.reach());
        let c = self.combat;
        let e = f.action.elapsed;
        if e < d.startup || e - STEP >= d.startup + d.active {
            return;
        }
        let p = ((e - d.startup) / d.active.max(1e-4)).clamp(0.0, 1.0);
        let tip = f.position + f.facing * (c.fist_start + p * (d.reach - c.fist_start));
        let from = f.fist_prev.unwrap_or(f.position + f.facing * c.fist_start);
        let mut best: Option<(usize, f32)> = None;
        for (j, o) in self.fighters.iter().enumerate() {
            if j == a || o.hp <= 0.0 || f.hit_mask & (1 << j) != 0 {
                continue;
            }
            let evading = match o.action.kind {
                ActionKind::Step => {
                    o.action.elapsed >= c.evade_from && o.action.elapsed <= c.evade_to
                }
                ActionKind::Roll => {
                    o.action.elapsed >= c.evade_from && o.action.elapsed <= c.roll_time * 0.7
                }
                ActionKind::Cast(crate::magic::BLINK) => true,
                _ => false,
            };
            if evading {
                continue;
            }
            let s = segment_param(from, tip, o.position);
            let closest = from + (tip - from) * s;
            if (o.position - closest).length() <= c.fist_radius + self.tuning.body_radius
                && best.is_none_or(|(_, bs)| s < bs)
            {
                best = Some((j, s));
            }
        }
        self.fighters[a].fist_prev = Some(tip);
        if let Some((j, s)) = best {
            self.fighters[a].hit_mask |= 1 << j;
            self.resolve(a, j, mv, from + (tip - from) * s);
        }
    }

    fn frontal(&self, d: usize, source: V2) -> bool {
        let f = &self.fighters[d];
        let to = source - f.position;
        if to.length_squared() < 1e-6 {
            return true;
        }
        f.facing.signed_angle_to(to.normalized()).abs() <= self.combat.guard_arc.to_radians()
    }

    fn resolve(&mut self, a: usize, d: usize, mv: MoveId, point: V2) {
        let c = self.combat;
        let def = MOVES[mv];
        let charge = self.fighters[a].heavy_charge;
        let push = def.push * (1.0 + c.heavy_push_gain * charge);
        let dir = (self.fighters[d].position - self.fighters[a].position).normalized();
        let dir = if dir.length_squared() > 0.0 {
            dir
        } else {
            self.fighters[a].facing
        };
        let frontal = self.frontal(d, self.fighters[a].position);
        let tgt = self.fighters[d].clone();

        let mut damage = 0.0;
        let mut crit = false;
        let outcome = if self.deathblow(a, d) {
            // posture broken, or caught unaware from behind: down in one
            let t = &mut self.fighters[d];
            damage = t.hp;
            t.hp = 0.0;
            t.posture = 0.0;
            t.velocity = t.velocity + dir * push;
            self.new_action(d, ActionKind::Knockdown, c.knockdown_time * 3.0);
            self.fighters[a].landed = true;
            Outcome::Deathblow
        } else if tgt.parry_left > 0.0 && frontal && tgt.action.free() {
            // parry: the attacker is thrown off balance and loses posture, the defender gets a counter window
            self.new_action(a, ActionKind::Staggered, c.stagger_time);
            let t = &mut self.fighters[d];
            t.parry_left = 0.0;
            t.counter_left = c.counter_window;
            let p = self.posture;
            self.add_posture(a, p.parried + def.guard_drain, false, true);
            self.add_posture(d, p.parry_cost, true, false);
            // a perfect parry answers by itself: the parrier steps into a heavy counter blow that knocks the
            // attacker down (presentation ragdolls the knockdown)
            let to = self.fighters[a].position - self.fighters[d].position;
            if to.length_squared() > 1e-6 {
                self.fighters[d].facing = to.normalized();
            }
            self.start_strike(d, COUNTER_BLOW);
            self.fighters[d].strike_target = Some(a);
            Outcome::Parried
        } else if tgt.action.kind == ActionKind::Guard && tgt.guard > 0.0 && frontal {
            let t = &mut self.fighters[d];
            t.guard = if charge >= 1.0 {
                0.0
            } else {
                t.guard - def.guard_drain * (1.0 + 3.0 * charge)
            };
            t.velocity = t.velocity + dir * (push / 3.0);
            let o = if t.guard <= 0.0 {
                t.guard = 0.0;
                t.guard_regen_wait = c.guard_regen_delay;
                self.new_action(d, ActionKind::GuardBreak, c.guard_break_time);
                Outcome::GuardBroken
            } else {
                t.guard_regen_wait = c.guard_regen_delay;
                Outcome::Blocked
            };
            self.add_posture(
                d,
                def.guard_drain * self.posture.block_share * (1.0 + charge),
                false,
                false,
            );
            o
        } else {
            // caught in the wind-up of their own strike, or during a counter window: counter-hit
            let counter = (matches!(tgt.action.kind, ActionKind::Strike(_))
                && tgt.action.phase() == Phase::Startup)
                || matches!(tgt.action.kind, ActionKind::HeavyCharge(_))
                || self.fighters[a].counter_left > 0.0;
            crit = self.crit(a, d);
            let raw = def.damage
                * (1.0 + c.heavy_damage_gain * charge)
                * if counter { c.counter_hit_mult } else { 1.0 }
                * if crit { self.posture.crit_mult } else { 1.0 };
            damage = crate::rpg::mitigate(raw, self.fighters[a].kit.stats(), tgt.kit.stats());
            let t = &mut self.fighters[d];
            t.hp = (t.hp - damage).max(0.0);
            t.velocity = t.velocity + dir * push;
            if t.hp <= 0.0 {
                self.new_action(d, ActionKind::Knockdown, c.knockdown_time * 3.0);
            } else if def.reaction == Reaction::Knockdown || charge >= 1.0 {
                self.new_action(d, ActionKind::Knockdown, c.knockdown_time);
            } else {
                self.new_action(d, ActionKind::HitStun, def.hit_stun);
            }
            self.fighters[a].landed = true;
            self.fighters[a].counter_left = 0.0;
            self.add_posture(
                d,
                def.guard_drain * self.posture.hit_share * (1.0 + charge),
                false,
                false,
            );
            if counter {
                Outcome::CounterHit
            } else {
                Outcome::Hit
            }
        };
        self.alarm(d, a);
        self.engage(a);
        self.engage(d);
        let stop = match outcome {
            Outcome::Hit | Outcome::CounterHit | Outcome::GuardBroken => def.hit_stop,
            Outcome::Blocked => def.hit_stop * 0.6,
            Outcome::Parried => def.hit_stop * 1.5,
            Outcome::Deathblow => def.hit_stop * 2.5,
        };
        self.fighters[a].hit_stop = stop;
        self.fighters[d].hit_stop = stop;
        if outcome == Outcome::Blocked && self.fighters[d].action.kind == ActionKind::Guard {
            self.fighters[d].block_stun = c.block_stun;
        }
        self.contacts.push(Contact {
            attacker: a,
            defender: d,
            source: Source::Move(mv),
            outcome,
            point,
            aim: def.aim,
            tick: self.tick,
            damage,
            crit,
        });
    }
}

/// Parameter of the closest point to p on segment a-b, in 0..1.
fn segment_param(a: V2, b: V2, p: V2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 < 1e-9 {
        return 0.0;
    }
    ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
}

/// Defensive read shared by the sparring partner and melee mobs: what to do about `t` this tick.
/// Deterministic (beats come from the tick), so replays hold.
pub fn defend(world: &World, me: usize, t: usize, skill: f32) -> Option<crate::Intent> {
    let (f, o) = (&world.fighters[me], &world.fighters[t]);
    let dist = (o.position - f.position).length();
    let mut intent = crate::Intent::default();
    let beat = |salt: u64| {
        ((world.tick / 6)
            .wrapping_mul(2654435761)
            .wrapping_add(salt * 97 + me as u64 * 31)
            % 100) as f32
            / 100.0
    };
    if let ActionKind::Strike(m) = o.action.kind {
        let d = MOVES[m];
        let incoming =
            o.action.phase() == Phase::Startup && dist < d.reach + d.lunge.max(0.6) + 0.4;
        if incoming && f.action.free() {
            let left = d.startup - o.action.elapsed;
            // a late tap is a parry; otherwise hold guard; low skill sometimes just eats it
            if left < 0.09 && beat(1) < skill * 0.5 {
                intent.guard = true;
                intent.guard_pressed = true;
            } else if beat(2) < skill {
                intent.guard = true;
                intent.guard_pressed = f.action.kind != ActionKind::Guard;
            } else if beat(3) < skill * 0.3 {
                intent.step = true;
                intent.move_dir = (f.position - o.position).normalized().rotated(1.2);
            }
            return Some(intent);
        }
    }
    // guard nearly broken: get out
    if f.guard < 30.0 && dist < 2.0 && f.action.free() {
        intent.move_dir = (f.position - o.position).normalized();
        intent.step = beat(4) < 0.2;
        return Some(intent);
    }
    // just blocked something: punish
    if world
        .contacts
        .iter()
        .any(|c| c.defender == me && c.outcome == Outcome::Blocked)
        && beat(5) < skill
    {
        intent.light = true;
        return Some(intent);
    }
    // a parry opened a counter window: take it
    if f.counter_left > 0.0 {
        intent.heavy = beat(6) < 0.4;
        intent.light = !intent.heavy;
        return Some(intent);
    }
    // keep guard up a moment while the attacker is still swinging
    if matches!(o.action.kind, ActionKind::Strike(_))
        && f.action.kind == ActionKind::Guard
        && dist < 1.8
    {
        intent.guard = true;
        return Some(intent);
    }
    None
}

/// Sparring partner: approaches to range, throws strings and the odd heavy, defends with `defend`.
pub fn spar_intent(world: &World, me: usize, target: usize) -> crate::Intent {
    let (f, t) = (&world.fighters[me], &world.fighters[target]);
    let to = t.position - f.position;
    let dist = to.length();
    let mut intent = crate::Intent::default();
    if f.hp <= 0.0 || t.hp <= 0.0 {
        return intent;
    }
    if matches!(f.action.kind, ActionKind::HeavyCharge(_)) {
        intent.heavy_held = f.action.elapsed < world.combat.heavy_charge_time * 0.75;
        return intent;
    }
    if let Some(d) = defend(world, me, target, 0.7) {
        return d;
    }
    let beat = (world.tick / 30) % 8;
    if dist > 1.05 {
        intent.move_dir = to.normalized();
        intent.run = dist > 4.0;
    } else if f.action.free() || matches!(f.action.kind, ActionKind::Strike(_)) {
        let press_tick = world.tick % 18 == 0;
        if beat == 7 {
            intent.heavy = press_tick;
            intent.heavy_held = press_tick;
        } else if beat >= 2 {
            intent.light = press_tick;
        } else {
            // circle while resetting
            intent.move_dir = to.normalized().rotated(std::f32::consts::FRAC_PI_2) * 0.6;
        }
    }
    intent
}
