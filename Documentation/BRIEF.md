# Guildbound Sim: Project Brief (single source of truth)
Owner and hub: The Rust Expert. Every decision lands here; if code disagrees with this page, the code is wrong or this page gets updated by the hub. Newer than any doc in Documentation/.

## 1. Rules (daviy)
- Nothing runs on daviy's PC (build, launch, commands) except by the hub, after daviy's okay. Others stage on their own computer and ask daviy for anything they need.
- No new test files. Existing tests may be updated. All checks happen live in the sandbox (metrics.jsonl, Studio).
- Boot straight into the locked sandbox, model in ragdoll and editable. The UI must be beautiful AND intuitive (HUD Layer Lab: navy/cyan, chamfered glowing panels, text >= 0.85 opacity).
- Remove bulk: one helper module per concern, no duplicate maths, no dead fragments.

## 2. Ownership (one owner per stage)
| Stage | Owner | Output |
|---|---|---|
| Rig, BoneIdx (65 skin joints, name-mapped to 13 capsules) | Hub | `anim_rig` |
| Pose targets, abilities, StrikePhases, TrailTargetSource | Animation | `BoneTarget{rot, omega, alpha}` parent-local, f64 internally, fixed dt 1/120 |
| Torque laws, caps, contact, balance | Physics | raw torques, `leg/arm_torque_limit`, `phys_math` |
| **Single motor writer** + integration on PC | Hub | one torque per joint per tick |
| Studio UI, panel kit, trails drawing | Grok Bot | systems split per panel, resources not globals |
| Event log, replay, undo/redo | Hub (Grok Bot's undo routes through it) | `event_log` |

## 3. Pipeline (each fixed step, 120 Hz, this order) — pass 6 v5 (maths: hive/SYSTEM_MATH.md)
Steps 1-4 run in FixedPostUpdate `.before(PhysicsSystems::First)`; steps 6+ `.after(PhysicsSystems::Last)`.
1. Input/events into `event_log` (Studio edits, BalanceSet, controller): the only source of truth.
2. `apply_edits`: the only pose writer; abilities are layers by Priority (Limits > Balance > Style > Effort). Stand test = rest only.
3. Contact: exact lowest point of each foot capsule minus per-foot rest_h (first contact, then mean of ticks 1..60; the old −2.65 cm was r·sinθ); on below 1 cm, off above 2.5 cm, 6-tick hold; c filtered 0.25/tick.
4. Motor writer: per joint, one torque per tick. Input is only apply_edits' final local (chain product parent→child, Animation's chain_target).
   - θ_err = log((R_p · O_p · local_target · O_c⁻¹) · R_c⁻¹), O from bone_world = capsule_world · O (load check |θ_err| < 1e-4 at rest)
   - transport with B_p = R_p·O_p: ω* = B_p ω_L ; α* = B_p α_L + ω_p × (B_p ω_L)
   - Stable PD, 3x3: τ = I_red·(Kp θ_err − Kd(ω_rel − ω*))/(1 + Kd dt + Kp dt²) + I_red(α* + dω_p) + gravity_ff + balance;
     I_red = (I_c⁻¹ + I_p⁻¹)⁻¹ from each body's OWN world inertia (phys_balance::reduced_inertia3/stable_pd3). Kp, Kd per unit inertia.
     ω_c × I_w ω_c is DROPPED (Avian integrates it). Tan's full −Kp dt ω_rel term: GUILDBOUND_TAN_FULL, off.
   - gravity_ff reaction form: (c_sub − p) × m_sub g up − c·(cop − p) × s M g up.
   - balance: ((kb e + kbd ė) × up)·M g, split ankles first (sole-edge cap c·s·M·g·d_edge), remainder to stance hips, entered negated on the child.
   - COM pull (GUILDBOUND_COM_PULL = k_com, 0.11 built in for the test build; `off` disables): k_com M g (com − com0)_xz × up, added to balance, × (1 − max(strike_boost, acro_active)); com0 re-anchors when acro_active (anim_acro_active, via Sandbox::start_acro) returns to 0.
   - no shaping clamp: one fault cap per joint |τ| ≤ 3·M·g N·m, clamps and logs `fault` (NaN → 0 + log).
   - Strike: Kp × 1.5 on arm joints for 6 ticks, eased to 0 by tick 9, 12-tick cooldown, each start logged. No cap is raised.
   - Apply +τ to child, −τ to parent; Στ logged every tick (torque_sum.jsonl). Avian joint motors off on the sandbox body.
   - Stance foot damping (default ON): F = −c·40·v_foot,xz (absolute), cap 0.8·s·M·g, external (not in Στ); `rel` mode optional.
   - Defaults: Kp 4000, Kd 126, kb 5, kbd 0.3, 12 substeps (global SubstepCount; 6 + contact frequency 3 = fallback, ruling open), foot damp 40.
   - Anti-tunnel: 1 m thick floor box, MaxAngularSpeed 50 rad/s per capsule, SweptCcd + speculative margin on feet/shins, first capsule below −0.1 m logged.
