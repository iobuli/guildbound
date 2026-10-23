"""Build coherent Trunks/Tapion sword schools from the configured local game archive."""
import json
import math
import numpy as np

import retarget_glb as R
from sparking_library import DATA, ROOT, catalog, export

NEUTRAL = {"exaggeration": 1.0, "hold": 0.0, "bias": 0.5, "stepped_fps": None, "speed": 1.0}


def decoded(clips, character, name):
    item = next(c for c in clips if c["character"] == character and c["name"] == name)
    export(item["id"])
    return json.loads((DATA / f"{item['id']}.json").read_text())


def merge(sections, name):
    result = dict(sections[0], name=name, frames=[])
    result["source"] = " + ".join(s["source"] for s in sections)
    offset = 0.0
    for section in sections:
        if section["bones"] != result["bones"]:
            raise ValueError(f"Skeleton changes within {name}")
        for frame in section["frames"]:
            t = offset + frame["t"]
            if not result["frames"] or t > result["frames"][-1]["t"] + 1e-7:
                result["frames"].append(dict(frame, t=t))
        offset += section["duration"]
    result["duration"] = offset
    return result


def weapon(label, source, scale):
    # Pack vertices are already in metres; remove leftover FBX node TRS, then -Z blade -> +Y.
    js, blob = R.statue.read_glb(ROOT / f"assets/packs/weapons/{source}.glb")
    for mesh in js["meshes"]:
        for primitive in mesh["primitives"]:
            points = R.statue.accessor_view(js, blob, primitive["attributes"]["POSITION"])[0].copy()
            colors = np.ones((len(points), 4))
            for i, point in enumerate(points):
                colors[i, :3] = ([0.52, 0.66, 0.78] if point[2] < -0.07 else
                                 [0.53, 0.31, 0.09] if point[2] < 0.035 or point[2] > 0.15 else
                                 [0.045, 0.025, 0.018])
            primitive["attributes"]["COLOR_0"] = R.add_accessor(js, blob, colors, 4, "VEC4")
    for material in js.get("materials", []):
        material.setdefault("pbrMetallicRoughness", {}).update(metallicFactor=0.75, roughnessFactor=0.32)
        material.get("extensions", {}).get("KHR_materials_specular", {}).update(specularFactor=1.0)
    js["buffers"][0]["byteLength"] = len(blob)
    for node in js["nodes"]:
        for key in ("matrix", "rotation", "translation", "scale"):
            node.pop(key, None)
        node["rotation"] = [math.sqrt(0.5), 0.0, 0.0, math.sqrt(0.5)]
        node["scale"] = scale
    path = ROOT / f"assets/weapons/{label.lower()}_sword.glb"
    path.parent.mkdir(parents=True, exist_ok=True)
    R.statue.write_glb(path, js, blob)
    return str(path.relative_to(ROOT / "assets")).replace("\\", "/")


def build(character, label, model, model_scale, blade_length):
    clips = catalog()["clips"]
    library = label.lower() + "_sword"
    sources, report = [], []

    def save(data):
        data["in_place"] = True
        path = DATA / f"{data['name']}_school.json"
        path.write_text(json.dumps(data), encoding="utf-8")
        sources.append(path)

    for number in range(1, 6):
        prefix = f"A_{character}_C_ATC_a{number:03d}_R1_{number}_"
        sections = [decoded(clips, character, prefix + part) for part in ("IN", "HT", "OT")]
        name = f"{label}_Cut_{number:02d}"
        data = merge(sections, name)
        save(data)
        if number == 1:
            save(dict(data, name=f"{label}_Sword_Combo"))  # Preserve existing viewer bookmarks.
        report.append({"track": f"{library}/{name}", "duration": data["duration"],
                       "contact_start": sections[0]["duration"],
                       "contact_end": sections[0]["duration"] + sections[1]["duration"],
                       "source": data["source"]})

    for suffix, alias in (("A_STD_b002_STD_G_LP", "Idle_Loop"), ("A_STD_d001_GRD_A_LP", "Guard_Loop")):
        data = decoded(clips, character, f"A_{character}_{suffix}")
        save(dict(data, name=f"{label}_{alias}"))

    output = ROOT / f"assets/anim/{library}.glb"
    partial = output.with_name(output.stem + ".partial.glb")
    partial.unlink(missing_ok=True)
    R.convert(ROOT / "assets/anim/ual2.glb", partial, sources, animation_only=True)
    partial.replace(output)

    def attack(number, name, damage, stamina, travel, heavy=False):
        timing = report[number - 1]
        end = min(timing["duration"], 1.0 if heavy else 0.72)
        contact = timing["contact_end"]
        result = {"name": name, "clip": {"track": timing["track"], "end": end,
                    "speed": 1.0, "style": NEUTRAL},
                "windup": round(timing["contact_start"] / end, 6),
                "active": round(contact / end, 6),
                "cancel": round(min((contact + 0.12) / end, 0.85), 6),
                "damage": damage, "poise": damage * 1.3, "knockback": 4.0 if heavy else 1.8,
                "stamina": stamina, "travel": travel, **({"charge": 0.65} if heavy else {})}
        if timing["duration"] > end + 0.05:
            result["recover"] = {"track": timing["track"], "start": end,
                                 "end": timing["duration"], "style": NEUTRAL}
        return result

    moveset = {
        "name": library, "kind": "melee",
        "weapon": {"mesh": weapon(label, model, model_scale), "hand": "r", "mass": 1.1,
                   "offset": [-0.025, 0.075, 0.0], "rotation_deg": [0, 90, 90], "length": blade_length,
                   "carry": {"at": "back", "grip": [-0.16, 1.5, -0.18],
                             "blade": [0.42, -0.9, 0.0], "face": [0.0, 0.0, -1.0]}},
        "stance": {"clip": {"track": f"{library}/{label}_Idle_Loop", "mask": "upper", "style": NEUTRAL},
                   "idle": f"{library}/{label}_Idle_Loop", "hold": 0.1, "moving_weight": 0.75},
        "light": [attack(1, f"{label} opening cut", 13, 10, 0.18),
                  attack(2, f"{label} return cut", 15, 12, 0.22),
                  attack(3, f"{label} chain finisher", 19, 14, 0.3)],
        "heavy": [attack(5, f"{label} charged finisher", 29, 23, 0.45, heavy=True)],
        "sprint_attack": attack(4, f"{label} advancing cut", 21, 18, 0.65),
        "air_attack": attack(5, f"{label} falling cut", 25, 20, 0.0),
        "guard": {"clip": {"track": f"{library}/{label}_Guard_Loop", "mask": "upper", "style": NEUTRAL},
                  "hold": 0.1, "parry_window": 0.18, "absorb": 0.8, "arc": 140},
    }
    path = ROOT / f"assets/anim/movesets/{library}.json"
    path.write_text(json.dumps(moveset, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {output.relative_to(ROOT)} and {path.relative_to(ROOT)}")
    return {"character": character, "style": library, "attacks": report}


if __name__ == "__main__":
    reports = [build("0080", "Trunks", "longsword", [0.6, 0.12, 1.0], 0.856),
               build("0660", "Tapion", "knight_sword", [0.9, 0.25, 1.0], 0.797)]
    (ROOT / "assets/anim/sword_styles.report.json").write_text(json.dumps(reports, indent=2) + "\n", encoding="utf-8")
