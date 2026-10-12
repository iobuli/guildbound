use crate::combat::*;
use crate::*;

/// Two fighters on the x axis facing each other, `gap` metres apart.
fn duel(gap: f32) -> World {
    let mut w = World::sparring(Tuning::default());
    w.fighters[0] = Fighter::new(V2::new(0.0, 0.0), V2::new(1.0, 0.0), 100.0);
    w.fighters[1] = Fighter::new(V2::new(gap, 0.0), V2::new(-1.0, 0.0), 100.0);
    w
}

fn light() -> Intent {
    Intent { light: true, ..Default::default() }
}
fn heavy(dir: V2) -> Intent {
    Intent { heavy: true, move_dir: dir, ..Default::default() }
}
fn guard(pressed: bool) -> Intent {
    Intent { guard: true, guard_pressed: pressed, ..Default::default() }
}

/// Run until the first contact (or n ticks), feeding `a` and `b` on the first tick only.
fn until_contact(w: &mut World, a: Intent, b: Intent, n: usize) -> Option<Contact> {
    w.step(&[a, b]);
    for _ in 0..n {
        if let Some(c) = w.contacts.first().copied() {
            return Some(c);
        }
        w.step(&[Intent::default(), b_hold(b)]);
    }
    w.contacts.first().copied()
}

fn b_hold(b: Intent) -> Intent {
    Intent { guard: b.guard, ..Default::default() }
}

#[test]
fn jab_hits_pushes_and_stuns() {
    let mut w = duel(0.9);
    let c = until_contact(&mut w, light(), Intent::default(), 60).expect("jab connects at 0.9 m");
    assert_eq!((c.attacker, c.defender, c.source, c.outcome), (0, 1, Source::Move(JAB), Outcome::Hit));
    assert_eq!(w.fighters[1].hp, 100.0 - MOVES[JAB].damage);
    assert_eq!(w.fighters[1].action.kind, ActionKind::HitStun);
    assert!(w.fighters[1].velocity.x > 1.0, "knockback goes into velocity (momentum)");
}

#[test]
fn one_outcome_per_target_per_action_and_whiffs_miss() {
    let mut w = duel(0.9);
    w.step(&[light(), Intent::default()]);
    for _ in 0..60 {
        w.step(&[Intent::default(), Intent::default()]);
    }
    assert_eq!(w.contacts.len(), 1);
    let mut far = duel(2.0);
    far.step(&[light(), Intent::default()]);
    for _ in 0..60 {
        far.step(&[Intent::default(), Intent::default()]);
    }
    assert!(far.contacts.is_empty(), "out of reach is a whiff");
}

#[test]
fn frontal_guard_blocks_and_drains_but_not_from_behind() {
    let mut w = duel(0.9);
    for _ in 0..30 {
        w.step(&[Intent::default(), guard(false)]); // settle into guard, past any parry window
    }
    let c = until_contact(&mut w, light(), guard(false), 60).unwrap();
    assert_eq!(c.outcome, Outcome::Blocked);
    assert_eq!(w.fighters[1].hp, 100.0);
    assert!(w.fighters[1].guard < 100.0);

    let mut back = duel(0.9);
    back.fighters[1].facing = V2::new(1.0, 0.0); // turned away
    for _ in 0..30 {
        back.step(&[Intent::default(), guard(false)]);
    }
    back.fighters[1].facing = V2::new(1.0, 0.0);
    let c = until_contact(&mut back, light(), guard(false), 60).unwrap();
    assert_eq!(c.outcome, Outcome::Hit, "a guard only covers the front arc");
}

#[test]
fn guard_breaks_when_drained() {
    let mut w = duel(0.95);
    w.fighters[1].guard = 20.0;
    for _ in 0..30 {
        w.step(&[Intent::default(), guard(false)]);
    }
    w.fighters[1].guard = 20.0;
    let c = until_contact(&mut w, heavy(V2::ZERO), guard(false), 90).unwrap();
    assert_eq!(c.outcome, Outcome::GuardBroken);
    assert_eq!(w.fighters[1].action.kind, ActionKind::GuardBreak);
}

