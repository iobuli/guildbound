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

/// Light string by depth (caps at four), heavy finishers by depth and direction (Decisions 15-17).
pub const LIGHT_STRING: [MoveId; 4] = [JAB, CROSS, HOOK, UPPERCUT];

const fn mv(
    name: &'static str, cluster: &'static str, heavy: bool, startup: f32, active: f32, recovery: f32, reach: f32,
    damage: f32, guard_drain: f32, push: f32, lunge: f32, hit_stun: f32, hit_stop: f32, reaction: Reaction,
) -> MoveDef {
    MoveDef { name, cluster, heavy, startup, active, recovery, reach, damage, guard_drain, push, lunge, hit_stun, hit_stop, reaction }
}

pub const MOVES: [MoveDef; 10] = [
    mv("jab", "jab", false, 0.10, 0.05, 0.18, 0.80, 4.0, 8.0, 1.5, 1.0, 0.28, 0.05, Reaction::Stun),
    mv("cross", "cross", false, 0.11, 0.05, 0.20, 0.85, 5.0, 9.0, 1.8, 1.2, 0.30, 0.06, Reaction::Stun),
    mv("hook", "hook", false, 0.13, 0.06, 0.22, 0.80, 6.0, 10.0, 2.2, 1.0, 0.32, 0.07, Reaction::Stun),
    mv("uppercut", "uppercut", false, 0.15, 0.06, 0.28, 0.80, 7.0, 12.0, 2.5, 0.8, 0.36, 0.08, Reaction::Stun),
    mv("front kick", "side kick", true, 0.22, 0.07, 0.36, 0.95, 9.0, 25.0, 4.5, 1.6, 0.45, 0.10, Reaction::Stun),
    mv("low kick", "roundhouse", true, 0.20, 0.07, 0.34, 0.90, 8.0, 22.0, 3.0, 1.2, 0.40, 0.09, Reaction::Stun),
    mv("roundhouse", "roundhouse", true, 0.26, 0.08, 0.40, 1.00, 12.0, 35.0, 6.0, 1.4, 0.50, 0.12, Reaction::Knockdown),
    mv("rising", "uppercut", true, 0.24, 0.08, 0.42, 0.90, 11.0, 30.0, 5.0, 1.0, 0.50, 0.11, Reaction::Knockdown),
    mv("sweep", "roundhouse", true, 0.22, 0.08, 0.40, 1.00, 9.0, 25.0, 2.0, 0.8, 0.45, 0.10, Reaction::Knockdown),
    mv("run punch", "run punch", false, 0.12, 0.06, 0.26, 0.90, 7.0, 14.0, 4.0, 0.0, 0.35, 0.08, Reaction::Stun),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    None,
    Guard,
    Strike(MoveId),
    HitStun,
    BlockStun,
    GuardBreak,
    /// Parried: the attacker is open.
    Staggered,
    Knockdown,
    GetUp,
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
    pub const IDLE: Action = Action { kind: ActionKind::None, elapsed: 0.0, length: 0.0, seq: 0 };

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Hit,
    CounterHit,
    Blocked,
    GuardBroken,
    Parried,
}

/// One resolved contact, for presentation (sparks, sound, camera). Never read back by the rules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub attacker: usize,
    pub defender: usize,
    pub mv: MoveId,
    pub outcome: Outcome,
    pub point: V2,
    pub tick: u64,
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
    pub counter_hit_mult: f32,
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
            counter_hit_mult: 1.5,
        }
    }
}

/// A press waiting for the current action to end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    pub heavy: bool,
    pub dir: StrikeDir,
}

impl World {
    fn new_action(&mut self, i: usize, kind: ActionKind, length: f32) {
        self.seq += 1;
        let f = &mut self.fighters[i];
        f.action = Action { kind, elapsed: 0.0, length, seq: self.seq };
        f.hit_mask = 0;
        f.landed = false;
        f.fist_prev = None;
        f.start_facing = f.facing;
    }

    fn start_strike(&mut self, i: usize, mv: MoveId) {
        self.new_action(i, ActionKind::Strike(mv), 0.0);
        let f = &mut self.fighters[i];
        f.velocity = f.velocity + f.facing * MOVES[mv].lunge;
        f.last_move = Some(mv);
    }

