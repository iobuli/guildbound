//! RPG layer: stats with additive modifiers, items, inventory, equipment, pickups and chests.
//! Reimagined from a read of Brackeys' RPG tutorial (no licence; nothing copied): a stat is a base value plus
//! the modifiers of what is equipped, equipping swaps with the bag, consumables restore, the world holds
//! items on the ground and chests that spill their loot. Damage comes from stats, never from animation.
//! Every change goes through `Intent` so replays stay deterministic.

use crate::{V2, World};

/// Additive stat modifiers. `power` and `armor` are ratings, `vitality` is max hp.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// +1% outgoing damage per point.
    pub power: f32,
    /// Incoming damage scaled by 100 / (100 + armor).
    pub armor: f32,
    pub vitality: f32,
    /// Percent chance a melee hit is critical (posture.rs).
    pub crit: f32,
}

impl Stats {
    fn add(self, o: Stats) -> Stats {
        Stats { power: self.power + o.power, armor: self.armor + o.armor, vitality: self.vitality + o.vitality, crit: self.crit + o.crit }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Head,
    Chest,
    Hands,
    Legs,
    Feet,
}

pub const SLOTS: [Slot; 5] = [Slot::Head, Slot::Chest, Slot::Hands, Slot::Legs, Slot::Feet];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemKind {
    Consumable { heal: f32, ki: f32 },
    Gear(Slot),
}

#[derive(Clone, Copy, Debug)]
pub struct ItemDef {
    pub name: &'static str,
    pub kind: ItemKind,
    pub mods: Stats,
}

pub type ItemId = u8;

const fn gear(name: &'static str, slot: Slot, power: f32, armor: f32, vitality: f32) -> ItemDef {
    ItemDef { name, kind: ItemKind::Gear(slot), mods: Stats { power, armor, vitality, crit: 0.0 } }
}

const fn keen(name: &'static str, slot: Slot, power: f32, crit: f32) -> ItemDef {
    ItemDef { name, kind: ItemKind::Gear(slot), mods: Stats { power, armor: 0.0, vitality: 0.0, crit } }
}

const fn potion(name: &'static str, heal: f32, ki: f32) -> ItemDef {
    ItemDef { name, kind: ItemKind::Consumable { heal, ki }, mods: Stats { power: 0.0, armor: 0.0, vitality: 0.0, crit: 0.0 } }
}

pub const ITEMS: &[ItemDef] = &[
    potion("healing draught", 35.0, 0.0),
    potion("ki tonic", 0.0, 40.0),
    gear("hand wraps", Slot::Hands, 10.0, 0.0, 0.0),
    gear("iron knuckles", Slot::Hands, 25.0, 0.0, 0.0),
    gear("padded vest", Slot::Chest, 0.0, 20.0, 10.0),
    gear("guild brigandine", Slot::Chest, 0.0, 45.0, 20.0),
    gear("leather cap", Slot::Head, 0.0, 10.0, 0.0),
    gear("travel boots", Slot::Feet, 0.0, 5.0, 5.0),
    gear("quilted trousers", Slot::Legs, 0.0, 15.0, 5.0),
    keen("duelist's gloves", Slot::Hands, 5.0, 15.0),
];

pub const HEALING_DRAUGHT: ItemId = 0;
pub const KI_TONIC: ItemId = 1;
pub const HAND_WRAPS: ItemId = 2;
pub const IRON_KNUCKLES: ItemId = 3;
pub const PADDED_VEST: ItemId = 4;
pub const BRIGANDINE: ItemId = 5;
pub const LEATHER_CAP: ItemId = 6;
pub const TRAVEL_BOOTS: ItemId = 7;
pub const QUILTED_TROUSERS: ItemId = 8;
pub const DUELIST_GLOVES: ItemId = 9;

pub const BAG_SIZE: usize = 20;
/// Reach for picking up (m) and for opening a chest (m).
pub const PICKUP_REACH: f32 = 1.2;
pub const CHEST_REACH: f32 = 1.5;

/// One inventory change, carried in `Intent` so it lands on a sim tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BagAction {
    /// Use a consumable or equip gear from this bag index.
    Use(u8),
    Unequip(Slot),
    Drop(u8),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kit {
    pub bag: Vec<ItemId>,
    pub worn: [Option<ItemId>; 5],
}

impl Kit {
    pub fn stats(&self) -> Stats {
        self.worn.iter().flatten().fold(Stats::default(), |s, &id| s.add(ITEMS[id as usize].mods))
    }

