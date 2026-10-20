"""Kev hand-to-hand: one unarmed moveset per enemy weapon, built from measured clips and transition cost.

    python3 tools/kev/h2h.py [tools/kev/h2h.json] [--write]

Reads h2h.json (the plan: strike pool, punch rule, weights) and every weapon moveset as an enemy:
  reach   weapon length + arm reach (m);  speed  mean windup of its light attacks (s);  ranged  has `aim`.
Each strike in the pool is timed from its own motion with clip_fk: the striking hand is the one with the
higher speed relative to the pelvis, `active` is the window where that speed stays above `fast_fraction`
of its peak, `cancel` is where it drops below `recover_fraction`. Timings are fractions of the clip window,
as the game's Attack expects.

For each enemy a light chain of n strikes (n from the enemy's windup: short chains against fast weapons,
long ones against slow) is chosen by brute force over the pool, minimising
    sum of transition costs (combos.py: pose + motion mismatch at each cut, degrees)
  + same-hand penalty for two strikes in a row from one hand
  + windup weight * sum of windups (heavier against fast enemies).
The first strike's lunge grows with the enemy's reach; the guard's parry window grows with the enemy's
windup; absorb drops against heavy weapons (so evasion pays) and rises against ranged ones, whose arc is
wider; the heavy is a charge against ranged enemies, a shoulder bash against heavy weapons, else the base kick.

Without --write it prints the plan; with --write it writes assets/anim/movesets/<prefix><enemy>.json and
tools/kev/results/h2h/report.{json,md}. Hand strikes outside `hand_strike_libs` are refused (the user's rule).
"""
import argparse
import copy
import itertools
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(REPO, "tools", "anim"))

import combos  # noqa: E402
import survey  # noqa: E402

_DURATIONS = {}


def track_duration(track):
    """Seconds of a library clip ("lib/clip"), from the animation's sampler times."""
    lib, name = track.split("/", 1)
    if lib not in _DURATIONS:
        path = lib_path(lib)
        _DURATIONS[lib] = {}
        if os.path.exists(path):
            js, binary = survey.read_glb(path)
            for a in js.get("animations", []):
                end = max(float(survey.floats(js, binary, s["input"]).max()) for s in a["samplers"]) if a["samplers"] else 0.0
                _DURATIONS[lib][a.get("name", "")] = end
    return _DURATIONS[lib].get(name)


def lib_path(lib):
    return os.path.join(REPO, "assets", "anim", f"{lib}.glb")


def check_punch_rule(pool, libs):
    """Every hand strike (light, or any entry on a hand bone) must come from an allowed library."""
    for e in pool:
        lib = e["track"].split("/", 1)[0]
        hand = e["kind"] == "light" or e.get("bone", "hand").startswith("hand")
        if hand and not any(lib.startswith(prefix) for prefix in libs):
            raise SystemExit(f"{e['track']}: hand strikes must come from {libs} (the punch rule)")


def time_strike(clip, fast, recover, end=None):
    """Striking hand and (windup, active, cancel) fractions of the window [0, end or duration]."""
    clip.precompute()
    fps = clip.fps
    speeds = {}
    for side in ("l", "r"):
        rel = []
        for f in range(clip.frames):
            w = clip.world(f)
            rel.append(np.asarray(clip.pos(w, f"hand_{side}")) - np.asarray(clip.pos(w, "pelvis")))
        rel = np.array(rel)
        speeds[side] = np.r_[0.0, np.linalg.norm(np.diff(rel, axis=0), axis=1) * fps]
    side = max(speeds, key=lambda s: speeds[s].max())
    v = speeds[side]
    window = end if end else clip.duration
    last = min(len(v) - 1, int(round(window * fps)))
    v = v[: last + 1]
    peak = int(np.argmax(v))
    top = v[peak]
    start = peak
    while start > 0 and v[start - 1] >= fast * top:
        start -= 1
    stop = peak
    while stop < len(v) - 1 and v[stop + 1] >= fast * top:
        stop += 1
    cancel = stop
    while cancel < len(v) - 1 and v[cancel] >= recover * top:
        cancel += 1
    frac = lambda f: round(min(1.0, (f / fps) / window), 3)  # noqa: E731
    return side, {"windup": frac(start), "active": max(frac(stop + 1), frac(start) + 0.05), "cancel": max(frac(cancel), frac(stop + 1)), "peak_mps": round(float(top), 2)}


