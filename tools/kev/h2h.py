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
import math
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(REPO, "tools", "anim"))

import combos  # noqa: E402
import nodes  # noqa: E402
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


def hip_yaw(clip, frame):
    """Facing (deg) of the hip line at a frame: the horizontal direction across thigh_l -> thigh_r."""
    w = clip.world(frame)
    d = np.asarray(clip.pos(w, "thigh_r")) - np.asarray(clip.pos(w, "thigh_l"))
    return math.degrees(math.atan2(d[0], d[2]))


def time_strike(clip, fast, recover, end=None):
    """Striking hand, (windup, active, cancel) fractions of the window [0, end or duration], and what
    the cut at the window end looks like: the hand's speed there as a share of its peak (high = the
    window ends mid-swing) and the hip facing at the window's start and end. Speeds are raw: the UAL
    hook moves its hand ~1.3 m between two keys (an authored snap), so its peak reads ~39 m/s; that is
    the asset, and the strike window is placed on it."""
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
    dist = np.array([np.linalg.norm(np.asarray(clip.pos(clip.world(f), f"hand_{side}")) - np.asarray(clip.pos(clip.world(f), "pelvis"))) for f in range(clip.frames)])
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
    return side, {
        "windup": frac(start), "active": max(frac(stop + 1), frac(start) + 0.05), "cancel": max(frac(cancel), frac(stop + 1)),
        "peak_mps": round(float(top), 2),
        "end_speed_mps": round(float(v[last]), 3),
        # Radial speed of the hand at the window end: negative = drawing back toward the body.
        "end_radial_mps": round(float((dist[last] - dist[max(0, last - 1)]) * fps), 3),
        "yaw_start": round(hip_yaw(clip, 0), 1),
        "yaw_end": round(hip_yaw(clip, last), 1),
    }


def load_strike_table(path):
    """{track: strongest strike} from the game's own cutter (repertoire::segment + strike_attack),
    exported by `cargo test -p guildbound export_strike_table -- --ignored`. Empty when missing."""
    if not path or not os.path.exists(path):
        return {}
    with open(path) as f:
        table = json.load(f)
    best = {}
    for s in table.get("strikes", []):
        if s["track"] not in best or s["speed"] > best[s["track"]]["speed"]:
            best[s["track"]] = s
    return best


def measure_pool(pool, timing, table=None):
    """Timing per pool entry. Where the game's cutter found a strike (`table`), its window and
    windup / active / cancel are used as they are, so Kev and the game agree; otherwise Kev measures
    the clip itself. Each entry records its `source`. Libraries are loaded once."""
    from clip_fk import Clip, GLTF2

    table = table or {}
    loaded = {}
    out = {}
    for e in pool:
        lib, name = e["track"].split("/", 1)
        if lib not in loaded:
            loaded[lib] = GLTF2().load(lib_path(lib))
        clip = Clip(lib_path(lib), name, gltf=loaded[lib])
        game = table.get(e["track"])
        end = game["attack"]["end"] if game else e.get("end")
        side, t = time_strike(clip, timing["fast_fraction"], timing["recover_fraction"], end)
        if game:
            a = game["attack"]
            t.update(windup=round(a["windup"], 3), active=round(a["active"], 3), cancel=round(a["cancel"], 3))
            side = game["bone"].rsplit("_", 1)[-1] if game["bone"].startswith("hand_") else side
            t["window"] = [round(a["start"], 3), round(a["end"], 3)]
            t["source"] = "game cutter"
        else:
            t["source"] = "kev fk"
        window = (t["window"][1] - t["window"][0]) if game else (e.get("end") or clip.duration)
        # Mid-swing: still moving fast at the window end and not drawing back. A hand returning to the
        # guard (the jab and cross windows end on the way back) is a recovery and may be cut.
        mid = t["end_speed_mps"] > timing["mid_swing_mps"] and t["end_radial_mps"] > -timing["retract_mps"]
        out[e["track"]] = {"hand": side, **t, "duration": round(clip.duration, 3), "windup_s": round(t["windup"] * window, 3), "mid_swing": mid}
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
        aim = m.get("aim") or {}
        # Seconds between shots: the draw, plus the reload clip, plus the throw / release clip after its start.
        shot = float(aim.get("draw_time", 0.0))
        for key, cap in (("reload", 9.0), ("release", 1.0)):
            c = aim.get(key)
            if c:
                # A release clip may run on into a long take (the bow's archery mocap): count its first second.
                shot += min(cap, max(0.0, (c.get("end") or track_duration(c["track"]) or 0.0) - (c.get("start") or 0.0)))
        out.append({
            "name": m["name"],
            "kind": m.get("kind"),
            "reach_m": round(float(weapon.get("length", 0.0)) + arm_reach, 3),
            "mass": float(weapon.get("mass", 1.0)),
            "windup_s": round(float(np.mean(windups)), 3) if windups else None,
            "ranged": bool(aim),
            "shot_s": round(shot, 3) if aim else None,
        })
    return out


