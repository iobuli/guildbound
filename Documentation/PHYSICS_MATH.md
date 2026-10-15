# Guildbound physics: maths spec (pass 6)

Owner: Physics. Written 2026-10-15 for an outside implementer (Claude Opus / Fable). Self-contained: every symbol is defined here.
Stack: Rust 2024, Bevy 0.19.1, Avian 0.7.0. Pure maths lives in `crates/sim` (`phys_*.rs`, staged at `/workspace/phys/`); the motor writer lives in `crates/game/src/sandbox.rs` (staged at `/workspace/pass6/game/`).

**Tags.** `[team]` = our own derivation or design, not from a paper. A paper citation (§, Eq.) appears only where `/workspace/hive/use/online_physics.md` or the other hive notes say the text was actually read. **Numbers** come from files under `/workspace/gb_lab/` (file named next to each), from code constants (file named), or are written *not measured*. "Reported" = stated by Grok Bot in the team room but no run file is on disk.

---

## 1. Conventions

| Item | Value | Source |
|---|---|---|
| Up axis | world $+Y$; ground plane is $(x,z)$ | brief §3, all code |
| Gravity | $g = 9.81\ \mathrm{m/s^2}$, $\mathbf g = -g\,\hat y$ | `phys_math::G` |
| Units | kg, m, s, rad, N, N·m | `phys_mass.rs` header |
| Tick | fixed $\Delta t = 1/120$ s; all logic on whole ticks | brief §3, `gb_lab/src/main.rs: DT` |
| Substeps | Avian `SubstepCount` $n_s$; sub-step $h = \Delta t / n_s$. Torques are written once per tick and held constant across substeps | lab `--substeps`, pass6 `GUILDBOUND_SUBSTEPS` |
| Scalars | sim state `f32`; accumulations and inverses `f64`, rounded once | `phys_voxel`, `reduced_inertia3`, `anim_abilities::derivs` |

**Schedule (per tick, brief §3).** Events → `apply_edits` (only pose writer) → contact → single motor writer (one torque per joint) → Avian step → report. Steps up to the motor writer run in `FixedPostUpdate` *before* `PhysicsSystems::First`; reporting runs after `PhysicsSystems::Last`. Nothing in `Update`.