    /// Choose the move for a press from the current string, the run speed and the held direction.
    fn pick(&self, i: usize, press: Press, chained: bool) -> MoveId {
        let f = &self.fighters[i];
        let depth = if chained { f.chain + 1 } else { 0 } as usize;
        let split = 0.5 * (self.tuning.walk_speed + self.tuning.run_speed);
        if press.heavy {
            return match press.dir {
                StrikeDir::Up => RISING,
                StrikeDir::Down => SWEEP,
                StrikeDir::Neutral => match depth {
                    0 | 1 => FRONT_KICK,
                    2 => LOW_KICK,
                    _ => ROUNDHOUSE,
                },
            };
        }
        if !chained && f.velocity.length() > split {
            return RUN_PUNCH;
        }
        LIGHT_STRING[depth.min(LIGHT_STRING.len() - 1)]
    }

    fn press(&mut self, i: usize, press: Press, chained: bool) {
        let mv = self.pick(i, press, chained);
        let chain = if chained { self.fighters[i].chain + 1 } else { 0 };
        self.start_strike(i, mv);
        // heavies finish the string; the light string caps at four
        self.fighters[i].chain = if MOVES[mv].heavy { 3 } else { chain.min(3) };
    }

    /// Timers, intent and the action clock for one fighter. Called before the motor.
    pub(crate) fn think(&mut self, i: usize, intent: &crate::Intent) {
        let c = self.combat;
        {
            let f = &mut self.fighters[i];
            f.parry_left = (f.parry_left - STEP).max(0.0);
            f.parry_recharge = (f.parry_recharge - STEP).max(0.0);
            f.counter_left = (f.counter_left - STEP).max(0.0);
            f.block_stun = (f.block_stun - STEP).max(0.0);
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
        if self.fighters[i].block_stun > 0.0 && self.fighters[i].action.kind == ActionKind::Guard {
            // a blocked hit pins the guard for a moment: no strike, no dropping guard
            return;
        }
        let press = if intent.heavy {
            Some(Press { heavy: true, dir: self.dir_of(i, intent) })
        } else if intent.light {
            Some(Press { heavy: false, dir: StrikeDir::Neutral })
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
                if let Some(p) = press {
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
                    if in_recovery && d.total() - act.elapsed <= c.buffer {
                        self.fighters[i].buffered = Some(p);
                    }
                }
                // wind-up retarget toward the stick, bounded, unlocked only
                if act.phase() == Phase::Startup && self.fighters[i].lock_target.is_none() && intent.move_dir.length_squared() > 0.04 {
                    let f = &mut self.fighters[i];
                    let turned = crate::turn_toward(f.facing, intent.move_dir.normalized(), self.tuning.turn_rate * STEP);
                    let off = f.start_facing.signed_angle_to(turned);
                    let max = c.retarget_max.to_radians();
                    f.facing = if off.abs() <= max { turned } else { f.start_facing.rotated(max.copysign(off)) };
                }
                self.fighters[i].action.elapsed += STEP;
                if self.fighters[i].action.elapsed >= d.total() {
                    self.finish(i, true);
                }
            }
            _ => {
                self.fighters[i].action.elapsed += STEP;
                if self.fighters[i].action.elapsed >= act.length {
                    if act.kind == ActionKind::Knockdown {
                        self.new_action(i, ActionKind::GetUp, c.get_up_time);
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
        } else {
            StrikeDir::Neutral
        }
    }

    /// Swept strike contact for one attacker, after the motor. One outcome per target per action.
    pub(crate) fn sweep(&mut self, a: usize) {
        let f = &self.fighters[a];
        let ActionKind::Strike(mv) = f.action.kind else { return };
        if f.hit_stop > 0.0 || f.hp <= 0.0 {
            return;
        }
        let d = MOVES[mv];
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
            let s = segment_param(from, tip, o.position);
            let closest = from + (tip - from) * s;
            if (o.position - closest).length() <= c.fist_radius + self.tuning.body_radius && best.is_none_or(|(_, bs)| s < bs) {
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
        let dir = (self.fighters[d].position - self.fighters[a].position).normalized();
        let dir = if dir.length_squared() > 0.0 { dir } else { self.fighters[a].facing };
        let frontal = self.frontal(d, self.fighters[a].position);
        let tgt = self.fighters[d].clone();

        let outcome = if tgt.parry_left > 0.0 && frontal && tgt.action.free() {
            // parry: the attacker is thrown off balance, the defender gets a counter window
            self.new_action(a, ActionKind::Staggered, c.stagger_time);
            let t = &mut self.fighters[d];
            t.parry_left = 0.0;
            t.counter_left = c.counter_window;
            Outcome::Parried
        } else if tgt.action.kind == ActionKind::Guard && tgt.guard > 0.0 && frontal {
            let t = &mut self.fighters[d];
            t.guard -= def.guard_drain;
            t.velocity = t.velocity + dir * (def.push / 3.0);
            if t.guard <= 0.0 {
                t.guard = 0.0;
                t.guard_regen_wait = c.guard_regen_delay;
                self.new_action(d, ActionKind::GuardBreak, c.guard_break_time);
                Outcome::GuardBroken
            } else {
                t.guard_regen_wait = c.guard_regen_delay;
                Outcome::Blocked
            }
        } else {
            // caught in the wind-up of their own strike, or during a counter window: counter-hit
            let counter = matches!(tgt.action.kind, ActionKind::Strike(_)) && tgt.action.phase() == Phase::Startup
                || self.fighters[a].counter_left > 0.0;
            let damage = def.damage * if counter { c.counter_hit_mult } else { 1.0 };
            let t = &mut self.fighters[d];
            t.hp = (t.hp - damage).max(0.0);
            t.velocity = t.velocity + dir * def.push;
            if t.hp <= 0.0 {
                self.new_action(d, ActionKind::Knockdown, c.knockdown_time * 3.0);
            } else if def.reaction == Reaction::Knockdown {
                self.new_action(d, ActionKind::Knockdown, c.knockdown_time);
            } else {
                self.new_action(d, ActionKind::HitStun, def.hit_stun);
            }
            self.fighters[a].landed = true;
            self.fighters[a].counter_left = 0.0;
            if counter { Outcome::CounterHit } else { Outcome::Hit }
        };
        let stop = match outcome {
            Outcome::Hit | Outcome::CounterHit | Outcome::GuardBroken => def.hit_stop,
            Outcome::Blocked => def.hit_stop * 0.6,
            Outcome::Parried => def.hit_stop * 1.5,
        };
        self.fighters[a].hit_stop = stop;
        self.fighters[d].hit_stop = stop;
        if outcome == Outcome::Blocked && self.fighters[d].action.kind == ActionKind::Guard {
            self.fighters[d].block_stun = c.block_stun;
        }
        self.contacts.push(Contact { attacker: a, defender: d, mv, outcome, point, tick: self.tick });
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

/// Sparring partner: approaches to range, throws jab strings and the odd heavy, guards when the player
/// winds up. Deterministic (driven by the tick, no randomness) so replays hold.
pub fn spar_intent(world: &World, me: usize, target: usize) -> crate::Intent {
    let (f, t) = (&world.fighters[me], &world.fighters[target]);
    let to = t.position - f.position;
    let dist = to.length();
    let mut intent = crate::Intent::default();
    if f.hp <= 0.0 || t.hp <= 0.0 {
        return intent;
    }
    let player_winding_up = matches!(t.action.kind, ActionKind::Strike(_)) && t.action.phase() == Phase::Startup;
    let beat = (world.tick / 30) % 8;
    if dist > 1.05 {
        intent.move_dir = to.normalized();
        intent.run = dist > 4.0;
    } else if player_winding_up && beat % 3 == 0 {
        intent.guard = true;
        intent.guard_pressed = f.action.kind != ActionKind::Guard;
    } else if f.action.free() || matches!(f.action.kind, ActionKind::Strike(_)) {
        let press_tick = world.tick % 18 == 0;
        if beat == 7 {
            intent.heavy = press_tick;
        } else if beat >= 2 {
            intent.light = press_tick;
        }
    }
    intent
}
