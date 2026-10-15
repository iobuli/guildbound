#[path = "../src/anim_ability.rs"] mod anim_ability;
#[path = "../src/anim_flow.rs"] mod anim_flow;
#[path = "../src/anim_flare.rs"] mod anim_flare;
#[path = "../src/anim_acro.rs"] mod anim_acro;
#[path = "../src/anim_rig.rs"] mod anim_rig;
#[path = "../src/anim_abilities.rs"] mod anim_abilities;
use crate::anim_ability::*;
use crate::anim_abilities::*;
use crate::anim_flow::StrikePhases;

fn ctx(speed: f32, phase: f32, ax: f32) -> AnimCtx<'static> {
    AnimCtx { rig: RigId::Master, bone_count: 79, bone_names: &[], speed, accel: Vec3([0.0, 0.0, ax]), yaw_rate: 0.5,
              gait_phase: phase, contacts: Contacts::default(), dt: 1.0 / 120.0 }
}
fn finite(o: &AnimOutput) -> bool {
    o.bones.iter().all(|(_, b)| b.rot.0.iter().chain(b.omega.0.iter()).chain(b.alpha.0.iter()).all(|x| x.is_finite()))
}
fn twist(q: [f32; 4]) -> f32 { 2.0 * q[0].atan2(q[3]) }

#[test]
fn modes_priorities_windows() {
    let c = ctx(5.0, 0.3, 2.0);
    let g = GaitAbility { id: 1, stride_fn: default_stride, leg_len: 0.88, frame: Frame::default() };
    let s = StrikeFlowAbility { id: 2, phases: StrikePhases { strike: 20, recovery: 35, len: 80 }, hit_confirmed: true };
    let f = FlareAbility::new(3);
    let a = AcroAbility::handspring(4, true);
    let (og, os, of, oa) = (g.targets(&c, 10), s.targets(&c, 25), f.targets(&c, 26), a.targets(&c, 100));
    assert_eq!((og.priority, os.priority, of.priority, oa.priority), (Priority::Effort, Priority::Style, Priority::Style, Priority::Style));
    assert!(matches!(og.mode, LayerMode::Masked(_)) && of.mode == LayerMode::Additive && oa.mode == LayerMode::Replace);
    let sw: Vec<u64> = (0..80).filter(|&t| s.targets(&c, t).strike_window).collect();
    let fw: Vec<u64> = (0..80).filter(|&t| f.targets(&c, t).strike_window).collect();
    println!("strike_flow window ticks {:?}..{:?} ({}), flare window {:?}..{:?} ({}), cancel {:?}",
             sw.first(), sw.last(), sw.len(), fw.first(), fw.last(), fw.len(), s.cancel_window(0));
    assert_eq!(sw.len(), 15);
    let cw = s.cancel_window(0).unwrap();
    assert!(!sw.iter().any(|t| *t >= cw.0 && *t < cw.1));
    assert!(s.finished(80) && !s.finished(79));
    assert!([&og, &os, &of, &oa].iter().all(|o| finite(o)));
    // registry: Style before Effort
    let mut r = AbilityRegistry::default();
    r.register(Box::new(GaitAbility { id: 1, stride_fn: default_stride, leg_len: 0.88, frame: Frame::default() }));
    r.register(Box::new(FlareAbility::new(3)));
    r.apply_event(&AbilityEvent::Start { id: 1, rig: RigId::Master, params: vec![] });
    r.apply_event(&AbilityEvent::Start { id: 3, rig: RigId::Master, params: vec![] });
    let out = r.collect(&c, 30);
    assert_eq!(out.iter().map(|o| o.priority).collect::<Vec<_>>(), vec![Priority::Style, Priority::Effort]);
}

