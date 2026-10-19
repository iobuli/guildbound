"""Evaluate clips, combos and body models from one JSON plan.

    python3 tools/kev/evaluate.py tools/kev/plan.json

The plan (see plan.json) names the clip folder, the body models, the reference rig and the thresholds.
One run does four things, in order, and writes everything under the plan's `out` folder:

  clips    per clip: survey verdict, jitter ratio (survey.py), foot slide (m/s, measured against the
           pelvis so a treadmill clip's travel is not counted), duration, keyframes.
  combos   every clip-to-clip transition scored by combos.py; the best next clips and the cheapest
           chains of `combo_length` clips.
  bodies   every hero model (body and outfit pieces): bone match, height, verdict (survey.py).
  sheets   contact sheets: each top chain played on the hero body, two frames per clip, and each body
           piece on the idle pose. Look at them before trusting the ranking in report.md.

Nothing under assets/ is written. Stdlib, numpy, pygltflib, scipy and Pillow (through the model tools).
"""
import argparse
import glob
import json
import math
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
for sub in (HERE, os.path.join(REPO, "tools", "anim"), os.path.join(REPO, "tools", "model")):
    sys.path.insert(0, sub)

import survey  # noqa: E402
import combos  # noqa: E402

def repo(path):
    return path if os.path.isabs(path) else os.path.join(REPO, path)


def foot_slide(clip, planted, fps):
    """(mean, p95) planted-foot slide speed in m/s, measured relative to the pelvis."""
    speeds = []
    frames = clip.frames
    for f in range(1, frames):
        w0, w1 = clip.world(f - 1), clip.world(f)
        p0, p1 = clip.pos(w0, "pelvis"), clip.pos(w1, "pelvis")
        for i, side in enumerate(("l", "r")):
            if planted[f - 1][i] and planted[f][i]:
                a = np.asarray(clip.pos(w0, "foot_" + side)) - p0
                b = np.asarray(clip.pos(w1, "foot_" + side)) - p1
                d = np.linalg.norm((b - a)[[0, 2]]) * fps
                speeds.append(float(d))
    if not speeds:
        return None, None
    return float(np.mean(speeds)), float(np.percentile(speeds, 95))


def clip_entry(path, ref_names, thr):
    from posture_audit import FPS, audit  # imported here: it reads the GLB itself

    name = os.path.splitext(os.path.basename(path))[0]
    facts = survey.analyse((path, os.path.relpath(path, REPO)))
    verdict = survey.verdict(facts, ref_names)
    entry = {
        "name": name,
        "verdict": verdict,
        "jitter_ratio": facts.get("jitter_p95"),
        "duration_s": None,
        "keys": None,
        "foot_slide_mean_mps": None,
        "foot_slide_p95_mps": None,
        "flags": [],
    }
    if facts.get("clip_list"):
        entry["keys"] = facts["clip_list"][0]["keys"]
    try:
        out, clip, ax, lowest = audit(path, name)
        entry["duration_s"] = out["duration"]
        mean, p95 = foot_slide(clip, out["planted"], FPS)
        entry["foot_slide_mean_mps"] = None if mean is None else round(mean, 3)
        entry["foot_slide_p95_mps"] = None if p95 is None else round(p95, 3)
    except Exception as exc:  # a clip the audit cannot read is reported, not fatal
        entry["flags"].append(f"audit failed: {type(exc).__name__} {exc}".strip())
    if entry["keys"] is not None and entry["keys"] < 10:
        entry["flags"].append("static (too few keys to judge)")
    if entry["jitter_ratio"] is not None and entry["jitter_ratio"] > thr["jitter_ratio"]:
        entry["flags"].append("jittery")
    if entry["foot_slide_p95_mps"] is not None and entry["foot_slide_p95_mps"] > thr["foot_slide_mps"]:
        entry["flags"].append("foot slide")
    return entry


def body_entry(path, ref_names, thr):
    rel = os.path.relpath(path, REPO)
    facts = survey.analyse((path, rel))
    verdict = survey.verdict(facts, ref_names)
    return {
        "path": os.path.relpath(path, repo("assets/characters/hero")),
        "verdict": verdict,
        "bones": facts.get("bones"),
        "canon_match": facts.get("canon_match"),
        "height_m": None if facts.get("height") is None else round(facts["height"], 3),
        "vertices": facts.get("vertices"),
        "in_height_range": facts.get("height") is not None and thr["height_m"][0] <= facts["height"] <= thr["height_m"][1],
    }


def chain_sheet(chain, donors, plan, out_png):
    """Two frames per clip, the chain played on the hero body, one row of frames per clip."""
    import preview
    import render

    body = preview.Part(os.path.join(repo(plan["bodies"]), plan["sheets"]["body"]))
    ims = []
    for clip_name in chain:
        stem = clip_name
        doc, clips, by_name = donors[stem]
        key = next(iter(clips))
        dur = clips[key][0]
        for t in (0.0, dur * 0.5):
            mats = preview.pose_mats(body.skin, f"{stem}/{key}", t, donors)
            surf = preview.surfaces([body], mats)
            ims.append(render.render(surf, (0, 1.0, 4.4), (0, 0.92, 0), size=(220, 380)))
    render.sheet(ims, cols=2 * len(chain)).save(out_png)