def chain_cost(seq, ends, timing, by_track, rules, weight, impact, enemy_windup, target=None):
    """Transition cost + same-hand penalty + tempo penalty - impact (poise) reward - target reward.

    Target: `target` maps a track to where its fist lands on the enemy's body (targets.SCORE: head 2,
    torso 1, leg 0.25, over 0); the mean over the chain, times `target_weight`, comes off the cost.

    Tempo: a strike only costs for the time its windup runs past the enemy's own windup, the window in
    which the enemy's swing lands first. Against a slow weapon a slow, heavy strike is free; against a
    fast one only quick strikes are."""
    cost = 0.0
    for a, b in zip(seq, seq[1:]):
        if timing[a]["mid_swing"]:
            return None  # a strike whose window ends mid-swing can only close the chain
        turn = abs((timing[a]["yaw_end"] - timing[b]["yaw_start"] + 180.0) % 360.0 - 180.0)
        cost += rules["facing_weight"] * turn
        t = combos.transition(ends[a][1], ends[b][0])
        # A crossfade hides a small cut; only the part above `crossfade_hides_deg` costs.
        cost += max(0.0, combos.total(t) - rules["crossfade_hides_deg"]) if t else 90.0
        if timing[a]["hand"] == timing[b]["hand"]:
            cost += rules["same_hand_penalty"]
    cost += weight * sum(max(0.0, timing[s]["windup_s"] - enemy_windup) for s in seq)
    cost -= impact * sum(by_track[s]["poise"] for s in seq) / len(seq)
    if target:
        cost -= rules.get("target_weight", 0.0) * sum(target.get(s, 0.0) for s in seq) / len(seq)
    return cost


def ranked_chains(light, ends, timing, by_track, rules, enemy, target=None):
    """Every valid chain for `enemy`, cheapest first: [(cost, seq)]."""
    windup = enemy["windup_s"] if enemy["windup_s"] is not None else 0.6
    n = next(n for limit, n in rules["chain_length_by_windup"] if windup <= limit)
    fast = windup <= rules["fast_enemy_windup"]
    weight = rules["windup_weight_fast_enemy"] if fast else rules["windup_weight_slow_enemy"]
    impact = rules["impact_weight_fast"] if fast else rules["impact_weight_slow"]
    out = []
    for seq in itertools.product(light, repeat=n):
        if any(a == b for a, b in zip(seq, seq[1:])):
            continue
        c = chain_cost(seq, ends, timing, by_track, rules, weight, impact, windup, target)
        if c is not None:
            out.append((round(c, 2), list(seq)))
    out.sort(key=lambda x: x[0])
    return out


def distinct(ranked, used, slack):
    """The cheapest chain not in `used` costing at most `slack` over the best, else the best."""
    best = ranked[0][0]
    for c, seq in ranked:
        if c > best + slack:
            break
        if tuple(seq) not in used:
            return c, seq
    return ranked[0]


def best_chain(light, ends, timing, by_track, rules, enemy, target=None):
    windup = enemy["windup_s"] if enemy["windup_s"] is not None else 0.6
    n = next(n for limit, n in rules["chain_length_by_windup"] if windup <= limit)
    fast = windup <= rules["fast_enemy_windup"]
    weight = rules["windup_weight_fast_enemy"] if fast else rules["windup_weight_slow_enemy"]
    impact = rules["impact_weight_fast"] if fast else rules["impact_weight_slow"]
    best = None
    for seq in itertools.product(light, repeat=n):
        if any(a == b for a, b in zip(seq, seq[1:])):
            continue
        c = chain_cost(seq, ends, timing, by_track, rules, weight, impact, windup, target)
        if c is None:
            continue
        if best is None or c < best[0]:
            best = (c, seq)
    return round(best[0], 2), list(best[1])


def attack(entry, t, travel):
    if "window" in t:
        clip = {"track": entry["track"], "start": t["window"][0], "end": t["window"][1]}
    else:
        clip = {"track": entry["track"], **({"end": entry["end"]} if entry.get("end") else {})}
    a = {
        "name": entry["name"],
        "clip": clip,
        "windup": t["windup"], "active": t["active"], "cancel": t["cancel"],
        "damage": entry["damage"], "poise": entry["poise"], "knockback": entry["knockback"],
        "stamina": entry["stamina"], "travel": round(travel, 2),
        "hitbox": {"bone": entry.get("bone", f"hand_{t['hand']}"), "radius": 0.18 if entry.get("bone") else 0.1},
    }
    if entry.get("recover"):
        a["recover"] = {"track": entry["recover"]}
    return a


