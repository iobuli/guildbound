# Guildbound: system maths of the single motor writer (pass 6)

Owner: The Rust Expert (hub). Written 2026-10-15 for an outside implementer (Claude Opus / Fable 2).
This file follows **one tick of the motor writer end to end** and says where each piece comes from. It does not repeat
the derivations already in the two specialist docs; it links to them:
- **PHYSICS_MATH.md** (`/workspace/hive/PHYSICS_MATH.md`, "PM"): inertia, Stable PD, reduced inertia, gravity_ff, balance, contact, voxel, modal.
- **ANIMATION_MATH.md** (`/workspace/hive/ANIMATION_MATH.md`, "AM"): rig, bone targets, chain product, rate transport.

Code: `/workspace/pass6/game/sandbox.rs` (writer, game crate), `/workspace/pass6/sim/phys_balance.rs` (pure maths, 3×3 block
copied verbatim from Physics' `/workspace/phys/phys_balance.rs`). Lab: `/workspace/gb_lab/` (headless Bevy 0.19.1 + Avian 0.7.0).
**Numbers:** every number is from a named lab file, a code constant, or is marked *computed* (arithmetic on file values shown
inline) or *reported* (said in the room, no file on disk). Nothing else.

---

## 0. Symbols, units, defaults

| Symbol | Meaning | Unit |
|---|---|---|
| $\Delta t$ | fixed tick, 1/120 | s |
| $n_s$, $h=\Delta t/n_s$ | Avian substeps, substep length | –, s |
| $R_p, R_c$ | parent / child capsule world rotation | – |
| $O_p, O_c$ | capsule→bone offset, `bone_world = capsule_world · O` | – |
| $B_p = R_pO_p$ | parent **bone** world rotation | – |
| $Q_L$ | chain-product local target (parent→child) | – |
| $\boldsymbol\theta$ | world error rotation vector | rad |
| $\boldsymbol\omega_p,\boldsymbol\omega_c$ | world angular velocities | rad/s |
| $\boldsymbol\omega_{rel}=\boldsymbol\omega_c-\boldsymbol\omega_p$ | relative angular velocity | rad/s |
| $\boldsymbol\omega_L,\boldsymbol\alpha_L$ | target rates in parent bone frame | rad/s, rad/s² |
| $\boldsymbol\omega^\star,\boldsymbol\alpha^\star$ | target rates in world | rad/s, rad/s² |
| $I_p, I_c$ | each body's own world inertia about its COM | kg·m² |
| $I_{red}$ | 3×3 reduced inertia | kg·m² |
| $k_p=\omega_n^2$, $k_d=2\zeta\omega_n$ | PD gains **per unit inertia** | s⁻², s⁻¹ |
| $k_b$ | balance position gain | m⁻¹·(N·m per M g) → see §7 |
| $k_{bd}$ | balance velocity gain | s·m⁻¹·(…) |
| $M$ | body mass | kg |
| $g$ | 9.81 (`phys_math::G`), up $=+\hat y$ | m/s² |
| $c_i\in[0,1]$ | filtered contact of foot $i$ | – |
| $s_i$ | weight share of foot $i$ ($\sum s_i=1$ over planted feet) | – |
| $\mathbf{cop}_i$ | centre of pressure of foot $i$ (foot ground point) | m |
| $m_{sub},\mathbf c_{sub}$ | mass / COM of the joint's child-side subtree | kg, m |
| $\mathbf p$ | joint anchor (child capsule start) | m |
| $\boldsymbol\tau$ | joint torque, applied $+$child $-$parent | N·m |
| $\Sigma\tau$ | sum of all applied motor torques in a tick | N·m |

**Defaults (pass 6 v4, env settings in `/workspace/pass6/README.md`):**

| Setting | Default | Source |
|---|---|---|
| `GUILDBOUND_KP` / `KD` | 4000 / 126 ($\omega_n=63.2$ rad/s, $\zeta=0.996$ *computed*) | `bal/ro_5_0.3_12.json` |
| `GUILDBOUND_KB` / `KBD` | 5 / 0.3 | same file; hub ruling 07:23 |
| `GUILDBOUND_SUBSTEPS` | 12 (TODO ruling: substeps vs solver iterations) | ruling 07:23; PM addendum |
| `GUILDBOUND_CONTACT_FREQ` | unset; **3** is the fallback when running 6 substeps | PM addendum (*reported*) |
| `GUILDBOUND_TAN_FULL` | off | ruling 07:23 |
| `GUILDBOUND_FOOT_DAMP` | off (on = 40 N·s/m) | ruling 07:23 |
| contact on / off / hold / filter | 0.01 m / 0.025 m / 6 ticks / 0.25 | `phys_balance::Contact` |
| fault cap | $3\,\text{m}\cdot Mg$ (`SAFETY_LEVER_M`) | `sandbox.rs` |
| strike | Kp × 1.5 on arm joints (`STRIKE_KP_SCALE`) | `sandbox.rs` |
| `MaxAngularSpeed` | 50 rad/s every capsule | `MAX_SPIN` |
| CCD | `SweptCcd::default()` + `SpeculativeMargin(0.05)` feet/toes/shins | `sandbox.rs` |
| floor | static cuboid 18×1×18 m, top at stage $y$ | pass6 `studio.rs` |

### Sign conventions

| Quantity | Convention |
|---|---|
| world axes | $+y$ up; ground plane $(x,z)$ |
| quaternion storage | `[x,y,z,w]`; $q_aq_b$ applies $q_b$ first; $v'=qvq^{-1}$ |
| $\log q$ | shortest arc ($q\to-q$ if $w<0$), returns full $\hat n\phi$ (AM writes $2\log$ of a half-angle log: same vector) |
| $\boldsymbol\theta$ | rotation taking $R_c$ to its target: $R_c^\star=\exp(\boldsymbol\theta)R_c$; $+\boldsymbol\tau\parallel\boldsymbol\theta$ on the child closes it |
| motor torque | $+\boldsymbol\tau$ on child, $-\boldsymbol\tau$ on parent |
| balance $\boldsymbol\tau_{bal}$ | wanted **on the body above**; enters the child joint torque **negated** (ankle child = foot, hip child = thigh) |
| gravity_ff | torque on the child that holds its subtree against gravity, minus the foot reaction |
| COM error $\mathbf e$ | COM − support centre, $(x,z)$ |
| sole height | $>0$ above floor top |

---

## 1. Schedule at 120 Hz

All in `FixedPostUpdate` (fixed 1/120 s), never `Update`. Rendering uses `TransformInterpolation`.

| # | System | Ordering | What it does |
|---|---|---|---|
| 1 | `possession::physical_drive` | before 2 | builds/keeps the doll; its own motor jobs are dropped when `Sandbox.started` |
| 2 | `sandbox::balance_tick` (exclusive) | `.after(physical_drive).before(props::physics_clock).before(PhysicsSystems::First)` | stand lock check → contact → balance → per-joint torque → `Forces::apply_torque` (+ optional foot force) → Στ line |
| 3 | `sandbox::apply_balance_toggle` | chained after 2 | applies logged `SetBalance` (takes effect next tick) |
| 4 | Avian step | `PhysicsSystems::First … Last`, $n_s$ substeps | torques/forces from 2 are held constant over all substeps |
| 5 | `sandbox::report` | `.after(PhysicsSystems::Last)` | COM, margin, hinge check; `metrics.jsonl` every 120 ticks |

Brief §3 lists steps 1–4 as "events → apply_edits → contact → motor"; in the stand test `apply_edits` yields the rest pose
only (rest-only mode, §11), so the target side of step 2 reads the rest rig directly.

---

## 2. Frames and the error

**Capsule offset.** Measured once at ragdoll-on: $R_{bone}=R_{capsule}O$ (PM §1). Load check: $\max|\boldsymbol\theta|<10^{-4}$
rad at rest over all joints (logged as `load_check` in `metrics.jsonl`; pass 5 measured $3\times10^{-7}$, PM §1).

**Chain target.** The capsule joint spans skin joints $k_1..k_n$ (`anim_abilities::chain`), parent first:
$Q_L=\prod_{i=1}^n q_{L,k_i}$ (AM §3). Rates come from the *product* at $t-1,t,t+1$, so link cross terms are exact.

**Error** (`sandbox::theta_err`):
$$\boldsymbol\theta=\log\!\big((R_p\,O_p\,Q_L\,O_c^{-1})\,R_c^{-1}\big).$$
$R_pO_p$ is the parent bone; $\cdot Q_L$ the child bone target; $\cdot O_c^{-1}$ back to the child capsule frame; $\cdot R_c^{-1}$
the world-frame difference (PM §3).

## 3. Rate transport $\omega^\star,\alpha^\star$ — frame fix

$\boldsymbol\omega_L,\boldsymbol\alpha_L$ are expressed in the **parent bone** frame (AM §2, §4). The world rotation of that frame
is $B_p=R_pO_p$, so (AM §4, PM addendum):
$$\boldsymbol\omega^\star=B_p\boldsymbol\omega_L,\qquad \boldsymbol\alpha^\star=B_p\boldsymbol\alpha_L+\boldsymbol\omega_p\times(B_p\boldsymbol\omega_L).$$
Derivation: $\frac{d}{dt}(B_p\boldsymbol\omega_L)=\dot B_p\boldsymbol\omega_L+B_p\dot{\boldsymbol\omega}_L$, and since $O_p$ is
constant in the parent capsule frame, $\dot B_p=[\boldsymbol\omega_p]_\times B_p$. (The ω_p in the cross term is the same
because $O_p$ rides on the parent body.)
**Fix applied in pass 6 v4:** v1–v3 passed $R_p$ to `rates_to_world_rel`; `sandbox.rs` now passes `b_p = r_p * j.o_p`.
Effect in the stand test: none ($\boldsymbol\omega_L=\boldsymbol\alpha_L=0$ at rest). Effect on clips: before the fix
$\boldsymbol\omega^\star$ was rotated by $O_p^{-1}$ relative to the true value — wrong whenever $O_p\ne I$ (*not measured*).

## 4. Stable PD with 3×3 reduced inertia

Full derivation: PM §4. Shipped law (`phys_balance::stable_pd3`):
$$\boldsymbol\tau_{pd}=I_{red}\,\frac{k_p\boldsymbol\theta-k_d(\boldsymbol\omega_{rel}-\boldsymbol\omega^\star)}{1+k_d\Delta t+k_p\Delta t^2},\qquad I_{red}=(I_c^{-1}+I_p^{-1})^{-1}$$
(matrix–vector product; `reduced_inertia3`: f64 adjugate inverse, symmetrised, singular guards PM §4.3).

**Tan et al. 2011 numerator.** Predicting the next state, $\mathbf a=k_p(\boldsymbol\theta-\Delta t\boldsymbol\omega')-k_d\boldsymbol\omega'$,
$\boldsymbol\omega'=\boldsymbol\omega_{rel}+\Delta t\,\mathbf a$, gives $\mathbf a\,D=k_p\boldsymbol\theta-(k_d+k_p\Delta t)\boldsymbol\omega_{rel}$,
$D=1+k_d\Delta t+k_p\Delta t^2$ (PM §4.2). The shipped law drops $-k_p\Delta t\,\boldsymbol\omega_{rel}$. At the defaults
$k_p\Delta t=33.3$ vs $k_d=126$ (*computed*), i.e. the shipped damping is $126/159.3=79\%$ of the full form.
`GUILDBOUND_TAN_FULL=on` restores it by scaling the rate term by $(k_d+k_p\Delta t)/k_d$ inside `stable_pd3` (same denominator). Off until the lab runs it.
At defaults $D = 1 + 126/120 + 4000/14400 = 2.328$ (*computed*).

**Why each body's own inertia.** Avian integrates each body separately: $\dot{\boldsymbol\omega}_b=I_b^{-1}\boldsymbol\tau_b$.
With $+\boldsymbol\tau$ on $c$ and $-\boldsymbol\tau$ on $p$, $\dot{\boldsymbol\omega}_{rel}=(I_c^{-1}+I_p^{-1})\boldsymbol\tau$, so
$\boldsymbol\tau=I_{red}\mathbf a$ produces exactly the requested relative acceleration (PM §4.3). A subtree inertia about the
joint is the wrong inertia here: the torque reaches the next body only through the joint constraint, inside the solve.
The scalar "about the error axis" form is not the projection of the 3×3 one: field `ired_rel_diff_max` is 0.39–0.59 in
every `ss/*.json` and `rel/*.json` (e.g. `ss/6_4000_126_3_0.json` 0.4224; failing `ss/6_4000_126_5_0.json` 0.588).

**Feed-forward** uses the same matrix: $\boldsymbol\tau_{ff}=I_{red}(\boldsymbol\alpha^\star+\dot{\boldsymbol\omega}_p)$,
$\dot{\boldsymbol\omega}_p\approx(\boldsymbol\omega_p^t-\boldsymbol\omega_p^{t-1})/\Delta t$ (PM §5). Gyro term dropped (Avian already
integrates it; PM §5). Lab has $\boldsymbol\alpha^\star=0$, so feed-forward is *not measured*.

### 4.1 Why the flat-gain law blew up
Pass 4/5 and lab `ladder/r1` used $\boldsymbol\tau=K_p\boldsymbol\theta-K_d\boldsymbol\omega_{rel}$ with **absolute** gains
(kp 400 N·m/rad, kd 40 N·m·s/rad, `gb_lab/src/main.rs` defaults). On one axis of a light body, explicit damping alone gives
$\omega_{n+1}=(1-K_d\Delta t/I)\,\omega_n$, stable only if $K_d\Delta t/I<2$.
Lab foot (`ragdoll.rs` defs: box 0.1×0.06×0.24 m, 1.2 kg): $I_{pitch}=\tfrac{1.2}{12}(0.06^2+0.24^2)=0.00612$ kg·m²
(*computed*, solid box, full extents). Then
- per tick: $K_d\Delta t/I = 40\cdot\tfrac1{120}/0.00612 = 54.5$ (*computed*);
- per substep at 6: $40\cdot\tfrac1{720}/0.00612=9.1$ (*computed*).
Both ≫ 2: the damper alone reverses and amplifies the foot's spin every step, independent of $K_p$. Observed: `ladder/r1.json`
peak_sat 398.4, flips 13.65/s, com_min 0.125 m. The pelvis (10 kg, much larger $I$) sees the same gain as stable — one
flat gain cannot suit a 1.2 kg foot and a 20 kg torso. Inertia scaling makes $k_d\Delta t$ the governing number (1.05 per
tick at defaults, *computed*), and the Stable-PD denominator keeps $k'+c'<1$ for any gains (PM §4.2 Jury argument).

## 5. Gravity feed-forward (reaction form)

Derivation PM §6. Per joint:
$$\boldsymbol\tau_g=(\mathbf c_{sub}-\mathbf p)\times(m_{sub}g\hat y)-c_i(\mathbf{cop}_i-\mathbf p)\times(s_iMg\hat y)$$
(`phys_balance::gravity_ff`; arms/trunk use $c=0$ → plain subtree compensation). Pass 6 takes $\mathbf{cop}_i$ = midpoint of the
foot capsule projected to the floor (`Measure::contact_pt`); the lab uses the foot box centre at $y=0$.

**The old whole-COM bug worked through.** Old stance term: $\boldsymbol\tau_{old}=-s_i(\mathbf c_{above}-\mathbf p)\times(m_{above}g\hat y)$.
For one planted foot ($s=c=1$), using $M\mathbf x_{com}=m_{sub}\mathbf c_{sub}+m_{above}\mathbf c_{above}$:
$$\boldsymbol\tau_{new}-\boldsymbol\tau_{old}=\big[m_{sub}(\mathbf c_{sub}-\mathbf p)+m_{above}(\mathbf c_{above}-\mathbf p)-M(\mathbf{cop}-\mathbf p)\big]\times g\hat y=M(\mathbf x_{com}-\mathbf{cop})\times g\hat y .$$
So the two agree only when the COM is directly over that foot's CoP. With two feet the COM is on the midline, so every
stance joint gets a false moment of size $s_iMg\,|\Delta_{xz}|$. Lab body $M=69.4$ kg (*computed* from defs; PM §14), so
$Mg=680.8$ N and the error is 6.8 N·m per cm of offset (*computed*); at $s=0.5$ and a 10 cm offset (*reported*) that is
34 N·m (*computed*), consistent with the reported 25–30 N·m per joint. Lab: `diag/SBn_4000` (gff off) com_min 0.093 m.

## 6. Balance $k_b,k_{bd}$ and the ankle-first split

Law PM §7 (`phys_balance::balance_kb`):
$$\boldsymbol\tau_{bal}=\big((k_b\mathbf e+k_{bd}\dot{\mathbf x}_{xz})\times\hat y\big)\,Mg\;r(t),\quad \mathbf e=(\mathbf x-\mathbf s)_{xz},\ \mathbf s=\frac{\sum c_i\mathbf{cop}_i}{\sum c_i},$$
$r(t)$ = smoothstep ramp over 60 ticks (`phys_balance::ramp`). Off when $\sum c\le0.05$.
Units: $k_b\mathbf e$ is a dimensionless lever fraction, so $\boldsymbol\tau$ is in N·m; $k_{bd}$ in s.
**Split** (`split_balance`): per foot, want $=c_is_i\boldsymbol\tau_{bal}$; the ankle takes up to its sole-edge cap
$c_is_iMg\,d_{edge}$ (`sandbox::ankle_cap`, $d_{edge}$ = ankle to the toe/heel/side edge the torque tips toward); the
remainder goes to that leg's hip. Both enter the joint torque negated (sign table). Lab uses a fixed $d_{edge}=0.12$ m (PM §7).
Data: `bal/ro_5_0.3_12.json` (kb 5, kbd 0.3, 12 substeps per its name): com_min 0.9806, com_final 0.9929, drift_final 0.0516 m,
flips 0.05/s, peak_sat 0.644.

## 7. Contact

**True lowest sole point.** Ragdoll capsules are `Collider::capsule(r, L-2r)` along local $\hat y$ with tips at the joint
points (`ragdoll.rs`), i.e. the old "ankle" and "toe" points are the **tips**, already including $r$. The old height
$\min(y_{ankle},y_{toe})-r$ therefore reads, for a foot pitched $\theta$ from horizontal, tip $=$ segment end $-r\sin\theta$ in $y$:
$$y_{old}=y_{seg}-r\sin\theta-r,\qquad y_{true}=y_{seg}-r,\qquad y_{old}-y_{true}=-r\sin\theta .$$
With $r=0.045$ m the live −2.65 cm needs $\sin\theta=0.589$, $\theta=36.1°$ (*computed*). Pass 6 uses
$y_{sole}=y_{centre}-|(R\hat y)_y|\,h_{half}-r-y_{floor}$ (exact for a capsule), so no rest offset is subtracted.
**Hysteresis + filter** (`phys_balance::Contact`): on when $y_{sole}<0.01$, off when $>0.025$, minimum 6 ticks between flips;
$c_{t+1}=c_t+0.25(b_t-c_t)$ (≈29 ms time constant, PM §8). $c$ (not $b$) drives gravity_ff, the split and the caps.
Lab with the filter: 0.05 flips/s in every standing run listed here; pass mark < 1/s.

## 8. Foot damping (setting, default off)

$$\mathbf F_i=-c_i\,40\,(\mathbf v_{foot,i}-\dot{\mathbf x})_{xz},\qquad|\mathbf F_i|\le0.75\,s_iMg$$
applied as a force on the foot (static-friction stand-in, external, excluded from Στ). Lab differs: absolute
velocity by default (`--damp_rel` for relative), $\mu=0.8$, share $c_i/\sum c\cdot\min(\sum c,1)$ (PM §8).
Data, 12 substeps, kb 5: `creep/a40__base.json` drift_final 0.015 m vs `creep/none__base.json` 0.0342 m (both com_min ≈0.981).
6 substeps, kb 3: `rel/rel40__base.json` 0.6408 m vs `rel/none__base.json` 0.6093 m — no benefit there.

## 9. Strike Kp boost

`phys_mass::strike_boost(t)`: 1 for $t\le5$, smoothstep down to 0 by $t=9$, restart when $t_{since\,start}\ge18$
(6 + 12 cooldown, `strike_can_restart`). Pass 6: $k_p(t)=k_p(1+0.5\,b(t))$ on arm joints, started by
`Sandbox::start_strike()` which logs `{"strike":{"tick":…}}`. No cap is raised; Stable PD is stable at any $k_p$. *Not measured.*

## 10. Fault cap, pairing, Στ

- **Fault cap:** $|\boldsymbol\tau|>3Mg$ N·m (≈2200 N·m at 75 kg, `sandbox.rs` comment) → scaled to the cap and a `fault` line;
  non-finite → zero and a `fault` line. `peak_sat` = max over counted joints of $|\boldsymbol\tau_{raw}|/\max(\text{cap},1)$,
  leg joints with $c<0.2$ excluded (`sat_ratio`). Note: lab `peak_sat` is against the lab's leg/arm limit formulas, not this cap — not comparable.
- **Pairing:** each joint applies $+\boldsymbol\tau$ child, $-\boldsymbol\tau$ parent, so motors are internal couples and cannot
  move the COM (PM §9). `GUILDBOUND_STEPS` without `paired` = child only, for the slide test.
- **Στ:** `torque_sum.jsonl`, one line per tick: `sum_tau` (should be 0), `max_tau`, `unpaired` (writes that missed a body),
  `com_v`, `c`. Lab: `stand_paired.jsonl` `sum_tau` 0.000000 every tick; `stand_unpaired.jsonl` non-zero (PM §9).
- **Below floor:** first tick any capsule's lowest point $<-0.1$ m → `below_floor` (once); 50 rad/s spin cap; CCD on feet/shins; 1 m floor box.

## 11. Stand test (rest-only)

Tick 0: log clip, model, step flags (`{"stand":{"tick":0,…}}`), stop clip playback, agent lock off. Each later tick: if the
model or clip changes, or playback restarts, write `{"stand":{"failed":…}}` and show it in the Physics panel. Targets are the
rest rig, so $\boldsymbol\omega_L=\boldsymbol\alpha_L=0$.

## 12. Substep findings (gate) and open questions

All 40 s stand, kd paired with kp, foot damping 0 unless noted. `ss/` files carry a `substeps` field.

| File | $n_s$ | kp/kd | kb | com_min | com_final | drift_final m | flips/s | peak_sat |
|---|---|---|---|---|---|---|---|---|
| `ss/4_4000_126_3_0.json` | 4 | 4000/126 | 3 | 0.1266 | 0.7651 | 8.3515 | 11.0 | 58.1 |
| `ss/4_4000_126_5_0.json` | 4 | 4000/126 | 5 | 0.1021 | 0.6121 | 16.1752 | 9.85 | 83.2 |
| `ss/6_3000_110_3_0.json` | 6 | 3000/110 | 3 | 0.9454 | 0.9826 | 0.2906 | 0.05 | 0.752 |
| `ss/6_4000_126_3_0.json` | 6 | 4000/126 | 3 | 0.9653 | 0.9877 | 0.6093 | 0.05 | 0.611 |
| `ss/6_8000_179_3_0.json` | 6 | 8000/179 | 3 | 0.9755 | 0.9893 | 0.2587 | 0.05 | 0.631 |
| `ss/6_4000_126_5_0.json` | 6 | 4000/126 | 5 | 0.1358 | 0.5159 | 2.2893 | 9.65 | 107.8 |
| `ss/6_2000_89_3_0.json` | 6 | 2000/89 | 3 | 0.1113 | 0.5116 | 7.9044 | 8.7 | 46.6 |
| `bal/ro_5_0.3_12.json` | 12 (name) | 4000/126 | 5 | 0.9806 | 0.9929 | 0.0516 | 0.05 | 0.644 |
| `creep/none__base.json` | 12 (`creep/jobs.txt` pattern; file has no field) | 4000/126 | 5 | 0.9806 | 0.9935 | 0.0342 | 0.05 | 0.644 |
| `bal/ro_5_0.6_24.json` | 24 (name) | 4000/126 | 5 (kbd 0.6) | 0.9871 | 0.9952 | 0.3152 | 0.05 | 0.713 |

Summary of the ss grid (48 files): at 4 substeps nothing stands (com_min ≤ 0.180); at 6, kb 5 falls for every kp 500–8000,
kb 3 stands only for kp ≥ 3000. *Reported, no file* (PM addendum): 12 + kb 3 → 0.24 m drift; 6 substeps + contact frequency 3
→ kb 5 stands; contact damping ×2 → falls; 24 substeps → 0.33 m; 12 substeps costs 0.6 ms/tick.

**Open questions.**
1. Substeps are global in Avian 0.7 (no solver-iteration setting, joint compliance already 0): 12 costs the whole game. Is
   6 + contact frequency 3 good enough? (Needs a run file.)
2. kb 3 vs 5 depends on substeps (above): the gain set is not robust; find why.
3. Full Tan numerator: untested in the lab.
4. Lab `sole_rest_height` is negative (−0.008 m in `bal/ro_5_0.3_12.json`, −0.016 m in `rel/*__base.json`): box sole sinks
   into the floor by ~1–1.6 cm at rest — Avian contact margin/penetration? Contact thresholds (1 / 2.5 cm) sit close to it.
5. Creep ~cm over 40 s even when standing; `ladder/r0.json` (motors off, lying) drifts 0.0117 m in 2 s, so some slide is in contacts, not motors.
6. $B_p$ fix, feed-forward and strike: not measured (stand test has zero target rates).
