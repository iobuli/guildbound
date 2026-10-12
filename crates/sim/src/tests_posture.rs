use crate::combat::*;
use crate::notice::NoticeState;
use crate::rpg::*;
use crate::*;

fn duel(gap: f32) -> World {
    let mut w = World::sparring(Tuning::default());
    w.fighters[0] = Fighter::new(V2::new(0.0, 0.0), V2::new(1.0, 0.0), 100.0);
    w.fighters[1] = Fighter::new(V2::new(gap, 0.0), V2::new(-1.0, 0.0), 100.0);
    w
}

fn jab() -> Intent {
    Intent { light: true, ..Default::default() }
}

fn run_until_contact(w: &mut World, defender: Intent) -> Option<Contact> {
    for _ in 0..90 {
        w.step(&[Intent::default(), defender]);
        if let Some(c) = w.contacts.pop() {
            return Some(c);
        }
    }
    None
}

#[test]
fn hits_and_blocks_build_posture_which_drains_after_a_wait() {
    let mut w = duel(0.9);
    w.step(&[jab(), Intent::default()]);
    let c = run_until_contact(&mut w, Intent::default()).expect("jab lands");
    assert_eq!(c.outcome, Outcome::Hit);
    let after_hit = w.fighters[1].posture;
    assert!((after_hit - MOVES[JAB].guard_drain * w.posture.hit_share).abs() < 1e-4);
    assert!((c.damage - MOVES[JAB].damage).abs() < 1e-4, "contact carries the damage dealt");
    // still waiting: no drain yet
    for _ in 0..60 {
        w.step(&[]);
    }
    assert_eq!(w.fighters[1].posture, after_hit);
    for _ in 0..240 {
        w.step(&[]);
    }
    assert_eq!(w.fighters[1].posture, 0.0, "drains once the wait is over");
}

#[test]
fn being_parried_breaks_posture_and_the_next_strike_is_a_deathblow() {
    let mut w = duel(0.9);
    w.fighters[0].posture = 80.0;
    w.step(&[jab(), Intent::default()]);
    // the defender taps guard just before contact
    let mut c = None;
    for k in 0..90 {
        let guard = Intent { guard_pressed: k == 4, guard: k >= 4, ..Default::default() };
        w.step(&[Intent::default(), guard]);
        if let Some(x) = w.contacts.pop() {
            c = Some(x);
            break;
        }
    }
    assert_eq!(c.unwrap().outcome, Outcome::Parried);
    assert_eq!(w.fighters[0].action.kind, ActionKind::Dizzy, "80 + parried cost fills posture");
    assert!(w.fighters[1].posture < w.posture.max, "parrying never breaks the parrier");

    // the parrier strikes the dizzy attacker once its hit-stop ends: deathblow
    while w.fighters[1].hit_stop > 0.0 {
        w.step(&[]);
    }
    let mut c = None;
    for k in 0..120 {
        let i = if k == 0 { jab() } else { Intent::default() };
        w.step(&[Intent::default(), i]);
        if let Some(x) = w.contacts.pop() {
            c = Some(x);
            break;
        }
    }
    let c = c.expect("strike on dizzy lands");
    assert_eq!(c.outcome, Outcome::Deathblow);
    assert_eq!(w.fighters[0].hp, 0.0);
}

#[test]
fn dizzy_wears_off_to_half_posture() {
    let mut w = duel(3.0);
    w.add_posture(1, 200.0, false, false);
    assert_eq!(w.fighters[1].action.kind, ActionKind::Dizzy);
    for _ in 0..((w.posture.dizzy_from_damage / STEP) as usize + 2) {
        w.step(&[]);
    }
    assert_eq!(w.fighters[1].action.kind, ActionKind::None);
    assert_eq!(w.fighters[1].posture, w.posture.max * w.posture.after_dizzy);
}

#[test]
fn an_unaware_mob_struck_from_behind_goes_down_in_one() {
    let mut w = duel(3.0);
    // mob at 0.8 m ahead of the player, facing away from them
    let m = mobs::spawn(&mut w, V2::new(0.8, 0.0), 100.0);
    w.fighters[m].facing = V2::new(1.0, 0.0);
    w.fighters[1].position = V2::new(-5.0, 5.0);
    w.step(&[jab()]);
    let mut c = None;
    for _ in 0..90 {
        w.step(&[]);
        if let Some(x) = w.contacts.iter().find(|c| c.defender == m) {
            c = Some(*x);
            break;
        }
    }
    assert_eq!(c.expect("backstab lands").outcome, Outcome::Deathblow);
    assert_eq!(w.fighters[m].hp, 0.0);
}

