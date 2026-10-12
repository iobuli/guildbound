//! Magic (Beta V2 revamp, boss 2026-10-12): mana, a five-skill bar with per-skill cooldowns, and skills that
//! each play differently: a chargeable homing fireball that explodes when charged, chain lightning that jumps
//! between enemies, a frost nova that freezes everyone close, an arcane blink (short teleport with dodge
//! frames) and a channelled healing light. No ki blast, no ki charge. Deterministic like the rest of the
//! sim: projectiles advance on the fixed step, hits resolve through the same guard rules as strikes.

use crate::combat::{ActionKind, Aim, Contact, Outcome, Source};
use crate::{STEP, V2, World};

pub type SpellId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Fire,
    Lightning,
    Frost,
    Arcane,
    Holy,
}

/// What a skill does when it goes off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpellKind {
    /// A flying shot (homing if `homing` > 0); explodes over `area` when charged.
    Projectile,
    /// Instant hit on the target, then jumps to up to `jumps` more enemies within `jump_range` of the last.
    Chain { jumps: u8, jump_range: f32, falloff: f32 },
    /// Goes off around the caster over `area`.
    Nova,
    /// Teleport up to `distance` toward the stick (or facing); dodges while it happens.
    Blink { distance: f32 },
    /// Restores hp to the caster at release.
    Heal { amount: f32 },
}

#[derive(Clone, Copy, Debug)]
pub struct SpellDef {
    pub name: &'static str,
    pub element: Element,
    pub kind: SpellKind,
    pub cost: f32,
    /// Wind-up before release, and recovery after it (s).
    pub prepare: f32,
    pub recovery: f32,
    /// Hold to charge up to this long (s); 0 = cannot be charged.
    pub max_charge: f32,
    pub speed: f32,
    pub radius: f32,
    pub range: f32,
    pub damage: f32,
    pub guard_drain: f32,
    pub push: f32,
    pub stun: f32,
    /// Degrees per second a projectile turns toward its target (0 = straight).
    pub homing: f32,
    /// Area radius on impact (or around the caster for a nova); 0 = single target.
    pub area: f32,
    pub cooldown: f32,
}

pub const FIREBALL: SpellId = 0;
pub const CHAIN_LIGHTNING: SpellId = 1;
pub const FROST_NOVA: SpellId = 2;
pub const BLINK: SpellId = 3;
pub const HEALING_LIGHT: SpellId = 4;
pub const SPELL_COUNT: usize = 5;

const fn spell(name: &'static str, element: Element, kind: SpellKind, cost: f32, prepare: f32, recovery: f32, cooldown: f32) -> SpellDef {
    SpellDef { name, element, kind, cost, prepare, recovery, max_charge: 0.0, speed: 0.0, radius: 0.0, range: 0.0, damage: 0.0, guard_drain: 0.0, push: 0.0, stun: 0.0, homing: 0.0, area: 0.0, cooldown }
}