**Quaternions.** Unit quaternion $q = (x,y,z,w)$, storage order `[x, y, z, w]` (`anim_abilities::Q4`). Composition $q_a q_b$ = apply $q_b$ first, then $q_a$ (Hamilton product, column vectors, $v' = q v q^{-1}$). Matrices $R(q)$ act on column vectors.

**Rotation vector.** $\log : SU(2)\to\mathbb R^3$, $\log(q) = \hat n\,\phi$ with $\phi\in[0,\pi]$; before taking it, flip $q\to -q$ when $w<0$ (shortest arc). $\exp(\boldsymbol\theta)$ is the inverse. All angular vectors ($\boldsymbol\theta,\boldsymbol\omega,\boldsymbol\alpha,\boldsymbol\tau$) are **world-frame** unless subscripted $L$ (parent-local).

**Bone vs capsule.** The rig has 65 skin joints mapped onto 13 physics capsules. Each capsule $k$ has a constant offset $O_k$ measured once at load:
$$ R_{\text{bone},k}^{\text{world}} = R_{\text{capsule},k}^{\text{world}}\; O_k .$$
Load check (logged, brief §4): at the rest pose, $|\boldsymbol\theta_{err}| < 10^{-4}$ rad on every joint, else $O$ or the chain order is wrong (pass 5 measured $3\times10^{-7}$, HANDOFF §3).

**Replay hash.** Hashed: the event log, our own sim state (e.g. `BalanceReport::hash_words`: tick, COM, support polygon, margin, saturation, slip as `f32::to_bits`; modal $q,\dot q$; the voxel store), and sampled target ticks. **Never** Avian internals (brief §4). Determinism rules: fixed iteration order (BTreeMap/BTreeSet, bone index order), `total_cmp` sorts, no HashMap.

---

## 2. Rigid limb mass and inertia

**Capsule** (radius $r$, cylinder length $l$, axis local $\hat y$), density $\rho$ [standard result]:
$m_c=\rho\pi r^2 l$, $m_h=\tfrac{4}{3}\rho\pi r^3$ (two hemispheres), $m=m_c+m_h$,
$$ I_{yy} = m_c\frac{r^2}{2} + m_h\frac{2r^2}{5},\qquad
I_{xx}=I_{zz} = m_c\Big(\frac{l^2}{12}+\frac{r^2}{4}\Big) + m_h\Big(\frac{2r^2}{5}+\frac{l^2}{4}+\frac{3lr}{8}\Big). $$
In the lab and pass 6 we do **not** compute this ourselves: Avian's `ComputedAngularInertia` (from the collider and mass) is used, rotated to world. `phys_mass::segment_props` (superseded for motion) uses a solid cylinder: $I_{yy}=mr^2/2$, $I_{xx}=I_{zz}=m(3r^2+l^2)/12$.

**Parallel axis** (point mass $m$ at offset $\mathbf d$ from the reference point):
$$ I_{\text{ref}} = I_{\text{com}} + m\big(|\mathbf d|^2 E_3 - \mathbf d\mathbf d^{\mathsf T}\big). $$

**World inertia** of body $b$ with orientation $R_b$: $I_{w} = R_b\, I_{\text{body}}\, R_b^{\mathsf T}$ (symmetric, positive definite). Avian's `ComputedAngularInertia::rotated(R).tensor()` is exactly this, about the body's own COM.

---

## 3. Joint error and target rates

Joint $j$ connects parent capsule $p$ and child capsule $c$. World orientations $R_p, R_c$; angular velocities $\boldsymbol\omega_p,\boldsymbol\omega_c$.

**Local target.** `apply_edits` gives a parent-local target per skin joint. Between two capsule bones there is a chain of skin joints $k_1\ldots k_n$ (spine, clavicle, twist), ordered parent→child; `chain(rig, from, to)` returns it. The composite local target is the ordered product
$$ Q_L = \prod_{i=1}^{n} q_{L,k_i}\quad(\text{left to right, parent first}). $$

**Error** (brief §3.4, `pass6 sandbox::theta_err`):
$$ \boldsymbol\theta_{err} = \log\!\Big(\big(R_p\,O_p\,Q_L\,O_c^{-1}\big)\,R_c^{-1}\Big). $$
Meaning: $R_c^{\star} = R_p O_p Q_L O_c^{-1}$ is the capsule-world target (bone target $R_pO_pQ_L$, converted back to the capsule frame by $O_c^{-1}$). The error is the world-frame rotation that takes $R_c$ to $R_c^\star$: $R_c^\star = \exp(\boldsymbol\theta_{err}) R_c$. **Sign:** a torque along $+\boldsymbol\theta_{err}$ on the child turns it toward the target. The lab's rest-pose version is $\boldsymbol\theta = \log\big((R_p\, q_{\text{rest,rel}}) R_c^{-1}\big)$.

**Target rates** (Animation, `chain_target` + `derivs`, f64): with $Q(t)$ the chain product at tick $t$,
$$ \boldsymbol\omega_L = \frac{\log\big(Q(t{+}1)Q(t{-}1)^{-1}\big)}{2\Delta t},\qquad
\boldsymbol\alpha_L = \frac{\log\big(Q(t{+}1)Q(t)^{-1}\big)-\log\big(Q(t)Q(t{-}1)^{-1}\big)}{\Delta t^2}. $$
These are rates of the *product*, so all cross terms between chain links are exact (no per-link sum). Note: these logs are left-differences, i.e. rates expressed in the frame where $Q$ is applied (the parent frame) [team].

**World conversion** (`rates_to_world_rel(R_p, ω_p, ω_L, α_L)`):
$$ \boldsymbol\omega^\star = R_p\boldsymbol\omega_L,\qquad \boldsymbol\alpha^\star = R_p\boldsymbol\alpha_L + \boldsymbol\omega_p\times(R_p\boldsymbol\omega_L). $$
*Derivation [team].* $\boldsymbol\omega^\star$ is the desired child-relative-to-parent velocity in world. Differentiate $\mathbf v(t) = R_p(t)\boldsymbol\omega_L(t)$ with $\dot R_p = [\boldsymbol\omega_p]_\times R_p$:
$\dot{\mathbf v} = [\boldsymbol\omega_p]_\times R_p\boldsymbol\omega_L + R_p\dot{\boldsymbol\omega}_L = \boldsymbol\omega_p\times(R_p\boldsymbol\omega_L) + R_p\boldsymbol\alpha_L.$ The desired *absolute* child acceleration is then $\boldsymbol\alpha^\star + \dot{\boldsymbol\omega}_p$ (used in §5). The constant $O$ offsets do not change rates because $O_p$ is fixed in the parent frame; strictly $\boldsymbol\omega_L$ should be rotated by $O_p$ as well ($R_pO_p\boldsymbol\omega_L$) if the chain's rates are in the bone frame — the staged code passes $R_p$; check this at integration and log the residual [team, open].

Relative velocity: $\boldsymbol\omega_{rel} = \boldsymbol\omega_c - \boldsymbol\omega_p$. Damping error: $\boldsymbol\omega_{rel} - \boldsymbol\omega^\star$ (for a static pose $\boldsymbol\omega^\star=0$, which is what the lab and the stand test use).

---

## 4. Stable PD with 3×3 reduced inertia

### 4.1 The law (pass 6 ruling 1; `phys_balance::stable_pd3`)
Gains are **per unit inertia**: $k_p=\omega_n^2$ [s⁻²], $k_d = 2\zeta\omega_n$ [s⁻¹].
$$ \boxed{\ \boldsymbol\tau = I_{red}\,\frac{k_p\,\boldsymbol\theta_{err} - k_d(\boldsymbol\omega_{rel}-\boldsymbol\omega^\star)}{1 + k_d\Delta t + k_p\Delta t^2}\ } $$
applied $+\boldsymbol\tau$ on the child, $-\boldsymbol\tau$ on the parent (§9). Current best: $k_p=4000$, $k_d=126$ ($\omega_n=63.2$ rad/s, $\zeta\approx1.0$).

### 4.2 Derivation from the implicit next state
Tan, Liu & Turk 2011 §3.1 (Eq. 3/5) compute the PD force from the *predicted next state* $(q+\Delta t\dot q,\ \dot q+\Delta t\ddot q)$, which keeps arbitrarily high gains stable at large $\Delta t$; §4.3 Eq. 14–15 applies a joint torque as $\tau_2=-\tau_1$. Our single-joint form [team]: let $\mathbf a = I_{red}^{-1}\boldsymbol\tau$ be the commanded relative angular acceleration and $\boldsymbol\omega' = \boldsymbol\omega_{rel}+\Delta t\,\mathbf a$. Ask for
$$ \mathbf a = k_p(\boldsymbol\theta_{err} - \Delta t\,\boldsymbol\omega') - k_d\boldsymbol\omega' . $$
Solving: $\mathbf a\,(1 + k_d\Delta t + k_p\Delta t^2) = k_p\boldsymbol\theta_{err} - (k_d + k_p\Delta t)\boldsymbol\omega_{rel}$.
The shipped (lab-verified) law keeps the denominator but uses $-k_d\boldsymbol\omega_{rel}$ in the numerator, i.e. it drops the $-k_p\Delta t\,\boldsymbol\omega_{rel}$ term (at $k_p=4000$: $k_p\Delta t=33.3$ vs $k_d=126$, about 26 % less damping than the full form). Both are stable (below); the full form is an untested option [team, not measured].

**Why the denominator stabilises [team].** Model one axis as a body against the world, torque held for the tick, semi-implicit Euler: $\omega_{n+1}=\omega_n+\Delta t\,a_n$, $x_{n+1}=x_n+\Delta t\,\omega_{n+1}$, with $a_n = -K x_n - C\omega_n$, $K=k_p/D$, $C=k_d/D$, $D=1+k_d\Delta t+k_p\Delta t^2$. With $k'=K\Delta t^2$, $c'=C\Delta t$ the characteristic polynomial is
$$ z^2-(2-c'-k')z+(1-c') = 0. $$
Jury conditions: $k'>0$; $0<c'<2$; $4-2c'-k'>0$. Here $k'+c' = (k_p\Delta t^2+k_d\Delta t)/D < 1$ for **every** $k_p,k_d>0$, so all three hold: the loop is stable at any gain. Without $D$ ($D=1$) the same scheme needs $4-2k_d\Delta t-k_p\Delta t^2>0$, i.e. $\omega_n\Delta t<2$ roughly. In the limit $k_p\to\infty$, $K\Delta t^2\to1$: the controller asks to close the error in one tick, never more.
Measured per-substep $\omega_n h$: Grok Bot reported 0.09–0.13 at 6 substeps (no run field; reported) — so the motor is not the unstable element at 6 substeps (§14).

### 4.3 3×3 reduced inertia (`phys_balance::reduced_inertia3`)
$$ \boxed{\ I_{red} = \big(I_c^{-1} + I_p^{-1}\big)^{-1}\ } $$
with $I_c, I_p$ each body's **own** world inertia about its **own** COM (§2), not subtree inertias about the joint.
*Derivation [team].* Avian applies a body torque as $\dot{\boldsymbol\omega}_b = I_b^{-1}\boldsymbol\tau_b$ per body. With $+\boldsymbol\tau$ on $c$ and $-\boldsymbol\tau$ on $p$, the relative angular acceleration from the motor alone is
$\dot{\boldsymbol\omega}_{rel} = I_c^{-1}\boldsymbol\tau + I_p^{-1}\boldsymbol\tau = (I_c^{-1}+I_p^{-1})\boldsymbol\tau$. Requiring $\dot{\boldsymbol\omega}_{rel}=\mathbf a$ gives $\boldsymbol\tau = I_{red}\mathbf a$. Subtree inertia would be wrong because spherical joints pass no torque to the next body within the step; the joint constraint's linear coupling is neglected (idealisation). Equal bodies give $I_{red}=I/2$; a static or zero parent gives $I_{red}=I_c$ (code guard).
*Implementation.* Symmetrise, invert by adjugate in f64, guard $|\det| \le 10^{-12}\,s^3$ ($s$ = largest entry) as singular; one singular input → return the other; both or sum singular → 0. Unit tests: equal diagonal → $I/2$; result symmetric with $0 < (I_{red})_{ii} < (I_c)_{ii}$.

**Why the scalar form fails.** The deprecated scalar uses $I_{red}^{(s)} = \frac{a b}{a+b}$ with $a=\hat n^{\mathsf T}I_p\hat n$, $b=\hat n^{\mathsf T}I_c\hat n$ about the error axis and applies $\boldsymbol\tau\parallel\mathbf a$. But $\hat n^{\mathsf T}(I_c^{-1}+I_p^{-1})^{-1}\hat n \ne$ harmonic mean of the projections unless $\hat n$ is a shared principal axis, and the true $\boldsymbol\tau$ is generally not parallel to $\mathbf a$ for anisotropic capsules (long axis vs cross axes differ by ~10×). Evidence: field `ired_rel_diff_max` = $\max |\hat n^{\mathsf T}I_{red}\hat n - I^{(s)}_{red}|/I^{(s)}_{red}$ over a run is 0.39–0.59 in every `gb_lab/ss/*.json` and `rel/*.json` run. Grok Bot reported that runs with the scalar form blow up even at 12 substeps; that run file is not on disk (*reported*).

---

## 5. Feed-forward

$$ \boldsymbol\tau_{ff} = I_{red}\big(\boldsymbol\alpha^\star + \dot{\boldsymbol\omega}_p\big) $$
(pass 6 ruling 3; Animation's request). $\dot{\boldsymbol\omega}_p$ = parent angular acceleration, estimated from the previous tick ($(\boldsymbol\omega_p^{t}-\boldsymbol\omega_p^{t-1})/\Delta t$). Using $I_{red}$ (not $I_w$ of the child) keeps the feed-forward on the same inertia as the PD, so the sum $\boldsymbol\tau = I_{red}(\mathbf a_{ff} + \mathbf a_{pd})$ is one acceleration request. Using the bigger child-alone or subtree inertia over-drives light arm joints (the overnight arm blow-up mechanism).
For the stand test $\boldsymbol\alpha^\star=0$ (lab `alpha_star = 0`), so this term is **not measured** in the lab.

**Dropped gyroscopic term.** The brief's $\boldsymbol\omega_c\times I_w\boldsymbol\omega_c$ is removed (pass 6 ruling 1, matches the lab). Justification [team]: Avian integrates each body's Euler equation including its own gyroscopic term, so adding it in the motor double-counts it for the child and leaves it unpaired on the parent (it is not an internal torque pair). At standing speeds $|\boldsymbol\omega|\lesssim 1$ rad/s it is also tiny ($\sim I\omega^2$). For strikes it is *not measured*.

---

## 6. Gravity compensation (reaction form)

`phys_balance::gravity_ff`, lab FIX (4). For joint at world point $\mathbf p$, let the **subtree** $S$ be the child side (foot-ward for legs), mass $m_{sub}$, COM $\mathbf c_{sub}$. For a leg, let $\mathbf{cop}$ be that foot's centre of pressure (lab: foot centre projected to $y=0$), $s$ its weight share, $M$ the total mass, $c$ its filtered contact.
$$ \boxed{\ \boldsymbol\tau_g = (\mathbf c_{sub}-\mathbf p)\times(m_{sub}\,g\,\hat y)\; -\; c\,(\mathbf{cop}-\mathbf p)\times(s\,M\,g\,\hat y)\ } $$
(code clamps $c\in[0,1]$, $s\ge0$; lab multiplies by $\min(\sum c,1)$ instead of $c$ with $s=c_i/\sum c$, which is the same product.) Swing limb ($c=0$): plain subtree gravity compensation.

*Derivation [team].* Static equilibrium of $S$ about $\mathbf p$. External moments on $S$: gravity $(\mathbf c_{sub}-\mathbf p)\times(-m_{sub}g\hat y)$; ground reaction $(\mathbf{cop}-\mathbf p)\times(sMg\hat y)$; motor torque $\boldsymbol\tau$ on the child; the joint's constraint force acts at $\mathbf p$ (zero moment). Sum = 0 ⇒ the box above. This is the Jacobian-transpose gravity compensation idea (Coros et al. 2010, *abstract only* — PDF not read) written per joint.

**Why the old whole-body lever was wrong.** The former stance term was $\boldsymbol\tau = (\mathbf c_{above}-\mathbf p)\times(-m_{above} g\hat y\, s)$: the lever to the COM of everything *above* the joint. That COM sits on the body midline, about 10 cm sideways of each ankle (reported), so both legs got the same false sideways moment, ~25–30 N·m per joint (reported), flipping the light foot ~160° within 10 ticks (reported). Physically the stance leg's load enters at its own foot (the CoP); the mass above is carried through the hip joint force, which has no moment about the leg's own joints except through the foot reaction. Lab: `diag/SBn_4000` (gff off) com_min 0.093 m; the reaction form plus balance and 12 substeps gives `bal/ro_5_0.3_12` com_min 0.981 m.

---

## 7. Balance

**COM and velocity:** $M=\sum m_b$, $\mathbf x=\frac1M\sum m_b\mathbf x_b$, $\dot{\mathbf x}=\frac1M\sum m_b\mathbf v_b$ (`body_com`).

**Support polygon:** convex hull (monotone chain, `total_cmp` sorted, CCW) of planted sole points in $(x,z)$ (`support_polygon`). **Centre** = area centroid by the shoelace formula about $P_0$ (`support_centre`):
$$ A_2=\sum_{i=1}^{n-2}\mathbf d_i\times\mathbf d_{i+1},\ \ \mathbf C = P_0 + \frac{\sum (\mathbf d_i+\mathbf d_{i+1})(\mathbf d_i\times\mathbf d_{i+1})}{3A_2},\quad \mathbf d_i=P_i-P_0 $$
(2-D cross). Fallback to the mean for $n<3$ or $|A_2|<10^{-9}$. Margin = signed distance to the hull (+ inside). **Lab simplification:** support centre $\mathbf s = \sum c_i\mathbf f_i/\sum c_i$ (contact-weighted foot centres).

**PD balance law (lab, `--kb --kbd`; best kb 5, kbd 0.3):** with $\mathbf e=(\mathbf x-\mathbf s)_{xz}$, $\mathbf v=\dot{\mathbf x}_{xz}$,
$$ \boldsymbol\tau_{bal} = \sigma\,\big(k_b\,\mathbf e + k_{bd}\,\mathbf v\big)\times\hat y\;M g ,\qquad \sigma=+1. $$
Units: $k_b$ dimensionless (fraction of $Mg$ per metre of offset, times metres → N·m), $k_{bd}$ in s. $\boldsymbol\tau_{bal}$ is the torque wanted *on the body above*; ankle joints get $-\boldsymbol\tau_{ankle}\,s_i$ on the foot, the hip of each stance leg (parent = pelvis) gets $-\boldsymbol\tau_{hip}\,s_i$. Lab finding: $\sigma=-1$ blows up (reported), so the sign is correct. Older `balance_torque` (unused in pass 6) was $\mathbf F=-M(k_p\mathbf e+k_d\mathbf v) - Mg\mathbf e/h$ at height $h$, $\boldsymbol\tau=(hF_z,0,-hF_x)$.

**Ankle-then-hip split** (`split_balance`, lab): ankles take up to $\tau_{cap} = \min(\sum c,1)\,M g\,d_{edge}$, $d_{edge}=0.12$ m (lab default; foot half-length), the remainder goes to the stance hips. Justification: the CoP must stay on the sole, so the ankle can supply at most weight × distance to the toe/heel edge (brief §3.4); beyond that, the hip/trunk (flywheel) channel (Pratt 2006 flywheel model, abstract and §V.D–E as noted) [team split]. A geometric $d_{edge}$ from the actual foot edges (`--geom_edge`) did not clearly help (reported).

**Capture point** (Pratt et al. 2006 §V.C, Eq. 7–10; Linear Inverted Pendulum $\ddot x=(g/z_0)x$):
$$ \mathbf x_{cap} = \mathbf x + \dot{\mathbf x}\sqrt{z_0/g},\qquad \omega_0=\sqrt{g/z_0}, $$
lab: $z_0 = \max(x_y - s_y, 0.3)$, error $\mathbf e+\mathbf v/\omega_0$, $\boldsymbol\tau = k_b(\cdot)\times\hat y\,Mg$. **Result:** `bal/g_cp` (CP) com_min 0.122 m, com_final 0.246 (fell) vs `bal/g_bal` (PD) com_min 0.962, com_final 0.986, same settings ($k_p$ 2500, rest_offset off). *Why [team hypothesis, not tested]:* with $z_0\approx1$ m, CP is PD with $k_{bd}=k_b/\omega_0\approx0.32k_b$ tied to $k_b$; it cannot be tuned independently, and it feeds the noisy whole-body velocity (contact transients) straight into ankle torque at the full $k_b$ gain.

---

## 8. Contact

**Sole height.** Lowest sole point in world $y$: lab = min over the 4 box-sole corners $\mathbf p+R(\pm0.05,-h_y,\pm0.12)$; pass 6 = exact lowest point of the capsule sole. **Rest offset:** the old live −2.65 cm sole rest height came from reading a capsule pitched by $\theta$ $r\sin\theta$ too low ($r=4.5$ cm, `pass6/game/sandbox.rs` comment); with the exact lowest point no offset subtraction is needed. In the lab, `rest_offset` means "spawn the body 5 mm above the floor" (`defs`), not a subtraction. Comparison: `ladder/r3` vs `r3_off` com_min 0.107/0.110, flips/s 12.5/12.2; `r4` vs `r4_off` 0.122/0.115, 12.0/11.9 (pre-fix runs, both fell): it explains none of the flips.

**Hysteresis** (`contact_hysteresis`): on when $y_{sole}<0.01$ m, stays on while $y_{sole}\le0.025$ m. (`phys_stance::Contact` — 3 cm / 5 cm with a 6-tick hold — is the older version; pass 6 v1 `Contact` = 1 cm / 2.5 cm + 6-tick hold.)

**Filter** (one pole, per tick): $c_{t+1} = c_t + 0.25\,(b_t - c_t)$, $b_t\in\{0,1\}$. Time constant $-\Delta t/\ln(0.75)\approx 29$ ms; reaches 0.9 after 8 ticks.

**Share.** Lab and pass 6: $s_i = c_i/\sum_j c_j$. Alternative (`phys_stance::weight_shares`): $w_i = 1/(d_{i}+0.02)$ with $d_i$ the horizontal foot-to-COM distance, normalised. $\sum s_i = 1$ over planted feet.

**Foot damping** (setting, default off):
$$ \mathbf F_i = -c_i\,k\,\mathbf v^{rel}_{xz},\qquad |\mathbf F_i|\le \mu\, s_i\min(\textstyle\sum c,1)\,Mg $$
applied as a force on the foot (static-friction model). Absolute: $\mathbf v^{rel}=\mathbf v_{foot}$; COM-relative: $\mathbf v^{rel}=\mathbf v_{foot}-\dot{\mathbf x}$. Lab $\mu=0.8$ (`--mu`), pass 6 `FLOOR_MU = 0.75`, $k=40$ N·s/m.
Results (40 s, $k_p$ 4000, $k_d$ 126):
- 12 substeps, kb 5, kbd 0.3, absolute $k=40$ over 6 variants (`creep/a40__*`): drift_final 7.7–30.9 mm, com_min ≥ 0.979, 0.05 flips/s. No damping (`creep/none__*`): drift_final 30–384 mm. $k=30$ or $50$ fail some variants (`a30__kb4` 130 mm, `a50__kbd6` 69 mm).
- 6 substeps, kb 3, kbd 0.3: none 609 mm; abs40 660 mm; rel20 655 mm; rel40 641 mm; rel60 643 mm (`rel/*__base`); 80 s: 74, 397, 253, 294, 482 mm. COM-relative keeps the body up but does not cut drift.
- Touchdown anchor spring (`creep/b*`, `ab_c`) and balance smoothing (`c_lp*`, `c_rate`, `c_both`) were worse: several fell (com_min ≈ 0.1).

---

## 9. Pairing

Every joint torque is applied as $+\boldsymbol\tau$ on the child and $-\boldsymbol\tau$ on the parent, so $\sum_{\text{bodies}}\boldsymbol\tau = 0$ every tick (Tan et al. §4.3 Eq. 14–15: $\tau_2=-\tau_1$). *Argument:* the total angular momentum about any point changes only by external moments: gravity, ground contact. A pure couple pair adds none, so motors cannot translate or spin the body as a whole; any slide must come from contacts (forces) or an unpaired term. Lab: $\Sigma\tau$ max = 0 in all paired runs (e.g. `stand_paired.jsonl`, `sum_tau` 0.000000); unpaired: `stand_unpaired.jsonl` last ticks $|\Sigma\tau|$ 73–89 N·m. Note: foot-damping forces and balance torques are *intended* external-like terms routed through the floor reaction; they are not motors.

---

## 10. Strike boost

`phys_mass::strike_boost(t)`, $t$ = whole ticks since the strike event:
$$ b(t)=\begin{cases}1 & t\le5\\ 1-S(u),\ u=\frac{t-5}{4},\ S(u)=3u^2-2u^3 & 6\le t\le 8\\ 0 & t\ge9\end{cases} $$
Restart allowed when ticks since the last start $\ge 18$ (6 hold + 12 cooldown). Every start logged as an event. Applied as a Kp blend: $k_p(t) = k_{p,base} + b(t)\,(k_{p,strike}-k_{p,base})$; pass 6 v2 uses $k_{p,strike}=1.5\,k_{p,base}$ on arm joints only (`STRIKE_KP_SCALE`). Stable PD stays stable at any $k_p$ (§4.2), so the boost cannot cause divergence; its effect on strikes is **not measured**.

---

## 11. Safety

- **Fault cap (pass 6):** per joint, $|\boldsymbol\tau| > 3\,\text{m}\cdot Mg$ N·m (`SAFETY_LEVER_M = 3.0`, ≈ 2200 N·m at 75 kg) → clamp and log `fault`; non-finite $\boldsymbol\tau$ → zero and log. Never shapes motion. **Lab** fault counter is different: a requested *foot force* $>3Mg$ N; `fault_3Mg_ticks` was not written to the ss/rel JSON summaries (*not measured on disk*; reported 0).
- **NaN watchdog:** any non-finite position/velocity → log the first tick and freeze (Avian PR #651 notes velocity cascades to NaN in joint chains; released Avian clamps max speed only at integration, not in contact solving).
- **Max angular speed:** `MaxAngularSpeed(50.0)` per capsule (pass 6 `MAX_SPIN`).
- **CCD:** `SweptCcd::default()` + `SpeculativeMargin(0.05)` on feet and shins (pass 6). The lab has no CCD.
- **Floor:** a 1 m thick static box with top at $y=0$ (lab: 40×1×40 m cuboid at $y=-0.5$; pass 6 studio: 18×1×18). Below-floor checks: first tick with any body $y<-0.1$ m (`below_floor_tick`), and COM $y<-1$ m → `fell_through_floor`.

---

## 12. Voxel physics (`phys_voxel`)

**Indexing.** Chunk edge $C_S=16$, coarse edge $C_C=4$ (each coarse cell = $4^3$ fine). Local index $i = x + 16(z + 16y)$ ($x$ fastest, then $z$, then $y$); inverse $x=i\bmod16$, $y=\lfloor i/256\rfloor$, $z=\lfloor i/16\rfloor\bmod16$. World→chunk: $\mathbf k=\lfloor \mathbf w/16\rfloor$ (Euclidean), local $=\mathbf w \bmod_{E} 16$. Store = `BTreeMap<[i32;3],Chunk>`; absent chunk = empty.

**Coarse mask** (u64, bit $b = c_x + 4c_z + 16c_y$): bit set ⇔ any fine cell in that $4^3$ block is solid (strict nesting, XCube §3 p3, §3.1 p3, Fig. 3). Rebuilt only for chunks dirtied by a `VoxelEdit` (the only writer, applied at whole ticks before `PhysicsSystems::First`).

**Greedy box merge [team]** (`merge_boxes`): seeds scanned $y,z,x$; from each unused solid seed of material $m$, grow $x$ while free, then $z$ while the whole $x$-row is free, then $y$ while the whole $xz$-slab is free; mark used. Boxes are per material. Full chunk → 1 box. Collider: half-extents $(\mathbf{max}-\mathbf{min})s/2$, centre = chunk origin $+(\mathbf{min}+\mathbf{max})s/2$. Lab clearance scene: 2119 voxels → 9 boxes (reported).

**Mass properties [team]** (`mass_props`, f64): cell edge $s$, centre $\mathbf r=(\mathbf w+\tfrac12)s$, $m=\rho_{mat}s^3$. Solid cube about its centre: $I = \tfrac{m s^2}{6}E_3$. Accumulate about the origin, then shift to the COM:
$$ I_0=\sum_k\Big[m_k\big(\tfrac{s^2}{6}+|\mathbf r_k|^2\big)E_3 - m_k\mathbf r_k\mathbf r_k^{\mathsf T}\Big],\quad \mathbf c=\frac{\sum m_k\mathbf r_k}{\sum m_k},\quad I_c = I_0 - M\big(|\mathbf c|^2E_3-\mathbf c\mathbf c^{\mathsf T}\big). $$
Densities kg/m³: stone 2600, dirt 1500, wood 700, ice 917, metal 7800; friction μ: 0.8, 0.6, 0.5, 0.05, 0.4. For Avian, diagonalise $I_c$ (principal axes) for off-axis pieces.

**Debris [team]** (`split_debris`): 6-neighbour flood fill over solid cells in BTreeMap order; a component is anchored if any cell has $y\le y_{ground}$ or is in `anchors`; each unanchored component (cells sorted) becomes a body with `mass_props`. Replaces Vox-E's graph-cut (§3.3 p5, Eq. 5), which is not implemented.

**Height and slope [team]:** $h(x,z)$ = top solid $y$ + 1 (top face, voxels), scanning down; slope by central differences $\big(\tfrac{h(x{+}1)-h(x{-}1)}2,\ \tfrac{h(z{+}1)-h(z{-}1)}2\big)$, missing neighbours fall back to $h_0$. Multiply by $s$ for metres.

**Clearance [team; coarse level from XCube §3.1]** (`clear_aabb`): for each chunk overlapping the AABB: skip absent/empty chunks, skip clear coarse bits, test fine cells only inside set coarse cells. Lab: 152 fine tests vs 552 brute force with identical answers; a capsule settled at 0.900 m (reported).

---

## 13. Modal sway (`phys_modal`)

DynamicTree §4 p5, Eq. 6–7: per mode $k$, $m_k\ddot q_k + c_k\dot q_k + k_kq_k = f_k$; displacement $\mathbf D = \sum_k\boldsymbol\phi_k q_k$. We use $c_k/m_k = 2\zeta\omega_k$, $k_k/m_k=\omega_k^2$, $\omega_k=2\pi f_k$.
**Step [team]** (semi-implicit Euler, not the paper's explicit Euler):
$\dot q \mathrel{+}= \Delta t\,(f/m - 2\zeta\omega\dot q - \omega^2 q)$, then $q \mathrel{+}= \Delta t\,\dot q$.
**Stability:** undamped, the update matrix has $\operatorname{tr}=2-\omega^2\Delta t^2$, $\det = 1$; eigenvalues on the unit circle iff $|\operatorname{tr}|<2$ ⇔ $\omega\Delta t<2$. Code uses margin $\omega\Delta t<1.9$ ⇒ $f<1.9/(2\pi\Delta t)=36.3$ Hz at 120 Hz; unstable modes are frozen at rest. (Explicit Euler has $\det=1+\omega^2\Delta t^2>1$: always gains energy.)
**Impulse projection:** a logged hit impulse $j$ at point $\mathbf p$: $\dot q_k \mathrel{+}= j\,\phi_k(\mathbf p)/m_k$. Render-only; $q,\dot q$ are in our hashed state. Lab sway (3 props × 1/3/8 Hz, ζ 0.05, hit at tick 60): energy never rose after the hit with both steppers; two runs bit-identical (reported).

---

## 14. Lab results, settings, open problems

Lab body (`gb_lab/src/ragdoll.rs`): 13 bodies, $M=69.4$ kg [sum of `defs`: pelvis 10, torso 20, head 5, 2×(thigh 8, shin 4, foot 1.2, upper arm 2.5, forearm 1.5)], COM start 1.0088 m, floor friction 0.8, 40 s = 4800 ticks unless noted. Pass mark (brief §5): COM $y$ within 5 cm for 30 s, flips < 1/s, peak_sat < 0.9.

| Run (file) | Setting | com_min m | com_final m | drift_final m | flips/s | peak_sat |
|---|---|---|---|---|---|---|
| `ladder/r0` | lie, motors off, 2 s | 0.084 | 0.084 | 0.0117 | 2.0 | 0 |
| `ladder/r1` | paired only, unscaled kp 400 kd 40, clamp | 0.125 | 0.288 | drift_max 7.38 | 13.65 | 398 |
| `ladder/r2` | + gff | 0.130 | 0.273 | drift_max 5.75 | 13.1 | 429 |
| `ladder/r3` | + filter | 0.107 | 0.312 | drift_max 3.00 | 12.5 | 453 |
| `ladder/r4` | + balance | 0.122 | 0.248 | drift_max 3.00 | 12.0 | 459 |
| `stand_paired.jsonl` (end) | kp 400 kd 40 k 1.2 | — | 0.181 | 1.408 | — | — (reported 3723) |
| `stand_unpaired.jsonl` (end) | same, unpaired | — | 0.116 | 0.447 | — | — (reported 2790) |
| `diag/C2` | scaled, no clamp, kp 630 kd 50, no balance, 40 s | 1.009 | 1.011 | 0.005 | 1.45 | 9.58 |
| `diag/S_4000` | scaled, kp 4000 kd 126, no balance | 0.108 | 0.241 | — | — | — |
| `diag/SBn_4000` | + balance, gff off | 0.093 | 0.336 | — | — | — |
| `bal/g_bal` | kp 2500 kd 100, PD balance | 0.962 | 0.986 | 0.469 | 0.05 | 0.587 |
| `bal/g_cp` | same, capture point | 0.122 | 0.246 | — | — | — |
| **`bal/ro_5_0.3_12`** (= best) | kp 4000 kd 126 kb 5 kbd 0.3, 12 substeps | **0.981** | **0.993** | **0.052** (drift_max 0.384) | **0.05** | **0.644** |
| `bal/ro_5_0.6_24` | kbd 0.6, 24 substeps | 0.987 | 0.995 | 0.315 | 0.05 | 0.713 |
| `creep/a40__base` | best + abs foot damp 40, 12 ss | 0.981 | — | 0.015 | 0.05 | 0.634 |
| `ss/6_4000_126_5_0` | best at 6 substeps | 0.136 | 0.516 | — | — | — |
| `ss/6_4000_126_3_0` | kb 3, 6 ss | 0.965 | 0.988 | 0.609 | 0.05 | 0.611 |
| `ss/6_8000_179_3_0` | kp 8000 kd 179, kb 3, 6 ss | 0.976 | 0.989 | 0.259 | 0.05 | 0.631 |
| `ss/4_*` (all 24) | 4 substeps | ≤ 0.180 | — | — | — | — |

(— = not in that file or not extracted. `diag/C2` stays up without balance but fails the pass mark on flips (1.45/s) and peak_sat (9.6). Run names `ro_*_12`/`_24` encode substeps; older files have no `substeps` field.) At 6 substeps, kb 5 falls for every $k_p$ tried (500–8000); kb 3 stands only for $k_p\ge3000$. Nothing stands at 4. The 3×3 vs scalar $I_{red}$ relative difference is 0.39–0.59 in every ss/rel run.

**Current best settings:** Stable PD (§4) with 3×3 $I_{red}$; $k_p=4000$, $k_d=126$; reaction-form gravity_ff; PD balance $k_b=5$, $k_{bd}=0.3$, ankle-then-hip; contact 1 / 2.5 cm, filter 0.25; no shaping clamp, fault cap $3Mg$·m; **12 substeps**; foot damping 40 (absolute) available, default off.

**Open problems.**
1. *Substeps 12 vs 6.* Pass 6 v2 ships `SUBSTEPS 6`, `KB 5`, which falls in the lab. Suspected cause: Avian joint stretch at fewer substeps (motor $\omega_nh$ is only ~0.1). `SubstepCount` is global (affects the whole game).
2. *Creep.* ~1 cm/s with both feet planted; chaotic and solver-sensitive (16 substeps or friction 1.0 drift ~0.16 m, 80 s creeps to 0.34 m: reported). Motors-off lying body also creeps 1.2 cm in 2 s (`ladder/r0`).
3. $R_pO_p$ vs $R_p$ in the rate conversion (§3) unverified.
4. Strike, feed-forward, and the full Tan numerator: not measured.

**Next experiments.** (a) Per-body/per-joint stiffness to get 12-substep quality only on the ragdoll (check whether Avian 0.7 joint compliance / solver settings allow it). (b) 6 substeps + kb 3 + weak COM pull to start point. (c) Full Tan numerator $-(k_d+k_p\Delta t)\boldsymbol\omega_{rel}$ at 6 substeps. (d) Gate rows with `stable_pd3`/`reduced_inertia3` from `phys_balance.rs` (code now matches the lab's 3×3). (e) Jab/flare scenes once standing passes, logging $\boldsymbol\alpha$ jump and pose error.

## References (as verified in `hive/use/online_physics.md`)
Tan, Liu, Turk, *Stable PD Controllers*, IEEE CG&A 31(4) 2011 (§3.1 Eq. 3/5, Eq. 10–11, §4.3 Eq. 14–15). Pratt, Carff, Drakunov, Goswami, *Capture Point*, Humanoids 2006 (§V.C Eq. 7–10). Coros et al. 2010 (abstract only). Avian 0.7 CCD docs; Avian PR #651. XCube §3, §3.1; Vox-E §3.3 Eq. 5; DynamicTree §4 Eq. 6–7.

## Addendum (2026-10-15 07:23, team rulings)
- **§3 frame fix:** target rates ω*, α* are in the parent bone frame, so `rates_to_world_rel` must be passed $B_p = R_p O_p$ (parent capsule world rotation × bone offset), not $R_p$. This matches θ_err. Pass 6 is being corrected (ruling by The Rust Expert, confirmed by Animation).
- **§14 substep results (Grok Bot, avian3d 0.7.0, 3×3 I_red, 40 s stand):** 12 substeps + kb 5 → drift 3.4 cm, 0.6 ms/tick (only passing config). 12 + kb 3 → 0.24 m. 6 substeps + contact frequency 3 → kb 5 stands, kb 3 drift 0.15 m (fails). Contact damping ×2 → falls. 24 substeps → 0.33 m drift, so the residual is wander, not solver error. Avian 0.7 has no solver-iteration setting, joint compliance is already 0, and `SubstepCount` is global.
- **Ruling:** pass 6 uses 12 substeps, kb 5. Foot damping 40 and the full Tan term $-k_p\,dt\,\omega_{rel}$ are settings that start off. Contact frequency 3 is the fallback for 6 substeps. Next lab runs: the full Tan term and a weak com pull back toward the start point.
