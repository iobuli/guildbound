//! Mob brains, reimagined from a read of FATE's monster AI (Documentation/reference/STUDY_NOTES.md):
//! wander near home, hunt inside an attention radius, give up past a leash radius, retaliate when hit,
//! melee rushes in, casters hold their distance and reposition. Deterministic: no RNG, choices come from
//! the tick and the mob's index, so replays hold.

use crate::combat::ActionKind;
use crate::{Intent, V2, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobState {
    Wander,
    Hunt,
    Return,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tactics {
    Melee,
    /// Keeps 3-6 m from the target and repositions (casters, ranged mobs).
    Keeper,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobBrain {
    pub state: MobState,
    pub tactics: Tactics,
    pub home: V2,
    /// Starts a hunt (m).
    pub attention: f32,
    /// Ends a hunt this far from home (m).
    pub leash: f32,
    pub target: usize,
    pub wander_to: V2,
    pub next_think: u64,
    /// Aggression 0..1: how often it presses in range.
    pub aggression: f32,
}

impl MobBrain {
    pub fn new(home: V2, tactics: Tactics) -> Self {
        Self {
            state: MobState::Wander,
            tactics,
            home,
            attention: 7.0,
            leash: 14.0,
            target: 0,
            wander_to: home,
            next_think: 0,
            aggression: 0.5,
        }
    }
}

fn hash(a: u64, b: u64) -> u64 {
    let mut h = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 31;
    h.wrapping_mul(0x1656_67B1_9E37_79F9)
}

/// 0..1 from the tick and an index.
fn unit(tick: u64, salt: u64) -> f32 {
    (hash(tick, salt) % 10_000) as f32 / 10_000.0
}

/// Update one mob's brain and return its intent for this tick.
pub fn mob_intent(world: &World, me: usize, brain: &mut MobBrain) -> Intent {
    let f = &world.fighters[me];
    let mut intent = Intent::default();
    if f.hp <= 0.0 {
        return intent;
    }
    let t = &world.fighters[brain.target.min(world.fighters.len() - 1)];
    let to_target = t.position - f.position;
    let dist = to_target.length();
    let from_home = (f.position - brain.home).length();

    // retaliate: being hit always starts a hunt on the attacker (FATE: respond to attack)
    if let Some(c) = world.contacts.iter().rev().find(|c| c.defender == me) {
        brain.target = c.attacker;
        brain.state = MobState::Hunt;
    }

    brain.state = match brain.state {
        MobState::Wander if dist < brain.attention && t.hp > 0.0 => MobState::Hunt,
        MobState::Hunt if from_home > brain.leash || t.hp <= 0.0 => MobState::Return,
        MobState::Return if from_home < 1.0 => MobState::Wander,
        s => s,
    };

    match brain.state {
        MobState::Wander => {
            if world.tick >= brain.next_think || (f.position - brain.wander_to).length() < 0.4 {
                let a = unit(world.tick, me as u64) * std::f32::consts::TAU;
                let r = 1.0 + 3.0 * unit(world.tick, me as u64 + 99);
                brain.wander_to = brain.home + V2::new(a.cos(), a.sin()) * r;
                brain.next_think = world.tick + 240 + (unit(world.tick, me as u64 + 7) * 360.0) as u64;
            }
            let d = brain.wander_to - f.position;
            if d.length() > 0.3 {
                intent.move_dir = d.normalized() * 0.5; // amble
            }
        }
        MobState::Return => {
            intent.move_dir = (brain.home - f.position).normalized();
        }
        MobState::Hunt => {
            let reach = 1.05;
            let (near, far) = match brain.tactics {
                Tactics::Melee => (0.0, reach),
                Tactics::Keeper => (3.0, 6.0),
            };
            if dist > far {
                intent.move_dir = to_target.normalized();
                intent.run = dist > 4.0;
            } else if dist < near {
                // too close for a keeper: back off at an angle
                let away = to_target.normalized() * -1.0;
                intent.move_dir = away.rotated(0.6);
            } else if brain.tactics == Tactics::Melee {
                if let Some(d) = crate::combat::defend(world, me, brain.target, 0.45) {
                    return d;
                }
                let beat = hash(world.tick / 20, me as u64) % 100;
                if (f.action.free() || matches!(f.action.kind, ActionKind::Strike(_))) && world.tick % 16 == (me as u64 % 16) {
                    if (beat as f32) < brain.aggression * 100.0 {
                        if beat % 7 == 0 { intent.heavy = true } else { intent.light = true }
                    }
                }
            } else {
                // keeper in its band: strafe around the target and throw ki blasts
                intent.move_dir = to_target.normalized().rotated(std::f32::consts::FRAC_PI_2) * 0.6;
                if f.action.free() && world.tick % 45 == (me as u64 * 7) % 45 {
                    intent.cast = Some(1);
                    intent.move_dir = V2::ZERO;
                }
            }
        }
    }
    intent
}

/// Add a mob fighter to the world; returns its index.
pub fn spawn(world: &mut World, at: V2, hp: f32) -> usize {
    let facing = (world.fighters[0].position - at).normalized();
    let facing = if facing.length_squared() > 0.0 { facing } else { V2::new(0.0, 1.0) };
    world.fighters.push(crate::Fighter::new(at, facing, hp));
    world.fighters.len() - 1
}
