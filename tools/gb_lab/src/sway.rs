//! Scene "sway": DynamicTree s4 Eq.6 modal oscillators, m q'' + c q' + k q = f.
//! Semi-implicit Euler (Physics' phys_modal.rs) vs exact damped step (--exact). Render-only: no Avian.
use crate::phys_modal::{apply_impulse, displacement, step_modal, Mode, ModalState};
use crate::{Args, DT};
use std::io::Write;

const HIT_TICK: u32 = 60;

/// Exact step of an underdamped oscillator over dt with zero force (closed form).
fn step_exact(s: &mut ModalState, modes: &[Mode], dt: f32) {
    for k in 0..modes.len() {
        let m = modes[k];
        let w = 2.0 * std::f64::consts::PI * m.freq_hz as f64;
        let z = m.zeta as f64;
        let wd = w * (1.0 - z * z).sqrt();
        let (q0, v0) = (s.q[k] as f64, s.qd[k] as f64);
        let dt = dt as f64;
        let e = (-z * w * dt).exp();
        let (c, sn) = ((wd * dt).cos(), (wd * dt).sin());
        let a = q0;
        let b = (v0 + z * w * q0) / wd;
        let q = e * (a * c + b * sn);
        let v = e * ((-z * w) * (a * c + b * sn) + (-a * wd * sn + b * wd * c));
        s.q[k] = q as f32;
        s.qd[k] = v as f32;
    }
}

fn energy(s: &ModalState, modes: &[Mode]) -> f64 {
    modes.iter().enumerate().map(|(k, m)| {
        let w = 2.0 * std::f64::consts::PI * m.freq_hz as f64;
        0.5 * m.mass as f64 * (s.qd[k] as f64).powi(2) + 0.5 * m.mass as f64 * w * w * (s.q[k] as f64).powi(2)
    }).sum()
}

fn simulate(args: &Args, mut log: Option<&mut std::fs::File>) -> (Vec<u32>, f64, f64, f64, u32) {
    let modes = [
        Mode { freq_hz: 1.0, zeta: 0.05, mass: 1.0 },
        Mode { freq_hz: 3.0, zeta: 0.05, mass: 1.0 },
        Mode { freq_hz: 8.0, zeta: 0.05, mass: 1.0 },
    ];
    let phi_hit = [1.0, 0.5, 0.25];
    let phi = [[1.0, 0.0, 0.0], [0.0, 0.0, 0.5], [0.25, 0.0, 0.0]];
    // three props, hit with different impulses
    let mut props: Vec<ModalState> = (0..3).map(|_| ModalState::new(3)).collect();
    let mut bits = Vec::new();
    let (mut e_peak, mut max_rise, mut max_rise_vs_peak) = (0.0f64, 0.0f64, 0.0f64);
    let mut rises = 0u32;
    let mut prev_e = 0.0f64;
    let zero = [0.0f32; 3];
    for t in 0..args.ticks {
        if t == HIT_TICK {
            for (i, p) in props.iter_mut().enumerate() {
                apply_impulse(p, &modes, &phi_hit, 0.5 * (i + 1) as f32);
            }
            prev_e = props.iter().map(|p| energy(p, &modes)).sum();
            e_peak = prev_e;
        }
        for p in props.iter_mut() {
            if args.exact { step_exact(p, &modes, DT) } else { step_modal(p, &modes, &zero, DT) }
        }
        let e: f64 = props.iter().map(|p| energy(p, &modes)).sum();
        if t >= HIT_TICK {
            if e > prev_e { rises += 1; max_rise = max_rise.max(e - prev_e); }
            if e > e_peak { max_rise_vs_peak = max_rise_vs_peak.max(e - e_peak); }
            prev_e = e;
        }
        for p in &props { for q in &p.q { bits.push(q.to_bits()); } }
        if let Some(f) = log.as_deref_mut() {
            let d = displacement(&props[0], &phi);
            let _ = writeln!(f, "{{\"tick\":{t},\"energy\":{e:.9},\"q0\":[{},{},{}],\"disp0\":[{},{},{}]}}",
                props[0].q[0], props[0].q[1], props[0].q[2], d[0], d[1], d[2]);
        }
    }
    let e_final: f64 = props.iter().map(|p| energy(p, &modes)).sum();
    (bits, e_peak, e_final, max_rise.max(max_rise_vs_peak), rises)
}

pub fn run(args: &Args) -> String {
    let mut file = args.out.as_ref().map(|p| std::fs::File::create(p).expect("open --out"));
    let (a, e_peak, e_final, max_rise, rises) = simulate(args, file.as_mut());
    let (b, _, _, _, _) = simulate(args, None);
    let identical = a == b;
    let mut h: u64 = 0xcbf29ce484222325;
    for x in &a { h ^= *x as u64; h = h.wrapping_mul(0x100000001b3); }
    let pass = identical && rises == 0;
    format!("{{\"scene\":\"sway\",\"stepper\":\"{}\",\"ticks\":{},\"hit_tick\":{HIT_TICK},\"energy_after_hit\":{e_peak:.6},\"energy_final\":{e_final:.6},\"ticks_energy_rose\":{rises},\"max_energy_rise\":{max_rise:.3e},\"bit_identical_two_runs\":{identical},\"q_hash\":\"{h:016x}\",\"pass\":{pass}}}",
        if args.exact { "exact_damped" } else { "semi_implicit_euler" }, args.ticks)
}
