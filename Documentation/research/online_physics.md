# Online research: physics sources for pass 6 (ranked)
Compiled 2026-10-15 by Grok Bot for Physics. Every URL below was loaded with WebFetch (or downloaded as a PDF) today.
Section/equation numbers are given ONLY where I read the actual text; everything else cites the abstract/landing page. [team] = our own inference.
PDFs saved (author-hosted, free copies): `papers/pdf/stable_pd.pdf`, `papers/pdf/capture_point_pratt2006.pdf` (text in `papers/txt/`).
SIMBICON and Coros PDFs timed out from the box (TLS/timeouts on cs.ubc.ca), so they are cited from their landing pages only.

## 1. Stable Proportional-Derivative Controllers — Tan, Liu, Turk, IEEE CG&A 31(4), 2011
URL: https://faculty.cc.gatech.edu/~turk/my_papers/stable_pd.pdf (mirror http://www.jie-tan.net/project/spd.pdf). Plugs into: **motor writer**.
- §3.1 Eq. 3/5: compute PD torque from the *predicted next state* (q + Δt·q̇, q̇ + Δt·q̈) instead of the current one → arbitrarily high gains stay stable at large Δt (they show 33 ms explicit Euler).
- Eq. 10–11 (multi-DOF): q̈ = (M + Kd·Δt)⁻¹(−C − Kp(q + q̇Δt − q̄) − Kd·q̇ + τext). For us [team]: per-joint diagonal approximation using each capsule's effective inertia about the joint is the cheap first version.
- Stability rule stated in the text: they saw no stability problems with **Kd ≥ Kp·Δt** (at 120 Hz: Kd ≥ Kp/120); keyframe example uses kd = 10·kp·Δt (§4.2).
- §4.3 Eq. 14–15: a joint torque between two bodies is applied as **τ2 = −τ1** — the same paired +τ/−τ rule we want; anything unpaired is a net external torque (our slide suspect).

## 2. Capture Point: A Step toward Humanoid Push Recovery — Pratt, Carff, Drakunov, Goswami, Humanoids 2006, pp. 200–207
URL: https://www.cs.cmu.edu/~cga/legs/Pratt_Goswami_Humanoids2006.pdf. Plugs into: **balance (split_balance)**.
- §V.C, Eq. 7–10: Linear Inverted Pendulum ẍ = (g/z0)·x; capture point x_capture = ẋ·√(z0/g) (per horizontal axis). With z0 ≈ 1.0 m, g = 9.81: ICP = com_xz + v_xz·0.319 s.
- Def. 3–4: if the capture point lies inside the support polygon the body can stop without stepping → ankle strategy; otherwise hip/flywheel or step [team: our ankle/hip split].
- Flywheel model (abstract, §V.D–E): trunk rotation (centroidal angular momentum) enlarges the capture region → justifies the hip channel in split_balance, saturated by a hip torque cap.

## 3. Generalized Biped Walking Control — Coros, Beaudoin, van de Panne, SIGGRAPH 2010 (ACM TOG 29(4) Art. 130)
URL: https://www.cs.ubc.ca/~van/papers/2010-TOG-gbwc/index.html (PDF linked there; download timed out). Plugs into: **gravity_ff, motor writer**.
- Abstract: PD tracking + inverted-pendulum foot placement + **gravity and velocity-error compensation via Jacobian-transpose control**. That is exactly our gravity_ff: for each body i, τ_joint += Jᵢᵀ·(−mᵢ·g) along the chain from stance foot to body i [team detail: sum over the chain, applied as paired torques].
- Abstract: "requires no character-specific tuning" — supports keeping gains in a per-joint table scaled by supported mass rather than hand-tuned per bone [team].

## 4. SIMBICON: Simple Biped Locomotion Control — Yin, Loken, van de Panne, SIGGRAPH 2007 (ACM TOG 26(3) Art. 105)
URL: https://www.cs.ubc.ca/~van/papers/Simbicon.htm. Code: JSimbicon Java applet linked there (licence not stated → ideas only). Plugs into: **balance, motor writer**.
- Abstract: low-gain PD tracking + balance feedback; robust to pushes (350 N, 0.2 s) and parameter variation.
- From the well-known method (not re-read today, so not cited to a section): torso and swing hip are controlled in *world* frame, and the stance-hip torque is computed as the negative of torso+swing-hip so the trunk is held up — our trunk-collapse fix candidate [team: verify in PDF before quoting].
- Abstract: feedback-error learning adds a predictive (feedforward) torque so gains can stay low → our feedforward term.

## 5. Avian 0.7.0 docs — `dynamics::ccd` module
URL: https://docs.rs/avian3d/latest/avian3d/dynamics/ccd/index.html (page shows Version 0.7.0). Licence: Avian is MIT/Apache-2.0 [not re-checked today]. Plugs into: **contact / fall-through**.
- Speculative contacts are on by default; per-entity `SpeculativeMargin`, global `NarrowPhaseConfig`.
- `SweptCcd::default()` (NonLinear = includes rotation) is the "never tunnel" safety net — put it on feet and shins; can combine with speculative.
- Docs advise thicker colliders than necessary and avoiding trimeshes for dynamic contact → thick box floor, and greedy-box voxel colliders rather than trimesh.
- Caveat: large speculative margins can cause ghost collisions; swept CCD causes "time loss".

