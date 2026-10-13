"""Exercise the running RPG hero's combat rules and mapped animation together.

Run with the beta at its menu: python -I Tools/check_combat.py
Uses real motor approach and combat inputs; never teleports or edits fighters.
"""
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("beta", ROOT / "Tools/check_beta.py")
beta = importlib.util.module_from_spec(spec)
spec.loader.exec_module(beta)
gb, step, reset, pose = beta.gb, beta.step, beta.reset, beta.pose
MAP = json.loads((ROOT / "Tools/blender/hero_mapping.json").read_text())["combat"]


def send(**flags):
    gb.call("gb.input", {"move": [0, 0], "ticks": 1, **flags})


def near():
    reset()
    import math
    ticks = math.ceil(2.2 / gb.call("gb.state")["tuning"]["walk_speed"] * 120) + 12
    send(move=[0, 1], ticks=ticks)
    step(ticks)
    send(ticks=1)
    step(24)


def action(name, limit=100):
    for _ in range(limit):
        state = gb.call("gb.state")
        if state["fighters"][0]["action"].startswith(name):
            return state
        step(1)
    raise AssertionError(f"never entered {name}: {state['fighters'][0]}")


def mapped(cluster):
    snapshot = pose()
    assert snapshot["rule_sample"], snapshot
    assert snapshot["clip"] in MAP[cluster], (cluster, snapshot)
    assert snapshot["contact_timing"] is not None, snapshot
    return snapshot


def main():
    result = {"checks": [], "pass": False}
    try:
        buttons = gb.call("gb.menu_pointer")["buttons"]
        button = next(n for n in ["Practice", "Resume", "Spar"] if n in buttons)
        gb.call("gb.menu_pointer", {"button": button, "event": "click"})
        beta.wait_for(lambda s: s["app_state"] == "Playing")
        near()
        send(light=True)
        step(1)
        clips = [mapped("jab")]
        step(8)
        state = gb.call("gb.state")
        snapshot = mapped("jab")
        marks = snapshot["contact_timing"]
        expected_seek = state["fighters"][0]["action_elapsed"] / 0.10 * marks["start"] * marks["duration_s"]
        assert abs(snapshot["seek_s"] - expected_seek) < 0.002, (snapshot, expected_seek)
        for threshold, name in zip([0.09, 0.12, 0.17], ["cross", "hook", "uppercut"]):
            # Queue before contact, outside hit-stop. This must link without
            # a second press after recovery begins.
            state = gb.call("gb.state")
            while state["fighters"][0]["action_elapsed"] < threshold:
                state = step(1)
            send(light=True)
            step(1)
            action(name)
            clips.append(mapped(name))
        step(100)
        assert gb.call("gb.state")["fighters"][1]["hp"] <= gb.call("gb.state")["fighters"][1]["max_hp"] - 22.0
        result["checks"].append({"kind": "four_hit_rush", "poses": clips})
        result["checks"].append({"kind": "measured_contact_clock", "pose": snapshot, "expected_seek_s": expected_seek})

        for vector, name, cluster in [([0, 0], "front kick", "front kick"), ([0, 1], "rising", "uppercut"), ([0, -1], "sweep", "low kick")]:
            near()
            send(move=vector, heavy=True, heavy_held=True, ticks=200)
            held = step(42)
            assert held["fighters"][0]["action"].startswith("smash " + name)
            before = mapped(cluster)
            send()
            released = step(1)
            assert released["fighters"][0]["action"].startswith(name)
            after = mapped(cluster)
            assert before["clip"] == after["clip"], "smash changed clip on release"
            step(100)
            assert gb.call("gb.state")["fighters"][1]["hp"] < gb.call("gb.state")["fighters"][1]["max_hp"]
            result["checks"].append({"kind": "charged_" + name, "held": before, "released": after})

        near()
        send(fighter=1, guard=True, ticks=300)
        step(30)  # settle beyond the parry window
        send(heavy=True, heavy_held=True, ticks=200)
        step(95)
        state = gb.call("gb.state")
        assert state["fighters"][1]["guard"] < 10.0, state
        assert state["fighters"][1]["action"] == "guardbreak", state
        result["checks"].append({"kind": "full_smash_guard_break", "pose": mapped("front kick")})

        near()
        send(move=[0, 1], heavy=True)
        step(45)
        dolls = gb.call("gb.physics")["ragdolls"]
        assert len(dolls) == 1 and dolls[0]["fighter"] == 1, dolls
        assert dolls[0]["bodies"] == 13 and dolls[0]["joints"] == 12 and dolls[0]["finite"], dolls
        assert dolls[0]["hinges"] == 4 and dolls[0]["max_hinge_axis_error_rad"] < 0.12, dolls
        assert dolls[0]["max_joint_error_m"] < 0.08 and dolls[0]["max_speed_mps"] < 15.0, dolls
        frozen = dolls[0]["hash"]
        import time
        time.sleep(0.3)
        assert gb.call("gb.physics")["ragdolls"][0]["hash"] == frozen, "ragdoll moved while paused"
        for _ in range(12):
            step(5)
            current = gb.call("gb.physics")["ragdolls"]
            assert current and current[0]["finite"] and current[0]["min_center_y"] > -0.1, current
            assert current[0]["max_joint_error_m"] < 0.08 and current[0]["max_speed_mps"] < 15.0, current
            assert current[0]["hinges"] == 4 and current[0]["max_hinge_axis_error_rad"] < 0.12, current
        beta.shot("beta-ragdoll")
        step(160)
        assert not gb.call("gb.physics")["ragdolls"], "recovery leaked ragdoll bodies"
        assert gb.call("gb.state")["fighters"][1]["action"] == "free"
        result["checks"].append({"kind": "physical_knockdown_pause_and_recovery", "initial": dolls[0]})

        reset()
        energy = gb.call("gb.state")["fighters"][0]["mana"]
        send(step=True, run=True)
        state = step(1)
        assert state["fighters"][0]["mana"] < energy - 7.0
        send(light=True)
        assert step(1)["fighters"][0]["action"] == "step"
        action("run punch")
        result["checks"].append({"kind": "homing_dash_rush", "pose": mapped("run punch")})

        near()
        send(heavy=True, heavy_held=True, ticks=200)
        step(30)
        gb.call("gb.control", {"state": "menu"})
        beta.wait_for(lambda s: s["app_state"] == "MainMenu")
        assert gb.call("gb.state")["fighters"][0]["action"] == "free"
        assert not pose()["rule_sample"], "cancelled smash retained its action clip"
        result["checks"].append({"kind": "menu_cancels_smash"})
        result["pass"] = True
    except BaseException as exc:
        result["error"] = str(exc)
        raise
    finally:
        (ROOT / "Documentation/checks/live/beta-combat-smoke.json").write_text(json.dumps(result, indent=2))
        gb.call("gb.control", {"state": "menu", "pause": True})
    print(f"BETA_COMBAT_PASS {len(result['checks'])} checks")


if __name__ == "__main__":
    main()
