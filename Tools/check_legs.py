"""Live leg quality probe: walk and run in eight directions and measure what the legs do.

Run the beta first, then: python -I Tools/check_legs.py [--modes blend,snap]
Per direction and gait it reports travel angle vs facing, planted-foot slip (m/s; 0 = feet stick),
ground penetration, foot yaw vs travel, knee angle range and the largest frame-to-frame knee jump (a pop).
Writes Documentation/checks/live/legs.json. Exits non-zero if a budget is exceeded.
"""
import argparse
import importlib.util
import json
import math
from pathlib import Path
import time

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("gb", ROOT / "Tools" / "gb.py")
gb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gb)
OUT = ROOT / "Documentation" / "checks" / "live"
STEP = 1.0 / 120.0

# Budgets (starting hypotheses): average planted slip, worst knee pop between samples.
SLIP_BUDGET = 0.35
POP_BUDGET_DEG = 35.0

DIRS = {"fwd": (0, 1), "fwd_right": (1, 1), "right": (1, 0), "back_right": (1, -1),
        "back": (0, -1), "back_left": (-1, -1), "left": (-1, 0), "fwd_left": (-1, 1)}


def wait_tick(target):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        s = gb.call("gb.state")
        if s["tick"] >= target:
            return s
        time.sleep(0.02)
    raise AssertionError("tick did not advance")


def step(n):
    before = gb.call("gb.state")["tick"]
    gb.call("gb.control", {"pause": True, "step": n})
    s = wait_tick(before + n)
    time.sleep(0.09)  # pose evaluation and transform propagation finish after the tick
    return s


def player_geo():
    row = next(r for r in gb.call("gb.presentation")["fighters"] if r["fighter"] == 0)
    return row["geometry"], row["clip"]


def angle(a, b, c):
    """Angle at b (deg) between b->a and b->c."""
    u = [a[i] - b[i] for i in range(3)]
    v = [c[i] - b[i] for i in range(3)]
    nu, nv = math.sqrt(sum(x * x for x in u)), math.sqrt(sum(x * x for x in v))
    if nu < 1e-6 or nv < 1e-6:
        return 180.0
    return math.degrees(math.acos(max(-1.0, min(1.0, sum(u[i] * v[i] for i in range(3)) / (nu * nv)))))


def yaw(v):
    return math.degrees(math.atan2(v[0], v[2]))


def wrap(d):
    return (d + 180.0) % 360.0 - 180.0