    pub fn worn_in(&self, slot: Slot) -> Option<ItemId> {
        self.worn[slot_index(slot)]
    }
}

fn slot_index(slot: Slot) -> usize {
    SLOTS.iter().position(|&s| s == slot).unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pickup {
    pub item: ItemId,
    pub position: V2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chest {
    pub position: V2,
    pub loot: Vec<ItemId>,
    pub open: bool,
}

/// What the player can reach right now, for the HUD prompt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reachable {
    Pickup(usize),
    Chest(usize),
}

/// Outgoing damage after the attacker's power and the defender's armor.
pub fn mitigate(raw: f32, attacker: Stats, defender: Stats) -> f32 {
    raw * (1.0 + attacker.power / 100.0) * 100.0 / (100.0 + defender.armor.max(0.0))
}

impl World {
    /// Nearest pickup in reach, else the nearest closed chest in reach.
    pub fn reachable(&self, i: usize) -> Option<Reachable> {
        let p = self.fighters.get(i)?.position;
        let near = |q: V2, r: f32| {
            let d = (q - p).length();
            (d <= r).then_some(d)
        };
        let pickup = self.pickups.iter().enumerate().filter_map(|(k, it)| near(it.position, PICKUP_REACH).map(|d| (k, d))).min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((k, _)) = pickup {
            return Some(Reachable::Pickup(k));
        }
        let chest = self.chests.iter().enumerate().filter(|(_, c)| !c.open).filter_map(|(k, c)| near(c.position, CHEST_REACH).map(|d| (k, d))).min_by(|a, b| a.1.total_cmp(&b.1));
        chest.map(|(k, _)| Reachable::Chest(k))
    }

    pub(crate) fn interact(&mut self, i: usize) {
        if self.fighters[i].hp <= 0.0 {
            return;
        }
        match self.reachable(i) {
            Some(Reachable::Pickup(k)) => {
                if self.fighters[i].kit.bag.len() < BAG_SIZE {
                    let item = self.pickups.remove(k).item;
                    self.fighters[i].kit.bag.push(item);
                    self.loot_rev += 1;
                }
            }
            Some(Reachable::Chest(k)) => {
                let chest = &mut self.chests[k];
                chest.open = true;
                let at = chest.position;
                let loot = std::mem::take(&mut chest.loot);
                let n = loot.len().max(1) as f32;
                // spill in a ring on the fighter's side of the chest
                let toward = (self.fighters[i].position - at).normalized();
                let toward = if toward.length_squared() > 0.0 { toward } else { V2::new(0.0, -1.0) };
                for (k, item) in loot.into_iter().enumerate() {
                    let a = (k as f32 - (n - 1.0) * 0.5) * 0.7;
                    self.pickups.push(Pickup { item, position: at + toward.rotated(a) * 0.9 });
                }
                self.loot_rev += 1;
            }
            None => {}
        }
    }

    pub(crate) fn bag_action(&mut self, i: usize, action: BagAction) {
        let f = &mut self.fighters[i];
        if f.hp <= 0.0 {
            return;
        }
        match action {
            BagAction::Use(k) => {
                let k = k as usize;
                let Some(&id) = f.kit.bag.get(k) else { return };
                match ITEMS[id as usize].kind {
                    // drunk over time and interruptible (posture.rs), not instant
                    ItemKind::Consumable { .. } => self.start_drink(i, id),
                    ItemKind::Gear(slot) => {
                        let s = slot_index(slot);
                        let old = f.kit.worn[s].replace(id);
                        f.kit.bag.remove(k);
                        if let Some(old) = old {
                            f.kit.bag.insert(k, old);
                        }
                    }
                }
            }
            BagAction::Unequip(slot) => {
                let s = slot_index(slot);
                if f.kit.worn[s].is_some() && f.kit.bag.len() < BAG_SIZE {
                    let id = f.kit.worn[s].take().unwrap();
                    f.kit.bag.push(id);
                }
            }
            BagAction::Drop(k) => {
                let k = k as usize;
                if k < f.kit.bag.len() {
                    let item = f.kit.bag.remove(k);
                    let at = f.position + f.facing * 0.6;
                    self.pickups.push(Pickup { item, position: at });
                    self.loot_rev += 1;
                }
            }
        }
        self.refresh_vitality(i);
    }

    /// Max hp follows vitality; current hp keeps its value, clamped.
    pub fn refresh_vitality(&mut self, i: usize) {
        let f = &mut self.fighters[i];
        f.max_hp = f.base_hp + f.kit.stats().vitality;
        f.hp = f.hp.min(f.max_hp);
    }

    /// A downed fighter drops what it carried as loot.
    pub(crate) fn drop_loot(&mut self) {
        for f in &mut self.fighters {
            if f.hp <= 0.0 && let Some(item) = f.loot.take() {
                self.pickups.push(Pickup { item, position: f.position });
                self.loot_rev += 1;
            }
        }
    }

    /// The sparring floor's stock: one chest and a few things lying around.
    pub fn stock_floor(&mut self) {
        self.chests.push(Chest { position: V2::new(5.5, 5.5), loot: vec![BRIGANDINE, IRON_KNUCKLES, HEALING_DRAUGHT], open: false });
        for (item, x, z) in [(HAND_WRAPS, -1.5, -3.5), (HEALING_DRAUGHT, 2.0, -3.0), (PADDED_VEST, -5.0, 1.0), (KI_TONIC, 4.5, 0.5)] {
            self.pickups.push(Pickup { item, position: V2::new(x, z) });
        }
        self.loot_rev += 1;
    }
}
