//! Posture, dizzy and deathblows; backstabs; crits; timed drinking.
//! Fused from reads of Snaiel's Godot4ThirdPersonCombatPrototype (MIT, Sekiro-style instability, dizzy, finisher,
//! backstab, interruptible flask) and Mustafa-Kum's UE5 melee system (no licence; crit rolls, move-cancel
//! windows) with our guard meter and consumables. Written from Documentation/reference/STUDY_NOTES.md.
//! - Posture fills from hits, blocks and being parried; parrying costs the parrier a little but can never
//!   break them. Recovery waits after each gain, then drains faster while guarding and slower when hurt.
//! - Full posture = dizzy. Any strike that lands on a dizzy fighter is a deathblow.
//! - A strike from behind on a mob that is not alert is a deathblow too.
//! - Crits are rolled from a hash of tick, attacker and defender: replays hold.

use crate::combat::ActionKind;
use crate::rpg::{ITEMS, ItemId, ItemKind};
use crate::{STEP, World};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PostureTuning {
    pub max: f32,
    /// Seconds after a gain before posture starts to drain.
    pub wait: f32,
    /// Drain per second at full hp, standing.
    pub regen: f32,
    pub guard_regen_mult: f32,
    /// Share of a move's guard_drain added to the defender on a clean hit / on a block.
    pub hit_share: f32,
    pub block_share: f32,
    /// Added to an attacker whose strike was parried (plus the move's guard_drain), and to the parrier.
    pub parried: f32,
    pub parry_cost: f32,
    pub dizzy_from_parry: f32,
    pub dizzy_from_damage: f32,
    /// Posture left (share of max) when dizzy wears off.
    pub after_dizzy: f32,
    pub deathblows: bool,
    /// Behind = more than this angle (deg) from the target's facing; within this range (m).
    pub backstab_angle: f32,
    pub backstab_range: f32,
    pub crit_mult: f32,
    /// A strike's recovery may be cancelled by movement once this share of it has passed.
    pub move_cancel: f32,
    pub drink_time: f32,
    /// Share of drink_time at which the item takes effect and is used up.
    pub drink_commit: f32,
}

impl Default for PostureTuning {
    fn default() -> Self {
        Self {
            max: 100.0,
            wait: 1.5,
            regen: 20.0,
            guard_regen_mult: 1.6,
            hit_share: 0.6,
            block_share: 0.5,
            parried: 20.0,
            parry_cost: 4.0,
            dizzy_from_parry: 2.0,
            dizzy_from_damage: 3.0,
            after_dizzy: 0.5,
            deathblows: true,
            backstab_angle: 125.0,
            backstab_range: 1.6,
            crit_mult: 1.75,
            move_cancel: 0.5,
            drink_time: 1.1,
            drink_commit: 0.55,
        }
    }
}

pub(crate) fn hash(a: u64, b: u64) -> u64 {
    let mut h = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 31;
    h.wrapping_mul(0x1656_67B1_9E37_79F9)
}

impl World {
    /// Drain posture after the wait; called every tick before actions advance.
    pub(crate) fn recover_posture(&mut self, i: usize) {
        let p = self.posture;
        let f = &mut self.fighters[i];
        if f.action.kind == ActionKind::Dizzy || f.hp <= 0.0 {
            return;
        }
        if f.posture_wait > 0.0 {
            f.posture_wait = (f.posture_wait - STEP).max(0.0);
            return;
        }
        let hurt = 0.35 + 0.65 * (f.hp / f.max_hp.max(1.0)).clamp(0.0, 1.0);
        let guard = if f.action.kind == ActionKind::Guard { p.guard_regen_mult } else { 1.0 };
        f.posture = (f.posture - p.regen * hurt * guard * STEP).max(0.0);
    }

    /// Add posture; `capped` gains can fill to just under max. Full posture makes the fighter dizzy.
    pub(crate) fn add_posture(&mut self, i: usize, amount: f32, capped: bool, from_parry: bool) {
        let p = self.posture;
        let f = &self.fighters[i];
        if f.hp <= 0.0 || f.action.kind == ActionKind::Dizzy || amount <= 0.0 {
            return;
        }
        let top = if capped { p.max - 0.01 } else { p.max };
        let f = &mut self.fighters[i];
        f.posture = (f.posture + amount).min(top.max(f.posture));
        f.posture_wait = p.wait;
        if f.posture >= p.max {
            f.posture = p.max;
            let length = if from_parry { p.dizzy_from_parry } else { p.dizzy_from_damage };
            self.new_action(i, ActionKind::Dizzy, length);
            self.fighters[i].velocity = crate::V2::ZERO;
        }
    }

    /// A strike landing now on `d` from `a` is a deathblow: `d` is dizzy, or `d` is an unalerted mob struck
    /// from behind at close range.
    pub(crate) fn deathblow(&self, a: usize, d: usize) -> bool {
        let p = self.posture;
        if !p.deathblows {
            return false;
        }
        let t = &self.fighters[d];
        if t.action.kind == ActionKind::Dizzy {
            return true;
        }
        let Some(n) = t.notice else { return false };
        if n.state == crate::notice::NoticeState::Alert {
            return false;
        }
        let to = self.fighters[a].position - t.position;
        to.length() <= p.backstab_range
            && to.length_squared() > 1e-6
            && t.facing.signed_angle_to(to.normalized()).abs() >= p.backstab_angle.to_radians()
    }

    /// Deterministic crit roll against the attacker's crit chance (percent).
    pub(crate) fn crit(&self, a: usize, d: usize) -> bool {
        let chance = self.fighters[a].kit.stats().crit;
        chance > 0.0 && (hash(self.tick, (a as u64) << 16 | d as u64) % 10_000) as f32 / 100.0 < chance
    }

    /// Start drinking a consumable from the bag; only from a free, grounded stance.
    pub(crate) fn start_drink(&mut self, i: usize, id: ItemId) {
        let f = &self.fighters[i];
        if !matches!(f.action.kind, ActionKind::None | ActionKind::Guard) || f.height > 0.0 {
            return;
        }
        self.new_action(i, ActionKind::Drink(id), self.posture.drink_time);
    }

    /// Advance a drink; the item works and is used up at the commit point. A drink cut short (hit,
    /// knocked down) never reaches it, so nothing is lost.
    pub(crate) fn advance_drink(&mut self, i: usize, id: ItemId) {
        let commit = self.posture.drink_time * self.posture.drink_commit;
        let mana_max = self.magic.mana_max;
        let f = &mut self.fighters[i];
        let before = f.action.elapsed;
        f.action.elapsed += STEP;
        if before < commit && f.action.elapsed >= commit
            && let Some(k) = f.kit.bag.iter().position(|&b| b == id)
            && let ItemKind::Consumable { heal, ki } = ITEMS[id as usize].kind
        {
            f.kit.bag.remove(k);
            f.hp = (f.hp + heal).min(f.max_hp);
            f.mana = (f.mana + ki).min(mana_max);
        }
    }
}
