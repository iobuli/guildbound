# Posing from a reference picture: the method, and what each pose taught

The goal is to make this faster every time. Each lesson below becomes a default or an automatic check in
Kev or the rig.

## Loop (per pose, in isolation)
1. **Read keypoints.**
   - Command: `tools/blender/refgrid.py <picture> x0 y0 w h out.png --zoom 4`.
   - Write the joints into `refs/<set>.json`: crown, nose, neck, shoulder, elbow, fist, hip, knee, ankle,
     heel, toe, for both kick and support sides.
   - Set `top` and `floor`. Set `height_px` when the figure leans: standing height is about 2 × the support
     hip-to-ankle distance.
   - Write the expected facings in view words (forward = toward the target, left = toward the camera).
2. **Check.**
   - Command: `tools/blender/posecheck.py <Set.phase>`.
   - Blender renders side (onion-skinned on the chart), front, three-quarter and the hand.
   - Kev reports: keypoint errors in heights, segment angles, facings, balance.
3. **Fix the largest error in Kev's report.** Rerun. Stop when:
   - every segment is within about 5°
   - the facings match
   - the centre of mass is over the support area
   - the views read right

   Leftover keypoint offsets are proportion differences, and they are fine. The motion morphs anyway
   (shoulders go back to carry the leg).
4. **Motion.**
   - Command: `karate.py --only <Set> --dump-frames DIR`, then `bodymap.py track ... --fps 60`.
   - Check: peak foot speed, support-foot slip, and the centre of mass in single support.

## Lessons (now defaults)
- **The knee and elbow are hinges.** The bone front = bone direction × hinge axis, built the same way at rest.
  A pole used directly as the front twists a hanging shin 180°.
- **Lifted limbs go in joint angles.** Hip, knee and ankle; shoulder, elbow and wrist. Blending the angles
  gives the snap kick's arc around the knee. Planted feet use IK, and a `plant` weight blends the two so the
  foot lands without a pop.
- **Arms hang in world space.** When the torso leans back by θ, a hanging arm needs shoulder extension of
  about θ, minus the arm's own forward angle in the chart.
- **Balance follows the lean.** Leaning back or away moves the centre of mass behind the foot, so move the
  pelvis forward and toward the support side (about 2 to 4 cm per 10° of lean). Keep the hips over the support
  foot until the kicking foot touches down.
- **The support knee keeps a slight bend** (hips about 1.5 cm lower). Otherwise IK clamps and the foot slides.
- **Strike timing.** Use a "snap" ease: smoothstep on t^1.4, peaking at about 61% of the way. Front kick
  extension takes 0.13 s, for a foot peak of about 16 m/s. The pullback is equally fast.
- **Head tilt.** The chart's head tilts forward about 20 to 40° relative to the leaned chest. For a sideways
  lean, roll the head to keep it level.
- **Fists.** The fingers curl 80° per joint. The thumb is aimed segment by segment at index_02, middle_02 and
  ring_02.

## Step 3 is now automatic: `posefit.py`
- **Command:** `blender -b --factory-startup --python tools/blender/posefit.py -- <Set.phase> <params...> [--rounds N]`.
  Parameter paths look like `yaw`, `pel.0`, `tilt.1`, `head.2`, `r.thigh.1`, `r.knee`, `hl.shoulder.0`.
- **Speed:** about 7 s per fit, in one Blender process with no rendering. Results merge into `refs/fit.json`,
  which `karate.py` applies over its poses. Then run `posecheck.py` for one rendered look.
- **Cost:** segments (10° scale) + facings past 25° + balance outside the support (0.01 heights) + depth out of
  the kick's plane (`in_plane`, 2 cm) + keypoints (0.15 heights, light).
- **Yoko geri chamber:** fitted in 2 runs. By hand, mae geri had taken about 12 iterations.

## Lessons from the fitter
- **The 2D chart has no depth.** List the joints that must stay in the kick's plane (`in_plane`). Lock the yaw
  the technique implies (side kick: hips 90° side-on).
- **Keep the keypoint weight light.** The chart's legs are longer than the mannequin's, so a strong keypoint
  pull raises the leg to reach the chart's foot.
- **Coupled moves need a manual seed.** "Lean away more and lower the leg" changes two numbers together, and
  one-at-a-time descent can't find it. Seed the joint move, then refit. To do: search in pairs, or random
  restarts.
- **Fit only the parameters that matter.** Body yaw, hip height and head angle pull other segments; add them
  when the support thigh or neck stays off.
