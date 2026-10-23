# Kev AutoAnim

Tracker-independent live motion bridge for Kev Studio. It turns AI pose landmarks and optional aligned depth into
stable 3D Kev targets, streams them to the existing rig solver, and records portable Kev reference JSON.

## Pipeline

```text
RGB/video -> pose tracker -> 2D keypoints --+
                                              +-> depth fusion -> confidence/outlier gate
depth camera or monocular depth ------------+                         |
                                                                       v
Kev naming -> metric calibration -> One-Euro smoothing -> foot locks -> /api/control
                                                                       +-> reference JSON -> ref bake
```

The package intentionally does not bundle a pose model. MediaPipe, MoveNet, YOLO-Pose, OpenPose, RealSense,
Azure Kinect, stereo depth, or a game/editor tracker can all emit the same frame contract. The core only needs
NumPy, remains deterministic, and can be tested without a camera or GPU.

## Quick test

From the repository root:

```powershell
python -I tools/kev/autoanim/cli.py simulate $env:TEMP\kev-motion.jsonl
python -I tools/kev/autoanim/cli.py process $env:TEMP\kev-motion.jsonl $env:TEMP\kev-reference.json
python -I tools/kev/autoanim/cli.py limit-test
python -I -m unittest discover -s tools/kev/autoanim -p "test_*.py"
```

With Kev Studio running in the browser:

```powershell
python -I tools/kev/autoanim/cli.py stream $env:TEMP\kev-motion.jsonl --realtime
```

To keep the capture, add `--output staging/kev/refs/my_take.json --name my_take`. Then in Kev Studio:

```text
ref bake my_take
ref baked my_take
```

## Frame contract

One JSON object per line:

```json
{"t":0.0,"space":"normalized_image","image":{"width":1280,"height":720},"landmarks":{"left_shoulder":{"x":0.58,"y":0.29,"z":-0.02,"confidence":0.98}}}
```

Supported spaces:

- `normalized_image`: X/Y are 0..1 and Z is tracker-relative depth. Best fallback; not metrically exact.
- `image_depth`: X/Y are pixels, Z is aligned metric depth. Include camera `fx, fy, cx, cy, width, height`.
- `camera_m`: camera coordinates in metres: X right, Y down, Z away.
- `kev`: already +Y-up/+Z-forward coordinates.

MediaPipe names are mapped to Kev automatically. Already-canonical Kev landmark names also pass through.

## Live RGB-D adapter

A camera adapter should run its chosen pose model, align depth to colour, then call:

```python
from autoanim import CameraIntrinsics, MotionPipeline, attach_depth

intrinsics = CameraIntrinsics(fx, fy, cx, cy, width, height)
fused = attach_depth(tracker_keypoints, aligned_depth_array, intrinsics, depth_scale=0.001)
frame = pipeline.process({
    "t": timestamp,
    "space": "image_depth",
    "image": {"width": width, "height": height},
    "intrinsics": intrinsics.__dict__,
    "landmarks": fused,
})
client.send(frame)
```

`attach_depth` samples a patch around each joint, rejects holes and depth edges with median/MAD filtering, and
reduces confidence when depth is missing. This matters more than dense depth: the rig solver needs reliable joint
depths, not a full depth image. Monocular depth can use the same path after scale calibration, but floor contacts
will be less stable than RGB-D or multi-view capture.

## Stability and limits

- Low-confidence joints predict briefly, then disappear instead of poisoning IK.
- Implausible point velocity is clamped before adaptive One-Euro smoothing.
- Feet lock only after low velocity near the estimated floor and release on a real lift or fast move.
- Kev's existing `quality.clean_targets` still enforces limb lengths, legal knee/elbow direction, floor, feet,
  palms, and fingers during solve.
- One camera cannot recover hidden limbs or exact twist. Two views, RGB-D, IMUs, or authored cleanup remain useful
  for combat contacts, fingers, spins, and floor-heavy moves.
- This layer creates motion data; it does not decide choreography. An AI planner can sequence takes using Kev's
  catalog/combos, while measured contacts and quality checks remain deterministic gates.