def run_config(name, d, run):
    gb.call("gb.control", {"reset": True, "pause": True, "opponent": "idle", "lock": True})
    wait_tick(0)
    # make room: back away from the partner first when the test direction heads toward it
    if d[1] > 0:
        gb.call("gb.input", {"move": [0, -1], "run": True, "ticks": 170})
        step(170)
    n = math.hypot(*d)
    gb.call("gb.input", {"move": [d[0] / n, d[1] / n], "run": run, "ticks": 400})
    step(50)  # reach steady gait
    samples = []
    for _ in range(40):
        s = step(2)
        geo, clip = player_geo()
        f = s["fighters"][0]
        sup = gb.call("gb.support")
        feet = next((r["feet"] for r in sup.get("fighters", []) if r.get("enabled") and r.get("fighter") == 0), None)
        geo["_planted"] = [ft["weight"] > 0.5 for ft in feet] if feet else None
        samples.append((geo, f, clip))
    rows = {"clips": sorted({c for _, _, c in samples if c})}
    # travel angle vs facing, from the sim
    trav = [wrap(yaw([f["velocity"][0], 0, f["velocity"][1]]) - yaw([f["facing"][0], 0, f["facing"][1]])) for _, f, _ in samples]
    rows["travel_vs_facing_deg"] = round(sum(trav) / len(trav), 1)
    rows["speed_mps"] = round(sum(math.hypot(*f["velocity"]) for _, f, _ in samples) / len(samples), 2)
    slips, pen, knees, pops, foot_yaws = [], [], {"L": [], "R": []}, 0.0, []
    for side in ("L", "R"):
        foot, thigh, calf, toe = f"B_{side}_Foot", f"B_{side}_Thigh", f"B_{side}_Calf", f"B_{side}_Toe0"
        hs = [g[foot][1] for g, _, _ in samples]
        floor = min(hs)
        prev = None
        prev_knee = None
        prev_planted = False
        for g, fi, _ in samples:
            p = g[foot]
            knee = angle(g[thigh], g[calf], g[foot])
            knees[side].append(knee)
            if prev_knee is not None:
                pops = max(pops, abs(knee - prev_knee))
            prev_knee = knee
            planted = g["_planted"][0 if side == "L" else 1] if g.get("_planted") else p[1] < floor + 0.005
            if planted and prev is not None and prev_planted:
                slips.append(math.hypot(p[0] - prev[0], p[2] - prev[2]) / (2 * STEP))
            prev = p
            prev_planted = planted
            pen.append(p[1])
            v = fi["velocity"]
            if planted and math.hypot(*v) > 0.3:
                fy = yaw([g[toe][i] - p[i] for i in range(3)])
                foot_yaws.append(abs(wrap(fy - yaw([v[0], 0, v[1]]))))
    rows["planted_slip_mps"] = round(sum(slips) / max(len(slips), 1), 3)
    rows["planted_slip_max_mps"] = round(max(slips, default=0.0), 3)
    rows["ankle_min_y"] = round(min(pen), 3)
    rows["knee_deg"] = {s: [round(min(v), 1), round(max(v), 1)] for s, v in knees.items()}
    rows["knee_pop_deg"] = round(pops, 1)
    rows["foot_yaw_vs_travel_deg"] = round(sum(foot_yaws) / max(len(foot_yaws), 1), 1)
    rows["pass"] = rows["planted_slip_mps"] <= SLIP_BUDGET and rows["knee_pop_deg"] <= POP_BUDGET_DEG
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--modes", default="blend,snap", help="comma list of blend, snap")
    ap.add_argument("--support", default="on", choices=["on", "off"], help="foot support IK during the run")
    ap.add_argument("--rules", default="on", choices=["on", "off"], help="joint authority + knee hinge layers")
    ap.add_argument("--dirs", default="", help="comma list of direction names to run (default all)")
    ap.add_argument("--gaits", default="walk,run")
    a = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    gb.call("gb.control", {"state": "play", "dev_panel": False})
    gb.call("gb.support", {"enabled": a.support == "on"})
    gb.call("gb.control", {"joint_rule": a.rules == "on", "knee_rule": a.rules == "on"})
    result = {}
    try:
        for mode in a.modes.split(","):
            gb.call("gb.control", {"blend_space": mode != "snap"})
            result[mode] = {}
            for gait, run in (("walk", False), ("run", True)):
                if gait not in a.gaits.split(","):
                    continue
                for name, d in DIRS.items():
                    if a.dirs and name not in a.dirs.split(","):
                        continue
                    r = run_config(name, d, run)
                    result[mode][f"{gait}_{name}"] = r
                    print(mode, gait, name, json.dumps(r))
    finally:
        gb.call("gb.control", {"blend_space": True, "reset": True, "pause": False, "state": "menu"})
        gb.call("gb.support", {"enabled": True})
        gb.call("gb.control", {"joint_rule": True, "knee_rule": True})
    (OUT / f"legs_{a.modes.replace(',', '_')}_support_{a.support}_rules_{a.rules}.json").write_text(json.dumps(result, indent=2))
    for mode, rows in result.items():
        bad = [k for k, r in rows.items() if not r["pass"]]
        slip = sum(r["planted_slip_mps"] for r in rows.values()) / len(rows)
        pop = max(r["knee_pop_deg"] for r in rows.values())
        print(f"LEGS {mode}: mean slip {slip:.3f} m/s, worst knee pop {pop:.1f} deg, over budget: {bad}")


if __name__ == "__main__":
    main()
