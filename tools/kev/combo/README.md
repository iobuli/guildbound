# Kev combos

Chains catalogue clips, grid pose sheets, and keyed landmark poses into one animation. One hermite spline runs through the whole combo, so a move starts where the last one ended and the tangent carries through. Playback is the Studio baked-replay path (`staging/kev/baked/{model}__{name}.json`), played once and held on the last frame.

Axes, same as `/api/control`: `+X` character-left, `+Y` up, `+Z` forward, metres. Reach error stays near zero while a target is inside the limb. A single foot only supports the body by about 5.5 cm, so a kick stacks the plant-side hip over the ball of the stance foot.

## Studio

```
combo list
combo show jab_cross_hook_roundhouse
combo bake jab_cross_hook_roundhouse
combo play jab_cross_hook_roundhouse
combo play my_combo.json --model ual1.glb
```

`combo play` bakes, then sends the frames over the existing baked channel with `loop` false. The viewer has to be on a model first (`model ual1.glb`). Offline:

```
python3 -I tools/kev/combo/__main__.py list
python3 -I tools/kev/combo/__main__.py bake all
python3 -I tools/kev/combo/__main__.py check
```

`check` exits 1 if any combo fails a gate. Strips need Pillow (`python3 -m pip install pillow`). Without it the bake still writes the replay and the metrics.

## File

```json
{
  "name": "my_combo",
  "model": "ual1.glb",
  "fps": 60,
  "moves": [
    {"id": "jab", "do": "jab", "clip": "Punch_Jab", "limb": "wrist_l", "plant": "l",
     "step": 0.14, "hands": {"l": "fist", "r": "guard"},
     "dur": {"antic": 0.12, "strike": 0.09, "hold": 0.05, "overshoot": 0.05, "follow": 0.12},
     "arc": 0.04, "ramp": 1.1}
  ]
}
```

Drop it in `tools/kev/combo/library/` or pass the path to `combo play`.

`do` is one of:

| do | what it chains |
| --- | --- |
| `jab`, `cross`, `clip` | a catalogue clip's arm shape (`clip`, `limb`), feet retargeted onto the stepped plant |
| `hook`, `straight` | a keyed punch. `hook` keeps the elbow high and crosses the centre line |
| `uppercut` | rising fist, heels up on the ball, pelvis rises with it |
| `roundhouse`, `spin_back` | plant foot pivots on the ball, kicking foot stays airborne, hip stacked over the ball |
| `jump_kick` | crouch, tuck, extend, land forward |
| `sheet` | a grid sheet in `tools/kev/grid/sheets/` (`sheet`: `jab`, `cross`, `front_kick`, `jump_kick`). The strike is the middle pose with the highest foot |
| `offsets` | authored phases, see below |

Shared fields:

- `plant` `l` or `r`. That foot steps by `step` metres and pivots `pivot` degrees on the ball (positive turns the left shoulder forward). `pitch` lifts the heel, in degrees. The other foot drags during the wind-up (`catch`, 0 to 1, default 0.45) and catches up on the follow-through.
- `hands` per side: `fist`, `guard`, `relaxed`, `open`, `cast`, `claw`, or a number `0` open to `1` fist. Fingers blend across the spline. Punches ship as fists.
- `dur` seconds: `antic` (wind-up), `strike`, `hold` (hit-stop, the pose is repeated with zero tangents), `overshoot`, `follow`. `ramp` shortens the strike but floors it (antic >= 0.08, strike >= 0.09, hold >= 0.045) so a fast rush cannot collapse into one frame.
- `arc` metres. The limb bows off the straight line between wind-up and impact. The elbow stays on the same side as the rest of the strike.

Kicks pivot the plant foot on the ball. The kicking ankle is aimed from the hip, so the thigh turns through the chamber instead of swinging up off the floor in one sample.

### Offsets

