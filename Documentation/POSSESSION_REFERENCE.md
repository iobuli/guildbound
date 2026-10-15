# Possession reference correction

The shared pose pipeline now restores bind lengths before IK, compares matching animations against the original RPG rig, applies model-specific joint limits, and keeps arms clear of the measured torso. Fighters and the Studio use the reference pass. The reference scene is hidden and each actor has its own source clock.

Joint rotations are transferred in aligned anatomical frames, with per-segment rest alignment. Segment length ratios are reported separately; bones retain the target model's lengths. In particular, wrists use their finger anchor as the reference direction. Clips absent from the source keep their existing animation and physical constraints.

Studio clip playback uses the model's verified foot-contact curves. A plant captures a world anchor; the foot releases on the clip's contact phase or when the raw pose moves beyond its reach. Pelvis reach correction and two-bone IK preserve segment lengths. Gameplay keeps its existing motor and support solver.

`gb.possess` reports `reference` (mapped joints, angular corrections, length ratios), `clip_support` (plant phase, contact, weight, anchor error), and `clearance` (estimated forearm penetration into the fitted torso). The centre-of-mass balance display is explicitly diagnostic. Weight comes from reference motion and contact/reach constraints; this is not an active ragdoll controller. Clothing and finger self-intersections are not covered by the torso approximation.

The Possession checkbox **Reference joint correction** toggles the reference pass. F2 returns to play. Changes remain uncommitted.

To show reference and target clips live without screenshots:

```powershell
python -I Tools/possession_live.py --reference --seconds 6
```

## Generated motion experiment

`gb.possess` accepts `motion`, plus `play`, `frame` and `step`. Motion keys supply bind-relative XYZ bone rotation offsets in degrees, a pelvis offset in metres, optional left/right grips, and optional `hands_m` targets in stage-relative metres. The engine interpolates poses locally, solves arm and planted-foot IK, then applies clearance and joint limits. `motion: null` returns to clip playback. Submitting a motion opens the Possession tab.

```json
{
  "motion": {
    "loop": true,
    "plants": ["left", "right"],
    "keys": [
      {"time": 0, "bones": {"B_Pelvis": [0, 0, 0]}, "pelvis_m": [0, -0.1, 0]},
      {"time": 1, "bones": {"B_Pelvis": [0, 12, 0]}, "pelvis_m": [0.06, -0.14, 0]},
      {"time": 2, "bones": {"B_Pelvis": [0, 0, 0]}, "pelvis_m": [0, -0.1, 0]}
    ]
  },
  "play": true
}
```

Generated poses are an experimental control surface, not finished fight animation. The first jab/cross/hook demo was deliberately simple and the boss judged it too basic. Existing authored motion is the current reference for improving it. Source/target live comparison captures and commands are local under `Documentation/checks/live/`.

Validation: the initial change passed 96 headless workspace tests. Later additions were compiled and exercised live; no further test runs were made after the boss requested live-only iteration. The last live reference sample compared 24 joints, reported no anatomical violations or estimated forearm penetration, and held the planted foot within 0.002 mm. These are individual samples, not a guarantee for every animation.
