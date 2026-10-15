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

## 3. Pipeline (each fixed step, 120 Hz, this order)
Steps 1-4 run in FixedPostUpdate `.before(PhysicsSystems::First)`; steps 6+ `.after(PhysicsSystems::Last)`.
1. Input/events into `event_log` (Studio edits, BalanceSet, controller): the only source of truth.
2. `apply_edits`: the only pose writer; abilities are layers by Priority (Limits > Balance > Style > Effort).
3. Contact: subtract each sole's rest offset first, then (down < 3 cm, up > 5 cm, 6-tick hold) produces planted factor c in [0,1].
4. Motor writer: per joint. Input is only apply_edits' final local: local_target = base_local · rot (base = accumulated pose for Additive/Masked, rest local for Replace).
   - θ_err = log((R_p · O_p · local_target · O_c⁻¹) · R_c⁻¹), world-frame shortest arc; O defined by bone_world = capsule_world · O, measured once at load
   - local_target = ordered product parent→child (left to right) of local_target,k over the skin-bone chain between the two capsule bones (spine, clavicle, twist); chain + O stored in anim_rig at load; Animation's chain_target(rig, from, to, pose) supplies it
   - omega* = R_p omega_L ;  alpha* = R_p alpha_L + omega_p x (R_p omega_L)
   - tau = I_w(alpha* + dω_p) + Kp θ_err + Kd(omega* - (ω_c - ω_p)) + ω_c x I_w ω_c + tau_hold + tau_bal,  I_w = R_c I_body R_cᵀ
   - ONE clamp: legs k·g·(c·M_above + (1-c)·m_below)·l_com, k=1.2; arms margin·(I·α_eff + m·g·l_com), α_eff <= 1.5·α_max inside a strike window, max 6 ticks per strike, 12-tick cooldown, logged.
   - tau_hold and ankle torque scale by c. Ankle clamp = c·share·M_total·g·d_edge (share = leg_hold weight share, sums to 1 over planted feet) (d_edge = ankle to the toe or heel edge the torque tips toward), so the foot never levers off. Balance off = tau_bal 0, tau_hold stays on.
   - Strike boost: Physics' strike_boost(ticks): 1.0 for ticks 0-5, eased to 0 by tick 9, 12-tick cooldown, each boost logged as an event.
   - Apply +tau to child, -tau to parent. Avian joint motors off on the sandbox body.
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
