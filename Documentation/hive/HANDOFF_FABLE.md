# Handoff for Fable - Guildbound standing / ragdoll review

Outside review of the Guildbound repo. Stack: Rust, Bevy 0.19.1, Avian 0.7.0, bevy_egui 0.40.1. Branch `build2.0`, commit `8a94ca9` local (more commits landing). Main source of truth: `/workspace/GUILDBOUND_BRIEF.md` (summarised below; read that file first). Also skim `/workspace/pass5_metrics.jsonl` and `/workspace/hive/README.md`.

## 1. Goal
- T-pose ragdoll body with powered joint motors that track animation clips as targets.
- Event-sourced fixed 1/120 s sim in `crates/sim`.
- Boot into a locked sandbox; F2 Studio edits bones live.

## 2. Pipeline and single motor writer
- Fixed 120 Hz order (brief s3): events into `event_log` -> `apply_edits` (only pose writer; layers by Priority) -> contact -> one motor writer (one torque per joint per tick, one clamp) -> Avian step -> metrics/trails.
- Motor: theta_err from local_target vs capsule world; tau = inertia terms + Kp/Kd + tau_hold + tau_bal; apply +tau child / -tau parent. Avian joint motors off on the sandbox body.
- One clamp: legs k*g*(c*M_above+(1-c)*m_below)*l_com; arms margin*(I*alpha_eff+m*g*l_com). Ankle cap = c*share*M*g*d_edge.
- Conventions: targets parent-relative, f64, fixed dt 1/120; motor writer converts with `rates_to_world_rel`; 13-capsule chain targets via `chain_target`.

## 3. Results
- Pass 4: fixed 120 Hz ok; com sinks 1.03 -> ~0.7; feet thrown ~1.3 m from per-function 294 N m caps.
- Pass 5: single writer builds; rest theta_err 3e-7; but com 1.03 -> 0.15 in 1-2 s with balance on and off; ~3 m slide; ~11.6 contact flips/s; peak_sat hundreds to ~16000. Invalid test: `nc_idle` clip drove targets and the model switched ~8 times mid-run. Frames `/workspace/pass5_frames/`, log `/workspace/pass5_metrics.jsonl`.

## 4. Known bugs
- Clip overwrites Pose edits (edits must be top layer in `apply_edits`).
- `anim_rig` hook gives every node a joint index -> 79 not 65 (should be index in `skin.joints`, else -1), shifting BoneIdx.
- Unexplained 3 m slide (suspect unpaired torque/force: old pelvis assist or `leg_hold`).
- Contact flips; ankle cap -> 0 on lift makes peak_sat meaningless; no gravity feed-forward (staged now).
- Animation open: acro poses not converted to real rig frames (handspring too far/fast); `anim_bspline` unverified, not wired.

Root causes ranked (Physics):
1. No gravity compensation + nothing holding the trunk (com 1.03 -> 0.15).
2. Unpaired torque/force -> 3 m slide.
3. Contact flicker ~11.6/s drives ankle caps to 0; peak_sat meaningless.

## 5. Pass 6 plan
- Rest-only mode (model locked, no clips, rest pose target, agent lock off).
- Pose edits = top layer in `apply_edits`.
- Fix 79 vs 65 hook; gravity_ff (c-blended stance/swing); split_balance ankles first then stance hips; all inside the one clamp.
- Contact via `phys_balance::Contact` (0.25/tick filter; on <1 cm, off >2.5 cm above rest height); `sat_ratio(tau, cap, c)`, cap floor 1 N m; log sum tau each tick (should be ~0).

## 6. Where code lives
- Staged from all three agents is committed under `staging/` in `build2.0` (not compiled in). Helper copies also at `/workspace/phys`, `/workspace/anim_staging`, `/workspace/gb_staging` (UI restyle + trails, not yet integrated).
- Physics staged: `phys_balance.rs` (gravity_ff, split_balance, sat_ratio(tau, cap, c), Contact, stable_pd3); `phys_mass.rs` (strike_boost).
- Animation in repo: `anim_gait`, `anim_interp`, `anim_flow`, `anim_flare`, `anim_acro`, `anim_rig`, `anim_flow_cut`. Staged to `staging/`: `anim_abilities.rs` (f64 rates, bone-id match, rates_to_world_rel, chain, chain_target), `anim_trail_target.rs`.
- In repo: `crates/sim` (event_log, phys_*, anim_*), `crates/game` (studio.rs, sandbox.rs, anim.rs, possession.rs).

## 7. Rules
- No new test files; no GPL; do not delete modules; no LLM at runtime; no engine change.
- Nothing runs on daviy's PC except by the hub (The Rust Expert) after daviy's okay.

## 8. Questions for Fable
- Why does the trunk collapse and slide (~3 m) once the pelvis assist is gone?
- Is the per-joint clamp formula sane (esp. ankle c*share*M*g*d_edge and the zero-cap on lift)?
- Capsule vs skin bone mapping: is the 13-capsule / 65-skin chain + O offset approach sound, and how should the 79 vs 65 hook bug be fixed cleanly?

## 9. Paper research: what to use
Full notes in `/workspace/hive/use/` (xcube, vox-e, mdr, dynamictree, gigavoxels). No paper states a licence: ideas only, no code or weights. Own ideas are [team].
1. Sparse store with strict nesting: coarse occupancy cell exists iff any fine cell inside exists; levels built by subdivide/prune [XCube s3 p3; s3.1 p3, Fig.3]; 2-3 levels, 16^3 coarsest is enough [XCube s4.4 p8, Tab.2]. Coarse-first clearance/broad-phase query is [team].
2. Masked merge as one logged VoxelEdit: G_r = M*G_e + (1-M)*G_i [Vox-E s3.3 p5, Eq.4].
3. Deterministic graph-cut to split damaged chunks into debris vs intact, smoothness from material difference [Vox-E s3.3 p5, Eq.5; supp p12-13]; fixed-order integer max-flow [team].
4. Modal sway for props/foliage, kicked by the combat hit event: m q'' + c q' + k q = f, D = sum phi_k q_k [DynamicTree s4 p5, Eq.6-7]; semi-implicit or closed-form step, render-only [team].
5. Voxel characters: skinned coarse shell carries static voxel detail [GigaVoxels-TER s2 p3-4; Fig.4 p5; s4.4 p10]; render-only, sim keeps capsules [team].
6. Optional smooth surfaces: per-voxel PCA-SDF S_i = S_B z_i, bases baked offline, seam averaging [MDR s3.3 p6; s4.4 p10, Tab.4]; never in sim state [team].
7. Correlation diff between chunk states for replay checks [Vox-E s3.2 p4, Eq.3] [team].
8. Offline authoring only: coarse-edit-then-regenerate [XCube s4.1 p6, Fig.6], text-guided edits [Vox-E s3.2 p4], image block-out [MDR abstract p1].

Relevance verdict: none of the five papers addresses articulated-body balance, joint torques, gravity compensation, foot contact or friction, so they do not help the trunk collapse, slide or contact flicker. Pass 6 (gravity feed-forward, ankle-then-hip balance split in one clamp, contact filter/hysteresis, sum-tau leak log) is team physics [team]; see `brainstorm.md` "Relevance to the current blocker".
