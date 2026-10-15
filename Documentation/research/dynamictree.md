# DynamicTree: what Fable can use (Physics)

Source: DynamicTree: Interactive Real Tree Animation via Sparse Voxel Spectrum (2025). No licence is stated in the PDF, so use the ideas only and reuse none of the code.

## Use
1. **Modal sway for props and foliage** [§4 p5, Eq.6, Eq.7]. Each mode is a 1-DOF oscillator, m·q'' + c·q' + k·q = f, and the shape is rebuilt as D = Σ φk·qk. Cheap: K modes per object, a few floats each.
   - Our change [team]: **not** explicit Euler (the paper's choice, §4 p5), because it gains energy on stiff modes. Use semi-implicit Euler (stable for ω·dt < 2, so any mode under about 38 Hz at 120 Hz) or the exact damped closed-form step (always stable).
   - Determinism [team]: fixed 1/120 s, fixed mode order, f32 state stored in our own sim state, and forces only from logged events (hits, wind gusts as events). That keeps replay and the hash exact.
2. **Hit response from the combat event** [team, using Eq.6]. A strike that hits a prop adds an impulse to q' for the modes, projected onto each mode shape at the hit point. Same hit event Animation uses, no second source.
3. **Mode shapes from voxels** [§3.2 p3 idea; team]. Mode shapes are stored per sparse voxel, and every vertex in a voxel shares it. That fits the shared voxel store: mode data per chunk, read-only, derived offline.

## Where it plugs in
- New sim module (Physics-owned, proposed name `phys_modal`): `ModalState { q, qd }` per object, plus `step_modal(state, params, impulses, dt)`. It runs in the fixed step **after** `PhysicsSystems::Last` (render-only, never fed back into Avian).
- Render side: the vertex shader or CPU deform reads D(t). This belongs to rendering/UI, not physics.
- Mode shapes and frequencies are baked offline into the asset. A hand-tuned first mode per prop (one frequency, one damping ratio) is enough to start.

## Doesn't apply
- The ML spectrum generator and diffusion model [§3 p3–4]: offline at most, licence unknown, never at runtime.
- 3D Gaussian splatting binding [§3.3]: we don't render with Gaussians.
- MPM simulation (the baseline they compare against, §1): too heavy and not needed.
- Anything on characters: sway is for props and foliage only. Bodies stay on Avian with the motor writer.
