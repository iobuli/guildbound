"""Live test the boss can watch: the hero sprints, then stops; logs what the run -> idle blend does.

python -I Tools/live_stop.py [seconds_running]

Prints, every 0.1 s around the stop: the dominant clip, speed, and the hands relative to the pelvis (a warp shows
as hands swinging through a pose neither clip has), plus the largest frame-to-frame jump of any tracked joint.
"""
import math
import sys
import time

sys.path.insert(0, __file__.replace("\\", "/").rsplit("/", 1)[0])
from gb import call  # noqa: E402

TRACK = ("B_L_Forearm.001", "B_R_Hand", "B_Head", "B_L_Foot", "B_R_Foot")


def me():
    return next(f for f in call("gb.presentation")["fighters"] if f["fighter"] == 0)


def main():
    run_s = float(sys.argv[1]) if len(sys.argv) > 1 else 2.0
    call("gb.control", {"state": "play"})
    call("gb.say", {"text": f"Claude live: sprint {run_s:.1f} s, then stop - watch the run -> idle blend"})
    ticks = int(run_s * 120)
    call("gb.input", {"fighter": 0, "move": [0.0, 1.0], "ticks": ticks, "run": True})
    t0 = time.time()
    prev = None
    worst = (0.0, "", 0.0)
    rows = []
    while time.time() - t0 < run_s + 2.0:
        f = me()
        g = f["geometry"]
        pel = g["B_Pelvis"]
        rel = {k: [g[k][i] - pel[i] for i in range(3)] for k in TRACK if k in g}
        t = time.time() - t0
        if prev:
            for k, v in rel.items():
                jump = math.dist(v, prev[k])
                if jump > worst[0] and t > run_s - 0.2:
                    worst = (jump, k, t)
        prev = rel
        if t > run_s - 0.4:
            rows.append(f"t {t:4.2f}  {f['clip'][:22]:22s}  hands-pelvis L {rel['B_L_Forearm.001'][1]:+.2f} R {rel['B_R_Hand'][1]:+.2f}  head {rel['B_Head'][1]:.2f}")
        time.sleep(0.1)
    print("\n".join(rows))
    print(f"largest joint jump around the stop: {worst[0] * 100:.1f} cm ({worst[1]}) at t {worst[2]:.2f}")


main()