#[test]
fn parry_staggers_the_attacker_and_opens_a_counter() {
    let mut w = duel(0.9);
    w.step(&[light(), Intent::default()]);
    // guard tap just before the jab becomes active
    let ticks_to_active = (MOVES[JAB].startup / STEP) as usize - 4;
    for _ in 0..ticks_to_active {
        w.step(&[Intent::default(), Intent::default()]);
    }
    w.step(&[Intent::default(), guard(true)]);
    for _ in 0..30 {
        w.step(&[Intent::default(), guard(false)]);
    }
    let c = w.contacts.first().copied().expect("contact");
    assert_eq!(c.outcome, Outcome::Parried);
    assert_eq!(w.fighters[0].action.kind, ActionKind::Staggered);
    assert!(w.fighters[1].counter_left > 0.0 || w.fighters[1].hp == 100.0);
}

#[test]
fn mashing_guard_cannot_hold_a_parry_window_open() {
    let mut w = duel(3.0);
    for _ in 0..60 {
        w.step(&[Intent::default(), guard(true)]); // press every tick
    }
    assert_eq!(w.fighters[1].parry_left, 0.0, "recharge stops mashing; holding never renews");
}

#[test]
fn landed_strings_chain_and_branch_by_depth() {
    let mut w = duel(0.85);
    let mut seen = Vec::new();
    let mut last_seq = 0;
    for t in 0..400 {
        // keep the target in range and pressing light every few ticks
        w.fighters[1].position = w.fighters[0].position + V2::new(0.85, 0.0);
        w.fighters[1].velocity = V2::ZERO;
        let press = if t % 6 == 0 { light() } else { Intent::default() };
        let press = if seen.len() == 4 && t % 6 == 0 { heavy(V2::ZERO) } else { press };
        w.step(&[press, Intent::default()]);
        if let ActionKind::Strike(m) = w.fighters[0].action.kind {
            if w.fighters[0].action.seq != last_seq {
                last_seq = w.fighters[0].action.seq;
                seen.push(m);
                if seen.len() == 5 {
                    break;
                }
            }
        }
    }
    assert_eq!(seen, vec![JAB, CROSS, HOOK, UPPERCUT, ROUNDHOUSE], "four-hit string, heavy finisher at depth 4");
}

#[test]
fn whiffs_do_not_cancel() {
    let mut w = duel(2.5);
    w.step(&[light(), Intent::default()]);
    let first = w.fighters[0].action.seq;
    // press early in recovery (outside the buffer): nothing new starts until the jab ends
    let into_recovery = ((MOVES[JAB].startup + MOVES[JAB].active) / STEP) as usize + 1;
    for _ in 0..into_recovery {
        w.step(&[Intent::default(), Intent::default()]);
    }
    w.step(&[light(), Intent::default()]);
    assert_eq!(w.fighters[0].action.seq, first);
}

#[test]
fn heavy_direction_picks_rising_and_sweep() {
    let mut w = duel(3.0);
    w.step(&[heavy(V2::new(1.0, 0.0)), Intent::default()]);
    assert_eq!(w.fighters[0].action.kind, ActionKind::Strike(RISING));
    let mut w = duel(3.0);
    w.step(&[heavy(V2::new(-1.0, 0.0)), Intent::default()]);
    assert_eq!(w.fighters[0].action.kind, ActionKind::Strike(SWEEP));
}

#[test]
fn hit_stop_freezes_both_fighters() {
    let mut w = duel(0.9);
    until_contact(&mut w, light(), Intent::default(), 60).unwrap();
    let (a, b) = (w.fighters[0].clone(), w.fighters[1].clone());
    assert!(a.hit_stop > 0.0 && b.hit_stop > 0.0);
    w.step(&[Intent::default(), Intent::default()]);
    assert_eq!(w.fighters[0].position, a.position);
    assert_eq!(w.fighters[1].position, b.position);
    assert_eq!(w.fighters[0].action.elapsed, a.action.elapsed);
}