def body_sheets(plan, donors, out_dir):
    """Each hero body piece worn on the body, idle pose, one front view per piece: one grid per gender."""
    import preview
    import render

    idle_stem = plan["sheets"]["pose_clip"]
    if idle_stem not in donors:
        import gbmesh as gm
        doc = gm.Doc(os.path.join(repo(plan["clips"]), idle_stem + ".glb"))
        donors[idle_stem] = (doc, gm.clips(doc), {n.name: i for i, n in enumerate(doc.g.nodes)})
    idle_key = next(iter(donors[idle_stem][1]))
    for sex in ("male", "female"):
        folder = os.path.join(repo(plan["bodies"]), sex)
        if not os.path.isdir(folder):
            continue
        body_path = os.path.join(folder, "body.glb")
        pieces = sorted(f for f in os.listdir(folder) if f.endswith(".glb") and f != "body.glb")
        ims = []
        for piece in pieces:
            parts = [preview.Part(body_path), preview.Part(os.path.join(folder, piece))]
            hid = preview.hidden_nodes(parts[0], [os.path.splitext(piece)[0]])
            mats = preview.pose_mats(parts[0].skin, f"{idle_stem}/{idle_key}", 0.0, donors)
            ims.append(render.render(preview.surfaces(parts, mats, hidden=hid), (0, 1.0, 4.4), (0, 0.92, 0), size=(180, 320)))
        if ims:
            render.sheet(ims, cols=8).save(os.path.join(out_dir, f"bodies_{sex}.png"))


def run(plan_path):
    plan = json.load(open(plan_path))
    thr = plan["thresholds"]
    out = repo(plan["out"])
    sheets_dir = os.path.join(out, "sheets")
    os.makedirs(sheets_dir, exist_ok=True)

    ref_names = {n for n in survey.reference_names(repo(plan["reference"]))}

    # clips
    clip_paths = sorted(glob.glob(os.path.join(repo(plan["clips"]), "*.glb")))
    clip_rows = [clip_entry(p, ref_names, thr) for p in clip_paths]
    print(f"clips: {len(clip_rows)} scored")

    # combos
    pairs = {}
    for p in clip_paths:
        first, last = combos.pose_ends(p)
        if first:
            pairs[os.path.splitext(os.path.basename(p))[0]] = (first, last)
    names, score = combos.build_matrix(pairs)
    best = combos.best_next(names, score, 5)
    chains = sorted((combos.chain(n, names, score, plan["combo_length"]) for n in names), key=lambda c: c["total_degrees"])
    top_chains = chains[: plan["top_chains"]]
    print(f"combos: {len(score)} transitions, best chain {top_chains[0]['total_degrees']} deg")

    # bodies
    body_paths = sorted(glob.glob(os.path.join(repo(plan["bodies"]), "**", "*.glb"), recursive=True))
    body_rows = [body_entry(p, ref_names, thr) for p in body_paths]
    print(f"bodies: {len(body_rows)} scored")

    # sheets: each top chain, and the idle pose of every body piece
    donors = {}
    for chain in top_chains:
        for stem in chain["clips"]:
            if stem not in donors:
                from preview import gm  # noqa: F401 - checks the model tools import
                doc = gm.Doc(os.path.join(repo(plan["clips"]), stem + ".glb"))
                donors[stem] = (doc, gm.clips(doc), {n.name: i for i, n in enumerate(doc.g.nodes)})
    for i, chain in enumerate(top_chains):
        chain_sheet(chain["clips"], donors, plan, os.path.join(sheets_dir, f"chain{i + 1}.png"))
    body_sheets(plan, donors, sheets_dir)
    print(f"sheets: {len(top_chains)} chains and the body pieces in {sheets_dir}")

    report = {
        "plan": plan_path,
        "clips": clip_rows,
        "combos": {"best_next": best, "chains": top_chains, "transitions": len(score)},
        "bodies": body_rows,
    }
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "report.json"), "w") as f:
        json.dump(report, f, indent=1)
    write_markdown(report, os.path.join(out, "report.md"), plan["top_clips"])
    return report


def write_markdown(report, path, top):
    lines = ["# Evaluation", "", "## Top clips (fewest flags, then lowest jitter)", ""]
    good = [c for c in report["clips"] if not c["flags"] and c["verdict"] == "usable"]
    good.sort(key=lambda c: (c["jitter_ratio"] if c["jitter_ratio"] is not None else 9e9, c["foot_slide_p95_mps"] or 0))
    lines += ["| clip | jitter | foot slide p95 m/s | keys |", "|---|---|---|---|"]
    for c in good[:top]:
        jit = "-" if c["jitter_ratio"] is None else f"{c['jitter_ratio']:.3f}"
        lines.append(f"| {c['name']} | {jit} | {c['foot_slide_p95_mps']} | {c['keys']} |")
    lines += ["", "## Flagged clips", ""]
    for c in report["clips"]:
        if c["flags"]:
            lines.append(f"- {c['name']}: {', '.join(c['flags'])}")
    lines += ["", "## Best chains", ""]
    for c in report["combos"]["chains"]:
        lines.append(f"- {c['total_degrees']} deg: " + " > ".join(c["clips"]))
    lines += ["", "## Bodies", "", "| piece | verdict | bones | height | in range |", "|---|---|---|---|---|"]
    for b in report["bodies"]:
        lines.append(f"| {b['path']} | {b['verdict']} | {b['bones']} | {b['height_m']} | {b['in_height_range']} |")
    lines += ["", "Chains of idle clips rank highest because idle clips barely move, so their cuts cost almost nothing.",
              "Read the chain sheets for motion before treating a low score as a good combo.", ""]
    open(path, "w").write("\n".join(lines))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("plan")
    args = ap.parse_args()
    run(os.path.abspath(args.plan))


if __name__ == "__main__":
    main()