Phases are `load`, optional `via`, `contact` (the hit), `follow`. Wrist and pelvis numbers are character-space offsets from the **entry** pose of that move, not stacked on the previous phase. `yaw` is degrees. `elbows` is `down` or `out`.

```json
{"id": "palm", "do": "offsets", "plant": "l", "step": 0.1, "reach": 0.94,
 "hands": {"l": "cast", "r": "cast"},
 "phases": {
   "load": {"pelvis": [0, -0.03, 0], "yaw": -12, "wrists": {"r": [0.04, -0.14, -0.06]}, "hands": {"l": "cast", "r": "cast"}},
   "contact": {"pelvis": [0, 0.02, 0.05], "yaw": 16, "wrists": {"r": [0.06, 0.04, 0.32]}, "hands": {"l": "open", "r": "open"}},
   "follow": {"wrists": {"r": [0.03, 0.02, 0.12]}, "hands": {"l": "cast", "r": "cast"}}
 }}
```

`x` is left, `y` is up, `z` is forward.

## Examples

All four bake on `ual1.glb`.

- `jab_cross_hook_roundhouse` uses `Punch_Jab` and `Punch_Cross`, then a keyed hook and a roundhouse.
- `rush_uppercut` is three fast punches into a rear uppercut. The rear heel lifts and the pelvis rises.
- `spin_back_jump_kick` pivots on the left ball into a back kick, then plays the `jump_kick` pose sheet.
- `brawler_mage` uses the same punch clips for the brawler half. `Brawler_*` / `Mage_*` in `tools/blender/brawler.py` are joint keys, not GLB clips, so the mage half is two `offsets` moves in that vocabulary: a palm thrust and a two-hand cast, hands opening on the hit.

## What keeps it from popping

- Hit and hold keys have zero tangents, so the impact pose holds.
- A planted foot is the same point on every key of the strike, and its tangent is zero.
- Elbow and knee poles may twist at most 32 deg per frame. A hand or an airborne foot may change aim at most 40 deg per frame. A pole that flips side (the forearm-roll pop near full extension) gets spread across frames instead of landing in one.
- Wrists are kept inside 93% of arm length, airborne ankles inside 95% of leg length. A sheet's short drawn spine is stretched back to the rig's neck before the arms are solved, so the shoulders the IK starts from are the rig's.

## Gates

Jerk is the same height-normalised third difference as `tools/kev/catalog/spatial.py`. It is reported and not gated: a snap is supposed to be sharp. The gates are the glitches.

| gate | limit |
| --- | --- |
| max joint-angle change per frame | 52 deg |
| segment-length drift vs rest | 1.5 cm |
| planted-foot slip | 1.5 cm |
| mean / max reach error | 3 cm / 8 cm |

Solved on `ual1.glb` at 60 fps (`results/metrics.json`):

| combo | sec | jerk | max step | bone | plant slip | drift | reach max | over foot | travel |
| --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| jab_cross_hook_roundhouse | 2.16 | 931 | 50.1 deg | lowerarm_l @ 11 | 1.27 cm | 0.01 cm | 0.0 cm | 0.66 | 0.92 m |
| rush_uppercut | 1.78 | 922 | 45.1 deg | lowerarm_l @ 9 | 0.20 cm | 0.01 cm | 0.0 cm | 0.82 | 0.49 m |
| spin_back_jump_kick | 1.61 | 1407 | 39.2 deg | upperarm_r @ 76 | 0.00 cm | 0.01 cm | 2.4 cm | 0.30 | 1.12 m |
| brawler_mage | 2.27 | 471 | 50.7 deg | upperarm_r @ 32 | 0.00 cm | 0.01 cm | 0.0 cm | 1.00 | 0.51 m |

`over_foot` is the fraction of frames the balance check puts the centre of mass over a foot. The spin/jump spends the airborne frames off that support, which is the jump. Strips (side row, then front row, hit frames included) are `results/<name>.png` and `results/contact_sheet.png`.

`tools/kev/combos.py` is the older clip-to-clip scorer. This package does not use it.
