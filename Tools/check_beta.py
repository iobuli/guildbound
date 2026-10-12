"""Live RPG controller smoke check through the game's local API.

Run the beta first, then: python -I Tools/check_beta.py
Writes evidence under Documentation/checks/live and leaves the game at its menu.
No keyboard injection, external app, or asset modification is required.
"""
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


def wait_for(predicate):
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        state = gb.call("gb.state")
        if predicate(state):
            return state
        time.sleep(0.04)
    raise AssertionError("Game did not reach the expected state within 8 seconds")


def step(n):
    before = gb.call("gb.state")["tick"]
    gb.call("gb.control", {"pause": True, "step": n})
    state = wait_for(lambda s: s["tick"] == before + n)
    # Pose evaluation and transform propagation finish after the sim tick.
    time.sleep(0.12)
    return state


def reset(lock=True):
    gb.call("gb.control", {"reset": True, "pause": True, "opponent": "idle", "lock": lock})
    wait_for(lambda s: s["tick"] == 0)
    time.sleep(0.12)


def pose():
    rows = gb.call("gb.presentation")["fighters"]
    assert len(rows) == 2 and all(r["finite"] and r["bones"] > 20 for r in rows)
    return next(r for r in rows if r["fighter"] == 0)


def shot(name):
    gb.call("gb.screenshot", {"path": f"Documentation/checks/live/{name}.png"})
    time.sleep(0.15)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    evidence = {"checks": []}
    try:
        gb.call("gb.control", {"state": "play", "dev_panel": False})
        wait_for(lambda s: s["app_state"] == "Playing")
        reset()
        expected = json.loads((ROOT / "Tools" / "blender" / "hero_mapping.json").read_text())["locomotion"]
        directions = [(0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1)]
        for running in [False, True]:
            for i, vector in enumerate(directions):
                reset()
                gb.call("gb.input", {"move": vector, "run": running, "ticks": 120})
                state = step(36)
                f = state["fighters"][0]
                x, z = f["velocity"]
                fx, fz = f["facing"]
                # side component along the character's right (-fz, fx): facing +Z with +Y up has its right
                # at -X. The old (fz, -fx) was the left and mirrored every side clip (measured: planted-foot
                # slip 3x-10x higher on side/diagonal moves, Tools/check_legs.py).
                actual = round(math.atan2(z * fx - x * fz, x * fx + z * fz) / (math.pi / 4)) % 8
                snapshot = pose()
                kind = "run" if running else "walk"
                assert snapshot["clip"] == expected[kind][actual], (kind, i, snapshot)
                assert math.hypot(*f["velocity"]) > 1.0
                frozen = snapshot["pose_hash"]
                time.sleep(0.18)
                assert pose()["pose_hash"] == frozen, "pose changed while paused"
                evidence["checks"].append({"kind": kind, "input_direction": i, "mapped_direction": actual, "pose": snapshot})
                if i in [0, 2, 4]: shot(f"beta-{kind}-{i}")
        reset(False)
        gb.call("gb.input", {"move": [0, 1], "jump": True, "ticks": 1})
        air = step(28)
        assert air["fighters"][0]["height"] > 0.5
        jump_pose = pose()
        assert jump_pose["clip"] == "Unarmed-Jump"
        shot("beta-jump")
        step(32)
        assert pose()["clip"] == "Unarmed-Fall"
        landed = step(100)
        assert landed["fighters"][0]["height"] == 0.0
        evidence["checks"].append({"kind": "jump_fall_land", "pose": jump_pose})
        reset()
        gb.call("gb.input", {"move": [0, 0], "roll": True, "ticks": 1})
        roll_state = step(12)
        assert pose()["clip"] == "Unarmed-DiveRoll-Forward1"
        hip = pose()["geometry"]["B_Pelvis"]
        motor = roll_state["fighters"][0]["position"]
        assert math.dist([hip[0], hip[2]], motor) < 0.03, "roll pose travelled ahead of the motor"
        shot("beta-roll")
        evidence["checks"].append({"kind": "roll", "pose": pose()})
        reset()
        gb.call("gb.input", {"move": [0, 1], "ticks": 136})
        approach = step(136)
        a, b = approach["fighters"]
        assert math.dist(a["position"], b["position"]) < 1.1, "strike check must start in jab range"
        gb.call("gb.input", {"move": [0, 0], "light": True, "ticks": 1})
        step(8)
        assert pose()["rule_sample"], "strike did not use the rule clock"
        shot("beta-strike")
        step(75)
        assert gb.call("gb.state")["fighters"][1]["hp"] < 100.0
        evidence["checks"].append({"kind": "strike_contact", "pass": True})
        reset()
        gb.call("gb.input", {"move": [0, 0], "cast": 0, "cast_held": True, "ticks": 120})
        step(30)
        assert "charging" in gb.call("gb.state")["fighters"][0]["action"]
        shot("beta-charge")
        gb.call("gb.control", {"state": "menu"})
        wait_for(lambda s: s["app_state"] == "MainMenu")
        assert gb.call("gb.state")["fighters"][0]["action"] == "free"
        evidence["checks"].append({"kind": "menu_cancels_held_charge", "pass": True})
        reset(False)
        shot("beta-polished-menu")
        evidence["pass"] = True
    except BaseException as exc:
        evidence["pass"] = False
        evidence["error"] = str(exc)
        raise
    finally:
        (OUT / "beta-controller-smoke.json").write_text(json.dumps(evidence, indent=2))
        gb.call("gb.control", {"state": "menu", "pause": True})
    print(f"BETA_CONTROLLER_PASS {len(evidence['checks'])} checks")


if __name__ == "__main__":
    main()
