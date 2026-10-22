"""Before/after numbers for the gait walk and a few catalogue clips.

    python3 -I tools/kev/physics/report.py
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
KEV = os.path.dirname(HERE)
sys.path.insert(0, KEV)

from physics import correct  # noqa: E402
from physics.sample import clip_landmarks, rest_landmarks  # noqa: E402
from physics.strips import save_strip  # noqa: E402
from physics.walk import frames  # noqa: E402

CLIPS = ("Walk_Loop", "Punch_Jab", "Punch_Cross", "Jump_Land", "Jump_Start", "Sword_Attack")
OUT = os.path.join(HERE, "results")


def _row(name, result):
    b, a = result["diagnostics"]["before"], result["diagnostics"]["after"]
    return {
        "name": name,
        "slip_mps": [b["slip_mps"], a["slip_mps"]],
        "slip_max_mps": [b["slip_max_mps"], a["slip_max_mps"]],
        "penetration_mm": [b["penetration_mm"], a["penetration_mm"]],
        "balance_violations": [b["balance_violations"], a["balance_violations"]],
        "jerk": [b["jerk"], a["jerk"]],
        "pelvis_travel_m": [b["pelvis_travel_m"], a["pelvis_travel_m"]],
        "mu_required": [b["mu_required"], a["mu_required"]],
        "pass": result["diagnostics"]["pass"],
    }


def _line(row):
    def pair(key, nd=3):
        b, a = row[key]
        return f"{b:.{nd}f}->{a:.{nd}f}"

    return (f"{row['name']:12} slip {pair('slip_mps')}  max {pair('slip_max_mps')}  "
            f"pen {pair('penetration_mm', 1)}  jerk {pair('jerk', 0)}  "
            f"trav {pair('pelvis_travel_m')}  bal {pair('balance_violations', 0)}")


def main():
    os.makedirs(OUT, exist_ok=True)
    rest = rest_landmarks("ual1.glb")
    rows = []
    walk = frames(rest, seconds=2.2, hz=30)
    got = correct(walk, dt=1.0 / 30.0, rest=rest)
    rows.append(_row("gait_walk", got))
    save_strip(os.path.join(OUT, "gait_walk.png"), walk, got["frames"], rest, got["floor"])
    for name in CLIPS:
        seq, dt, _ = clip_landmarks("ual1.glb", name, fps=30)
        got = correct(seq, dt=dt, rest=rest)
        rows.append(_row(name, got))
        save_strip(os.path.join(OUT, f"{name}.png"), seq, got["frames"], rest, got["floor"])
        print(_line(rows[-1]), flush=True)
    print(_line(rows[0]), flush=True)
    with open(os.path.join(OUT, "metrics.json"), "w") as f:
        json.dump(rows, f, indent=1)
        f.write("\n")


if __name__ == "__main__":
    main()
