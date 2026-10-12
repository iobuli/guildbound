//! Magic: ki resource, spell table, casting and charging, projectiles, area bursts.
//! Built from the GDD (D18: tap fire / hold to charge orange-blue-white, overheld burst, elements) and the
//! BT3 study (ki charge on a held button, cheap fast ki blasts, a big charged shot). Deterministic like the
//! rest of the sim: projectiles advance on the fixed step, hits resolve through the same guard rules as strikes.

use crate::combat::{ActionKind, Aim, Contact, Outcome, Source};
use crate::{STEP, V2, World};

pub type SpellId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Fire,
    Ki,
    Lightning,
    Force,
}

#[derive(Clone, Copy, Debug)]
pub struct SpellDef {
    pub name: &'static str,
    pub element: Element,
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
    /// Area radius on impact (or around the caster for a burst); 0 = single target.
    pub area: f32,
    /// A burst goes off around the caster instead of flying.
    pub burst: bool,
    pub cooldown: f32,
}

pub const FIREBALL: SpellId = 0;
pub const KI_BLAST: SpellId = 1;
pub const LIGHTNING: SpellId = 2;
pub const BURST: SpellId = 3;

pub const SPELLS: [SpellDef; 4] = [
    SpellDef { name: "fireball", element: Element::Fire, cost: 12.0, prepare: 0.22, recovery: 0.30, max_charge: 2.0, speed: 14.0, radius: 0.25, range: 18.0, damage: 10.0, guard_drain: 15.0, push: 2.5, stun: 0.35, homing: 40.0, area: 1.5, burst: false, cooldown: 0.4 },
    SpellDef { name: "ki blast", element: Element::Ki, cost: 4.0, prepare: 0.08, recovery: 0.12, max_charge: 0.0, speed: 20.0, radius: 0.15, range: 16.0, damage: 3.0, guard_drain: 5.0, push: 0.8, stun: 0.18, homing: 120.0, area: 0.0, burst: false, cooldown: 0.12 },
    SpellDef { name: "lightning", element: Element::Lightning, cost: 18.0, prepare: 0.18, recovery: 0.35, max_charge: 0.0, speed: 32.0, radius: 0.2, range: 22.0, damage: 9.0, guard_drain: 20.0, push: 1.0, stun: 0.6, homing: 0.0, area: 0.0, burst: false, cooldown: 1.2 },
    SpellDef { name: "burst", element: Element::Force, cost: 30.0, prepare: 0.30, recovery: 0.45, max_charge: 0.0, speed: 0.0, radius: 0.0, range: 0.0, damage: 8.0, guard_drain: 30.0, push: 7.0, stun: 0.5, homing: 0.0, area: 2.4, burst: true, cooldown: 3.0 },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MagicTuning {
    pub ki_max: f32,
    pub ki_regen: f32,
    /// Ki per second while holding the charge button (Q).
    pub ki_charge_rate: f32,
    /// A charged shot scales damage and size by 1 + this * charge01.
    pub charge_gain: f32,
}

impl Default for MagicTuning {
    fn default() -> Self {
        Self { ki_max: 100.0, ki_regen: 4.0, ki_charge_rate: 35.0, charge_gain: 2.0 }
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
        if self.fighters.get(i).is_some_and(|f| matches!(f.action.kind, ActionKind::Charge(_) | ActionKind::HeavyCharge(_) | ActionKind::KiCharge | ActionKind::Guard)) {
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

    /// Magic intent for one fighter: ki regen and charge, cast presses, charge holds. Called from think.
    pub(crate) fn think_magic(&mut self, i: usize, intent: &crate::Intent) -> bool {
        let m = self.magic;
        let f = &mut self.fighters[i];
        f.ki = (f.ki + m.ki_regen * STEP).min(m.ki_max);
        f.spell_cooldown = (f.spell_cooldown - STEP).max(0.0);
        let act = f.action;
        match act.kind {
            ActionKind::KiCharge => {
                if intent.ki_charge {
                    f.ki = (f.ki + m.ki_charge_rate * STEP).min(m.ki_max);
                    f.action.elapsed += STEP;
                } else {
                    self.end_magic(i);
                }
                return true;
            }
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
        if intent.ki_charge {
            self.begin(i, ActionKind::KiCharge);
            return true;
        }
        if let Some(slot) = intent.cast {
            let s = (slot as usize).min(SPELLS.len() - 1);
            let d = SPELLS[s];
            let f = &self.fighters[i];
            if f.ki >= d.cost && f.spell_cooldown <= 0.0 {
                self.fighters[i].ki -= d.cost;
                self.fighters[i].charge = 0.0;
                if d.max_charge > 0.0 {
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
        self.fighters[i].spell_cooldown = d.cooldown;
        if d.burst {
            let center = self.fighters[i].position;
            self.area(i, s, center, d.area, charge, true);
            return;
        }
        let f = &self.fighters[i];
        let target = f.strike_target;
        let spawn = f.position + f.facing * (self.tuning.body_radius + d.radius + 0.1);
        self.projectiles.push(Projectile { owner: i, spell: s, position: spawn, direction: f.facing, charge, traveled: 0.0, target, alive: true });
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
                    self.area(p.owner, p.spell, at, d.area * p.scale(gain).sqrt(), p.charge, false);
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
                    self.area(p.owner, p.spell, to, d.area * p.scale(gain).sqrt(), p.charge, false);
                    continue;
                }
            }
            self.projectiles[k] = p;
        }
        self.projectiles.retain(|p| p.alive);
    }

    fn area(&mut self, owner: usize, s: SpellId, center: V2, radius: f32, charge: f32, burst: bool) {
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
            let _ = burst;
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
        let share = if outcome == Outcome::Hit { self.posture.hit_share } else { self.posture.block_share };
        self.add_posture(d, def.guard_drain * scale * share, false, false);
        self.alarm(d, owner);
        self.contacts.push(Contact { attacker: owner, defender: d, source: Source::Spell(s), outcome, point, aim: Aim::Body, tick: self.tick, damage, crit: false });
    }
}
