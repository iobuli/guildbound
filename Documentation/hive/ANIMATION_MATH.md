# Guildbound animation maths (Animation agent)
Frames, units and conventions. All internal maths is f64 at a fixed DT = 1/120 s, and ticks are integers (u64).
Quaternions are unit, Hamilton product, and rotate a vector by v' = q v q*. Angles are in rad.

## 1. Rig (anim_rig.rs)
- Node{name, joint:i32 (index in skin.joints, else -1), parent, t, r}. There are 65 skin joints mapped onto 13 capsules.
- FK: W_i = W_parent · (t_i, r_i), so R_i = R_p·r_i and p_i = p_p + R_p·t_i. The self-check is |FK − bind| < 1e-4 (a synthetic rig gives 7.5e-17).
- Flex axis per bone: the axis of the smallest-variance rotation over a sample clip. "Purity" is the share of rotation about that axis.
- rest_diff(a,b,deg) lists bones whose rest rotation differs by more than deg: angle = 2·acos(|⟨q_a,q_b⟩|).

## 2. Bone targets
BoneTarget{rot, omega, alpha}, relative to the parent frame, with the target post-multiplied:
  local_target = base_local · rot
Derivatives from three samples q_{t-1}, q_t, q_{t+1} (f64), with Log = full-angle rotation vector (axis·angle):
  Δ₊ = Log(q_t* · q_{t+1}),   Δ₋ = Log(q_{t-1}* · q_t)
  ω_L = (Δ₊ + Δ₋) / (2·DT)
  α_L = (Δ₊ − Δ₋) / DT²
At a clip edge (a missing neighbour) the derivative is one-sided and mirrored, so α = 0 there. Neighbours are matched by bone id, never by index, so a bone appearing or disappearing doesn't zero ω.

## 3. Capsule chain targets (chain, chain_target)
A capsule joint covers skin joints j_1..j_n (parent to child). Its target is
  Q(t) = Π_{k=1..n} local_k(t)   (ordered product, parent first)
evaluated at t−1, t and t+1, then put through §2. This captures the intermediate bones between capsules.

## 4. Into the motor's world frame (rates_to_world_rel)
Rates are in the PARENT BONE frame. With B_p = R_p·O_p (the parent capsule's world rotation times its bone offset) and parent world angular velocity ω_p:
  ω* = B_p·ω_L
  α* = B_p·α_L + ω_p × (B_p·ω_L)
Callers pass B_p, not R_p, as `rp` to rates_to_world_rel.
Error (with capsule offset O, where bone_world = capsule_world·O):
  θ_err = Log( (R_p·O_p·local_target·O_c⁻¹) · R_c⁻¹ )   (full-angle rotation vector, axis·angle, world; matches PHYSICS_MATH)
ω_rel = ω_c − ω_p.

## 5. Motor (the Rust Expert's writer, pass 6 ruling)
Stable PD [Tan et al. 2011], 3×3 reduced inertia:
  I_red = (I_c⁻¹ + I_p⁻¹)⁻¹   (each body's own world inertia, not the subtree's)
  τ = I_red·[ (Kp·θ_err + Kd·(ω* − ω_rel)) / (1 + Kd·dt + Kp·dt²) + α* ] + τ_gravity_ff + τ_bal
Feed-forward uses the same I_red [ruled]. Apply +τ to the child and −τ to the parent, so Στ = 0.
Kp = ω_n² and Kd = 2ζω_n. Pass 6: KP 4000, KD 126.

## 6. Strike boost
Kp_arm → 1.5·Kp for 6 ticks around the strike tick, easing off, then a 12-tick cooldown. Arm joints only.
The strike window and peak-speed tick come from StrikePhases; the peak tick = argmax_t |ω*_hand(t)|.

## 7. Flare and acro
- Flare: a momentum-carrying layer added on top of the base pose. Its bone set is fixed (including SPINE2), so there is no step on the strike tick. finished() is tested in whole ticks.
- Acro (handspring and flips): keyframed root path plus bone targets. Outside the clip, raw() is empty, and an empty clip is a no-op.
  Open issue: travel is 2.7 m and landing is about 5.4 m/s, which is too strong. Scale the root path by s, so v_land scales by s.
- Clearance gate before an acro starts [XCube §3.1 p3 coarse level, plus a team check]: the swept capsule volume must be free in the coarse occupancy level.

## 8. Trails (anim_trail_target.rs)
p_model(bone, t) = FK(pose_t) for each bone in the set; world = model_to_world·p_model (applied by Grok Bot).
Key ticks are flagged for the brighter strike window. Ghost trails use the target pose, so the gap between the target and the real trail is the tracking error. Foot error colours switch at 1 cm and 3 cm.

## 9. Gait
dpos is in the character's own frame (z forward) on purpose. Planned: foot placement from the height cache and stride adapting to ledges [team].

## 10. Voxel character (render-only) [GigaVoxels §2 p3–4, §4.4 p10]
A static voxel volume follows a skinned shell. First step [team]: rigid per-bone chunks, with a linear blend weight w(x) = clamp((x − (l − b))/(2b), 0, 1) across a band b at each joint.
Seam check: the worst gap between neighbouring voxels during a 150° bend must stay under 1 voxel.

## 11. B-spline smoothing (not in the build)
Uniform cubic B-spline on log-quaternions per bone. Gate: the α jump at window seams must be under 1%, checked live.

## 12. Open issues
- The hook gives every node a joint index (79 vs 65). It must use the position in skin.joints, and −1 otherwise.
- Pose edits must be the top layer in apply_edits.
- Bone names are guesses until the [anim_rig] log is checked.