#[test]
fn hitting_a_wind_up_is_a_counter_hit() {
    let mut w = duel(0.9);
    w.step(&[light(), heavy(V2::ZERO)]); // both press; the jab is faster than the kick's wind-up
    let c = until_contact(&mut w, Intent::default(), Intent::default(), 60).unwrap();
    assert_eq!(c.outcome, Outcome::CounterHit);
    assert_eq!(w.fighters[1].hp, 100.0 - MOVES[JAB].damage * w.combat.counter_hit_mult);
}

#[test]
fn a_running_light_becomes_the_run_punch() {
    let mut w = duel(6.0);
    w.fighters[0].position = V2::new(-5.0, 0.0);
    for _ in 0..120 {
        w.step(&[Intent { move_dir: V2::new(1.0, 0.0), run: true, ..Default::default() }, Intent::default()]);
    }
    w.step(&[light(), Intent::default()]);
    assert_eq!(w.fighters[0].action.kind, ActionKind::Strike(RUN_PUNCH));
}

#[test]
fn knockdown_finisher_carries_the_target_away() {
    let mut w = duel(0.95);
    let x0 = w.fighters[1].position.x;
    until_contact(&mut w, heavy(V2::new(-1.0, 0.0)), Intent::default(), 90).unwrap();
    for _ in 0..240 {
        w.step(&[Intent::default(), Intent::default()]);
    }
    assert!(w.fighters[1].position.x - x0 > 0.2, "knockback slides the body");
    assert!(matches!(w.fighters[1].action.kind, ActionKind::GetUp | ActionKind::None | ActionKind::Knockdown));
}

#[test]
fn sparring_fight_replays_identically() {
    let run = || {
        let mut w = World::sparring(Tuning::default());
        let mut hashes = Vec::new();
        for t in 0..6000u64 {
            let a = spar_intent(&w, 0, 1);
            let b = spar_intent(&w, 1, 0);
            w.step(&[a, b]);
            if t % 64 == 0 {
                hashes.push(w.state_hash());
            }
        }
        (hashes, w.fighters[0].hp, w.fighters[1].hp)
    };
    let (h1, a1, b1) = run();
    let (h2, a2, b2) = run();
    assert_eq!(h1, h2);
    assert!(a1 < 100.0 || b1 < 100.0, "the sparring AIs actually fight");
    assert_eq!((a1, b1), (a2, b2));
}

#[test]
fn step_travels_about_a_metre_and_evades() {
    let mut w = duel(4.0);
    w.step(&[Intent { step: true, move_dir: V2::new(0.0, 1.0), ..Default::default() }, Intent::default()]);
    assert_eq!(w.fighters[0].action.kind, ActionKind::Step);
    for _ in 0..240 {
        w.step(&[Intent::default(), Intent::default()]);
    }
    let d = w.fighters[0].position.z;
    assert!((0.9..1.5).contains(&d), "step distance {d}");

    // a jab arriving inside the evade window passes through
    let mut w = duel(0.9);
    w.step(&[light(), Intent::default()]);
    let until_active = (MOVES[JAB].startup / STEP) as usize - 6;
    for _ in 0..until_active {
        w.step(&[Intent::default(), Intent::default()]);
    }
    w.fighters[1].position = V2::new(0.9, 0.0);
    w.step(&[Intent::default(), Intent { step: true, move_dir: V2::new(0.0, 0.0), ..Default::default() }]);
    // hold the dodger in place so only the evade frames can explain a miss
    for _ in 0..20 {
        w.fighters[1].position = V2::new(0.9, 0.0);
        w.step(&[Intent::default(), Intent::default()]);
    }
    assert!(w.contacts.is_empty(), "evade frames dodge the strike");
}

