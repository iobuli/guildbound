"""Show clips live in Possession, with runtime rig corrections. No screenshots.

python -I Tools/possession_live.py --reference --seconds 6
python -I Tools/possession_live.py --clips Unarmed-Attack-L1,Unarmed-Attack-R1,hp_hook_left_A
Stops changing clips if the boss selects a different clip or generated motion.
"""
import argparse
import json
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "Tools"))
from gb import call


def load(model, hero):
    call("gb.possess", {"motion": None, "clear": True, "auto_hands": True,
                        "grip": None, "look": "off", "breath": False})
    call("gb.studio", {"open": True, "spawn": model, "hero_clips": hero,
                       "rules": True, "tab": 1})
    for _ in range(100):
        time.sleep(0.1)
        if call("gb.possess", {"bones": True}).get("bones"):
            return
    raise RuntimeError("stage model did not finish loading")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", action="store_true")
    parser.add_argument("--seconds", type=float, default=6)
    parser.add_argument("--clips", default="Unarmed-Attack-L1,Unarmed-Attack-R1,hp_hook_left_A,Unarmed-Run-Forward")
    args = parser.parse_args()
    call("gb.control", {"state": "play", "pause": True})
    time.sleep(0.2)
    models = [("packs/hero/fighter.glb", True)]
    if args.reference:
        models.insert(0, ("packs/_sources/rpg_hero/fighter.glb", False))
    report = []
    for model, hero in models:
        load(model, hero)
        names = call("gb.studio").get("clip_list", [])
        for clip in args.clips.split(","):
            if clip not in names:
                continue
            call("gb.studio", {"clip": clip, "time": 0, "hero_clips": hero, "tab": 1})
            call("gb.possess", {"play": True})
            print(f"LIVE {model}: {clip}", flush=True)
            start = time.monotonic()
            worst_clearance = 0
            while time.monotonic() - start < args.seconds:
                time.sleep(0.2)
                s = call("gb.possess")
                if s["clip"] != clip or s.get("motion") is not None or s.get("playing") is False:
                    print("Boss changed the stage; leaving it under their control.", flush=True)
                    return
                worst_clearance = max(worst_clearance, s.get("clearance", {}).get("forearm_penetration_m", 0))
            report.append({"model": model, "clip": clip, "worst_forearm_penetration_m": worst_clearance,
                           "last_pose": s.get("numbers"), "foot_support": s.get("clip_support")})
    output = ROOT / "Documentation/checks/live/possession-live-tour.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print("Live tour finished; last animation remains playing.", flush=True)


if __name__ == "__main__":
    main()
