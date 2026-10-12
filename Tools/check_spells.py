"""Live mage pose, charge/release and projectile launch checks on the RPG hero."""
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("beta", ROOT / "Tools/check_beta.py")
beta = importlib.util.module_from_spec(spec)
spec.loader.exec_module(beta)
gb, step, reset, pose = beta.gb, beta.step, beta.reset, beta.pose


def send(**flags):
    gb.call("gb.input", {"move": [0, 0], "ticks": 1, **flags})


def main():
    evidence = {"checks": [], "pass": False}
    try:
        gb.call("gb.control", {"state": "play"})
        beta.wait_for(lambda s: s["app_state"] == "Playing")
        reset()
        send(cast=0, cast_held=True, ticks=240)
        step(60)
        charge_pose = pose()
        assert charge_pose["clip"] == "mage_fireball_load" and charge_pose["rule_sample"], charge_pose
        beta.shot("beta-mage-fireball-charge")
        send()
        released = step(1)
        assert released["fighters"][0]["action"] == "cast fireball"
        assert not released["projectiles"], "fireball launched before its cast point"
        release_pose = pose()
        assert release_pose["clip"] == "mage_fireball_release" and release_pose["rule_sample"], release_pose
        assert not step(20)["projectiles"]
        launched = step(7)
        assert len(launched["projectiles"]) == 1 and launched["projectiles"][0]["spell"] == "fireball", launched
        launch_pose = pose()
        marks = launch_pose["contact_timing"]
        elapsed = launched["fighters"][0]["action_elapsed"]
        expected = (marks["peak"] + (1 - marks["peak"]) * (elapsed - 0.22) / 0.30) * marks["duration_s"]
        assert abs(launch_pose["seek_s"] - expected) < 0.002, (launch_pose, expected)
        beta.shot("beta-mage-fireball-launch")
        step(90)
        assert gb.call("gb.state")["fighters"][1]["hp"] < 100.0
        evidence["checks"].append({"kind": "charged_fireball_pose_launch_and_hit", "charge": charge_pose, "release": release_pose, "launch": launch_pose})

        reset()
        send(cast=0)
        step(3)
        assert pose()["clip"] == "mage_fireball", pose()
        assert not gb.call("gb.state")["projectiles"]
        step(28)
        assert gb.call("gb.state")["projectiles"]
        evidence["checks"].append({"kind": "tap_fireball_mage_windup"})

        lightning = []
        for _ in range(2):
            reset()
            send(cast=2)
            step(12)
            snapshot = pose()
            assert snapshot["clip"] in ["mage_lightning_left", "mage_lightning_right"] and snapshot["rule_sample"], snapshot
            assert not gb.call("gb.state")["projectiles"]
            beta.shot("beta-" + snapshot["clip"])
            assert step(12)["projectiles"]
            step(75)
            assert gb.call("gb.state")["fighters"][1]["hp"] < 100.0
            lightning.append(snapshot)
        assert lightning[0]["clip"] != lightning[1]["clip"], "lightning should alternate mage hands"
        evidence["checks"].append({"kind": "lightning_mage_hands_launch_and_hit", "poses": lightning})
        evidence["pass"] = True
    except BaseException as exc:
        evidence["error"] = str(exc)
        raise
    finally:
        (ROOT / "Documentation/checks/live/beta-spells-smoke.json").write_text(json.dumps(evidence, indent=2))
        gb.call("gb.control", {"state": "menu", "pause": True})
    print(f"BETA_SPELLS_PASS {len(evidence['checks'])} checks")


if __name__ == "__main__":
    main()
