"""Kev targets: where each strike of each hand-to-hand plan lands on each enemy body.

    python3 tools/kev/targets.py [--report tools/kev/results/h2h/report.json] [--strikes staging/kev/strikes.json]
                                 [--rigs RIG.glb ...] [--out tools/kev/results/h2h]

The attacker is the game's hero body. For each strike in the h2h pool, the fist's height at the strike's speed peak (the
game cutter's peak when the strike table has one, else the hand's fastest frame), interpolated between keys is measured with clip_fk as a share
of the attacker's height, then turned into metres with the hero body's height from nodes.py. Each enemy rig is
parsed by nodes.py at its own size. The fist lands in the body region whose vertical span (`span_h`, its joints'
lowest to highest point) holds that height, checked head first, then torso, then leg, or the nearest one when the
fist is between spans. A fist above the top of the skull is `over`, so it misses. A fist on the legs is `leg`,
a weak target.

Writes targets.json and targets.md: per plan and rig, the regions its chain lands in, and a score (head 2,
torso 1, leg 0.25, over 0) so a plan can be compared across body sizes.
"""
import argparse
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(REPO, "tools", "anim"))

import h2h  # noqa: E402
import nodes  # noqa: E402

HERO = "assets/characters/hero/male/body.glb"
RIGS = [
    "assets/characters/hero/male/body.glb",
    "assets/packs/company_characters/glb/elf.glb",
    "assets/packs/company_characters/glb/knight.glb",
    "assets/packs/company_characters/glb/mage.glb",
    "assets/packs/company_characters/glb/dragon.glb",
    "assets/packs/mech_00/glb/Mech_Char_Mesh.glb",
]
SCORE = {"head": 2.0, "torso": 1.0, "leg": 0.25, "over": 0.0, "under": 0.0}
# Top of the skull above the head joint, as a share of height (an adult head is about 1/8 of stature and the
# head joint sits near the skull's base).
SKULL = 0.12
# Region families checked top-down: the first whose span holds the fist height wins.
ORDER = [("head", ("head",)), ("torso", ("torso",)), ("leg", ("leg_l", "leg_r", "leg"))]


def units(rep):
    """Metres per rig unit (nodes.py guesses cm for heights over 20)."""
    return 0.01 if rep.get("units_guess") == "cm" else 1.0


def land(fist_m, rep):
    """The region a fist at `fist_m` metres above the floor lands in on the rig `rep` (nodes.parse).
    Spans come from joints, so a fist between two regions (between the top chest joint and the neck) takes the
    nearest one. The head joint sits at the skull's base, so the body ends SKULL above the head span's top."""
    h = rep["height"] * units(rep)
    x = fist_m / h
    regions = rep["regions"]
    spans = []
    for name, keys in ORDER:
        for k in keys:
            if k in regions:
                spans.append((name, *regions[k]["span_h"]))
    head_top = max((hi for name, _, hi in spans if name == "head"), default=1.0)
    if x > head_top + SKULL:
        return "over"
    if x < 0.0:
        return "under"
    for name, lo, hi in spans:
        if lo <= x <= hi:
            return name
    return min(spans, key=lambda sp: min(abs(x - sp[1]), abs(x - sp[2])))[0]


def fist_height(clip, side, peak_s, end_s=None):
    """Fist height above the lowest foot at the strike's speed peak (impact lands at full speed: a hook's
    fist is still sweeping down through it), linearly interpolated between the two keys around it, as a
    share of the head's height at frame 0. `end_s` is unused, kept for callers."""
    clip.precompute()
    last = clip.frames - 1
    x = min(max(peak_s * clip.fps, 0.0), float(last))
    f0 = int(np.floor(x))
    f1, u = min(f0 + 1, last), x - f0
    w0 = clip.world(0)
    floor = min(clip.pos(w0, "foot_l")[1], clip.pos(w0, "foot_r")[1])
    head = clip.pos(w0, "head")[1] - floor
    y = (1 - u) * clip.pos(clip.world(f0), f"hand_{side}")[1] + u * clip.pos(clip.world(f1), f"hand_{side}")[1]
    return float((y - floor) / head)


def fastest_frame(clip, side):
    clip.precompute()
    rel = np.array([np.asarray(clip.pos(clip.world(f), f"hand_{side}")) - np.asarray(clip.pos(clip.world(f), "pelvis")) for f in range(clip.frames)])
    v = np.r_[0.0, np.linalg.norm(np.diff(rel, axis=0), axis=1)]
    return int(np.argmax(v)) / clip.fps


def strike_heights(pool, timing, table):
    """name -> (track, fist height / attacker height) for every pool entry thrown with a hand."""
    from clip_fk import Clip, GLTF2

    loaded, out = {}, {}
    for e in pool:
        t = timing.get(e["track"])
        if not t:
            continue
        lib, name = e["track"].split("/", 1)
        if lib not in loaded:
            loaded[lib] = GLTF2().load(h2h.lib_path(lib))
        clip = Clip(h2h.lib_path(lib), name, gltf=loaded[lib])
        side = t["hand"]
        game = table.get(e["track"])
        peak = game["peak"] if game else fastest_frame(clip, side)
        end = game["end"] if game else None
        out[e["name"]] = (e["track"], round(fist_height(clip, side, peak, end), 3))
    return out


