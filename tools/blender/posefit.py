"""Kev fits a pose: the rig's numbers adjusted until Kev's body map says the pose matches the reference.

    blender -b --factory-startup --python tools/blender/posefit.py -- Yoko_Geri.kick r.thigh.0 r.thigh.1 pel.0 ... [--rounds 4]

Runs in one Blender process with no rendering: pose -> Kev pose file -> `bodymap.compare` + `balance` -> cost.
Cost = sum (segment angle error / 10 deg)^2 + sum (keypoint error / 0.08 heights)^2 (light: proportions differ) + sum (facing error past 25 deg / 25)^2 + (centre of mass outside the
support / 0.01 heights)^2. Coordinate descent: each parameter tried at +/- step, kept if the cost drops; the steps
halve each round (angles from 8 deg, lengths from 0.02 m). The result is merged into `refs/fit.json`, which
`karate.py` applies over its poses, and printed. Then `posecheck.py` renders it for a look.
"""
import json
import os

import numpy as np
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(HERE)), "tools/kev"))
import karate  # noqa: E402
import bodymap  # noqa: E402

REPO = os.path.dirname(os.path.dirname(HERE))
TMP = "/tmp/posefit_pose.json"


def main():
    argv = sys.argv[sys.argv.index("--") + 1:]
    rounds = int(argv[argv.index("--rounds") + 1]) if "--rounds" in argv else 4
    names = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--rounds")]
    target, params = names[0], names[1:]
    set_name, _, phase = target.partition(".")
    with open(os.path.join(REPO, "staging/kev/bodymap/ual2.json")) as f:
        bm = json.load(f)
    with open(os.path.join(HERE, "refs/karate.json")) as f:
        ref = json.load(f)
    rig = karate.setup()
    pz = karate.POSES[set_name][phase]

    def cost(verbose=False):
        q = karate.full(pz)
        karate.pose(rig, q, karate.origin_for(q))
        karate.dump(rig, TMP)
        D = bodymap.load_pose(TMP, bm)
        L, F = bodymap.posed_landmarks(bm, D), bodymap.posed_facings(bm, D)
        rows, segs, facing = bodymap.compare(bm, L, F, ref, set_name, phase)
        bal = bodymap.balance(bm, L)
        # Depth the side view cannot show: listed joints stay in the kick's plane (through the kicking hip).
        side = bodymap.unit(np.cross(np.array(bm["body"]["up"]), np.array(ref.get("view", {}).get("forward", bm["body"]["forward"]))))
        lm = ref["sets"][set_name]["landmarks"]
        hip = L[lm["hip_k"]]
        depth = [abs(float((L[lm[n]] - hip) @ side)) for n in ref["sets"][set_name]["phases"][phase].get("in_plane", [])]
        c = sum((d / 0.02) ** 2 for d in depth) + sum((d / 10) ** 2 for _, _, d in segs) + sum((max(0.0, e - 25) / 25) ** 2 for *_, e in facing) + (bal["off_foot"] / 0.01) ** 2 + sum((e / 0.15) ** 2 for *_, e in rows)
        if verbose:
            print("  segments", [(a, b, round(d, 1)) for a, b, d in segs])
            print("  depth (m)", [round(d, 3) for d in depth])
            print("  facings", [(n, w, g, round(e)) for n, w, g, e in facing], "balance off", round(bal["off_foot"], 3))
        return c

    best = cost(True)
    print(f"start cost {best:.3f}")
    steps = {p: (0.02 if p.split(".")[0] == "pel" else float(os.environ.get("FIT_STEP", 8.0))) for p in params}
    for r in range(rounds):
        for p in params:
            v0 = karate.get_param(pz, p)
            for d in (steps[p], -steps[p]):
                karate.set_param(pz, p, v0 + d)
                c = cost()
                if c < best - 1e-6:
                    best, v0 = c, v0 + d
                    break
            karate.set_param(pz, p, v0)
        steps = {p: s / 2 for p, s in steps.items()}
        print(f"round {r + 1}: cost {best:.3f}")
    cost(True)
    fit_path = karate.FITS
    try:
        with open(fit_path) as f:
            fits = json.load(f)
    except (OSError, ValueError):
        fits = {}
    fits.setdefault(target, {}).update({p: round(karate.get_param(pz, p), 4) for p in params})
    with open(fit_path, "w") as f:
        json.dump(fits, f, indent=1)
    print("FIT", target, fits[target])


main()
