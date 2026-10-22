"""Studio console: combo list | combo play <name|file> | combo bake <name|file> | combo show <name>."""
import json
import os

from .bake import bake_combo
from .compile import list_combos, load_combo


def _name(a):
    return " ".join(a) if a else ""


def handle(args, studio):
    sub = args[0] if args else "list"
    rest = args[1:]
    if sub == "list":
        rows = []
        for n in list_combos():
            c = load_combo(n)
            moves = ", ".join(m.get("id", m["do"]) for m in c["moves"])
            rows.append(f"{n:28} {c.get('model', ''):12} {moves}")
        return "\n".join(rows) or "(no combos)"
    if sub == "show" and rest:
        c = load_combo(rest[0])
        c.pop("_path", None)
        return json.dumps(c, indent=1)
    if sub in ("play", "bake") and rest:
        target = rest[0]
        model = None
        if "--model" in rest:
            model = rest[rest.index("--model") + 1]
        row = bake_combo(studio, target, model=model, render=True)
        r = row["report"]
        line = (f"combo '{r['name']}' -> {r['model']}: {r['frames']} frames, {r['duration_s']}s, "
                f"jerk {r['jerk']}, max joint step {r['max_joint_step_deg']} deg ({r['max_joint_step_bone']}), "
                f"plant slip {r['plant_slip_cm']} cm, segment drift {r['segment_drift_cm']} cm, "
                f"reach {r['reach_mean_cm']}/{r['reach_max_cm']} cm, over_foot {r['over_foot']:.0%}")
        if r["fails"]:
            line += "\nFAIL " + "; ".join(r["fails"])
        if sub == "play":
            if not studio.STATE.get("model"):
                return line + "\n(baked; open the viewer and `model " + r["model"] + "` then `combo play` to see it)"
            if studio.STATE["model"] != r["model"]:
                studio.send_cmd({"cmd": "model", "file": r["model"]})
            studio.stop_ref()
            b = row["payload"]
            studio.send_cmd({"cmd": "baked", "fps": b["fps"], "frames": b["frames"], "loop": False})
            line += "\nplaying once (holds the last frame)"
        else:
            line += "\nwrote " + os.path.basename(row["baked"])
        return line
    return "combo list | combo show <name> | combo bake <name|file.json> [--model ual1.glb] | combo play <name|file.json>"
