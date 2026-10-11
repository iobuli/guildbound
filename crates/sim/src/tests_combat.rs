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
    assert_eq!((c.attacker, c.defender, c.mv, c.outcome), (0, 1, JAB, Outcome::Hit));
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
