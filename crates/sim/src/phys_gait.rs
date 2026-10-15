//! Physics: lean and foot placement for the controller (PHYSICS_MATH.md §17). Pure, per tick.
//! Lean: tan φ = a/g with a = (v_want − v)/τ. Placement: capture point + k·(v − v_want).
//! A step fires when the capture point leaves the support region (balance-driven, no clock).
use crate::math::Vec3;

const G: f64 = 9.81;

#[derive(Clone, Copy, Debug)]
pub struct GaitParams {
    pub tau: f64,
    pub k_place: f64,
    pub max_lean: f64,
    pub half_stance: f64,
    pub support_r: f64,
    pub max_step: f64,
}
impl Default for GaitParams {
    fn default() -> Self {
        GaitParams {
            tau: 0.3,
            k_place: 0.1,
            max_lean: 0.35,
            half_stance: 0.1,
            support_r: 0.08,
            max_step: 0.9,
        }
    }
}

/// `foot` (0 = L, 1 = R) should lift now and swing to `target` (world, floor y). Not a touchdown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepEvent {
    pub foot: usize,
    pub target: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct GaitCmd {
    pub lean: f64,
    pub lean_dir: Vec3,
    pub com_target: Vec3,
    pub step: Option<StepEvent>,
    /// Foot waiting to lift while weight shifts onto the other one. Pass back next tick as `pending`.
    pub pending: Option<usize>,
}

/// Release threshold: the swing foot lifts only when its contact share is below this.
pub const UNLOAD_SHARE: f64 = 0.2;

