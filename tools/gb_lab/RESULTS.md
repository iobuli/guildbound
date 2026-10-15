# gb_lab results (pass 6 review)

Headless Bevy/Avian lab, run on the agent box only.

## Fixes
- Motor law: inertia-scaled Stable PD (Tan et al. 2011):
  tau = I_red*(Kp*theta - Kd*w_rel) / (1 + Kd*dt + Kp*dt^2),
  I_red = I_p*I_c / (I_p + I_c), each body's own world inertia. Replaces flat-gain motors + per-joint clamp (light arms overshot and spun out).
- gravity_ff: reaction form, ground reaction applied at each foot's own centre of pressure (not whole-body CoM lever).

## Results
- Best stand: COM 0.981 / 0.993 m (min/mean), drift 5.2 cm over 40 s, contact flips 0.05/s, no floor tunnelling.
- Foot damping D40 (setting, default off): drift 8-31 mm across 6 configs; creep returns with small param changes or 80 s runs.
- Motors-off lie test: 1.2 cm in 2 s (floor/contacts OK).

## Open
- Substep gate at 6 and 4 substeps: in progress.
- Slow long-run creep; next: damp stance feet against CoM lateral velocity.
