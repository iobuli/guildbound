"""Check the built Windows stack reserve and stress a running beta's lifecycle.

python -I Tools/check_beta_liveness.py --seconds 90 --cycles 30
The check returns to the menu and polls the local API through the observation
period. An exited or unresponsive game fails instead of being reported as live.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import struct
import time

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("gb", ROOT / "Tools" / "gb.py")
gb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gb)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--seconds", type=float, default=90)
    p.add_argument("--cycles", type=int, default=30)
    a = p.parse_args()
    exe = (ROOT / "target" / "debug" / "guildbound.exe").read_bytes()
    pe = struct.unpack_from("<I", exe, 0x3c)[0] + 24
    assert struct.unpack_from("<H", exe, pe)[0] == 0x20b, "expected a PE32+ executable"
    reserve = struct.unpack_from("<Q", exe, pe + 72)[0]
    assert reserve >= 8 * 1024 * 1024, f"main-thread stack reserve is only {reserve} bytes"
    started = time.monotonic()
    result = {"stack_reserve_bytes": reserve, "cycles": 0, "pointer_events": 0, "polls": 0, "pass": False}
    out = ROOT / "Documentation" / "checks" / "live" / "beta-liveness.json"
    try:
        gb.call("gb.control", {"state": "menu", "pause": True})
        deadline = time.monotonic() + 8
        while not gb.call("gb.menu_pointer")["buttons"]:
            assert time.monotonic() < deadline, "menu did not become ready"
            time.sleep(0.04)
        for i in range(a.cycles):
            buttons = gb.call("gb.menu_pointer")["buttons"]
            for button in buttons:
                for event in ["over", "out"]:
                    gb.call("gb.menu_pointer", {"button": button, "event": event})
                    result["pointer_events"] += 1
            preferred = ["Practice", "Resume", "New spar", "Spar"] if i % 2 else ["New spar", "Spar", "Resume", "Practice"]
            button = next(name for name in preferred if name in buttons)
            gb.call("gb.menu_pointer", {"button": button, "event": "over"})
            gb.call("gb.menu_pointer", {"button": button, "event": "click"})
            result["pointer_events"] += 2
            time.sleep(0.12)
            assert gb.call("gb.state")["app_state"] == "Playing", "MB1 click did not start/resume play"
            gb.call("gb.control", {"pause": True, "step": 1})
            time.sleep(0.06)
            gb.call("gb.control", {"state": "menu"})
            time.sleep(0.12)
            state = gb.call("gb.state")
            assert state["app_state"] == "MainMenu"
            result["cycles"] = i + 1
        print(f"MENU_POINTER_CYCLES_PASS {result['cycles']} events={result['pointer_events']}", flush=True)
        next_report = time.monotonic() + 15
        while time.monotonic() - started < a.seconds:
            state = gb.call("gb.state")
            result["polls"] += 1
            if time.monotonic() >= next_report:
                print(f"LIVE {time.monotonic() - started:.0f}s {state['app_state']}", flush=True)
                next_report += 15
            time.sleep(0.5)
        result["observed_s"] = round(time.monotonic() - started, 2)
        result["pass"] = True
        print(f"BETA_LIVENESS_PASS {result['observed_s']}s", flush=True)
    except BaseException as exc:
        result["error"] = str(exc)
        raise
    finally:
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