pub const SPELLS: [SpellDef; SPELL_COUNT] = [
    SpellDef { max_charge: 1.5, speed: 14.0, radius: 0.25, range: 22.0, damage: 14.0, guard_drain: 18.0, push: 3.0, stun: 0.4, homing: 40.0, area: 1.8,
        ..spell("fireball", Element::Fire, SpellKind::Projectile, 15.0, 0.22, 0.30, 1.0) },
    SpellDef { range: 14.0, damage: 11.0, guard_drain: 20.0, push: 1.0, stun: 0.55,
        ..spell("chain lightning", Element::Lightning, SpellKind::Chain { jumps: 2, jump_range: 5.0, falloff: 0.7 }, 20.0, 0.20, 0.35, 3.0) },
    SpellDef { damage: 8.0, guard_drain: 25.0, push: 2.5, stun: 1.3, area: 3.0,
        ..spell("frost nova", Element::Frost, SpellKind::Nova, 25.0, 0.28, 0.40, 8.0) },
    spell("arcane blink", Element::Arcane, SpellKind::Blink { distance: 5.0 }, 15.0, 0.05, 0.15, 4.0),
    spell("healing light", Element::Holy, SpellKind::Heal { amount: 40.0 }, 30.0, 0.65, 0.30, 10.0),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MagicTuning {
    pub mana_max: f32,
    /// Mana per second in combat; out of combat it regenerates `rest_mult` times faster.
    pub mana_regen: f32,
    pub rest_mult: f32,
    /// A charged shot scales damage and size by 1 + this * charge01.
    pub charge_gain: f32,
}

impl Default for MagicTuning {
    fn default() -> Self {
        Self { mana_max: 120.0, mana_regen: 5.0, rest_mult: 3.0, charge_gain: 2.0 }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projectile {
    pub owner: usize,
    pub spell: SpellId,
    pub position: V2,
    pub direction: V2,
    pub charge: f32,
    pub traveled: f32,
    pub target: Option<usize>,
    pub alive: bool,
}

impl Projectile {
    pub fn scale(&self, gain: f32) -> f32 {
        1.0 + gain * self.charge
    }
}

impl World {
    /// Cancel a held action without releasing a projectile. Other committed actions
    /// keep their rule clock. Used when the input owner loses focus or opens a menu.
    pub fn cancel_held_action(&mut self, i: usize) {
        if self.fighters.get(i).is_some_and(|f| matches!(f.action.kind, ActionKind::Charge(_) | ActionKind::HeavyCharge(_) | ActionKind::Guard)) {
            self.end_magic(i);
            self.fighters[i].parry_left = 0.0;
            self.fighters[i].heavy_charge = 0.0;
        }
    }

    fn spell_target(&self, i: usize) -> Option<usize> {
        let f = &self.fighters[i];
        f.lock_target.filter(|&t| t < self.fighters.len() && self.fighters[t].hp > 0.0).or_else(|| {
            self.fighters
                .iter()
                .enumerate()
                .filter(|(j, o)| *j != i && o.hp > 0.0)
                .map(|(j, o)| (j, o.position - f.position))
                .filter(|(_, d)| d.length() < 20.0 && f.facing.signed_angle_to(d.normalized()).abs() < 0.9)
                .min_by(|a, b| a.1.length().total_cmp(&b.1.length()))
                .map(|(j, _)| j)
        })
    }

    /// Magic intent for one fighter: mana regen, cooldowns, cast presses, charge holds. Called from think.
    pub(crate) fn think_magic(&mut self, i: usize, intent: &crate::Intent) -> bool {
        let m = self.magic;
        let resting = !self.in_combat(i);
        let f = &mut self.fighters[i];
        let regen = if resting { m.mana_regen * m.rest_mult } else { m.mana_regen };
        f.mana = (f.mana + regen * STEP).min(m.mana_max);
        for cd in &mut f.cooldowns {
            *cd = (*cd - STEP).max(0.0);
        }
        let act = f.action;
        match act.kind {
            ActionKind::Charge(s) => {
                let d = SPELLS[s];
                let f = &mut self.fighters[i];
                f.action.elapsed += STEP;
                f.charge = (f.action.elapsed / d.max_charge).min(1.0);
                if !intent.cast_held || f.action.elapsed >= d.max_charge {
                    let charge = f.charge;
                    self.release(i, s, charge);
                }
                return true;
            }
            ActionKind::Cast(s) => {
                let d = SPELLS[s];
                let f = &mut self.fighters[i];
                let before = f.action.elapsed;
                f.action.elapsed += STEP;
                if before < d.prepare && f.action.elapsed >= d.prepare {
                    let charge = f.charge;
                    self.fire(i, s, charge);
                }
                if self.fighters[i].action.elapsed >= d.prepare + d.recovery {
                    self.end_magic(i);
                }
                return true;
            }
            _ => {}
        }
        if !act.free() || f.height > 0.0 || f.crouch_left > 0.0 {
            return false;
        }
        if let Some(slot) = intent.cast {
            let s = (slot as usize).min(SPELLS.len() - 1);
            let d = SPELLS[s];
            let f = &self.fighters[i];
            if f.mana >= d.cost && f.cooldowns[s] <= 0.0 {
                self.fighters[i].mana -= d.cost;
                self.fighters[i].charge = 0.0;
                self.engage(i);
                if let SpellKind::Blink { .. } = d.kind {
                    // a blink goes where the stick points (or straight ahead), never at the target
                    if intent.move_dir.length_squared() > 0.04 {
                        self.fighters[i].facing = intent.move_dir.normalized();
                    }
                    self.seq += 1;
                    let seq = self.seq;
                    self.fighters[i].action = crate::Action { kind: ActionKind::Cast(s), elapsed: 0.0, length: 0.0, seq };
                } else if d.max_charge > 0.0 {
                    // held: charge first; a tap releases on the next tick at zero charge
                    self.begin(i, ActionKind::Charge(s));
                } else {
                    self.begin(i, ActionKind::Cast(s));
                }
                return true;
            }
        }
        false
    }

    fn begin(&mut self, i: usize, kind: ActionKind) {
        self.seq += 1;
        let target = self.spell_target(i);
        let f = &mut self.fighters[i];
        f.action = crate::Action { kind, elapsed: 0.0, length: 0.0, seq: self.seq };
        f.strike_target = target;
        if let Some(t) = target {
            let to = self.fighters[t].position - self.fighters[i].position;
            if to.length_squared() > 1e-4 {
                self.fighters[i].facing = to.normalized();
            }
        }
    }

    fn release(&mut self, i: usize, s: SpellId, charge: f32) {
        self.fighters[i].charge = charge;
        let seq = self.fighters[i].action.seq;
        // Commit the release wind-up; its rule clock launches once at prepare.
        // A quick tap needs the same readable cast point as a held release.
        self.fighters[i].action = crate::Action { kind: ActionKind::Cast(s), elapsed: 0.0, length: 0.0, seq };
    }

    fn end_magic(&mut self, i: usize) {
        self.seq += 1;
        let f = &mut self.fighters[i];
        f.action = crate::Action { kind: ActionKind::None, elapsed: 0.0, length: 0.0, seq: self.seq };
        f.charge = 0.0;
    }

    fn fire(&mut self, i: usize, s: SpellId, charge: f32) {
        let d = SPELLS[s];
        self.fighters[i].cooldowns[s] = d.cooldown;
        match d.kind {
            SpellKind::Nova => {
                let center = self.fighters[i].position;
                self.area(i, s, center, d.area, charge);
            }
            SpellKind::Blink { distance } => {
                let f = &mut self.fighters[i];
                let half = self.tuning.arena_half_size - self.tuning.body_radius;
                let to = f.position + f.facing * distance;
                f.position = V2::new(to.x.clamp(-half, half), to.z.clamp(-half, half));
                f.velocity = V2::ZERO;
            }
            SpellKind::Heal { amount } => {
                let f = &mut self.fighters[i];
                f.hp = (f.hp + amount).min(f.max_hp);
            }
            SpellKind::Chain { jumps, jump_range, falloff } => {
                let Some(first) = self.fighters[i].strike_target.filter(|&t| (self.fighters[t].position - self.fighters[i].position).length() <= d.range) else { return };
                let mut hit = vec![first];
                let mut scale = 1.0;
                let mut from = self.fighters[i].position;
                let mut at = first;
                loop {
                    let to = self.fighters[at].position;
                    let dir = (to - from).normalized();
                    let dir = if dir.length_squared() > 0.0 { dir } else { self.fighters[i].facing };
                    self.magic_hit_scaled(i, at, s, 0.0, to, dir, scale);
                    if hit.len() > jumps as usize {
                        break;
                    }
                    let next = (0..self.fighters.len())
                        .filter(|&j| j != i && !hit.contains(&j) && self.fighters[j].hp > 0.0)
                        .map(|j| (j, (self.fighters[j].position - to).length()))
                        .filter(|&(_, dist)| dist <= jump_range)
                        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
                    let Some((j, _)) = next else { break };
                    hit.push(j);
                    from = to;
                    at = j;
                    scale *= falloff;
                }
            }
            SpellKind::Projectile => {
                let f = &self.fighters[i];
                let target = f.strike_target;
                let spawn = f.position + f.facing * (self.tuning.body_radius + d.radius + 0.1);
                self.projectiles.push(Projectile { owner: i, spell: s, position: spawn, direction: f.facing, charge, traveled: 0.0, target, alive: true });
            }
        }
    }

    /// Advance every projectile one fixed step: homing, travel, first body hit, range and walls.
    pub(crate) fn advance_projectiles(&mut self) {
        let half = self.tuning.arena_half_size;
        let gain = self.magic.charge_gain;
        for k in 0..self.projectiles.len() {
            let mut p = self.projectiles[k];
            if !p.alive {
                continue;
            }
            let d = SPELLS[p.spell];
            if let Some(t) = p.target.filter(|&t| self.fighters[t].hp > 0.0) {
                let to = self.fighters[t].position - p.position;
                if to.length_squared() > 1e-4 && d.homing > 0.0 {
                    p.direction = crate::turn_toward(p.direction, to.normalized(), d.homing * STEP);
                }
            }
            let from = p.position;
            let step = d.speed * STEP;
            let to = from + p.direction * step;
            let r = d.radius * p.scale(gain).sqrt() + self.tuning.body_radius;
            let mut hit: Option<(usize, f32)> = None;
            for (j, o) in self.fighters.iter().enumerate() {
                if j == p.owner || o.hp <= 0.0 {
                    continue;
                }
                let ab = to - from;
                let s = if ab.length_squared() > 1e-9 { ((o.position - from).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
                let closest = from + ab * s;
                if (o.position - closest).length() <= r && hit.is_none_or(|(_, hs)| s < hs) {
                    hit = Some((j, s));
                }
            }
            p.traveled += step;
            if let Some((j, s)) = hit {
                let at = from + (to - from) * s;
                p.alive = false;
                self.projectiles[k] = p;
                if d.area > 0.0 && p.charge > 0.05 {
                    self.area(p.owner, p.spell, at, d.area * p.scale(gain).sqrt(), p.charge);
                } else {
                    self.magic_hit(p.owner, j, p.spell, p.charge, at, p.direction);
                }
                continue;
            }
            p.position = to;
            if p.traveled >= d.range || to.x.abs() > half || to.z.abs() > half {
                p.alive = false;
                if d.area > 0.0 && p.charge > 0.05 {
                    self.projectiles[k] = p;
                    self.area(p.owner, p.spell, to, d.area * p.scale(gain).sqrt(), p.charge);
                    continue;
                }
            }
            self.projectiles[k] = p;
        }
        self.projectiles.retain(|p| p.alive);
    }

    fn area(&mut self, owner: usize, s: SpellId, center: V2, radius: f32, charge: f32) {
        for j in 0..self.fighters.len() {
            if j == owner || self.fighters[j].hp <= 0.0 {
                continue;
            }
            let away = self.fighters[j].position - center;
            let dist = (away.length() - self.tuning.body_radius).max(0.0);
            if dist > radius {
                continue;
            }
            let dir = if away.length_squared() > 1e-6 { away.normalized() } else { self.fighters[owner].facing };
            // falloff to half at the edge
            let falloff = 1.0 - 0.5 * (dist / radius.max(1e-3));
            self.magic_hit_scaled(owner, j, s, charge, center, dir, falloff);
        }
    }

    fn magic_hit(&mut self, owner: usize, d: usize, s: SpellId, charge: f32, point: V2, dir: V2) {
        self.magic_hit_scaled(owner, d, s, charge, point, dir, 1.0);
    }

    /// Resolve a spell on a defender through the same guard rules as strikes (no parry against magic).
    fn magic_hit_scaled(&mut self, owner: usize, d: usize, s: SpellId, charge: f32, point: V2, dir: V2, falloff: f32) {
        let def = SPELLS[s];
        let c = self.combat;
        let scale = (1.0 + self.magic.charge_gain * charge) * falloff;
        let frontal = {
            let f = &self.fighters[d];
            let to = point - f.position;
            to.length_squared() < 1e-6 || f.facing.signed_angle_to(to.normalized()).abs() <= c.guard_arc.to_radians()
        };
        let guarding = self.fighters[d].action.kind == ActionKind::Guard && self.fighters[d].guard > 0.0 && frontal;
        let mut damage = 0.0;
        let outcome = if guarding {
            let t = &mut self.fighters[d];
            t.guard -= def.guard_drain * scale;
            t.velocity = t.velocity + dir * (def.push * scale / 3.0);
            t.guard_regen_wait = c.guard_regen_delay;
            if t.guard <= 0.0 {
                t.guard = 0.0;
                self.seq += 1;
                let seq = self.seq;
                self.fighters[d].action = crate::Action { kind: ActionKind::GuardBreak, elapsed: 0.0, length: c.guard_break_time, seq };
                Outcome::GuardBroken
            } else {
                Outcome::Blocked
            }
        } else {
            damage = crate::rpg::mitigate(def.damage * scale, self.fighters[owner].kit.stats(), self.fighters[d].kit.stats());
            let t = &mut self.fighters[d];
            t.hp = (t.hp - damage).max(0.0);
            t.velocity = t.velocity + dir * def.push * scale;
            self.seq += 1;
            let seq = self.seq;
            let (kind, length) = if t.hp <= 0.0 { (ActionKind::Knockdown, c.knockdown_time * 3.0) } else if def.push * scale > 5.0 { (ActionKind::Knockdown, c.knockdown_time) } else { (ActionKind::HitStun, def.stun) };
            t.action = crate::Action { kind, elapsed: 0.0, length, seq };
            Outcome::Hit
        };
        self.fighters[d].hit_stop = 0.04;
        self.engage(d);
        self.engage(owner);
        let share = if outcome == Outcome::Hit { self.posture.hit_share } else { self.posture.block_share };
        self.add_posture(d, def.guard_drain * scale * share, false, false);
        self.alarm(d, owner);
        self.contacts.push(Contact { attacker: owner, defender: d, source: Source::Spell(s), outcome, point, aim: Aim::Body, tick: self.tick, damage, crit: false });
    }
}
