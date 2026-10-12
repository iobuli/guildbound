"""Measure live planted-foot error, fixed lengths, pause and release behaviour."""
import importlib.util
import json
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("beta", ROOT / "Tools/check_beta.py")
beta = importlib.util.module_from_spec(spec)
spec.loader.exec_module(beta)
gb, step, reset, pose = beta.gb, beta.step, beta.reset, beta.pose


def player():
    return next(s for s in gb.call("gb.support")["fighters"] if s["fighter"] == 0)


def main():
    evidence = {"checks": [], "pass": False}
    try:
        gb.call("gb.control", {"state": "play"})
        beta.wait_for(lambda s: s["app_state"] == "Playing")
        gb.call("gb.support", {"enabled": True})
        for name, vector, running in [("idle", [0, 0], False), ("walk", [0, -1], False), ("strafe", [1, 0], False), ("run", [0, -1], True)]:
            reset(name == "strafe")
            gb.call("gb.input", {"move": vector, "run": running, "ticks": 400})
            rows = []
            held = []
            for _ in range(60):
                step(4)
                row = player()
                assert row["enabled"], (name, row)
                for foot in row["feet"]:
                    assert foot["length_error_m"] < 0.0002, "IK changed a rest length"
                    if foot["phase"] == "planted" and foot["weight"] > 0.98 and foot["contact"] > 0.65:
                        held.append(foot["anchor_error_m"])
                rows.append(row)
            evidence["checks"].append({"kind": name, "planted_samples": len(held), "max_anchor_error_m": max(held) if held else None, "samples": rows})
            assert held, f"{name} never reached a fully planted sample"
            assert max(held) < 0.02, (name, max(held))
            frozen = pose()["pose_hash"]
            time.sleep(0.2)
            assert pose()["pose_hash"] == frozen, "paused support pose changed"
            beta.shot("beta-support-" + name)
        reset(False)
        gb.call("gb.input", {"move": [0, 0], "ticks": 60})
        step(40)
        assert any(f["phase"] == "planted" for f in player()["feet"])
        gb.call("gb.input", {"move": [0, 1], "jump": True, "ticks": 1})
        step(28)
        assert not player()["enabled"]
        assert all(f["weight"] == 0.0 for f in player()["feet"]), "jump retained foot anchors"
        step(150)
        assert player()["enabled"]
        evidence["checks"].append({"kind": "jump_releases_and_landing_restores_support"})
        evidence["pass"] = True
    except BaseException as exc:
        evidence["error"] = str(exc)
        raise
    finally:
        (ROOT / "Documentation/checks/live/beta-support-smoke.json").write_text(json.dumps(evidence, indent=2))
        gb.call("gb.control", {"state": "menu", "pause": True})
    print(f"BETA_SUPPORT_PASS {len(evidence['checks'])} checks")


if __name__ == "__main__":
    main()