## 6. Avian PR #651 "Also clamp angular and linear velocities to max speeds while solving constraints" (open, 2025)
URL: https://github.com/avianphysics/avian/pull/651. Plugs into: **runaway / fall-through**.
- Describes joint-constrained chains cascading velocity to huge values then NaN ("objects disappear") — matches our COM −46 m event.
- Important: in released Avian `MaxAngularSpeed`/`MaxLinearSpeed` are applied at velocity integration only, NOT during contact solving (the PR is unmerged). So a 50 rad/s cap helps but is not a guarantee; also add a NaN/|v| watchdog that logs and freezes.

## 7. Solver2D — Erin Catto, Box2D blog, 2024-02-05
URL: https://box2d.org/posts/2024/02/solver2d/. Plugs into: **contact, joint softness tuning**.
- Soft constraints parameterised by hertz + damping ratio; keep hertz well below the step rate (≤30 Hz at 60 Hz per Nyquist) → at 120 Hz keep motor/contact stiffness ≲ 60 Hz [team scaling].
- Accumulated-impulse clamping ("clamp the total impulse, not the iteration's") is crucial to avoid jitter — same idea for our one-clamp-per-joint.
- Sub-stepping beats more iterations; strong, anchored friction is key to stable resting (TGS_Sticky) → relevant to the slide.
- Baumgarte can create energy/jitter; relaxation pass removes it → if contact flicker persists, check Avian's contact softness settings before filtering.

## 8. Cracking destruction — Dennis Gustafsson, Voxagon blog, 2014-05-13
URL: https://blog.voxagon.se/2014/05/13/cracking-destruction.html. Plugs into: **voxel store / debris split**.
- After a carve, recompute connected components (id-merge over touching shapes) to find pieces that fall off.
- Break *inside* the step: solve with capped impulses on breakables, break where the cap is hit, re-solve (≤3 solves/step) → breakage keeps momentum instead of "collide then split".
- For determinism [team]: run the split at the tick boundary from the logged VoxelEdit, components ordered by min voxel index.

## 9. Teardown interview — Software Engineering Daily ep. 1772 (Gustafsson, 2024), transcript
URL: https://softwareengineeringdaily.com/wp-content/uploads/2024/12/SED1772-Teardown.txt. Plugs into: **voxel store / debris**.
- [0:05:20] each object has its own voxel grid that moves freely — dynamic debris = new rigid body with its own local grid.
- [0:07:02] voxels make destruction robust: integer grid maths, no degenerate triangles; ~10 cm voxels.
- [0:10:50, 0:22:48] no structural integrity simulated; only connectivity — and connectivity search over huge objects was a hard problem they had to solve after early access → bound our flood fill to the edited chunk + neighbours [team].

## 10. Meshing in a Minecraft Game — Mikola Lysenko, 0fps.net, 2012-06-30
URL: https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/. Code: author's demo on GitHub; author says in comments it is free for commercial use (no formal licence checked). Plugs into: **greedy box colliders**.
- Greedy merge: sweep, take the lexicographically first quad, grow width then height, remove, repeat; linear time; ≤ 8× optimal (proved in post).
- Cache results; re-run a fast mesher on edit and a better one when idle (we must keep the collider output deterministic per tick [team]).
- A commenter notes the same method works for building a smaller set of collision cubes — our 3D box-merge extension is [team].

## Not found / not verified (do not cite)
- Euphoria/NaturalMotion and Ubisoft/EA active-ragdoll GDC talks: no freely loadable primary source verified today.
- Pratt's Virtual Model Control paper: not fetched; Coros (#3) covers the Jacobian-transpose idea we need.
- Inertia from voxels: standard sum of box inertias + parallel-axis theorem [team]; no source fetched.

## Top 3 to apply to pass 6
1. **Stable PD in the motor writer (Tan et al., §3.1 Eq. 3/5, §4.3 Eq. 14–15).** Replace τ = −kp(q−q̄) − kd·q̇ with τ = −kp(q + Δt·q̇ − q̄) − kd(q̇ + Δt·q̈_est) with q̈_est from the per-joint diagonal form of Eq. 11 (q̈ = (−kp(q+q̇Δt−q̄) − kd·q̇ + τ_ff)/(I_eff + kd·Δt)), Δt = 1/120, kd ≥ kp/120. Apply each result once as +τ child / −τ parent. Expected: high stiffness without the 7k–22k saturation spikes, and no net external torque (slide).
2. **Jacobian-transpose gravity compensation + capture-point split (Coros 2010 abstract; Pratt 2006 Eq. 10).** gravity_ff: for every body, add Jᵀ(−m·g) to each joint on its path back to the stance foot(s), split between feet by vertical load. Balance: ICP = com_xz + v_xz·√(z0/g); ankle torque ∝ (ICP − CoP_center) while ICP is inside the foot polygon; spill the remainder to hips (flywheel). Fixes the 1.03 → 0.15 m collapse.
3. **Anti-tunnelling bundle (Avian 0.7 CCD docs + PR #651).** `SweptCcd::default()` on feet/shins, thick box floor, `MaxAngularSpeed(50.0)` per capsule, plus our own per-tick watchdog (non-finite or |ω| > 60 rad/s → log tick and freeze), because the released clamp is not applied inside contact solving. Fixes the −46 m fall-through and gives a first-bad-tick log.
