"""Compile progression and regional variants from valid game moveset JSON."""
import copy
import json


ATTACK_SLOTS = ("light", "heavy")
SINGLE_ATTACKS = ("sprint_attack", "air_attack")


def _attacks(moveset):
    for slot in ATTACK_SLOTS:
        for attack in moveset.get(slot, []):
            yield slot, attack
    for slot in SINGLE_ATTACKS:
        if moveset.get(slot):
            yield slot, moveset[slot]


def validate_moveset(moveset):
    errors = []
    if not moveset.get("name") or not moveset.get("kind"):
        errors.append("moveset needs name and kind")
    names = set()
    for slot, attack in _attacks(moveset):
        name = attack.get("name")
        if not name or name in names:
            errors.append(f"{slot}: missing or duplicate attack name {name!r}")
        names.add(name)
        clip = attack.get("clip", {})
        if not clip.get("track"):
            errors.append(f"{name}: clip.track missing")
        windows = [float(attack.get(k, -1)) for k in ("windup", "active", "cancel")]
        if not (0 <= windows[0] < windows[1] <= windows[2] <= 1):
            errors.append(f"{name}: expected 0 <= windup < active <= cancel <= 1, got {windows}")
        if float(clip.get("speed", 1.0)) <= 0:
            errors.append(f"{name}: clip speed must be positive")
    return errors


def compile_variant(base, style_name, profile, strength=0.5, rank=0.5, track_links=None, name=None):
    """Create a game-schema moveset variant; base remains unchanged.

    ``track_links`` connects generated/exported clips when available. Until then the
    same authored tracks receive new tempo and combat tuning, so progression can be
    tested before a new GLB is exported.
    """

    strength = max(0.0, min(1.0, float(strength)))
    rank = max(0.0, min(1.0, float(rank)))
    links = track_links or {}
    out = copy.deepcopy(base)
    out["name"] = name or f"{base['name']}__{style_name}__s{round(strength * 10):02d}r{round(rank * 10):02d}"
    tempo = float(profile.get("tempo", 1.0)) * (0.9 + 0.2 * rank)
    power = 0.72 + 0.68 * strength
    discipline = 0.82 + 0.28 * rank
    for _slot, attack in _attacks(out):
        clip = attack["clip"]
        clip["track"] = links.get(clip["track"], clip["track"])
        clip["speed"] = round(float(clip.get("speed", 1.0)) * tempo, 4)
        for key in ("damage", "poise", "knockback"):
            if key in attack:
                attack[key] = round(float(attack[key]) * power, 3)
        if "stamina" in attack:
            attack["stamina"] = round(float(attack["stamina"]) * (1.12 - 0.18 * rank + 0.08 * strength), 3)
        if "travel" in attack:
            attack["travel"] = round(float(attack["travel"]) * (0.8 + 0.4 * strength) +
                                     float(profile.get("power_travel_m", 0.0)) * 0.25, 3)
        # Better rank narrows dead recovery without shifting the active contact window.
        attack["cancel"] = round(max(float(attack["active"]), min(1.0, float(attack["cancel"]) * discipline)), 4)
    guard = out.get("guard")
    if guard:
        guard["parry_window"] = round(float(guard["parry_window"]) * (0.7 + 0.45 * rank), 4)
        guard["absorb"] = round(min(1.0, float(guard["absorb"]) * (0.85 + 0.25 * strength)), 4)
    errors = validate_moveset(out)
    if errors:
        raise ValueError("invalid compiled moveset: " + "; ".join(errors))
    manifest = {"generated_by": "kev-autoanim", "base": base["name"], "moveset": out["name"],
                "region_style": style_name, "strength": strength, "rank": rank,
                "profile": profile, "track_links": links}
    return out, manifest


def compile_file(base_path, output_path, manifest_path, style_name, profile, strength=0.5, rank=0.5,
                 track_links=None, name=None):
    with open(base_path, encoding="utf-8") as f:
        base = json.load(f)
    out, manifest = compile_variant(base, style_name, profile, strength, rank, track_links, name)
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)
        f.write("\n")
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")
    return out, manifest
