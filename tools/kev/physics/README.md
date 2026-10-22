# Kev physics pass

A deterministic pass on landmark frames. The clip stays the author. Physics does not replace the animation and it does not import MuJoCo or PyBullet (neither is required, and neither is assumed to be installed). Turn any piece off with a config flag.

World axes match Studio: +Y up, +Z forward, +X character-left, metres.

## What one frame does

1. **Ground.** The stance ankle drives the root, so a foot that slides backward on a treadmill becomes the body travelling forward. The heel, ball or toe is pinned where it planted, including across a roll from heel to toe, and a sole point is rotated up rather than left through the floor. When that ankle reverses (toe-off) it stops driving, so the pelvis does not reverse with it, but the sole stays pinned until it lifts. Drift slower than a stroll is ignored, so a landing that merely settles does not become a walk. Knockback can still slide a planted foot; friction (`mu * g`) stops it.
2. **Balance.** If the centre of mass falls outside the shoes (each planted foot is a heel-toe quad widened by 4.5 cm, not Studio's 5.5 cm single-foot slack), the pelvis and trunk step back over the support by at most 1.2 cm this frame, limited by leg reach. A swing foot that is about to land on an already-falling pose is biased toward the capture point.
3. **Momentum.** A foot that arrives faster than 0.55 m/s downward loads a spring in the body (a landing compresses, then stands back up). Airborne, horizontal travel from the last push coasts. `air_follow` blends the root toward a ballistic arc; it defaults to 0 so authored jumps are kept.
4. **Secondary.** Wrists, fingers and the head follow their targets on underdamped springs, then the wrist is put back on the forearm and the head on the neck so those bones never stretch.

`hit_stop(seconds)` freezes the last pose. `impulse([x, y, z])` adds root velocity (horizontal slides, upward hops, downward adds to the landing spring) and kicks the wrist and head springs the other way. A combo calls these on the strike frame.

Plausibility is reported, not simulated by a rigid-body engine: `mu_required` is the horizontal centre-of-mass acceleration while grounded, in units of g. A large value means the clip asks for more grip than `mu` (default 1).

## API

```python
from physics import correct, PhysicsPass

out = correct(frames, dt=1/30, rest=rest, events=[{"frame": 4, "hit_stop": 0.08},
                                                   {"frame": 4, "impulse": [0, 0, 2]}])
out["frames"]            # same shape as the input
out["diagnostics"]["before"]
out["diagnostics"]["after"]   # slip, penetration, balance violations, jerk, travel, mu_required
out["diagnostics"]["pass"]    # compression, pelvis shift, step, root offset

sim = PhysicsPass(floor=0.015, rest=rest)
sim.step(landmarks, 1/30)     # {"landmarks", "diag"}
sim.hit_stop(0.08)
sim.impulse([0.0, 0.0, 3.0])
```

`frames` is a list of `{landmark: [x, y, z]}` or `{t, landmarks}` samples. `rest` is the bind pose in the same space (used for the sole and for leg reach). Knee landmarks may be the anatomical knee or an IK pole; poles are carried with the hip-ankle line and are not treated as a shin contact.

Flags in `config`: `contact`, `balance`, `momentum`, `secondary`, and `air_follow`.

Studio: `phys on|off|toggle|status|reset`. While it is on, `/api/control` runs the pass on the landmark targets before the kinematic solve and the viewer draws those targets in orange against the solved pose. `POST /api/physics` with `{"frames", "dt", "rest", "events"}` corrects a batch without the viewer. That is the entry a combo sequence should use.

## Metrics

`python3 -I tools/kev/physics/report.py` writes `results/metrics.json` and a PNG strip per clip (authored row, then corrected; +Z to the right, +Y up, each panel centred on its own pelvis so the pose can be compared; the centre-of-mass dot is yellow, the feet are marked).

Slip is the horizontal speed of a sole point (heel, ball or toe) while it is within 2 cm of the floor. Penetration is how far the lowest sole point goes through that floor. The floor is the bind-pose sole height when `rest` is passed (about 1.5 cm on UAL1), not the mesh origin. Balance counts a grounded frame whose centre of mass misses the shoe outline by more than 1 cm.

On the gait walk from `kev_walk.py` the stance foot is a treadmill. The pass turns that into root travel, drops the mean planted slip by more than an order of magnitude, and clears the floor. Catalogue punches keep their feet and lose the small penetrations. `Jump_Start` loses a large foot-through-floor error. Jerk (mean third derivative of the wrists, head and ankles) can rise on locomotion because the root now accelerates onto the skate and the foot roll moves the ankle; it should not explode, and a strike's wrist still settles onto the forearm instead of snapping.