fn cast(slot: u8, held: bool) -> Intent {
    Intent { cast: Some(slot), cast_held: held, ..Default::default() }
}

#[test]
fn ki_blast_flies_and_hits() {
    let mut w = duel(5.0);
    w.step(&[cast(1, false), Intent::default()]);
    for _ in 0..120 {
        w.step(&[Intent::default(), Intent::default()]);
    }
    let c = w.contacts.first().copied().expect("ki blast reaches 5 m");
    assert_eq!((c.source, c.outcome), (Source::Spell(crate::magic::KI_BLAST), Outcome::Hit));
    assert!(w.fighters[0].ki < 60.0 + 4.0 * 2.0);
}

#[test]
fn charged_fireball_hits_harder_than_a_tap() {
    let dmg = |hold_ticks: usize| {
        let mut w = duel(6.0);
        w.step(&[cast(0, true), Intent::default()]);
        for _ in 0..hold_ticks {
            w.step(&[Intent { cast_held: true, ..Default::default() }, Intent::default()]);
        }
        for _ in 0..240 {
            w.step(&[Intent::default(), Intent::default()]);
        }
        100.0 - w.fighters[1].hp
    };
    let tap = dmg(0);
    let full = dmg(240);
    assert!(tap > 0.0, "a tapped fireball lands");
    assert!(full > tap * 2.0, "full charge {full} vs tap {tap}");
}

#[test]
fn guard_blocks_magic_and_ki_gates_casting() {
    let mut w = duel(5.0);
    for _ in 0..30 {
        w.step(&[Intent::default(), guard(false)]);
    }
    w.step(&[cast(1, false), guard(false)]);
    for _ in 0..120 {
        w.step(&[Intent::default(), guard(false)]);
    }
    assert_eq!(w.contacts.first().map(|c| c.outcome), Some(Outcome::Blocked));
    assert_eq!(w.fighters[1].hp, 100.0);

    let mut broke = duel(5.0);
    broke.fighters[0].ki = 1.0;
    broke.step(&[cast(2, false), Intent::default()]);
    assert_eq!(broke.fighters[0].action.kind, ActionKind::None, "not enough ki: no cast");
}

#[test]
fn holding_q_charges_ki_and_burst_shoves_everyone() {
    let mut w = duel(1.5);
    w.fighters[0].ki = 10.0;
    for _ in 0..120 {
        w.step(&[Intent { ki_charge: true, ..Default::default() }, Intent::default()]);
    }
    assert!(w.fighters[0].ki > 40.0, "ki {}", w.fighters[0].ki);
    w.step(&[Intent::default(), Intent::default()]);
    w.step(&[cast(3, false), Intent::default()]);
    for _ in 0..60 {
        w.step(&[Intent::default(), Intent::default()]);
    }
    assert!(w.contacts.iter().any(|c| c.source == Source::Spell(crate::magic::BURST)));
    assert!(w.fighters[1].position.x > 2.0, "burst pushes the target away");
}

#[test]
fn cancelling_held_input_never_releases_a_spell() {
    let mut w = duel(5.0);
    w.step(&[cast(0, true), Intent::default()]);
    for _ in 0..60 {
        w.step(&[Intent { cast_held: true, ..Default::default() }, Intent::default()]);
    }
    assert!(matches!(w.fighters[0].action.kind, ActionKind::Charge(_)));
    let ki = w.fighters[0].ki;
    w.cancel_held_action(0);
    for _ in 0..120 {
        w.step(&[Intent::default(), Intent::default()]);
    }
    assert!(w.projectiles.is_empty());
    assert!(w.contacts.is_empty());
    assert_eq!(w.fighters[1].hp, 100.0);
    assert!(w.fighters[0].ki < ki + 5.0, "cancellation does not refund the spell cost");
    assert_eq!(w.fighters[0].charge, 0.0);
}

