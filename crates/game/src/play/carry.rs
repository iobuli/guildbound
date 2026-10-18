//! What each character carries, in kilograms, for the carry posture (`motion::carry`): the
//! equipped weapon in its hand (`WeaponDef.mass`, `hand`), or 30 % of it on the body once
//! sheathed (`sheath`); a held tool (`tools::Holding`) in its hand. Writes `motion::carry::Load`,
//! eased toward the new value so drawing or swapping a weapon settles instead of snapping.
use super::moveset::Movesets;
use super::sheath::Sheath;
use super::tools::{Holding, Tool};
use super::{Fighter, Loadout};
use crate::motion::carry::Load;
use bevy::prelude::*;

/// A sheathed weapon's share felt on the body.
const SHEATHED: f32 = 0.3;
/// How fast the felt load follows the real one (1/s).
const EASE: f32 = 6.0;

fn tool_mass(tool: Tool) -> f32 {
    match tool {
        Tool::Axe => 1.6,
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, carry.in_set(super::PlaySet::Select));
}

#[allow(clippy::type_complexity)]
fn carry(
    time: Res<Time>,
    movesets: Option<Res<Movesets>>,
    mut commands: Commands,
    mut carriers: Query<(Entity, &Loadout, Option<&Sheath>, Option<&Holding>, Option<&mut Load>), With<Fighter>>,
) {
    let k = 1.0 - (-EASE * time.delta_secs()).exp();
    for (entity, loadout, sheath, holding, load) in &mut carriers {
        let mut want = Load::default();
        if let Some(def) = movesets.as_ref().and_then(|m| m.get(loadout.equipped())).and_then(|m| m.weapon.as_ref()) {
            let mass = if def.mass.is_finite() { def.mass.max(0.0) } else { 0.0 };
            if sheath.is_none_or(|s| s.in_hand()) {
                want.hand[if def.hand == "l" { 0 } else { 1 }] += mass;
            } else {
                want.back += mass * SHEATHED;
            }
        }
        if let Some(h) = holding {
            want.hand[h.hand.min(1)] += tool_mass(h.tool);
        }
        match load {
            Some(mut l) => {
                let next = Load {
                    hand: [l.hand[0] + (want.hand[0] - l.hand[0]) * k, l.hand[1] + (want.hand[1] - l.hand[1]) * k],
                    back: l.back + (want.back - l.back) * k,
                };
                if *l != next {
                    *l = next;
                }
            }
            None => {
                commands.entity(entity).try_insert(want);
            }
        }
    }
}
