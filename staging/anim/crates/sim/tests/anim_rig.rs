#[path = "../src/anim_rig.rs"]
mod anim_rig;
use anim_rig::*;

const X: V = [1.0, 0.0, 0.0];
const PAIRS: [(&str, &str); 6] = [
    ("B_Pelvis", "B_L_Thigh"), ("B_L_Thigh", "B_L_Calf"), ("B_L_Clavicle", "B_L_UpperArm"),
    ("B_L_UpperArm", "B_L_Forearm"), ("B_Pelvis", "B_Spine"), ("B_R_UpperArm", "B_R_Forearm"),
];

fn load(file: &str) -> Option<Rig> {
    let p = format!("{}/tests/fixtures/{file}", env!("CARGO_MANIFEST_DIR"));
    match std::fs::read_to_string(&p) { Ok(t) => Some(Rig::parse(&t).unwrap()), Err(_) => { println!("SKIP: {p} missing (run tools/anim_rig_dump.ps1)"); None } }
}

/// Apply parent flex a and child flex b (model X) as local deltas, FK, and measure both back.
fn check(rig: &Rig, parent: usize, child: usize, a: f64, b: f64) -> (f64, f64) {
    let (pn, cn) = (rig.joints[parent], rig.joints[child]);
    let lp = qmul(rig.to_local_pre(parent, axis_angle(X, a)), rig.nodes[pn].r);
    let lc = qmul(rig.to_local_pre(child, axis_angle(X, b)), rig.nodes[cn].r);
    let ov = |k: usize| if k == pn { Some(lp) } else if k == cn { Some(lc) } else { None };
    let (wp, _) = rig.compute_world(pn, &ov);
    let (wc, _) = rig.compute_world(cn, &ov);
    let dp = qmul(wp, qconj(rig.world_rest(parent).0)); // parent's model-space delta
    let dc_rel = qmul(qconj(dp), qmul(wc, qconj(rig.world_rest(child).0))); // child delta relative to parent's motion
    ((twist_angle(dp, X) - a).abs(), (twist_angle(dc_rel, X) - b).abs())
}

fn run(file: &str) -> Option<Rig> {
    let rig = load(file)?;
    println!("== {file}: {} nodes, {} joints", rig.nodes.len(), rig.joints.len());
    let mut worst: f64 = 0.0;
    for (p, c) in PAIRS {
        let (Some(pj), Some(cj)) = (rig.joint_by_name(p), rig.joint_by_name(c)) else { println!("   missing {p}/{c}"); continue };
        for &(a, b) in &[(0.3, 0.9), (-0.5, 1.4), (1.2, -0.7), (2.0, 2.4)] {
            let (ea, eb) = check(&rig, pj, cj, a, b);
            worst = worst.max(ea).max(eb);
        }
        let (ax, m) = dominant(rig.local_axis(cj, X));
        let l = rig.local_axis(cj, X);
        println!("   {c:<14} flexes about local {ax} (purity {m:.4}; axis [{:.3},{:.3},{:.3}])", l[0], l[1], l[2]);
    }
    println!("   worst flexion error {worst:.2e} rad");
    assert!(worst < 1e-4);
    Some(rig)
}

#[test]
fn master_and_mannequin() {
    let m = run("anim_rig_master.txt");
    let q = run("anim_rig_mannequin.txt");
    if let (Some(m), Some(q)) = (m, q) {
        let mut diffs = 0;
        for j in 0..m.joints.len() {
            let name = &m.nodes[m.joints[j]].name;
            match q.joint_by_name(name) {
                None => println!("   mannequin lacks {name}"),
                Some(k) => {
                    let (a, b) = (m.world_rest(j).0, q.world_rest(k).0);
                    let d = qmul(a, qconj(b));
                    let ang = 2.0 * d[3].abs().min(1.0).acos();
                    if ang > 1e-3 { diffs += 1; println!("   rest frame differs: {name} {:.2} deg (joint {j} vs {k})", ang.to_degrees()); }
                }
            }
        }
        println!("   {diffs} joints with differing world rest rotation (>0.06 deg)");
    }
}

#[test]
fn synthetic_rig() {
    // Root rotated -90 about X (Z-up export) with a twisted child chain.
    let txt = "node 0 -1 -1 0 0 0 -0.7071067811865476 0 0 0.7071067811865476 Armature\n\
               node 1 0 0 0 0 1 0.3 0.2 0.1 0.927 B_Pelvis\n\
               node 2 1 1 0.1 -0.05 0 0.1 -0.5 0.3 0.806 B_L_Thigh\n\
               node 3 2 2 0 0.44 0 0.05 0.1 -0.6 0.79 B_L_Calf\n";
    let rig = Rig::parse(txt).unwrap();
    let (ea, eb) = check(&rig, 0, 1, 0.7, -1.1);
    let (ec, ed) = check(&rig, 1, 2, -0.4, 2.2);
    println!("synthetic errors {ea:.1e} {eb:.1e} {ec:.1e} {ed:.1e}");
    assert!(ea.max(eb).max(ec).max(ed) < 1e-9);
    // post form equals pre form applied to the rest local
    let d = axis_angle(X, 0.8);
    let a = qmul(rig.to_local_pre(1, d), rig.nodes[2].r);
    let b = qmul(rig.nodes[2].r, rig.to_local_post(1, d));
    assert!(a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-12));
}
