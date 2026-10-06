"""Plain-text character sheet. Presentation only; reads the model, never changes it."""
from __future__ import annotations

from typing import List

from . import data
from .character import GROUPS, PCT_STATS, Character, item_price

LABELS = {
    "max_hp": "Max HP", "hp_regen": "HP Regen /s", "armor": "Armor", "magic_resist": "Magic Resist",
    "tenacity": "Tenacity", "attack_damage": "Attack Damage", "attack_speed": "Attack Speed /s",
    "crit_chance": "Crit Chance", "crit_damage": "Crit Damage", "armor_pen": "Armor Pen",
    "magic_pen": "Magic Pen", "spell_power": "Spell Power", "lifesteal": "Lifesteal",
    "max_mana": "Max Mana", "mana_regen": "Mana Regen /s", "cdr": "Cooldown Red.",
    "attunement_slots": "Attunement Slots", "move_speed": "Move Speed m/s", "climb_speed": "Climb Speed m/s",
    "jump_height": "Jump Height m", "stamina": "Stamina", "carry": "Carry Capacity kg",
    "stealth": "Stealth", "perception": "Perception",
}


def _val(c: Character, stat: str) -> str:
    r = c.stat(stat)
    if stat in PCT_STATS:
        s = f"{r.value * 100:.0f}%"
    elif stat == "crit_damage":
        s = f"{r.value * 100:.0f}%"
    elif stat in ("max_hp", "max_mana", "attack_damage", "armor", "magic_resist", "stamina", "attunement_slots",
                  "spell_power", "stealth", "perception", "armor_pen", "magic_pen"):
        s = f"{r.value:.0f}"
    else:
        s = f"{r.value:.2f}"
    return s + (" *" if r.note else "")


def render_sheet(c: Character) -> str:
    race = data.races()[c.race]
    L: List[str] = []
    bar = "=" * 62
    L.append(bar)
    standing = "-"
    if c.calling:
        standing = f"{data.jobs()['jobs'][c.calling]['name']} ({c.proficiency_tier(c.calling)})"
    L.append(f" {c.name.upper()}   {race['name']}   Lv {c.level}   Calling: {standing}")
    L.append(f" XP {c.xp:.0f}   Gold {c.gold}   Corruption {c.corruption:.0f}/100")
    L.append(f" Passive - {race['passive']['name']}: {race['passive']['text']}")
    L.append(bar)
    mhp, mmp = c.stat("max_hp").value, c.stat("max_mana").value
    L.append(f" HP   {c.hp:7.0f} / {mhp:<6.0f}   Mana {c.mana:6.0f} / {mmp:.0f}")
    a = {k: c.attribute(k) for k in ("str", "agi", "int")}
    L.append(f" STR {a['str'].value:5.1f}    AGI {a['agi'].value:5.1f}    INT {a['int'].value:5.1f}")
    L.append("-" * 62)
    for title, stats in GROUPS:
        if title == "Defence":
            stats = [s for s in stats if s != "max_hp"]
        if title == "Magic":
            stats = [s for s in stats if s != "max_mana"]
        cells = [f"{LABELS[s]:<18}{_val(c, s):>8}" for s in stats]
        L.append(f" {title}")
        for i in range(0, len(cells), 2):
            L.append("   " + "   ".join(cells[i:i + 2]))
    L.append(f"   DPS (incl. crit) {c.dps():>7.1f}     Carried {c.carried_weight():.1f} kg")
    L.append("-" * 62)
    L.append(" Equipment")
    for slot, iid in c.equipment.items():
        it = data.items()[iid]
        L.append(f"   {slot:<11}{it['name']:<24} T{it['tier']}  ~{item_price(iid)}g")
    L.append(f" Active effects  (slots {c.slots_used()}/{c.stat('attunement_slots').value:.0f})")
    if not c.effects:
        L.append("   (none)")
    for e in c.effects:
        left = "sustained" if e.expires_at is None else f"{max(0, e.expires_at - c.clock):.0f}s left"
        tag = f"{e.aspect.upper()} " if e.aspect else ""
        L.append(f"   {tag}{e.name:<34}{e.kind:<11}{left}")
    L.append(" Jobs")
    for job in [c.calling] + c.sidelines:
        if job:
            st = c.job_standing(job)
            L.append(f"   {data.jobs()['jobs'][job]['name']:<10}{st['kind']:<9}{st['tier']:<11}"
                     f"pay x{st['pay']:.2f}  xp x{st['xp']:.2f}")
    L.append(bar)
    L.append(" * = a cap or slow is affecting this value")
    return "\n".join(L)