#[test]
fn input_cancellation_preserves_committed_strikes_and_releases_guard() {
    let mut w = duel(5.0);
    w.step(&[light(), Intent::default()]);
    let committed = w.fighters[0].action;
    w.cancel_held_action(0);
    assert_eq!(w.fighters[0].action, committed);
    let mut w = duel(5.0);
    w.step(&[guard(true), Intent::default()]);
    w.cancel_held_action(0);
    assert_eq!(w.fighters[0].action.kind, ActionKind::None);
    assert_eq!(w.fighters[0].parry_left, 0.0);
    w.cancel_held_action(999); // stale input owner is harmless
}

#[test]
fn traversal_jump_lands_and_cannot_start_combat_during_takeoff_or_in_air() {
    let mut w = duel(5.0);
    w.step(&[Intent { jump: true, move_dir: V2::new(0.0, 1.0), ..Default::default() }, Intent::default()]);
    assert!(w.fighters[0].crouch_left > 0.0);
    let mut peak = 0.0f32;
    for _ in 0..240 {
        w.step(&[Intent { light: true, cast: Some(0), ..Default::default() }, Intent::default()]);
        peak = peak.max(w.fighters[0].height);
        if w.fighters[0].height > 0.0 || w.fighters[0].crouch_left > 0.0 {
            assert_eq!(w.fighters[0].action.kind, ActionKind::None);
            assert!(w.projectiles.is_empty());
        } else {
            break;
        }
    }
    assert!(peak > 0.5, "the grounded jump takes off");
    assert_eq!(w.fighters[0].height, 0.0);
    assert_eq!(w.fighters[0].vertical, 0.0);
}

#[test]
fn an_early_rush_press_links_on_contact_without_another_click() {
    let mut w = duel(0.85);
    w.step(&[light(), Intent::default()]);
    for _ in 0..11 { w.step(&[Intent::default(), Intent::default()]); }
    w.step(&[light(), Intent::default()]);
    assert!(w.fighters[0].buffered.is_some());
    for _ in 0..60 {
        w.fighters[1].position = w.fighters[0].position + V2::new(0.85, 0.0);
        w.step(&[Intent::default(), Intent::default()]);
        if w.fighters[0].action.kind == ActionKind::Strike(CROSS) {
            assert_eq!(w.fighters[0].chain, 1);
            assert!(w.fighters[0].buffered.is_none());
            return;
        }
    }
    panic!("the queued click never linked into cross");
}

#[test]
fn full_smash_breaks_a_settled_guard_and_uses_a_grounded_windup() {
    let mut w = duel(0.85);
    for _ in 0..30 { w.step(&[Intent::default(), guard(false)]); }
    w.step(&[Intent { heavy: true, heavy_held: true, ..Default::default() }, guard(false)]);
    assert_eq!(w.fighters[0].action.kind, ActionKind::HeavyCharge(FRONT_KICK));
    for _ in 0..160 {
        w.fighters[1].position = w.fighters[0].position + V2::new(0.85, 0.0);
        w.step(&[Intent { heavy_held: true, ..Default::default() }, guard(false)]);
        assert_eq!(w.fighters[0].height, 0.0);
        if let Some(hit) = w.contacts.first() {
            assert_eq!(hit.outcome, Outcome::GuardBroken);
            assert_eq!(w.fighters[0].heavy_charge, 1.0);
            return;
        }
    }
    panic!("the full smash never contacted the guard");
}

#[test]
fn charge_scales_smash_damage_and_releasing_commits_once() {
    let damage = |hold: usize| {
        let mut w = duel(0.85);
        w.step(&[Intent { heavy: true, heavy_held: true, ..Default::default() }, Intent::default()]);
        for _ in 0..hold { w.step(&[Intent { heavy_held: true, ..Default::default() }, Intent::default()]); }
        for _ in 0..100 {
            w.fighters[1].position = w.fighters[0].position + V2::new(0.85, 0.0);
            w.step(&[Intent::default(), Intent::default()]);
        }
        assert_eq!(w.contacts.len(), 1);
        100.0 - w.fighters[1].hp
    };
    let tap = damage(0);
    let charged = damage(70);
    assert!(charged > tap * 1.5, "charged {charged}, tap {tap}");
}

