//! Physics: modal sway for props and foliage (DynamicTree §4 p5, Eq.6–7), stepped with
//! semi-implicit Euler at a fixed dt instead of the paper's explicit Euler. Render-only, deterministic.

/// One mode: natural frequency (Hz), damping ratio, modal mass.
#[derive(Clone, Copy, Debug)]
pub struct Mode { pub freq_hz: f32, pub zeta: f32, pub mass: f32 }

/// Per-object modal state, stored in our own sim state (hashable).
#[derive(Clone, Debug, Default)]
pub struct ModalState { pub q: Vec<f32>, pub qd: Vec<f32> }

impl ModalState {
    pub fn new(n: usize) -> Self { Self { q: vec![0.0; n], qd: vec![0.0; n] } }
}

/// True when semi-implicit Euler is stable for this mode at dt (ω·dt < 2, with margin 1.9).
pub fn mode_stable(m: Mode, dt: f32) -> bool { 2.0 * core::f32::consts::PI * m.freq_hz * dt < 1.9 }

/// Add a logged hit impulse: each mode gets `j · phi_k_at_hit / mass_k` on its velocity.
pub fn apply_impulse(s: &mut ModalState, modes: &[Mode], phi_at_hit: &[f32], j: f32) {
    for k in 0..modes.len().min(s.qd.len()).min(phi_at_hit.len()) {
        s.qd[k] += j * phi_at_hit[k] / modes[k].mass.max(1e-6);
    }
}

/// One fixed step: m·q'' + c·q' + k·q = f, velocity first then position (semi-implicit).
/// Modes that would be unstable at dt are frozen at rest rather than allowed to blow up.
pub fn step_modal(s: &mut ModalState, modes: &[Mode], force: &[f32], dt: f32) {
    for k in 0..modes.len().min(s.q.len()) {
        let m = modes[k];
        if !mode_stable(m, dt) { s.q[k] = 0.0; s.qd[k] = 0.0; continue; }
        let w = 2.0 * core::f32::consts::PI * m.freq_hz;
        let f = force.get(k).copied().unwrap_or(0.0) / m.mass.max(1e-6);
        let acc = f - 2.0 * m.zeta * w * s.qd[k] - w * w * s.q[k];
        s.qd[k] += acc * dt;
        s.q[k] += s.qd[k] * dt;
    }
}

/// Rebuild displacement at one point: D = Σ φk·qk (Eq.7). `phi` is [mode][xyz].
pub fn displacement(s: &ModalState, phi: &[[f32; 3]]) -> [f32; 3] {
    let mut d = [0.0f32; 3];
    for (k, p) in phi.iter().enumerate().take(s.q.len()) {
        for a in 0..3 { d[a] += p[a] * s.q[k]; }
    }
    d
}
