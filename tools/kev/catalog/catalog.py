"""Animation catalogue with reference data.

    python3 -I catalog.py add-clip  NAME MODEL.glb CLIP [--tags a,b]
    python3 -I catalog.py add-sheet NAME SHEET.json     [--tags a,b] [--model karate.glb]
    python3 -I catalog.py smooth    NAME|all [--min-cut 2.0] [--beta 0.6]
    python3 -I catalog.py list | info NAME | rebuild

index.json (committed, small) lists every animation with where it came from, tags and before/after metrics.
The reference data itself - Kev landmark tracks at 60 fps, raw and smoothed - lives in staging/kev/refs (git-ignored)
and is rebuilt from the sources by `rebuild`. The Studio reads the same files: `ref play NAME`, or `cat play NAME`.
"""
import argparse
import importlib.util
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import spatial  # noqa: E402

INDEX = os.path.join(HERE, "index.json")
STUDIO = os.path.join(HERE, "..", "studio", "server.py")
FPS = 60


def studio():
    sys.path.insert(0, os.path.join(HERE, ".."))
    sys.path.insert(0, os.path.dirname(STUDIO))
    spec = importlib.util.spec_from_file_location("kev_studio_server", STUDIO)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def load_index():
    return json.load(open(INDEX)) if os.path.exists(INDEX) else {"animations": {}}


def save_index(ix):
    with open(INDEX, "w") as f:
        json.dump(ix, f, indent=1, sort_keys=True)


def ref_path(srv, name, raw=False):
    return os.path.join(srv.REFDIR, name + ("__raw" if raw else "") + ".json")


def arrays(ref):
    names = sorted(ref["samples"][0]["landmarks"])
    L = np.array([[s["landmarks"][n] for n in names] for s in ref["samples"]], float)
    return names, L


def with_landmarks(ref, names, L):
    out = dict(ref)
    out["samples"] = [{**s, "landmarks": {n: [round(float(x), 5) for x in L[i, k]] for k, n in enumerate(names)}}
                      for i, s in enumerate(ref["samples"])]
    return out


def ref_from_sheet(srv, sheet_path, model):
    sys.path.insert(0, os.path.join(HERE, "..", "grid"))
    import posesheet
    sheet = json.load(open(sheet_path))
    info = srv.model_info(model)
    lms = {n: posesheet.lift(sheet, p, srv, info) for n, p in sheet["poses"].items()}
    seq, total = sheet["sequence"], sheet["sequence"][-1][1]
    samples = []
    for k in range(int(round(total * FPS)) + 1):
        t = k / FPS
        i = max(j for j in range(len(seq)) if seq[j][1] <= t) if t < total else len(seq) - 2
        a, b = seq[i], seq[min(i + 1, len(seq) - 1)]
        u = 0.0 if b[1] == a[1] else min(1.0, (t - a[1]) / (b[1] - a[1]))
        u = posesheet.ease(b[2] if len(b) > 2 else "smooth", u)
        lm = {key: [x + (y - x) * u for x, y in zip(lms[a[0]][key], lms[b[0]][key])] for key in lms[a[0]]}
        samples.append({"t": round(t, 4), "landmarks": lm, "hands": {}, "com": [0, 0, 0]})
    bm = info["bm"]
    # sheet landmarks are already in this rig's metres; scale_to() re-bases by pelvis, so store pelvis-relative poses
    return {"model": model, "clip": os.path.basename(sheet_path), "height": bm["body"]["height"], "floor": bm["body"]["floor"],
            "duration": samples[-1]["t"], "samples": samples}


def do_smooth(srv, name, min_cut, beta):
    ix = load_index()
    e = ix["animations"][name]
    raw = json.load(open(ref_path(srv, name, raw=True)))
    names, L = arrays(raw)
    H, floor = raw["height"], raw["floor"]
    before = spatial.metrics(L, names, FPS, floor, H)
    S, locked = spatial.smooth(L, names, FPS, floor, H, min_cut, beta)
    after = spatial.metrics(S, names, FPS, floor, H)
    with open(ref_path(srv, name), "w") as f:
        json.dump(with_landmarks(raw, names, S), f)
    e["metrics_raw"], e["metrics"] = before, after
    e["smoothing"] = {"min_cut_hz": min_cut, "beta": beta, "foot_frames_locked": locked}
    save_index(ix)
    return before, after


def add(srv, name, source, tags, ref):
    os.makedirs(srv.REFDIR, exist_ok=True)
    with open(ref_path(srv, name, raw=True), "w") as f:
        json.dump(ref, f)
    ix = load_index()
    ix["animations"][name] = {"source": source, "tags": tags, "model": ref["model"], "fps": FPS, "duration_s": ref["duration"]}
    save_index(ix)
    return do_smooth(srv, name, 4.0, 6.0)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd")
    ap.add_argument("args", nargs="*")
    ap.add_argument("--tags", default="")
    ap.add_argument("--model", default="karate.glb")
    ap.add_argument("--min-cut", type=float, default=4.0)
    ap.add_argument("--beta", type=float, default=6.0)
    a = ap.parse_args()
    tags = [t for t in a.tags.split(",") if t]
    if a.cmd == "list":
        for n, e in sorted(load_index()["animations"].items()):
            m = e.get("metrics", {})
            print(f"{n:22} {e['source']['type']:5} {e['duration_s']:5.2f}s  jerk {m.get('jerk', '-'):>8}  reach {m.get('wrist_reach_h', '-')}  tags {','.join(e['tags'])}")
        return
    if a.cmd == "info":
        print(json.dumps(load_index()["animations"][a.args[0]], indent=1))
        return
    srv = studio()
    if a.cmd == "add-clip":
        name, model, clip = a.args
        srv.grab(model, clip, name)
        ref = json.load(open(os.path.join(srv.REFDIR, name + ".json")))
        b, s = add(srv, name, {"type": "clip", "model": model, "clip": clip}, tags, ref)
    elif a.cmd == "add-sheet":
        name, sheet = a.args
        sheet = os.path.relpath(os.path.abspath(sheet), HERE)
        b, s = add(srv, name, {"type": "sheet", "sheet": sheet, "image": json.load(open(os.path.join(HERE, sheet)))["image"]}, tags,
                   ref_from_sheet(srv, os.path.join(HERE, sheet), a.model))
    elif a.cmd == "smooth":
        names = list(load_index()["animations"]) if a.args[0] == "all" else a.args
        for n in names:
            b, s = do_smooth(srv, n, a.min_cut, a.beta)
            print(f"{n:22} jerk {b['jerk']:>9} -> {s['jerk']:>9}   slip {b['foot_slip_hs']} -> {s['foot_slip_hs']}")
        return
    elif a.cmd == "rebuild":
        for n, e in load_index()["animations"].items():
            src = e["source"]
            if src["type"] == "clip":
                srv.grab(src["model"], src["clip"], n)
                ref = json.load(open(os.path.join(srv.REFDIR, n + ".json")))
            else:
                ref = ref_from_sheet(srv, os.path.join(HERE, src["sheet"]), e["model"])
            with open(ref_path(srv, n, raw=True), "w") as f:
                json.dump(ref, f)
            do_smooth(srv, n, e["smoothing"]["min_cut_hz"], e["smoothing"]["beta"])
            print("rebuilt", n)
        return
    print(f"{a.args[0]}: jerk {b['jerk']} -> {s['jerk']}, foot slip {b['foot_slip_hs']} -> {s['foot_slip_hs']}, reach {b['wrist_reach_h']} -> {s['wrist_reach_h']}")


if __name__ == "__main__":
    main()
