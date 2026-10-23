"""Deterministic regional/rank/strength variants of Kev landmark references."""
import copy
import json
import math
import os

import numpy as np


HERE = os.path.dirname(os.path.abspath(__file__))


def load_styles(path=os.path.join(HERE, "styles.json")):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def _unit(v):
    n = float(np.linalg.norm(v))
    return v / n if n > 1e-8 else np.zeros(3)


def _smoothstep(a, b, x):
    u = float(np.clip((x - a) / max(b - a, 1e-6), 0.0, 1.0))
    return u * u * (3.0 - 2.0 * u)


def style_reference(reference, profile, strength=0.5, rank=0.5, name=None):
    """Return a styled copy. Strength/rank are 0..1 and never mutate source data.

    Spatial edits are landmark targets; Kev's existing cleanup restores exact limb
    lengths during bake.  Timing, power, guard and stance remain independently tunable.
    """

    strength = float(np.clip(strength, 0.0, 1.0))
    rank = float(np.clip(rank, 0.0, 1.0))
    out = copy.deepcopy(reference)
    old_duration = max(float(reference.get("duration", 0.0)), 1e-6)
    tempo = float(profile.get("tempo", 1.0)) * (0.9 + 0.2 * rank)
    contact = float(profile.get("contact_phase", 0.55))
    power = float(profile.get("power_travel_m", 0.0)) * (0.35 + 0.9 * strength)
    wobble = (1.0 - rank) * 0.025
    for sample in out.get("samples", []):
        phase = float(sample["t"]) / old_duration
        sample["t"] = round(float(sample["t"]) / max(tempo, 0.1), 6)
        p = {k: np.asarray(v, float) for k, v in sample.get("landmarks", {}).items()}
        if "pelvis" not in p:
            continue
        up = _unit(p.get("neck", p["pelvis"] + [0, 1, 0]) - p["pelvis"])
        left = _unit(p.get("hip_l", p["pelvis"] + [1, 0, 0]) - p.get("hip_r", p["pelvis"] - [1, 0, 0]))
        forward = _unit(np.cross(left, up))
        # Back-load before contact, drive through it, then settle.
        drive = _smoothstep(contact - 0.18, contact, phase) - _smoothstep(contact + 0.04, min(1.0, contact + 0.3), phase)
        anticipation = _smoothstep(max(0.0, contact - 0.4), contact - 0.18, phase) - _smoothstep(contact - 0.18, contact, phase)
        root_shift = forward * power * (drive - 0.35 * anticipation)
        root_shift += left * float(profile.get("lateral_flow_m", 0.0)) * math.sin(phase * math.tau)
        lean = forward * float(profile.get("forward_lean_m", 0.0)) * math.sin(math.pi * phase)
        noise = left * wobble * math.sin(phase * math.tau * 2.17 + 0.7)
        for n in list(p):
            p[n] = p[n] + root_shift
            if n in ("neck", "head_joint", "crown", "nose", "shoulder_l", "shoulder_r"):
                p[n] = p[n] + lean + noise
        guard = up * float(profile.get("guard_height_m", 0.0)) * (0.6 + 0.4 * rank)
        for side in "lr":
            sign = 1.0 if side == "l" else -1.0
            widen = left * sign * float(profile.get("stance_width_m", 0.0)) * (0.5 + strength * 0.5)
            for n, share in ((f"hip_{side}", 0.1), (f"knee_{side}", 0.45), (f"ankle_{side}", 1.0),
                             (f"heel_{side}", 1.0), (f"ball_{side}", 1.0), (f"toe_tip_{side}", 1.0)):
                if n in p:
                    p[n] = p[n] + widen * share
            for n, share in ((f"elbow_{side}", 0.45), (f"wrist_{side}", 1.0), (f"knuckle_{side}", 1.0)):
                if n in p:
                    p[n] = p[n] + guard * share
        sample["landmarks"] = {k: [round(float(x), 6) for x in v] for k, v in p.items()}
    out["duration"] = round(old_duration / max(tempo, 0.1), 6)
    out["name"] = name or f"{reference.get('name', 'motion')}_styled"
    out["clip"] = out["name"]
    out.setdefault("source", {})["style"] = {"profile": profile, "strength": strength, "rank": rank,
                                                "tempo": tempo}
    return out


def style_file(input_path, output_path, style_name, strength=0.5, rank=0.5, styles_path=None):
    with open(input_path, encoding="utf-8") as f:
        source = json.load(f)
    styles = load_styles(styles_path or os.path.join(HERE, "styles.json"))
    if style_name not in styles:
        raise ValueError(f"unknown style {style_name!r}; choose from {', '.join(sorted(styles))}")
    name = f"{source.get('name', 'motion')}__{style_name}__s{round(strength * 10):02d}r{round(rank * 10):02d}"
    out = style_reference(source, styles[style_name], strength, rank, name)
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=1)
        f.write("\n")
    return out
