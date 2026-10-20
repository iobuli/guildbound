"""Blind variations: each kick re-aimed at new targets with no reference picture, fitted from body context alone.

    blender -b --factory-startup --python tools/blender/variants.py -- [--render DIR] [--export GLB]

For every (kick, target) in `TARGETS`, the perfected kick pose is the starting point and Kev's body map scores:
- the striking surface (ball of the foot for mae geri, heel for yoko / ushiro) to the target (2 cm scale)
- the centre of mass over the support foot
- the kick's facings (as in the chart fits)
- staying near the perfected pose (so it is still the same technique)
Coordinate descent on the kick's own parameters finds the pose. Each variation is then animated with the
technique's full sequence (shift, chamber, strike, lock, pullback, plant, settle) and chained into
`Karate_Variations`, with a red marker at each target while its kick is out.
Targets are in the body frame of the stance (Blender: +X left, -Y forward, +Z up), metres.
"""
import json
import math
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(HERE)), "tools/kev"))
import bpy  # noqa: E402
from mathutils import Vector  # noqa: E402
import karate  # noqa: E402
import bodymap  # noqa: E402

REPO = os.path.dirname(os.path.dirname(HERE))
TMP = "/tmp/variants_pose.json"
STRIKE = {"Mae_Geri": "ball_r", "Yoko_Geri": "heel_r", "Ushiro_Geri": "heel_r"}
PARAMS = {
    "Mae_Geri": ["r.thigh.0", "r.thigh.1", "r.knee", "r.foot", "tilt.0", "chest.0", "pel.0", "pel.1", "pel.2", "yaw"],
    "Yoko_Geri": ["r.thigh.1", "r.thigh.0", "r.thigh.2", "tilt.1", "chest.1", "pel.0", "pel.1", "pel.2", "yaw"],
    "Ushiro_Geri": ["r.thigh.0", "r.thigh.1", "tilt.0", "chest.0", "pel.0", "pel.1", "pel.2", "yaw"],
}
FACING = {"Mae_Geri": {"kneecap_r": "up", "sole_l": "down"}, "Yoko_Geri": {"sole_r": "forward", "sole_l": "down"},
          "Ushiro_Geri": {"sole_r": "forward", "toes_r": "down", "sole_l": "down"}}
# (kick, label, target): forward is -Y for mae, the kick side for yoko (body turns), behind for ushiro (body turns).
TARGETS = [
    ("Mae_Geri", "low", (-0.08, -0.70, 0.55)), ("Mae_Geri", "high", (-0.08, -0.62, 1.35)), ("Mae_Geri", "wide", (-0.38, -0.70, 1.00)),
    ("Yoko_Geri", "low", (0.0, -0.85, 0.75)), ("Yoko_Geri", "head", (0.0, -0.80, 1.55)),
    ("Ushiro_Geri", "low", (0.0, -0.85, 0.55)), ("Ushiro_Geri", "high", (0.0, -0.80, 1.20)),
]


def gltf(v):
    return np.array([v[0], v[2], -v[1]])