def measure_pool(pool, timing):
    """Timing per pool entry, loading each library once (pygltflib parses slowly)."""
    from clip_fk import Clip, GLTF2

    loaded = {}
    out = {}
    for e in pool:
        lib, name = e["track"].split("/", 1)
        if lib not in loaded:
            loaded[lib] = GLTF2().load(lib_path(lib))
        clip = Clip(lib_path(lib), name, gltf=loaded[lib])
        side, t = time_strike(clip, timing["fast_fraction"], timing["recover_fraction"], e.get("end"))
        window = e.get("end") or clip.duration
        out[e["track"]] = {"hand": side, **t, "duration": round(clip.duration, 3), "windup_s": round(t["windup"] * window, 3)}
    return out


def ends_for(pool):
    libs = sorted({e["track"].split("/", 1)[0] for e in pool})
    ends = {}
    for lib in libs:
        for name, fl in combos.clip_ends(lib_path(lib)).items():
            ends[f"{lib}/{name}"] = fl
    return ends


def enemy_profiles(movesets_dir, base, prefix, arm_reach):
    out = []
    for f in sorted(os.listdir(movesets_dir)):
        if not f.endswith(".json"):
            continue
        with open(os.path.join(movesets_dir, f)) as fh:
            m = json.load(fh)
        if m["name"] == base or m["name"].startswith(prefix):
            continue
        weapon = m.get("weapon") or {}
        windups = []
        for a in m.get("light", []):
            clip = a["clip"]
            window = (clip.get("end") or track_duration(clip["track"]) or 1.0) - (clip.get("start") or 0.0)
            windups.append(a["windup"] * window)
        out.append({
            "name": m["name"],
            "kind": m.get("kind"),
            "reach_m": round(float(weapon.get("length", 0.0)) + arm_reach, 3),
            "mass": float(weapon.get("mass", 1.0)),
            "windup_s": round(float(np.mean(windups)), 3) if windups else None,
            "ranged": bool(m.get("aim")),
        })
    return out


def chain_cost(seq, ends, timing, by_track, rules, weight, impact, enemy_windup):
    """Transition cost + same-hand penalty + tempo penalty - impact (poise) reward.

    Tempo: a strike only costs for the time its windup runs past the enemy's own windup, the window in
    which the enemy's swing lands first. Against a slow weapon a slow, heavy strike is free; against a
    fast one only quick strikes are."""
    cost = 0.0
    for a, b in zip(seq, seq[1:]):
        t = combos.transition(ends[a][1], ends[b][0])
        # A crossfade hides a small cut; only the part above `crossfade_hides_deg` costs.
        cost += max(0.0, combos.total(t) - rules["crossfade_hides_deg"]) if t else 90.0
        if timing[a]["hand"] == timing[b]["hand"]:
            cost += rules["same_hand_penalty"]
    cost += weight * sum(max(0.0, timing[s]["windup_s"] - enemy_windup) for s in seq)
    cost -= impact * sum(by_track[s]["poise"] for s in seq) / len(seq)
    return cost


def best_chain(light, ends, timing, by_track, rules, enemy):
    windup = enemy["windup_s"] if enemy["windup_s"] is not None else 0.6
    n = next(n for limit, n in rules["chain_length_by_windup"] if windup <= limit)
    fast = windup <= rules["fast_enemy_windup"]
    weight = rules["windup_weight_fast_enemy"] if fast else rules["windup_weight_slow_enemy"]
    impact = rules["impact_weight_fast"] if fast else rules["impact_weight_slow"]
    best = None
    for seq in itertools.product(light, repeat=n):
        if any(a == b for a, b in zip(seq, seq[1:])):
            continue
        c = chain_cost(seq, ends, timing, by_track, rules, weight, impact, windup)
        if best is None or c < best[0]:
            best = (c, seq)
    return round(best[0], 2), list(best[1])


def attack(entry, t, travel):
    a = {
        "name": entry["name"],
        "clip": {"track": entry["track"], **({"end": entry["end"]} if entry.get("end") else {})},
        "windup": t["windup"], "active": t["active"], "cancel": t["cancel"],
        "damage": entry["damage"], "poise": entry["poise"], "knockback": entry["knockback"],
        "stamina": entry["stamina"], "travel": round(travel, 2),
        "hitbox": {"bone": entry.get("bone", f"hand_{t['hand']}"), "radius": 0.18 if entry.get("bone") else 0.1},
    }
    if entry.get("recover"):
        a["recover"] = {"track": entry["recover"]}
    return a


