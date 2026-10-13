"""Live mage pose, charge/release and projectile launch checks on the RPG hero."""
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAP = json.loads((ROOT / "Tools" / "blender" / "hero_mapping.json").read_text())
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
        # charging holds the RPG stance (boss: the old load pose was weird); cast clips come from hero_mapping
        assert charge_pose["clip"] == MAP["roles"]["spell_charge"], charge_pose
        beta.shot("beta-mage-fireball-charge")
        send()
        released = step(1)
        assert released["fighters"][0]["action"] == "cast fireball"
        assert not released["projectiles"], "fireball launched before its cast point"
        release_pose = pose()
        assert release_pose["clip"] in MAP["spells"]["fireball_charged"] and release_pose["rule_sample"], release_pose
        assert not step(48)["projectiles"]
        launched = step(7)
        assert len(launched["projectiles"]) == 1 and launched["projectiles"][0]["spell"] == "fireball", launched
        launch_pose = pose()
        marks = launch_pose["contact_timing"]
        elapsed = launched["fighters"][0]["action_elapsed"]
        expected = (marks["peak"] + (1 - marks["peak"]) * (elapsed - 0.42) / 0.50) * marks["duration_s"]
        assert abs(launch_pose["seek_s"] - expected) < 0.002, (launch_pose, expected)
        beta.shot("beta-mage-fireball-launch")
        step(90)
        assert gb.call("gb.state")["fighters"][1]["hp"] < gb.call("gb.state")["fighters"][1]["max_hp"]
        evidence["checks"].append({"kind": "charged_fireball_pose_launch_and_hit", "charge": charge_pose, "release": release_pose, "launch": launch_pose})

        reset()
        send(cast=0)
        step(3)
        assert pose()["clip"] in MAP["spells"]["fireball"], pose()
        assert not gb.call("gb.state")["projectiles"]
        step(50)
        assert gb.call("gb.state")["projectiles"]
        evidence["checks"].append({"kind": "tap_fireball_mage_windup"})

        lightning = []
        for _ in range(2):
            reset()
            send(cast=1)
            step(12)
            snapshot = pose()
            assert snapshot["clip"] in MAP["spells"]["chain lightning"] and snapshot["rule_sample"], snapshot
            assert gb.call("gb.state")["fighters"][1]["hp"] == gb.call("gb.state")["fighters"][1]["max_hp"], "chain lightning hits at its release, not before"
            beta.shot("beta-" + snapshot["clip"])
            step(42)
            assert gb.call("gb.state")["fighters"][1]["hp"] < gb.call("gb.state")["fighters"][1]["max_hp"]
            lightning.append(snapshot)
        assert lightning[0]["clip"] != lightning[1]["clip"], "chain lightning alternates casting hands"
        evidence["checks"].append({"kind": "chain_lightning_hands_and_hit", "poses": lightning})
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