# Jump clips in the allowed libraries: a punch thrown at the top of one rides the jump's lift (the jump on the
# legs, the punch on the upper body, as two layered UAL clips).
JUMPS = ["ual2/NinjaJump_Start", "ual1/Jump_Start"]


def jump_lift(track):
    """Highest the lower foot leaves the floor in `track`, as a share of the head's height at frame 0."""
    from clip_fk import Clip, GLTF2

    lib, name = track.split("/", 1)
    c = Clip(h2h.lib_path(lib), name, gltf=GLTF2().load(h2h.lib_path(lib)))
    c.precompute()
    w0 = c.world(0)
    floor = min(c.pos(w0, "foot_l")[1], c.pos(w0, "foot_r")[1])
    head = c.pos(w0, "head")[1] - floor
    return float(max(min(c.pos(c.world(f), "foot_l")[1], c.pos(c.world(f), "foot_r")[1]) - floor for f in range(c.frames)) / head)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", default=os.path.join(HERE, "results", "h2h", "report.json"))
    ap.add_argument("--config", default=os.path.join(HERE, "h2h.json"))
    ap.add_argument("--strikes", default=os.path.join(REPO, "staging", "kev", "strikes.json"))
    ap.add_argument("--rigs", nargs="*", default=RIGS)
    ap.add_argument("--out", default=os.path.join(HERE, "results", "h2h"))
    args = ap.parse_args()
    report = json.load(open(args.report))
    pool = json.load(open(args.config))["pool"]
    table = h2h.load_strike_table(args.strikes) if os.path.exists(args.strikes) else {}
    rules = nodes.load_rules()
    hero = nodes.parse(os.path.join(REPO, HERO), rules)
    hero_m = hero["height"] * units(hero)
    heights = strike_heights(pool, report["timing"], table)
    rigs = {}
    for rel in args.rigs:
        path = os.path.join(REPO, rel)
        if os.path.exists(path):
            rep = nodes.parse(path, rules)
            if not rep.get("error"):
                rigs[os.path.splitext(os.path.basename(rel))[0]] = rep
    lands = {s: {r: land(h * hero_m, rep) for r, rep in rigs.items()} for s, (_, h) in heights.items()}
    lifts = {j: jump_lift(j) for j in JUMPS}
    jump, lift = max(lifts.items(), key=lambda kv: kv[1])
    jump_lands = {s: {r: land((h + lift) * hero_m, rep) for r, rep in rigs.items()} for s, (_, h) in heights.items()}
    plans = []
    for p in report["plans"]:
        row = {"enemy": p["enemy"]["name"], "chain": p["chain"], "rigs": {}}
        for r in rigs:
            hit = [lands[s][r] if s in lands else "?" for s in p["chain"]]
            row["rigs"][r] = {"lands": hit, "score": round(sum(SCORE.get(x, 0.0) for x in hit) / max(len(hit), 1), 2)}
        plans.append(row)
    os.makedirs(args.out, exist_ok=True)
    body = {
        "attacker_height_m": round(hero_m, 3),
        "rig_height_m": {r: round(rep["height"] * units(rep), 3) for r, rep in rigs.items()},
        "fist_height": {s: {"track": t, "share_of_attacker": h, "metres": round(h * hero_m, 3)} for s, (t, h) in heights.items()},
        "lands": lands, "plans": plans,
        "jump": {"track": jump, "lift_m": round(lift * hero_m, 3), "lifts_m": {j: round(v * hero_m, 3) for j, v in lifts.items()}, "lands": jump_lands},
    }
    with open(os.path.join(args.out, "targets.json"), "w") as f:
        json.dump(body, f, indent=1)
    names = list(rigs)
    lines = ["# Where each strike lands, per enemy body", "",
             f"Attacker: hero body, {hero_m:.2f} m. Fist height at the strike's peak, against each rig at its own size.", "",
             "| strike | fist (m) | " + " | ".join(f"{r} ({body['rig_height_m'][r]} m)" for r in names) + " |",
             "|---|---|" + "---|" * len(names)]
    for s, (t, h) in heights.items():
        lines.append(f"| {s} | {h * hero_m:.2f} | " + " | ".join(lands[s][r] for r in names) + " |")
    lines += ["", f"## Thrown at the top of a jump ({jump}, feet {lift * hero_m:.2f} m off the floor)", "",
              "The jump on the legs and the punch on the upper body, both UAL clips. Not yet in the game: it needs a layered attack.", "",
              "| strike | fist (m) | " + " | ".join(names) + " |", "|---|---|" + "---|" * len(names)]
    for s_, (t, h) in heights.items():
        lines.append(f"| {s_} | {(h + lift) * hero_m:.2f} | " + " | ".join(jump_lands[s_][r] for r in names) + " |")
    lines += ["", "## Plans (score per strike: head 2, torso 1, leg 0.25, over 0)", "",
              "| vs | chain | " + " | ".join(names) + " |", "|---|---|" + "---|" * len(names)]
    for p in plans:
        lines.append(f"| {p['enemy']} | {' > '.join(p['chain'])} | " + " | ".join(f"{p['rigs'][r]['score']}" for r in names) + " |")
    with open(os.path.join(args.out, "targets.md"), "w") as f:
        f.write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
