# Pass 6 patch set (box only, not applied, not pushed)

Base: pass-5 files as built (p5b/out/sandbox.rs, body_cmds.rs), fable_stage/ui/studio.rs, /workspace/phys/*.rs.
Type-checked on the box: sim crate compiles clean; sandbox.rs compiles against bevy 0.19.1 + avian3d 0.7.0
with stubs for possession/ragdoll/studio/agent (harness /workspace/pass6_check; egui UI fns unchanged and not checked).

| File (repo path) | Change |
|---|---|
| crates/sim/src/phys_balance.rs | v3: Physics' `reduced_inertia3` (f64 adjugate inverse, symmetrised), `stable_pd3` + its inline tests3; scalar forms dropped. v2: reaction-form `gravity_ff`, plus `balance_kb` (lab kb/kbd form); removed now-unused `balance_torque`, `BalanceGains`, `balance_gains`. v1: `sat_ratio(tau, cap, c) -> Option` (cap floor 1 N·m, None when c<0.2); new `Contact` (on<1 cm / off>2.5 cm, 6-tick hold, 0.25/tick filter -> c). Removed dead `saturation`, `foot_slip`, `BalanceReport`, `balance_cap`, old `filter_contact`/`contact_hysteresis`. |
| crates/sim/src/phys_stance.rs | Removed `Contact` (moved) and `leg_hold` (replaced by `gravity_ff`). Keeps `weight_shares`. |
| crates/game/src/sandbox.rs | v3–v4: full 3x3 I_red = (I_c^-1+I_p^-1)^-1 for PD and feed-forward (matrix-vector). v2: Stable PD on reduced inertia (no working clamp; safety cap 3·M·g N·m logs `fault`), I_red feed-forward, reaction gravity_ff at foot CoP, strike = Kp×1.5 (arms) with 12-tick cooldown via `Sandbox::start_strike`, optional stance-foot damping, `SubstepCount` setting. Removed gyro term, segment_props inertia, arm/leg/ankle-edge caps. v1: Motor writer: gravity_ff + split_balance inside the one clamp; exact capsule sole height; anti-tunnel; Στ log; stand mode; step flags. Removed dead `mod core`, per-joint subtree rebuild, sole_rest. |
| crates/game/src/sandbox_core.rs | DELETE (nothing uses it since pass 5). |
| crates/game/src/studio.rs | Floor collider 1 m thick (top still at stage); `Studio::set_tab(i)` accepts 0..=9 (Pose). |
| crates/game/src/body_cmds.rs | Agent banner moved to bottom-centre, click-through, smaller text. |

Run knobs (env): GUILDBOUND_STEPS=none | paired | paired,ff | paired,ff,contact | paired,ff,contact,balance (default all).
GUILDBOUND_STAND=off restores old boot (agent lock on, no clip/model check).
New logs in target/live: metrics.jsonl gets `stand`, `below_floor`, `below_floor_tick`, `stand_failed`; torque_sum.jsonl has one line per tick (sum_tau, max_tau, unpaired, com_v, c).

## Settings (env) — v5 (lab gb_lab/tan/all.json, all2.json); v4, hub ruling 2026-10-15 07:23
| Variable | Default | Meaning |
|---|---|---|
| GUILDBOUND_KP / GUILDBOUND_KD | 4000 / 126 | Stable PD gains per unit inertia (1/s², 1/s) |
| GUILDBOUND_KB / GUILDBOUND_KBD | 5 / 0.3 | balance gains (bal/ro_5_0.3_12.json) |
| GUILDBOUND_SUBSTEPS | 12 | Avian SubstepCount (global). TODO(ruling): substeps vs solver iterations open |
| GUILDBOUND_CONTACT_FREQ | unset (Avian default) | contact_frequency_factor; 3 = fallback when running 6 substeps |
| GUILDBOUND_TAN_FULL | off | `on` adds Tan's −kp·dt·ω_rel numerator term; off until the lab confirms |
| GUILDBOUND_FOOT_DAMP | abs | stance-foot damping 40 N·s/m on the foot's own x,z velocity, cap 0.8·share·M·g; `rel` = foot − COM velocity; `off` |
| GUILDBOUND_COM_PULL | 0.11 (test build; `off` disables) | k_com (lab best 0.1, sweep 0.07–0.12): k_com·M·g·(com−com0)_xz × up added to balance, faded by 1 − max(strike_boost, acro_active) |
| GUILDBOUND_STEPS | paired,ff,contact,balance | ablation order; `none` = motors off |
| GUILDBOUND_STAND | on | rest-only stand test; `off` = old boot |

v4 changes: full 3x3 `reduced_inertia3` / `stable_pd3` (synced verbatim from /workspace/phys/phys_balance.rs) for PD and
feed-forward; target-rate transport uses B_p = R_p·O_p; kb 5, substeps 12; contact-frequency and Tan settings.
Compile: sim crate clean + Physics' 4 inline tests3 pass; sandbox.rs type-checks (harness /workspace/pass6_check).
Older copies and step diffs in v1/. Maths: /workspace/hive/SYSTEM_MATH.md.

v5: foot damping on by default, absolute velocity, mu 0.8 (lab); COM pull option (k_com, default off) with com0 re-anchor
when `Sandbox.acro_active` (Animation, stub 0) returns to 0; TAN_FULL stays off. Lab evidence (12 substeps, kb 5, 40 s / 80 s
drift_final m): base 0.0342 / 0.218; fd40 0.015 / 0.3375; pull0.1 0.0194 / 0.0907; pull0.1+fd40 0.0186 / 0.0465;
pull ≥ 0.5 diverges (all.json). Diff v4->v5: v1/sandbox_v4_to_v5.diff.

v5b: contact thresholds measured from per-foot rest_h (first-contact value, then mean of ticks 1..60); acro_active from
anim_acro_active::acro_active via `Sandbox::start_acro(len)`; COM pull 0.11 built in; stand scene adds Avian PhysicsDebugPlugin
and turns the skeleton overlay on. Rendered foot is NOT lifted by −rest_h (render follows physics transforms; skipped).
Install steps: APPLY.md. Diff v5->v5b: v1/sandbox_v5_to_v5b.diff.