#[test]
fn omega_matches_hinge_rate() {
    // acro: bone angles are hinges about X, so omega_x should equal the finite-difference angle rate
    let a = AcroAbility::handspring(4, false);
    let c = ctx(0.0, 0.0, 0.0);
    let mut worst: f32 = 0.0; let mut peak: f32 = 0.0;
    for t in 2..a.clip.poses.len() as u64 - 2 {
        let o = a.targets(&c, t);
        let (p, m) = (a.targets(&c, t + 1), a.targets(&c, t - 1));
        for ((b, bt), ((_, bp), (_, bm))) in o.bones.iter().zip(p.bones.iter().zip(m.bones.iter())) {
            let mut d = twist(bp.rot.0) - twist(bm.rot.0);
            if d > std::f32::consts::PI { d -= std::f32::consts::TAU } else if d < -std::f32::consts::PI { d += std::f32::consts::TAU }
            let rate = d * 60.0;
            worst = worst.max((bt.omega.0[0] - rate).abs()); peak = peak.max(rate.abs());
            assert!(bt.omega.0[1].abs() < 1e-3 && bt.omega.0[2].abs() < 1e-3, "bone {b}");
        }
    }
    println!("acro omega vs hinge rate: worst err {worst:.2e} rad/s, peak {peak:.2} rad/s");
    assert!(worst < 1e-2 * peak.max(1.0));
}

#[test]
fn root_motion_sums_to_travel_and_finishes() {
    for a in [AcroAbility::handspring(4, false), AcroAbility::handspring(5, true), AcroAbility::backflip(6, false), AcroAbility::backflip(7, true)] {
        let c = ctx(0.0, 0.0, 0.0);
        let n = a.clip.poses.len() as u64;
        let (mut z, mut y) = (0.0f64, 0.0f64);
        for t in 0..n { let r = a.targets(&c, t).root.unwrap(); z += r.dpos.0[2] as f64; y += r.dpos.0[1] as f64; }
        let tz = a.clip.poses[n as usize - 1].pelvis[0] - a.clip.poses[0].pelvis[0];
        let ty = a.clip.poses[n as usize - 1].pelvis[1] - a.clip.poses[0].pelvis[1];
        println!("{}: {} ticks, root dpos sum z {z:.4} (clip {tz:.4}), y {y:.5} (clip {ty:.5})", a.name(), n);
        assert!((z - tz).abs() < 1e-4 && (y - ty).abs() < 1e-4);
        assert!(a.finished(n) && !a.finished(n - 1));
    }
}

#[test]
fn gait_and_flare_numbers_and_determinism() {
    let g = GaitAbility { id: 1, stride_fn: default_stride, leg_len: 0.88, frame: Frame::default() };
    for v in [1.5f32, 4.0, 7.0] {
        let amp = (0..120).map(|i| twist(g.targets(&ctx(v, i as f32 / 120.0, 0.0), 0).bones[0].1.rot.0).abs()).fold(0.0, f32::max);
        let o = g.targets(&ctx(v, 0.0, 0.0), 0);
        println!("gait {v} m/s: thigh swing amp {:.1} deg, thigh omega at phase 0 {:.2} rad/s, root dz/tick {:.4}", amp.to_degrees(), o.bones[0].1.omega.0[0], o.root.unwrap().dpos.0[2]);
    }
    let f = FlareAbility::new(3);
    let c = ctx(6.0, 0.25, 3.0);
    let peak_w = (0..90).flat_map(|t| f.targets(&c, t).bones).map(|(_, b)| b.omega.0.iter().map(|x| x.abs()).fold(0.0, f32::max)).fold(0.0, f32::max);
    println!("flare peak |omega| component {peak_w:.2} rad/s; finished at tick {:?}", (0..400u64).find(|&t| f.finished(t)));
    for t in 0..200u64 {
        let (a, b) = (f.targets(&c, t), f.targets(&c, t));
        assert!(finite(&a));
        for ((ba, x), (bb, y)) in a.bones.iter().zip(&b.bones) {
            assert_eq!(ba, bb);
            assert!(x.rot.0.iter().chain(x.omega.0.iter()).chain(x.alpha.0.iter()).zip(y.rot.0.iter().chain(y.omega.0.iter()).chain(y.alpha.0.iter())).all(|(p, q)| p.to_bits() == q.to_bits()));
        }
    }
}