def build(plan):
    rules, pool = plan["rules"], plan["pool"]
    check_punch_rule(pool, plan["hand_strike_libs"])
    movesets_dir = os.path.join(REPO, plan["movesets"])
    with open(os.path.join(movesets_dir, plan["base"] + ".json")) as f:
        base = json.load(f)
    timing = measure_pool(pool, plan["timing"])
    ends = ends_for(pool)
    by_track = {e["track"]: e for e in pool}
    light = [e["track"] for e in pool if e["kind"] == "light"]
    enemies = enemy_profiles(movesets_dir, plan["base"], plan["out_prefix"], rules["arm_reach_m"])
    sets, report = {}, []
    for enemy in enemies:
        cost, seq = best_chain(light, ends, timing, by_track, rules, enemy)
        first_travel = min(rules["travel_max"], rules["travel_base"] + rules["travel_per_m_reach"] * max(0.0, enemy["reach_m"] - 1.0))
        m = copy.deepcopy(base)
        m["name"] = plan["out_prefix"] + enemy["name"]
        m["light"] = [attack(by_track[t], timing[t], first_travel if i == 0 else rules["travel_base"]) for i, t in enumerate(seq)]
        heavy_weapon = enemy["mass"] >= rules["heavy_weapon_mass"]
        if enemy["ranged"]:
            heavy = next(e for e in pool if e["kind"] == "charge")
            m["heavy"] = [attack(heavy, timing[heavy["track"]], rules["travel_max"])]
        elif heavy_weapon:
            heavy = next(e for e in pool if e["kind"] == "heavy")
            m["heavy"] = [attack(heavy, timing[heavy["track"]], first_travel)]
        windup = enemy["windup_s"] if enemy["windup_s"] is not None else 0.6
        g = m["guard"]
        lo, hi = rules["parry_range"]
        g["parry_window"] = round(min(hi, max(lo, rules["parry_base"] + rules["parry_per_s_windup"] * windup)), 3)
        g["absorb"] = rules["absorb_vs_ranged"] if enemy["ranged"] else rules["absorb_vs_heavy"] if heavy_weapon else rules["absorb_default"]
        g["arc"] = rules["arc_vs_ranged"] if enemy["ranged"] else rules["arc_default"]
        sets[m["name"]] = m
        report.append({"enemy": enemy, "chain": [by_track[t]["name"] for t in seq], "cost": cost,
                       "heavy": m["heavy"][0]["name"] if m["heavy"] else None, "guard": g})
    return sets, report, timing


def write(sets, report, timing, plan):
    movesets_dir = os.path.join(REPO, plan["movesets"])
    for name, m in sets.items():
        with open(os.path.join(movesets_dir, name + ".json"), "w") as f:
            json.dump(m, f, indent=2)
            f.write("\n")
    out = os.path.join(REPO, "tools", "kev", "results", "h2h")
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "report.json"), "w") as f:
        json.dump({"timing": timing, "plans": report}, f, indent=1)
    lines = ["# Hand-to-hand plans by enemy weapon", "", "## Strike timing (fractions of the clip window)", "",
             "| track | hand | windup | active | cancel | peak m/s |", "|---|---|---|---|---|---|"]
    for t, v in timing.items():
        lines.append(f"| {t} | {v['hand']} | {v['windup']} | {v['active']} | {v['cancel']} | {v['peak_mps']} |")
    lines += ["", "## Plans", "", "| enemy | reach m | windup s | ranged | chain | cost | heavy | parry s | absorb | arc |",
              "|---|---|---|---|---|---|---|---|---|---|"]
    for r in report:
        e, g = r["enemy"], r["guard"]
        lines.append(f"| {e['name']} | {e['reach_m']} | {e['windup_s']} | {e['ranged']} | {' > '.join(r['chain'])} | {r['cost']} | {r['heavy']} | {g['parry_window']} | {g['absorb']} | {g['arc']} |")
    with open(os.path.join(out, "report.md"), "w") as f:
        f.write("\n".join(lines) + "\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("plan", nargs="?", default=os.path.join(HERE, "h2h.json"))
    ap.add_argument("--write", action="store_true")
    args = ap.parse_args()
    with open(args.plan) as f:
        plan = json.load(f)
    sets, report, timing = build(plan)
    for t, v in timing.items():
        print(f"{t:<22} hand {v['hand']}  windup {v['windup']:.3f} active {v['active']:.3f} cancel {v['cancel']:.3f}  peak {v['peak_mps']} m/s")
    for r in report:
        print(f"vs {r['enemy']['name']:<10} {' > '.join(r['chain']):<40} cost {r['cost']:<7} heavy {r['heavy']}  parry {r['guard']['parry_window']} absorb {r['guard']['absorb']}")
    if args.write:
        write(sets, report, timing, plan)
        print(f"wrote {len(sets)} movesets and tools/kev/results/h2h/report.md")


if __name__ == "__main__":
    main()