fn horiz(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// One tick. `v_want`: wanted horizontal velocity (world). `feet`: world foot positions; `feet_down`;
/// `swinging`: a foot already in swing (no new event until it lands). `heading_right`: unit +side.
pub fn gait_cmd(
    com: Vec3,
    com_v: Vec3,
    v_want: Vec3,
    rest_com_h: f64,
    feet: [Vec3; 2],
    feet_down: [bool; 2],
    swinging: bool,
    just_landed: Option<usize>,
    share: [f64; 2],
    pending: Option<usize>,
    heading_right: Vec3,
    floor_y: f64,
    p: GaitParams,
) -> GaitCmd {
    let v = horiz(com_v);
    let vw = horiz(v_want);
    let a = (vw - v) * (1.0 / p.tau);
    let al = a.length();
    let lean = (al / G).atan().min(p.max_lean);
    let lean_dir = if al > 1e-9 {
        a * (1.0 / al)
    } else {
        Vec3::default()
    };
    let w = (rest_com_h / G).sqrt();
    let cp = horiz(com) + v * w;
    let com_target = Vec3::new(cp.x, rest_com_h, cp.z);
    let mut step = None;
    // Running/walking: the swing foot's touchdown immediately sends the other foot (no double support needed).
    // From standing: both feet down, nothing swinging, capture point outside the support region.
    let pick = if pending.is_some() {
        pending
    } else if let Some(f) = just_landed {
        let moving = vw.length() > 0.05 || (cp - horiz(feet[f])).length() > p.support_r;
        if moving { Some(1 - f) } else { None }
    } else if !swinging && feet_down[0] && feet_down[1] {
        let mid = horiz((feet[0] + feet[1]) * 0.5);
        if (cp - mid).length() > p.support_r {
            // swing the foot farther from the capture point
            Some(
                if (horiz(feet[0]) - cp).length() >= (horiz(feet[1]) - cp).length() {
                    0
                } else {
                    1
                },
            )
        } else {
            None
        }
    } else {
        None
    };
    let mut pending_out = None;
    let mut com_target = com_target;
    if let Some(foot) = pick {
        if feet_down[foot] && share[foot] >= UNLOAD_SHARE {
            // Weight shift: hold the step, move the COM target over the stance foot.
            let st = horiz(feet[1 - foot]);
            com_target = Vec3::new(st.x, rest_com_h, st.z);
            pending_out = Some(foot);
        } else {
            let side = if foot == 0 { 1.0 } else { -1.0 };
            let mut tgt = cp + (v - vw) * p.k_place + horiz(heading_right) * (side * p.half_stance);
            let stance = horiz(feet[1 - foot]);
            let off = (tgt - stance).clamp_length(p.max_step);
            tgt = stance + off;
            step = Some(StepEvent {
                foot,
                target: Vec3::new(tgt.x, floor_y, tgt.z),
            });
        }
    }
    GaitCmd {
        lean,
        lean_dir,
        com_target,
        step,
        pending: pending_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn feet() -> [Vec3; 2] {
        [Vec3::new(0.1, 0.0, 0.0), Vec3::new(-0.1, 0.0, 0.0)]
    }
    #[test]
    fn still_no_step() {
        let c = gait_cmd(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::default(),
            Vec3::default(),
            1.0,
            feet(),
            [true; 2],
            false,
            None,
            [0.5, 0.5],
            None,
            Vec3::new(1.0, 0.0, 0.0),
            0.0,
            GaitParams::default(),
        );
        assert!(c.step.is_none() && c.lean == 0.0);
    }
    #[test]
    fn push_steps_forward() {
        let c = gait_cmd(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            1.0,
            feet(),
            [true; 2],
            false,
            None,
            [0.5, 0.5],
            None,
            Vec3::new(1.0, 0.0, 0.0),
            0.0,
            GaitParams::default(),
        );
        assert!(c.step.is_none() && c.pending.is_some()); // loaded: shift first
        let f = c.pending.unwrap();
        let mut sh = [0.9, 0.9];
        sh[f] = 0.1;
        let c2 = gait_cmd(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            1.0,
            feet(),
            [true; 2],
            false,
            None,
            sh,
            c.pending,
            Vec3::new(1.0, 0.0, 0.0),
            0.0,
            GaitParams::default(),
        );
        let s = c2.step.unwrap();
        assert_eq!(s.foot, f);
        assert!(s.target.z > 0.2);
    }
    #[test]
    fn want_speed_leans_forward() {
        let c = gait_cmd(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::default(),
            Vec3::new(0.0, 0.0, 2.0),
            1.0,
            feet(),
            [true; 2],
            false,
            None,
            [0.5, 0.5],
            None,
            Vec3::new(1.0, 0.0, 0.0),
            0.0,
            GaitParams::default(),
        );
        assert!(c.lean > 0.3 && c.lean_dir.z > 0.99);
    }
    #[test]
    fn run_touchdown_sends_other_foot() {
        let c = gait_cmd(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 4.0),
            Vec3::new(0.0, 0.0, 4.0),
            1.0,
            feet(),
            [true, false],
            true,
            Some(0),
            [1.0, 0.0],
            None,
            Vec3::new(1.0, 0.0, 0.0),
            0.0,
            GaitParams::default(),
        );
        assert_eq!(c.step.unwrap().foot, 1);
    }
}

/// SIMBICON stance-hip pelvis control (Yin et al. 2007). The pelvis has no motor, so the swing hip
/// tracks a world-frame target and the stance hip absorbs the rest: τ_stance = −τ_trunk − τ_swing.
/// `tau_trunk` = τ_pelvis_want + τ_spine, where τ_pelvis_want is a world-frame PD on the pelvis
/// (upright + heading) and τ_spine is the spine motor torque (its −τ reaction lands on the pelvis).
/// Sign: child +τ, parent −τ, so pelvis net = −(τ_spine + τ_hL + τ_hR) = τ_pelvis_want.
/// In double support, split by contact share s (stance gets s, the other hip 1 − s of the correction).
/// Returns (τ_hip_L, τ_hip_R) to *replace* the PD hip torques. `swing`: None in double support.
pub fn simbicon_hips(
    tau_trunk: Vec3,
    tau_hip: [Vec3; 2],
    swing: Option<usize>,
    share: [f64; 2],
) -> [Vec3; 2] {
    match swing {
        Some(sw) => {
            let st = 1 - sw;
            let mut o = tau_hip;
            o[st] = -(tau_trunk + tau_hip[sw]);
            o
        }
        None => {
            let total = -(tau_trunk);
            let s = share[0] + share[1];
            let (a, b) = if s > 1e-9 {
                (share[0] / s, share[1] / s)
            } else {
                (0.5, 0.5)
            };
            [total * a, total * b]
        }
    }
}

#[cfg(test)]
mod simbicon_tests {
    use super::*;
    #[test]
    fn pelvis_net_zero() {
        let tt = Vec3::new(10.0, 2.0, -3.0);
        let sw = Vec3::new(-4.0, 1.0, 5.0);
        let h = simbicon_hips(tt, [Vec3::default(), sw], Some(1), [1.0, 0.0]);
        let net = tt + h[0] + h[1];
        assert!(net.length() < 1e-12);
        let d = simbicon_hips(tt, [Vec3::default(); 2], None, [0.5, 0.5]);
        assert!((tt + d[0] + d[1]).length() < 1e-12);
    }
}