def build(plan, strikes=None):
    rules, pool = plan["rules"], plan["pool"]
    check_punch_rule(pool, plan["hand_strike_libs"])
    movesets_dir = os.path.join(REPO, plan["movesets"])
    with open(os.path.join(movesets_dir, plan["base"] + ".json")) as f:
        base = json.load(f)
    timing = measure_pool(pool, plan["timing"], load_strike_table(strikes))
    ends = ends_for(pool)
    by_track = {e["track"]: e for e in pool}
    light = [e["track"] for e in pool if e["kind"] == "light"]
    enemies = enemy_profiles(movesets_dir, plan["base"], plan["out_prefix"], rules["arm_reach_m"])
    lands = landing(plan, pool, timing, strikes)
    target = lands.get(rig_name(plan.get("target_rig", "")), {}).get("score")
    sets, report = {}, []
    # Every weapon gets its own chain: the cheapest one no other weapon uses yet, within `variety_slack` of
    # its best (two weapons that score alike would otherwise fight the same way).
    used = set()
    # Per body, the same variety rule, so the target body's row is the chain the game ships.
    used_on = {r: set() for r in lands}
    for enemy in enemies:
        ranked = ranked_chains(light, ends, timing, by_track, rules, enemy, target)
        cost, seq = distinct(ranked, used, rules.get("variety_slack", 0.0))
        used.add(tuple(seq))
        first_travel = min(rules["travel_max"], rules["travel_base"] + rules["travel_per_m_reach"] * max(0.0, enemy["reach_m"] - 1.0))
        m = copy.deepcopy(base)
        m["name"] = plan["out_prefix"] + enemy["name"]
        m["light"] = [attack(by_track[t], timing[t], first_travel if i == 0 else rules["travel_base"]) for i, t in enumerate(seq)]
        heavy_weapon = enemy["mass"] >= rules["heavy_weapon_mass"]
        kind = lambda k: next(e for e in pool if e["kind"] == k)  # noqa: E731
        if enemy["ranged"]:
            # Against a heavy launcher, bash through its slow reload; against rapid light throws, stay quick
            # with the kick; against a bow, close the distance with the charge.
            if heavy_weapon:
                heavy = kind("heavy")
                m["heavy"] = [attack(heavy, timing[heavy["track"]], rules["travel_max"])]
            elif (enemy["shot_s"] or 9.0) <= rules["fast_shot_s"]:
                pass  # the base set's kick stays
            else:
                heavy = kind("charge")
                m["heavy"] = [attack(heavy, timing[heavy["track"]], rules["travel_max"])]
        elif heavy_weapon:
            heavy = kind("heavy")
            m["heavy"] = [attack(heavy, timing[heavy["track"]], first_travel)]
        # Defence follows the weapon's own numbers: the parry window from its wind-up (or the time between
        # shots), the share a block absorbs from its mass, the guard arc wider for light scattered throws.
        windup = enemy["windup_s"] if enemy["windup_s"] is not None else min(0.6, (enemy["shot_s"] or 0.6) * 0.5)
        g = m["guard"]
        lo, hi = rules["parry_range"]
        g["parry_window"] = round(min(hi, max(lo, rules["parry_base"] + rules["parry_per_s_windup"] * windup)), 3)
        if enemy["ranged"]:
            g["absorb"] = round(min(0.9, max(0.4, rules["absorb_vs_ranged"] + 0.1 - 0.1 * enemy["mass"])), 3)
            g["arc"] = round(rules["arc_vs_ranged"] - 20 + 40 / (1 + enemy["mass"]))
        else:
            g["absorb"] = rules["absorb_vs_heavy"] if heavy_weapon else rules["absorb_default"]
            g["arc"] = rules["arc_default"]
        sets[m["name"]] = m
        # The same enemy weapon on every other body: what the planner would pick there (report only),
        # under the same "no two weapons fight alike" rule as the shipped sets.
        on = {}
        for r, l in lands.items():
            ranked_r = ranked_chains(light, ends, timing, by_track, rules, enemy, l["score"])
            c2, s2 = distinct(ranked_r, used_on[r], rules.get("variety_slack", 0.0))
            used_on[r].add(tuple(s2))
            on[r] = {"chain": [by_track[t]["name"] for t in s2], "lands": [l["region"].get(t, "?") for t in s2], "cost": c2}
            if all(l["region"].get(t) == "leg" for t in light):
                # No allowed punch reaches above this body's legs (a giant): open with the kick instead.
                on[r]["heavy"] = base["heavy"][0]["name"]
                on[r]["note"] = "legs only: no allowed punch reaches higher"
        report.append({"enemy": enemy, "chain": [by_track[t]["name"] for t in seq], "cost": cost,
                       "lands": [lands.get(rig_name(plan.get("target_rig", "")), {}).get("region", {}).get(t, "?") for t in seq],
                       "heavy": m["heavy"][0]["name"] if m["heavy"] else None, "guard": g, "on_rigs": on})
    return sets, report, timing