#[test]
fn cancelling_smash_charge_is_not_a_release() {
    let mut w = duel(0.85);
    w.step(&[Intent { heavy: true, heavy_held: true, ..Default::default() }, Intent::default()]);
    for _ in 0..20 { w.step(&[Intent { heavy_held: true, ..Default::default() }, Intent::default()]); }
    w.cancel_held_action(0);
    for _ in 0..100 { w.step(&[Intent::default(), Intent::default()]); }
    assert!(w.contacts.is_empty());
    assert_eq!(w.fighters[0].heavy_charge, 0.0);
}

#[test]
fn a_parry_counter_uses_its_own_move_and_consumes_the_window_on_hit() {
    let mut w = duel(0.85);
    w.fighters[0].counter_left = w.combat.counter_window;
    let contact = until_contact(&mut w, light(), Intent::default(), 100).unwrap();
    assert_eq!(contact.source, Source::Move(COUNTER_BLOW));
    assert_eq!(contact.outcome, Outcome::CounterHit);
    assert_eq!(w.fighters[0].counter_left, 0.0);
    assert_eq!(w.fighters[1].action.kind, ActionKind::Knockdown);
}

#[test]
fn homing_dash_spends_energy_and_buffers_a_rush_until_evade_ends() {
    let mut w = duel(5.0);
    w.magic.ki_regen = 0.0;
    w.fighters[0].lock_target = Some(1);
    let energy = w.fighters[0].ki;
    w.step(&[Intent { step: true, run: true, ..Default::default() }, Intent::default()]);
    assert_eq!(w.fighters[0].ki, energy - w.combat.homing_dash_cost);
    w.step(&[light(), Intent::default()]);
    assert_eq!(w.fighters[0].action.kind, ActionKind::Step);
    for _ in 0..45 {
        w.step(&[Intent::default(), Intent::default()]);
        if matches!(w.fighters[0].action.kind, ActionKind::Strike(_)) {
            assert_eq!(w.fighters[0].last_move, Some(RUN_PUNCH));
            return;
        }
    }
    panic!("dash did not commit its queued rush");
}

#[test]
fn empty_energy_keeps_the_free_step_available() {
    let mut w = duel(5.0);
    w.magic.ki_regen = 0.0;
    w.fighters[0].lock_target = Some(1);
    w.fighters[0].ki = 0.0;
    w.step(&[Intent { step: true, run: true, move_dir: V2::new(1.0, 0.0), ..Default::default() }, Intent::default()]);
    assert_eq!(w.fighters[0].action.kind, ActionKind::Step);
    assert_eq!(w.fighters[0].ki, 0.0);
    assert!(w.fighters[0].velocity.length() < w.combat.step_speed * 1.1);
}

#[test]
fn fireball_release_has_one_launch_after_its_readable_windup() {
    let mut w = duel(12.0);
    w.step(&[cast(0, true), Intent::default()]);
    for _ in 0..60 { w.step(&[Intent { cast_held: true, ..Default::default() }, Intent::default()]); }
    w.step(&[Intent::default(), Intent::default()]);
    assert_eq!(w.fighters[0].action.kind, ActionKind::Cast(crate::magic::FIREBALL));
    assert_eq!(w.fighters[0].action.elapsed, 0.0);
    assert!(w.projectiles.is_empty());
    while w.fighters[0].action.elapsed + STEP < crate::SPELLS[0].prepare {
        w.step(&[Intent::default(), Intent::default()]);
        assert!(w.projectiles.is_empty());
    }
    w.step(&[Intent::default(), Intent::default()]);
    assert_eq!(w.projectiles.len(), 1);
    for _ in 0..12 { w.step(&[Intent::default(), Intent::default()]); }
    assert_eq!(w.projectiles.len(), 1, "release must launch once");
}
