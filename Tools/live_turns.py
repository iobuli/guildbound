"""Live movement demo the boss can watch: walk, a 90 deg turn, a full reversal, a run on a curve, a stop.

python -I Tools/live_turns.py

Prints the facing, its turning speed and the ground speed every 0.1 s, so smooth turns (easing in and settling,
plant-and-turn on a reversal) show in numbers as well as on screen.
"""
import math
import sys
import time

sys.path.insert(0, __file__.replace("\\", "/").rsplit("/", 1)[0])
from gb import call  # noqa: E402


def sample(label, seconds):
    t0 = time.time()
    while time.time() - t0 < seconds:
        f = call("gb.state")["fighters"][0]
        v = f["velocity"]
        face = f.get("facing")
        heading = math.degrees(math.atan2(face[0], face[1])) if face else float("nan")
        print(f"{label:14s} t {time.time() - t0:4.1f}  heading {heading:7.1f}  speed {math.hypot(v[0], v[1]):4.2f}")
        time.sleep(0.1)


def go(move, seconds, run=False):
    call("gb.input", {"fighter": 0, "move": move, "ticks": int(seconds * 120), "run": run})


call("gb.control", {"state": "play"})
time.sleep(1.5)
call("gb.say", {"text": "Claude live: walk, turn 90, reverse, run a curve, stop - watch the turns ease in and settle"})
steps = [
    ("walk north", [0.0, 1.0], 1.5, False),
    ("turn 90 east", [1.0, 0.0], 1.2, False),
    ("reverse west", [-1.0, 0.0], 1.5, False),
    ("run curve", [-0.7, -0.7], 1.5, True),
    ("stop", [0.0, 0.0], 1.0, False),
]
for label, move, seconds, run in steps:
    if move != [0.0, 0.0]:
        go(move, seconds, run)
    sample(label, seconds)