5. Avian step.
6. Report (metrics.jsonl every 120 ticks), then trails sampling (after report).
Nothing in this chain may live in Update. Rendering uses TransformInterpolation.

## 4. Conventions
- Load check (logged live, not a test): |θ_err| at rest pose < 1e-4 rad on every joint, else O or chain order is wrong.
- Bone order = BoneIdx everywhere (trails, targets, metrics). Ghost and actual measure the same point (joint world position).
- Sim crate stays pure (no Bevy). Game crate does serde (rig.json) and gizmos.
- Replay hash = event log + our own state + sampled target ticks; never Avian internals. Cloud numbers will be close to the PC's, not identical.

## 5. Order of work (agreed)
1. Restore the half-applied pass-5 files from target\batch_backup\*_p5.rs (needs daviy's okay).
2. Single motor writer + one clamp (Physics/Animation staged code).
3. Reliable contact (fix the sole −2.7 cm offset).
4. Stand at com y ≈ 1.0 m, first with balance off, then on. Pass mark: com y within 5 cm for 30 s, contact flips < 1/s, peak_sat < 0.9.
5. Then UI restyle + trails, then anim_trail_feed, then overnight cloud branch (needs "commit and push a work branch").

## 6. Status (hub updates)
- Pass 5: single writer in, body falls in 1-2 s (com 0.15). Invalid as a measurement: model switched ~8x, nc_idle clip drove targets, agent lock up.
- Pass 6 plan: (a) standing test = model locked, Rest-only mode (no abilities, apply_edits gets rest pose only), agent lock off; (b) Pose-page user edits = top layer in apply_edits over clips; (c) anim_rig hook: joint = index in skin.joints, else -1 (fixes 79 vs 65); (d) Physics gravity_ff (c-blended stance/swing) + capture-point balance split ankles-first then stance hips, all inside the one clamp; (e) c filtered 0.25/tick, contact on <1 cm off >2.5 cm; peak_sat = |τ|/max(cap,1), excluding c<0.2; (f) log Στ over all joints each tick (should be ~0) to find the unpaired force behind the 3 m slide.
- Pass 5 hub items: props.rs time bank consumes StepRequest; balance_tick + BALANCE_REQUEST move before PhysicsSystems::First and become a resource (no mutex); verify egui 0.40 names; toolbar_ui must sit below the Studio title bar.
- Pass 4 live: fixed 120 Hz OK; body sinks (com 1.03 to ~0.7), feet thrown up to 1.3 m. Cause: per-function 294 N·m caps. Fix = section 3.4.
- Staged and reviewed: Physics `/workspace/phys/`, Animation `/workspace/anim_staging/`, Grok Bot `/workspace/gb_staging/`.

## 7. Hive library
- Research notes live in /workspace/hive/ (Grok Bot curates). Reference only: an idea becomes work only when the hub adds it to section 5.
