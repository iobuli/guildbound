use crate::combat::*;
use crate::rpg::*;
use crate::*;

fn duel(gap: f32) -> World {
    let mut w = World::sparring(Tuning::default());
    w.fighters[0] = Fighter::new(V2::new(0.0, 0.0), V2::new(1.0, 0.0), 100.0);
    w.fighters[1] = Fighter::new(V2::new(gap, 0.0), V2::new(-1.0, 0.0), 100.0);
    w
}

fn bag(action: BagAction) -> Intent {
    Intent { bag: Some(action), ..Default::default() }
}

fn first_hit(w: &mut World) -> f32 {
    let before = w.fighters[1].hp;
    w.step(&[Intent { light: true, ..Default::default() }, Intent::default()]);
    for _ in 0..60 {
        if !w.contacts.is_empty() {
            break;
        }
        w.step(&[]);
    }
    before - w.fighters[1].hp
}

#[test]
fn power_and_armor_scale_melee_damage() {
    let mut w = duel(0.9);
    assert_eq!(first_hit(&mut w), MOVES[JAB].damage, "no gear: base damage");

    let mut w = duel(0.9);
    w.fighters[0].kit.worn[2] = Some(IRON_KNUCKLES);
    assert!((first_hit(&mut w) - MOVES[JAB].damage * 1.25).abs() < 1e-4, "+25 power = +25%");

    let mut w = duel(0.9);
    w.fighters[1].kit.worn[1] = Some(BRIGANDINE);
    assert!((first_hit(&mut w) - MOVES[JAB].damage * 100.0 / 145.0).abs() < 1e-4, "45 armor");
}

#[test]
fn equip_swaps_with_the_bag_and_vitality_moves_max_hp() {
    let mut w = duel(3.0);
    w.fighters[0].kit.bag = vec![PADDED_VEST, BRIGANDINE];
    w.step(&[bag(BagAction::Use(0))]);
    assert_eq!(w.fighters[0].kit.worn_in(Slot::Chest), Some(PADDED_VEST));
    assert_eq!(w.fighters[0].kit.bag, vec![BRIGANDINE]);
    assert_eq!(w.fighters[0].max_hp, 110.0);
    w.step(&[bag(BagAction::Use(0))]);
    assert_eq!(w.fighters[0].kit.worn_in(Slot::Chest), Some(BRIGANDINE));
    assert_eq!(w.fighters[0].kit.bag, vec![PADDED_VEST], "old piece goes back where the new one was");
    assert_eq!(w.fighters[0].max_hp, 120.0);
    w.step(&[bag(BagAction::Unequip(Slot::Chest))]);
    assert_eq!(w.fighters[0].kit.worn_in(Slot::Chest), None);
    assert_eq!(w.fighters[0].max_hp, 100.0);
    assert_eq!(w.fighters[0].hp, 100.0, "losing vitality clamps hp to the new max");
}

#[test]
fn consumables_restore_and_are_used_up() {
    let mut w = duel(3.0);
    w.fighters[0].hp = 50.0;
    w.fighters[0].mana = 10.0;
    w.fighters[0].kit.bag = vec![HEALING_DRAUGHT, KI_TONIC];
    w.step(&[bag(BagAction::Use(0))]);
    assert_eq!(w.fighters[0].action.kind, ActionKind::Drink(HEALING_DRAUGHT));
    assert_eq!(w.fighters[0].hp, 50.0, "drinking takes time");
    for _ in 0..140 {
        w.step(&[]);
    }
    assert_eq!(w.fighters[0].hp, 85.0);
    w.step(&[bag(BagAction::Use(0))]);
    for _ in 0..140 {
        w.step(&[]);
    }
    assert!(w.fighters[0].mana >= 50.0);
    assert!(w.fighters[0].kit.bag.is_empty());
}

#[test]
fn interact_picks_up_the_nearest_item_then_opens_a_chest() {
    let mut w = duel(3.0);
    w.pickups.push(Pickup { item: HAND_WRAPS, position: V2::new(0.5, 0.0) });
    w.chests.push(Chest { position: V2::new(-1.0, 0.0), loot: vec![HEALING_DRAUGHT, LEATHER_CAP], open: false });
    assert_eq!(w.reachable(0), Some(Reachable::Pickup(0)), "pickups before chests");
    let rev = w.loot_rev;
    w.step(&[Intent { interact: true, ..Default::default() }]);
    assert_eq!(w.fighters[0].kit.bag, vec![HAND_WRAPS]);
    assert!(w.pickups.is_empty() && w.loot_rev > rev);
    w.step(&[Intent { interact: true, ..Default::default() }]);
    assert!(w.chests[0].open && w.chests[0].loot.is_empty());
    assert_eq!(w.pickups.len(), 2, "chest spills its loot");
    assert!(w.pickups.iter().all(|p| (p.position - w.chests[0].position).length() < 1.0));
    assert_eq!(w.reachable(0).map(|r| matches!(r, Reachable::Pickup(_))), Some(true));
}

#[test]
fn a_downed_fighter_drops_its_loot_once() {
    let mut w = duel(3.0);
    w.fighters[1].loot = Some(QUILTED_TROUSERS);
    w.fighters[1].hp = 0.0;
    w.step(&[]);
    w.step(&[]);
    assert_eq!(w.pickups, vec![Pickup { item: QUILTED_TROUSERS, position: w.fighters[1].position }]);
}

#[test]
fn bag_changes_are_part_of_the_state_hash() {
    let mut a = duel(3.0);
    let mut b = duel(3.0);
    a.fighters[0].kit.bag = vec![HAND_WRAPS];
    b.fighters[0].kit.bag = vec![HAND_WRAPS];
    a.step(&[bag(BagAction::Use(0))]);
    b.step(&[]);
    assert_ne!(a.state_hash(), b.state_hash());
}