#[test]
fn an_alert_mob_is_not_backstabbed() {
    let mut w = duel(3.0);
    let m = mobs::spawn(&mut w, V2::new(0.8, 0.0), 100.0);
    w.fighters[m].facing = V2::new(1.0, 0.0);
    w.fighters[1].position = V2::new(-5.0, 5.0);
    w.alarm(m, 0);
    w.step(&[jab()]);
    let mut c = None;
    for _ in 0..90 {
        w.step(&[]);
        if let Some(x) = w.contacts.iter().find(|c| c.defender == m) {
            c = Some(*x);
            break;
        }
    }
    assert_eq!(c.unwrap().outcome, Outcome::Hit);
}

#[test]
fn notice_fills_in_the_cone_ignores_behind_and_hears_running() {
    let mut w = duel(30.0);
    let m = mobs::spawn(&mut w, V2::new(0.0, 0.0), 100.0);
    w.fighters[m].facing = V2::new(1.0, 0.0);
    // player 7 m ahead, inside the outer cone but outside the inner one
    w.fighters[0].position = V2::new(7.0, 0.5);
    w.fighters[1].position = V2::new(-30.0, 0.0);
    for _ in 0..60 {
        w.step(&[]);
    }
    let n = w.fighters[m].notice.unwrap();
    assert!(n.level > 0.0 && n.state == NoticeState::Unaware, "slowly getting suspicious: {n:?}");
    for _ in 0..1200 {
        w.step(&[]);
    }
    assert_eq!(w.fighters[m].notice.unwrap().state, NoticeState::Alert);

    // behind and walking: never noticed
    let mut w = duel(30.0);
    let m = mobs::spawn(&mut w, V2::new(0.0, 0.0), 100.0);
    w.fighters[m].facing = V2::new(1.0, 0.0);
    w.fighters[0].position = V2::new(-2.0, 0.0);
    w.fighters[1].position = V2::new(-30.0, 0.0);
    for _ in 0..240 {
        w.step(&[]);
    }
    assert_eq!(w.fighters[m].notice.unwrap().level, 0.0);
    // behind and running close: heard
    for _ in 0..240 {
        w.fighters[0].velocity = V2::new(0.0, 4.0);
        w.step(&[]);
        w.fighters[0].position = V2::new(-2.0, 0.0);
    }
    assert!(w.fighters[m].notice.unwrap().level > 0.0);
}

#[test]
fn an_inner_cone_sighting_is_an_instant_alert() {
    let mut w = duel(30.0);
    let m = mobs::spawn(&mut w, V2::new(0.0, 0.0), 100.0);
    w.fighters[m].facing = V2::new(1.0, 0.0);
    w.fighters[0].position = V2::new(2.0, 0.0);
    w.step(&[]);
    assert_eq!(w.fighters[m].notice.unwrap().state, NoticeState::Alert);
}

#[test]
fn a_drink_cut_short_keeps_the_item() {
    let mut w = duel(0.9);
    w.fighters[1].hp = 50.0;
    w.fighters[1].kit.bag = vec![HEALING_DRAUGHT];
    w.step(&[jab(), Intent { bag: Some(BagAction::Use(0)), ..Default::default() }]);
    assert!(matches!(w.fighters[1].action.kind, ActionKind::Drink(_)));
    for _ in 0..140 {
        w.step(&[]);
    }
    assert_eq!(w.fighters[1].kit.bag, vec![HEALING_DRAUGHT], "hit before the commit point");
    assert!(w.fighters[1].hp < 50.0);
}

