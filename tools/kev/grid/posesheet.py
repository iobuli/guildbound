"""Pose sheet -> animation. A pose sheet holds joints read off a gridded image (grid.py) as fractional cell coordinates;
this lifts each pose to 3D landmarks for a rig, solves it with Kev Studio's controller, eases between the poses and bakes
the result as a track the Studio plays.

    python3 -I posesheet.py verify SHEET.json OUT.png        # draw the read joints on the grid image
    python3 -I posesheet.py bake   SHEET.json MODEL.glb NAME  # -> staging/kev/baked/MODEL__NAME.json

Sheet format (see sheets/*.json): image, box, cell, zoom, facing ("right"|"left"), floor_row, poses{name: joints},
sequence [[pose, time_s, ease]...]. Joints: crown nose neck shoulder hip (single points: the body's midline) and
elbow_l wrist_l elbow_r wrist_r knee_l ankle_l toe_l knee_r ankle_r toe_r. Side view: the image gives forward and
height, the left/right offsets come from a fixed body half-width (per-joint "lat" in the sheet overrides it).
"""
import importlib.util
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
STUDIO = os.path.join(HERE, "..", "studio")
sys.path.insert(0, os.path.join(HERE, ".."))
sys.path.insert(0, STUDIO)
sys.argv = sys.argv[:1] + sys.argv[1:]

LAT = {"shoulder": 0.115, "hip": 0.055, "elbow": 0.15, "wrist": 0.12, "knee": 0.07, "ankle": 0.065, "toe": 0.07}


def studio():
    spec = importlib.util.spec_from_file_location("kev_studio_server", os.path.join(STUDIO, "server.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def lift(sheet, pose, srv, info):
    """One pose's joints -> landmark targets (world metres) for this rig."""
    bm = info["bm"]
    up, fwd = np.array(bm["body"]["up"], float), np.array(bm["body"]["forward"], float)
    left = np.cross(up, fwd)
    H, floor = bm["body"]["height"], bm["body"]["floor"]
    d = 1.0 if sheet.get("facing", "right") == "right" else -1.0
    s = H / (sheet["floor_row"] - pose["crown"][1])
    hip = np.array(pose["hip"], float)
    pelvis = srv.rest_lm(info, "pelvis")
    base = pelvis - up * (pelvis @ up) + up * (floor + (sheet["floor_row"] - hip[1]) * s)

    def at(joint, lat=0.0, name=None):
        c, r = pose[joint]
        lat = pose.get("lat", {}).get(name or joint, lat)
        return base + fwd * ((c - hip[0]) * d * s) + up * ((hip[1] - r) * s) + left * (lat * H)

    lm = {"pelvis": at("hip"), "hip_l": at("hip", LAT["hip"]), "hip_r": at("hip", -LAT["hip"]),
          "neck": at("neck"), "shoulder_l": at("shoulder", LAT["shoulder"]), "shoulder_r": at("shoulder", -LAT["shoulder"]),
          "crown": at("crown"), "nose": at("nose")}
    lm["head_joint"] = lm["neck"] + (lm["crown"] - lm["neck"]) * 0.3
    for side, sg in (("l", 1), ("r", -1)):
        for j in ("elbow", "wrist"):
            lm[f"{j}_{side}"] = at(f"{j}_{side}", sg * LAT[j], j)
        lm[f"knee_{side}"] = at(f"knee_{side}", sg * LAT["knee"], "knee")
        lm[f"ankle_{side}"] = at(f"ankle_{side}", sg * LAT["ankle"], "ankle")
        toe = at(f"toe_{side}", sg * LAT["toe"], "toe")
        lm[f"ball_{side}"] = lm[f"ankle_{side}"] + (toe - lm[f"ankle_{side}"]) * 0.7
    return {k: [float(x) for x in v] for k, v in lm.items()}


def ease(kind, u):
    return {"smooth": u * u * (3 - 2 * u), "snap": 1 - (1 - u) ** 3, "out": 1 - (1 - u) ** 2, "linear": u}.get(kind, u)


def bake(sheet, model, name, fps=60):
    srv = studio()
    info = srv.model_info(model)
    lms = {n: lift(sheet, p, srv, info) for n, p in sheet["poses"].items()}
    seq = sheet["sequence"]
    Q, P, frames, errs = {}, {}, [], []
    total = seq[-1][1]
    for k in range(int(round(total * fps)) + 1):
        t = k / fps
        i = max(j for j in range(len(seq)) if seq[j][1] <= t) if t < total else len(seq) - 2
        a, b = seq[i], seq[min(i + 1, len(seq) - 1)]
        u = 0.0 if b[1] == a[1] else min(1.0, (t - a[1]) / (b[1] - a[1]))
        u = ease(b[2] if len(b) > 2 else "smooth", u)
        lm = {key: [x + (y - x) * u for x, y in zip(lms[a[0]][key], lms[b[0]][key])] for key in lms[a[0]]}
        srv.apply_frame_into(info, Q, P, lm, None)
        for side in "lr":
            srv.curl_hand(info, Q, P, side, sheet.get("hands", 1.0))  # the sheet's figures hold fists
        frames.append({"q": {bn: [round(float(x), 5) for x in q] for bn, q in Q.items()}, "p": [round(float(x), 5) for x in P["pelvis"]]})
        m = srv.kev_read(info, Q, P)
        errs.append(float(np.mean([np.linalg.norm(np.array(m["landmarks"][key]) - np.array(lm[key])) for key in lm if key in m["landmarks"]])))
    out = {"model": model, "ref": name, "fps": fps, "duration": total, "frames": frames, "mean_cm": round(100 * float(np.mean(errs)), 2)}
    d = os.path.join(srv.REPO, "staging", "kev", "baked")
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, f"{model[:-4]}__{name}.json"), "w") as f:
        json.dump(out, f)
    return out