def fit(rig, bm, name, target, rounds=7):
    pz = json.loads(json.dumps(karate.POSES[name]["kick"]), object_hook=None)
    pz = {k: (tuple(v) if isinstance(v, list) else ({kk: (tuple(vv) if isinstance(vv, list) else vv) for kk, vv in v.items()} if isinstance(v, dict) else v)) for k, v in pz.items()}
    params = PARAMS[name]
    base = {p: karate.get_param(pz, p) for p in params}
    up, fwd = np.array(bm["body"]["up"]), np.array([0.0, 0.0, 1.0])
    left = np.cross(up, fwd)
    t = gltf(target)

    def cost():
        q = karate.full(pz)
        karate.pose(rig, q, karate.origin_for(q))
        karate.dump(rig, TMP)
        D = bodymap.load_pose(TMP, bm)
        L, F = bodymap.posed_landmarks(bm, D), bodymap.posed_facings(bm, D)
        c = (np.linalg.norm(L[STRIKE[name]] - t) / 0.02) ** 2
        c += (bodymap.balance(bm, L)["off_foot"] / 0.01) ** 2
        dirs = {"up": up, "down": -up, "forward": fwd, "back": -fwd, "left": left, "right": -left}
        for f, w in FACING[name].items():
            e = math.degrees(math.acos(max(-1.0, min(1.0, float(F[f] @ dirs[w])))))
            c += (max(0.0, e - 30) / 25) ** 2
        c += 0.02 * sum(((karate.get_param(pz, p) - base[p]) / (0.05 if p.startswith("pel") else 15.0)) ** 2 for p in params)
        return c, float(np.linalg.norm(L[STRIKE[name]] - t))

    best, miss = cost()
    steps = {p: (0.02 if p.startswith("pel") else 10.0) for p in params}
    for _ in range(rounds):
        for p in params:
            v0 = karate.get_param(pz, p)
            for d in (steps[p], -steps[p]):
                moved = False
                # Keep stepping while it helps, so a parameter can travel as far as the target needs.
                for _ in range(12):
                    karate.set_param(pz, p, v0 + d)
                    c, m = cost()
                    if c < best - 1e-6:
                        best, miss, v0, moved = c, m, v0 + d, True
                    else:
                        break
                if moved:
                    break
            karate.set_param(pz, p, v0)
        steps = {p: s / 2 for p, s in steps.items()}
    return pz, best, miss


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    opt = {argv[i]: argv[i + 1] for i in range(0, len(argv) - 1, 2)}
    with open(os.path.join(REPO, "staging/kev/bodymap/ual2.json")) as f:
        bm = json.load(f)
    rig = karate.setup()
    seq, marks = [], []
    shifts = {"Mae_Geri": karate.MAE_SHIFT, "Yoko_Geri": karate.YOKO_SHIFT, "Ushiro_Geri": karate.USHIRO_SHIFT}
    lands = {"Mae_Geri": karate.MAE_LAND, "Yoko_Geri": karate.YOKO_LAND, "Ushiro_Geri": karate.USHIRO_LAND}
    drive = {"Mae_Geri": 0.13, "Yoko_Geri": 0.20, "Ushiro_Geri": 0.20}
    for name, label, target in TARGETS:
        pz, c, miss = fit(rig, bm, name, target)
        print(f"VARIANT {name} {label}: target {target} miss {miss * 100:.1f} cm cost {c:.2f}")
        ch = karate.POSES[name]["chamber"]
        seq += [(karate.STANCE, 0.18, "smooth", "stance"), (shifts[name], 0.10, "smooth", "shift"), (ch, 0.18, "smooth", "chamber"),
                (pz, drive[name], "snap", f"kick:{name}:{label}"), (pz, 0.10, "linear", "lock"), (ch, 0.14, "smooth", "rechamber"),
                (lands[name], 0.18, "smooth", "land"), (karate.STANCE, 0.22, "smooth", "stance")]
        marks.append((name, label, target))
    karate.SEQUENCES["Variations"] = seq
    act, keys = karate.build(rig, ["Variations"], "Karate_Variations")
    act.use_fake_user = True
    sc = bpy.context.scene
    if "--render" in opt:
        out = opt["--render"].rstrip("/")
        karate.camera(sc)
        sc.render.resolution_x, sc.render.resolution_y = 960, 720
        co = sc.camera
        co.data.type = "PERSP"
        co.data.lens = 40
        co.location = (4.2, -3.6, 1.6)
        karate.look_at(co, (0, -0.3, 0.85))
        # A red marker at each target, shown while its kick is out.
        mat = bpy.data.materials.new("target")
        kicks = [(n, f) for (n, tag, f) in keys if tag.startswith("kick:")]
        for i, ((name, label, target), (_, f)) in enumerate(zip(marks, kicks)):
            bpy.ops.mesh.primitive_uv_sphere_add(radius=0.06, location=target)
            s = bpy.context.active_object
            s.color = (0.9, 0.1, 0.1, 1)
            for fr, hide in ((1, True), (f - 14, True), (f - 13, False), (f + 14, False), (f + 15, True)):
                s.hide_render = hide
                s.keyframe_insert("hide_render", frame=max(1, fr))
        rig.arm.animation_data.action = act
        sc.frame_start, sc.frame_end = 1, int(act.frame_range[1])
        sc.render.filepath = f"{out}/v_"
        bpy.ops.render.render(animation=True)
    if "--export" in opt:
        rig.arm.animation_data.action = None
        for o in bpy.data.objects:
            o.select_set(o.type in ("ARMATURE", "MESH") and not o.name.startswith("Sphere"))
        bpy.ops.export_scene.gltf(filepath=opt["--export"], export_format="GLB", use_selection=True,
                                  export_animation_mode="ACTIONS", export_force_sampling=True)
    print("VARIANTS DONE", [(n, t, f) for n, t, f in keys if t.startswith("kick:")])


main()