def rig_name(path):
    return os.path.splitext(os.path.basename(path))[0]


def landing(plan, pool, timing, strikes):
    """rig -> {"region": track -> region, "score": track -> score} for the target rig and the other rigs
    present (targets.py: fist height at the strike's peak against each body's region spans)."""
    import targets

    rigs = [plan.get("target_rig")] + plan.get("other_rigs", [])
    rigs = [r for r in rigs if r and os.path.exists(os.path.join(REPO, r))]
    if not rigs:
        return {}
    rules = nodes.load_rules()
    hero = nodes.parse(os.path.join(REPO, targets.HERO), rules)
    hero_m = hero["height"] * targets.units(hero)
    heights = targets.strike_heights(pool, timing, load_strike_table(strikes))
    track_of = {e["name"]: e["track"] for e in pool}
    out = {}
    for r in rigs:
        rep = nodes.parse(os.path.join(REPO, r), rules)
        if rep.get("error"):
            continue
        region = {track_of[n]: targets.land(h * hero_m, rep) for n, (_, h) in heights.items()}
        out[rig_name(r)] = {"region": region, "score": {t: targets.SCORE[x] for t, x in region.items()}}
    return out


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
             "`game cutter`: taken from the game's own strike table (repertoire::segment), so Kev and the game agree.",
             "`kev fk`: the game's cutter found no strike in this clip; Kev measured it from its motion.", "",
             "| track | source | hand | windup | active | cancel | peak m/s | mid-swing end |", "|---|---|---|---|---|---|---|---|"]
    for t, v in timing.items():
        lines.append(f"| {t} | {v['source']} | {v['hand']} | {v['windup']} | {v['active']} | {v['cancel']} | {v['peak_mps']} | {v['mid_swing']} |")
    lines += ["", "## Plans", "", "| enemy | reach m | windup s | ranged | chain | cost | heavy | parry s | absorb | arc |",
              "|---|---|---|---|---|---|---|---|---|---|"]
    for r in report:
        e, g = r["enemy"], r["guard"]
        lines.append(f"| {e['name']} | {e['reach_m']} | {e['windup_s']} | {e['ranged']} | {' > '.join(r['chain'])} | {r['cost']} | {r['heavy']} | {g['parry_window']} | {g['absorb']} | {g['arc']} |")
    rigs = list(report[0].get("on_rigs", {})) if report else []
    if rigs:
        lines += ["", "## Where the chain lands, and the chain picked for each body", "",
                  "The game's enemies wear the target rig; the chain above is planned on it. The other bodies show what the",
                  "planner would pick against the same weapon on a body of that size (targets.py).", "",
                  "| enemy | " + " | ".join(rigs) + " |", "|---|" + "---|" * len(rigs)]
        for r in report:
            cells = [f"{' > '.join(r['on_rigs'][g]['chain'])} ({', '.join(r['on_rigs'][g]['lands'])})"
                     + (f" - {r['on_rigs'][g]['note']}, heavy {r['on_rigs'][g]['heavy']}" if r['on_rigs'][g].get('note') else "") for g in rigs]
            lines.append(f"| {r['enemy']['name']} | " + " | ".join(cells) + " |")
    with open(os.path.join(out, "report.md"), "w") as f:
        f.write("\n".join(lines) + "\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("plan", nargs="?", default=os.path.join(HERE, "h2h.json"))
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--strikes", default=os.path.join(REPO, "staging", "kev", "strikes.json"),
                    help="the game's strike table (cargo test -p guildbound export_strike_table -- --ignored)")
    args = ap.parse_args()
    with open(args.plan) as f:
        plan = json.load(f)
    sets, report, timing = build(plan, args.strikes)
    for t, v in timing.items():
        print(f"{t:<22} {v['source']:<12} hand {v['hand']}  windup {v['windup']:.3f} active {v['active']:.3f} cancel {v['cancel']:.3f}  mid {v['mid_swing']}")
    for r in report:
        print(f"vs {r['enemy']['name']:<10} {' > '.join(r['chain']):<40} cost {r['cost']:<7} heavy {r['heavy']}  parry {r['guard']['parry_window']} absorb {r['guard']['absorb']}")
    if args.write:
        write(sets, report, timing, plan)
        print(f"wrote {len(sets)} movesets and tools/kev/results/h2h/report.md")


if __name__ == "__main__":
    main()