def verify(sheet, out, root):
    sys.path.insert(0, HERE)
    import grid
    from PIL import ImageDraw
    im = grid.overlay(os.path.join(root, sheet["image"]), sheet["cell"], sheet.get("box"), sheet["zoom"])
    d = ImageDraw.Draw(im)
    z = sheet["cell"] * sheet["zoom"]
    bones = [("crown", "neck"), ("neck", "shoulder"), ("shoulder", "hip"), ("shoulder", "elbow_l"), ("elbow_l", "wrist_l"),
             ("shoulder", "elbow_r"), ("elbow_r", "wrist_r"), ("hip", "knee_l"), ("knee_l", "ankle_l"), ("ankle_l", "toe_l"),
             ("hip", "knee_r"), ("knee_r", "ankle_r"), ("ankle_r", "toe_r")]
    for p in sheet["poses"].values():
        for a, b in bones:
            d.line([(p[a][0] * z, p[a][1] * z), (p[b][0] * z, p[b][1] * z)], fill=(255, 0, 255) if a.endswith("_r") or b.endswith("_r") or b == "knee_r" else (0, 200, 0), width=3)
        for k, v in p.items():
            if isinstance(v, list):
                d.ellipse([v[0] * z - 4, v[1] * z - 4, v[0] * z + 4, v[1] * z + 4], fill=(255, 200, 0))
    im.save(out)
    print("wrote", out)


if __name__ == "__main__":
    cmd, sheet_path = sys.argv[1], sys.argv[2]
    sheet = json.load(open(sheet_path))
    if cmd == "verify":
        verify(sheet, sys.argv[3], os.environ.get("GRID_IMAGES", "."))
    elif cmd == "bake":
        r = bake(sheet, sys.argv[3], sys.argv[4])
        print({k: r[k] for k in ("model", "ref", "duration", "mean_cm")}, len(r["frames"]), "frames")