#[test]
fn crits_follow_the_crit_stat_and_replay_identically() {
    let mut crits = 0;
    for gap in 0..200 {
        let mut w = duel(0.9);
        w.tick = gap * 7;
        w.fighters[0].kit.worn[2] = Some(DUELIST_GLOVES);
        w.step(&[jab(), Intent::default()]);
        let c = run_until_contact(&mut w, Intent::default()).unwrap();
        if c.crit {
            crits += 1;
            assert!(c.damage > MOVES[JAB].damage * 1.05 * 1.5);
        }
    }
    assert!((10..=55).contains(&crits), "about 15% of 200: {crits}");
    let mut a = duel(0.9);
    let mut b = duel(0.9);
    a.fighters[0].kit.worn[2] = Some(DUELIST_GLOVES);
    b.fighters[0].kit.worn[2] = Some(DUELIST_GLOVES);
    for k in 0..400 {
        let i = if k % 50 == 0 { jab() } else { Intent::default() };
        a.step(&[i]);
        b.step(&[i]);
    }
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn moving_cancels_late_recovery_only() {
    let mut w = duel(3.0);
    w.step(&[jab()]);
    let d = MOVES[JAB];
    let walk = Intent { move_dir: V2::new(0.0, 1.0), ..Default::default() };
    // moving during startup changes nothing
    w.step(&[walk]);
    assert!(matches!(w.fighters[0].action.kind, ActionKind::Strike(_)));
    let late = d.startup + d.active + d.recovery * w.posture.move_cancel;
    while w.fighters[0].action.elapsed < late {
        w.step(&[]);
    }
    w.step(&[walk]);
    assert_eq!(w.fighters[0].action.kind, ActionKind::None);
}

#[test]
fn a_perfect_parry_answers_with_a_heavy_counter_that_knocks_down() {
    let mut w = duel(0.9);
    w.step(&[jab(), Intent::default()]);
    let mut parried = false;
    for k in 0..200 {
        let guard = Intent { guard_pressed: k == 4, guard: (4..10).contains(&k), ..Default::default() };
        w.step(&[Intent::default(), guard]);
        parried |= w.contacts.iter().any(|c| c.outcome == Outcome::Parried);
        if w.contacts.iter().any(|c| c.attacker == 1 && c.defender == 0) {
            break;
        }
    }
    assert!(parried);
    assert_eq!(w.fighters[1].last_move, Some(COUNTER_BLOW), "the parrier counters by itself");
    assert!(matches!(w.fighters[0].action.kind, ActionKind::Knockdown | ActionKind::Dizzy), "{:?}", w.fighters[0].action.kind);
}

#[test]
fn fighting_enters_combat_mode_which_lingers_then_ends() {
    let mut w = duel(3.0);
    assert!(!w.in_combat(0));
    w.step(&[jab(), Intent::default()]);
    assert!(w.in_combat(0));
    for _ in 0..((w.combat.combat_linger / STEP) as usize + 5) {
        w.step(&[]);
    }
    assert!(!w.in_combat(0));
    w.fighters[0].lock_target = Some(1);
    assert!(w.in_combat(0), "lock-on is combat");
}

#[test]
fn relaxed_movement_walks_runs_and_sprints_on_a_held_run_out_of_combat() {
    let mut w = duel(30.0);
    w.tuning.relaxed = true;
    w.tuning.arena_half_size = 60.0;
    let go = Intent { move_dir: V2::new(0.0, 1.0), ..Default::default() };
    for _ in 0..240 {
        w.step(&[go]);
    }
    assert!((w.fighters[0].velocity.length() - w.tuning.walk_speed).abs() < 0.1, "the stick walks");
    let held = (w.tuning.sprint_after_s / crate::STEP) as usize;
    for _ in 0..held.saturating_sub(10) {
        w.step(&[Intent { run: true, ..go }]);
    }
    assert!((w.fighters[0].velocity.length() - w.tuning.run_speed).abs() < 0.15, "run runs before the hold");
    for _ in 0..240 {
        w.step(&[Intent { run: true, ..go }]);
    }
    assert!((w.fighters[0].velocity.length() - w.tuning.sprint_speed).abs() < 0.1, "a held run sprints");
    w.step(&[go]);
    assert_eq!(w.fighters[0].run_held, 0.0, "letting go resets the hold");
    w.fighters[0].lock_target = Some(1);
    for _ in 0..240 {
        w.step(&[go]);
    }
    assert!((w.fighters[0].velocity.length() - w.tuning.walk_speed).abs() < 0.1, "combat walk");
}
