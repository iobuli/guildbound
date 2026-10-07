"""The character model. Base values are the only truth; every number on the sheet is derived
from them through the modifier stack in stats.py, recomputed on demand."""
from __future__ import annotations

import math

from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

from . import data
from .stats import Modifier, StatResult, damage_taken_fraction, resolve

# ---- tuning (move to data/ once it settles) -------------------------------------------------

# derived stat -> (constant base, per-level base, [(attribute, op, per point)])
DERIVED = {
    "max_hp":       (200.0, 10.0, [("str", "flat", 22.0)]),
    "hp_regen":     (1.0, 0.0, [("str", "flat", 0.10)]),
    "armor":        (5.0, 0.0, [("agi", "flat", 0.15)]),
    "magic_resist": (5.0, 0.0, []),
    "attack_damage": (10.0, 0.0, []),      # + weapon base + scaling attribute (special-cased)
    "attack_speed": (1.0, 0.0, [("agi", "add_pct", 0.01)]),  # base replaced by weapon aps
    "crit_chance":  (0.05, 0.0, []),
    "crit_damage":  (1.75, 0.0, []),
    "armor_pen":    (0.0, 0.0, []),
    "magic_pen":    (0.0, 0.0, []),
    "lifesteal":    (0.0, 0.0, []),
    "spell_power":  (0.0, 0.0, [("int", "flat", 1.0)]),
    "max_mana":     (50.0, 0.0, [("int", "flat", 12.0)]),
    "mana_regen":   (1.0, 0.0, [("int", "flat", 0.05)]),
    "cdr":          (0.0, 0.0, []),
    "tenacity":     (0.0, 0.0, []),
    "power":        (1.0, 0.0, [("str", "add_pct", 0.008)]),  # impact of blows: stagger, knockdown, shove
    "move_speed":   (6.0, 0.0, []),
    "climb_speed":  (1.5, 0.0, []),
    "jump_height":  (1.2, 0.0, []),
    "stamina":      (100.0, 0.0, []),
    "carry":        (30.0, 0.0, [("str", "flat", 2.0)]),
    "stealth":      (10.0, 0.0, []),
    "perception":   (10.0, 0.0, [("int", "flat", 0.2)]),
    "attunement_slots": (3.0, 0.0, []),   # + level//10 + int//50 (special-cased)
}
PCT_STATS = {"crit_chance", "tenacity", "cdr", "lifesteal"}
GROUPS = [
    ("Offence", ["attack_damage", "attack_speed", "power", "crit_chance", "crit_damage", "armor_pen", "magic_pen",
                 "spell_power", "lifesteal"]),
    ("Defence", ["max_hp", "hp_regen", "armor", "magic_resist", "tenacity"]),
    ("Magic", ["max_mana", "mana_regen", "cdr", "attunement_slots"]),
    ("Movement", ["move_speed", "climb_speed", "jump_height", "stamina", "carry"]),
    ("Senses", ["stealth", "perception"]),
]
EQUIP_SLOTS = ["weapon", "offhand", "head", "body", "hands", "feet", "accessory1", "accessory2"]
STOMACH_LIMIT = 3
FIST_DAMAGE, FIST_APS = 0.0, 1.0
HP_FLOOR_FRACTION = 0.10  # Dark costs never kill you: sustained effects collapse at 10% HP


def xp_to_next(level: int) -> int:
    return 100 * level * level


# ---- active effects --------------------------------------------------------------------------

@dataclass
class ActiveEffect:
    key: str
    name: str
    kind: str                       # spell | consumable | backlash
    started_at: float
    expires_at: Optional[float]     # None = sustained
    slots: int = 0
    upkeep: float = 0.0
    hp_drain: float = 0.0
    mods: List[Modifier] = field(default_factory=list)
    backlash: Optional[dict] = None
    family: str = ""
    aspect: str = ""


# ---- character -------------------------------------------------------------------------------

class Character:
    def __init__(self, name: str, race: str, level: int = 1, calling: Optional[str] = None,
                 sidelines: Optional[List[str]] = None, gold: int = 0):
        if race not in data.races():
            raise ValueError(f"unknown race {race!r}")
        self.name = name
        self.race = race
        self.level = level
        self.xp = 0
        self.gold = gold
        self.calling = calling
        self.sidelines = list(sidelines or [])
        self.proficiency: Dict[str, float] = {}
        self.allocated = {"str": 0, "agi": 0, "int": 0}
        self.equipment: Dict[str, str] = {}
        self.inventory: List[str] = []
        self.effects: List[ActiveEffect] = []
        self.cooldowns: Dict[str, float] = {}
        self.corruption = 0.0
        self.clock = 0.0
        self.hp = 0.0
        self.mana = 0.0
        self.restore()

    # -- construction helpers --
    def restore(self) -> None:
        self.hp = self.stat("max_hp").value
        self.mana = self.stat("max_mana").value

    def _clamp(self) -> None:
        self.hp = min(self.hp, self.stat("max_hp").value)
        self.mana = min(self.mana, self.stat("max_mana").value)

    def equip(self, item_id: str) -> None:
        item = data.items()[item_id]
        slot = item["slot"]
        if slot is None:
            raise ValueError(f"{item['name']} cannot be equipped")
        if slot == "accessory":
            slot = "accessory1" if "accessory1" not in self.equipment else "accessory2"
        self.equipment[slot] = item_id
        self._clamp()

    def unequip(self, slot: str) -> None:
        self.equipment.pop(slot, None)
        self._clamp()

    def add_item(self, item_id: str) -> None:
        self.inventory.append(item_id)

    # -- modifiers --
    def _weapon(self) -> Optional[dict]:
        wid = self.equipment.get("weapon")
        return data.items()[wid].get("weapon") if wid else None

    def carried_weight(self) -> float:
        ids = list(self.equipment.values()) + self.inventory
        return sum(data.items()[i]["weight"] for i in ids)

    def _modifiers(self) -> List[Modifier]:
        mods: List[Modifier] = []
        race = data.races()[self.race]
        for m in race["passive"]["mods"]:
            mods.append(Modifier(f"Race: {race['name']} / {race['passive']['name']}", m["stat"], m["op"], m["value"],
                                 m.get("group")))
        for slot, item_id in self.equipment.items():
            item = data.items()[item_id]
            for m in item["mods"]:
                mods.append(Modifier(f"Item: {item['name']}", m["stat"], m["op"], m["value"], m.get("group")))
        for e in self.effects:
            mods.extend(e.mods)
        return mods

    def _all_modifiers(self, attrs: Dict[str, float]) -> List[Modifier]:
        mods = self._modifiers()
        # Encumbrance is just another modifier, so it shows up in the breakdown.
        carry_cap = resolve("carry", *self._base("carry"), self._with_attr_mods("carry", mods, attrs), self.clock).value
        w = self.carried_weight()
        if w > carry_cap:
            over = min(0.6, (w - carry_cap) / carry_cap)
            mods.append(Modifier(f"Overburdened ({w:.1f}/{carry_cap:.1f} kg)", "move_speed", "slow", over))
        return mods

    # -- attributes --
    def attribute(self, attr: str) -> StatResult:
        race = data.races()[self.race]
        base = race["base"][attr] + race["growth"][attr] * (self.level - 1) + self.allocated.get(attr, 0)
        label = f"{race['name']} {race['base'][attr]} + {race['growth'][attr]}/lvl x {self.level - 1}"
        return resolve(attr, base, label, self._modifiers(), self.clock)

    def _attrs(self) -> Dict[str, float]:
        return {a: self.attribute(a).value for a in ("str", "agi", "int")}

    # -- derived stats --
    def _base(self, stat: str) -> Tuple[float, str]:
        const, per_level, _ = DERIVED[stat]
        base = const + per_level * self.level
        label = f"{const:g}" + (f" + {per_level:g} x lvl {self.level}" if per_level else "")
        w = self._weapon()
        if stat == "attack_damage":
            wid = self.equipment.get("weapon")
            wd = w["base_damage"] if w else FIST_DAMAGE
            base = const + wd
            label = f"{const:g} unarmed" + (f" + {wd:g} {data.items()[wid]['name']}" if w else "")
        elif stat == "attack_speed":
            aps = w["aps"] if w else FIST_APS
            base = aps
            label = f"weapon {aps:g}/s"
        return base, label

    def _with_attr_mods(self, stat: str, mods: List[Modifier], attrs: Dict[str, float]) -> List[Modifier]:
        out = list(mods)
        for attr, op, per_point in DERIVED[stat][2]:
            out.append(Modifier(f"{attr.upper()} {attrs[attr]:.1f}", stat, op, per_point * attrs[attr]))
        if stat == "attack_damage":
            w = self._weapon()
            scale = w["scales"] if w else "str"
            out.append(Modifier(f"{scale.upper()} {attrs[scale]:.1f} (weapon scaling)", stat, "flat", attrs[scale]))
        return out

    def stat(self, stat: str) -> StatResult:
        attrs = self._attrs()
        base, label = self._base(stat)
        if stat == "attunement_slots":
            extra = self.level // 10 + int(attrs["int"] // 50)
            base += extra
            label += f" + {self.level // 10} (lvl/10) + {int(attrs['int'] // 50)} (INT/50)"
        if stat == "carry":
            mods = self._modifiers()
        else:
            mods = self._all_modifiers(attrs)
        return resolve(stat, base, label, self._with_attr_mods(stat, mods, attrs), self.clock)

    # -- combat maths --
    def dps(self) -> float:
        ad = self.stat("attack_damage").value
        aps = self.stat("attack_speed").value
        cc = self.stat("crit_chance").value
        cd = self.stat("crit_damage").value
        return ad * aps * (1.0 + cc * (cd - 1.0))

    def take_physical(self, raw: float, attacker_pen: float = 0.0) -> float:
        armor = self.stat("armor").value * (1.0 - attacker_pen)
        dmg = raw * damage_taken_fraction(armor)
        self.hp = max(0.0, self.hp - dmg)
        return dmg

    def take_magic(self, raw: float, attacker_pen: float = 0.0) -> float:
        mr = self.stat("magic_resist").value * (1.0 - attacker_pen)
        dmg = raw * damage_taken_fraction(mr)
        self.hp = max(0.0, self.hp - dmg)
        return dmg

    # -- magic --
    def slots_used(self) -> int:
        return sum(e.slots for e in self.effects)

    def _find(self, key: str) -> Optional[ActiveEffect]:
        return next((e for e in self.effects if e.key == key), None)

    def cast(self, spell_id: str) -> Tuple[bool, str]:
        sp = data.spells()[spell_id]
        if self._find(spell_id):
            return False, f"{sp['name']} is already active"
        if self.cooldowns.get(spell_id, 0.0) > self.clock:
            return False, f"{sp['name']} is on cooldown ({self.cooldowns[spell_id] - self.clock:.1f}s)"
        if self.mana < sp["cost"]:
            return False, f"not enough mana for {sp['name']}"
        slots = sp["slots"]
        if slots and self.slots_used() + slots > self.stat("attunement_slots").value:
            return False, f"no free attunement slot for {sp['name']}"
        cdr = self.stat("cdr").value
        self.mana -= sp["cost"]
        self.cooldowns[spell_id] = self.clock + sp["cooldown"] * (1.0 - cdr)
        expires = self.clock + sp["duration"] if sp["mode"] == "timed" else None
        mods = [Modifier(sp["name"], m["stat"], m["op"], m["value"], m.get("group"), expires) for m in sp["mods"]]
        self.effects.append(ActiveEffect(spell_id, sp["name"], "spell", self.clock, expires, slots,
                                         sp.get("upkeep", 0.0), sp.get("hp_drain", 0.0), mods,
                                         sp.get("backlash"), sp["family"], sp["aspect"]))
        self.corruption = min(100.0, self.corruption + sp.get("corruption", 0.0))
        return True, f"cast {sp['name']}"

    def dismiss(self, spell_id: str) -> None:
        e = self._find(spell_id)
        if e:
            self._end(e, "dismissed")

    def consume(self, item_id: str) -> Tuple[bool, str]:
        c = data.consumables()[item_id]
        if sum(1 for e in self.effects if e.kind == "consumable") >= STOMACH_LIMIT:
            return False, "stomach is full (3 active consumables)"
        if self._find(item_id):
            return False, f"{c['name']} is already active"
        expires = self.clock + c["duration"]
        mods = [Modifier(c["name"], m["stat"], m["op"], m["value"], m.get("group"), expires) for m in c["mods"]]
        self.effects.append(ActiveEffect(item_id, c["name"], "consumable", self.clock, expires, mods=mods))
        return True, f"consumed {c['name']}"

    def _end(self, e: ActiveEffect, reason: str) -> None:
        if e in self.effects:
            self.effects.remove(e)
        if e.backlash:
            b = e.backlash
            expires = self.clock + b["duration"]
            mods = [Modifier(f"Backlash: {b['name']} (from {e.name})", m["stat"], m["op"], m["value"], m.get("group"),
                             expires) for m in b["mods"]]
            self.effects.append(ActiveEffect(f"backlash:{e.key}", f"{b['name']} (from {e.name})", "backlash",
                                             self.clock, expires, mods=mods))
        self._clamp()

    def tick(self, dt: float = 1.0) -> List[str]:
        """Advance the simulation. Returns event strings."""
        events: List[str] = []
        self.clock += dt
        for e in list(self.effects):
            if e.expires_at is not None:
                if self.clock >= e.expires_at:
                    self._end(e, "expired")
                    events.append(f"{e.name} expired")
                continue
            cost = e.upkeep * dt
            if self.mana < cost:
                self._end(e, "no mana")
                events.append(f"{e.name} collapsed: out of mana")
                continue
            self.mana -= cost
            if e.hp_drain:
                max_hp = self.stat("max_hp").value
                if self.hp - e.hp_drain * dt < max_hp * HP_FLOOR_FRACTION:
                    self._end(e, "hp floor")
                    events.append(f"{e.name} collapsed: too weak to sustain")
                    continue
                self.hp -= e.hp_drain * dt
        self.hp = min(self.stat("max_hp").value, self.hp + self.stat("hp_regen").value * dt)
        self.mana = min(self.stat("max_mana").value, self.mana + self.stat("mana_regen").value * dt)
        return events

    # -- xp / jobs --
    def gain_xp(self, amount: float, job: Optional[str] = None) -> List[str]:
        out: List[str] = []
        race = data.races()[self.race]
        amount *= 1.0 + race.get("xp_bonus", 0.0)
        if job:
            st = self.job_standing(job)
            self.proficiency[job] = min(100.0, self.proficiency.get(job, 0.0) + amount * 0.02 * st["xp"])
            amount *= st["xp"]
        self.xp += amount
        while self.xp >= xp_to_next(self.level):
            self.xp -= xp_to_next(self.level)
            self.level += 1
            out.append(f"{self.name} reached level {self.level}")
        self._clamp()
        return out

    def proficiency_tier(self, job: str) -> str:
        p = self.proficiency.get(job, 0.0)
        name = data.jobs()["tiers"][0]["name"]
        for t in data.jobs()["tiers"]:
            if p >= t["min"]:
                name = t["name"]
        return name

    def job_standing(self, job: str) -> dict:
        rules = data.jobs()["standing_rules"]
        if job == self.calling:
            kind = "calling"
        elif job in self.sidelines:
            kind = "sideline"
        else:
            kind = "dabble"
        r = dict(rules[kind])
        if kind == "dabble":
            cut = data.races()[self.race].get("untrained_penalty_cut", 0.0)
            r["failure"] = r["failure"] * (1.0 - cut)
        prof = self.proficiency.get(job, 0.0)
        r["pay"] = r["pay"] * (0.85 + 0.30 * prof / 100.0)
        r["kind"] = kind
        r["tier"] = self.proficiency_tier(job)
        r["proficiency"] = prof
        return r

    # -- explanation --
    def explain(self, stat: str) -> str:
        r = self.stat(stat)
        head = f"{stat} = {r.value:.3g}" + (f"   [{r.note}]" if r.note else "")
        return "\n".join([head] + ["  " + ln for ln in r.lines])


def _half_up(x: float) -> int:
    return int(math.floor(x + 0.5))


def item_category(item_id: str) -> str:
    """'gear', 'consumable', 'core' or 'material' (what a merchant files it under)."""
    if item_id in data.consumables():
        return "consumable"
    if data.items().get(item_id, {}).get("slot") is not None:
        return "gear"
    return "core" if item_id.startswith("core_") else "material"


def item_price(item_id: str) -> int:
    """What an item is worth (mirrors game/economy.gd Economy.value, to the gold piece).

    Gear: the gold value of its stats + weapon damage + premium; penalties count against it, but never
    below a quarter of its bonuses (or 10). Consumables: 10 + premium + stat value x duration / 1 h.
    Materials: their premium. Rounded half up."""
    gv = data.gold_values()
    cons = data.consumables()
    if item_id in cons:
        c = cons[item_id]
        t = 10.0 + float(c.get("premium", 0))
        for m in c["mods"]:
            t += float(gv.get(m["stat"], {}).get(m["op"], 0)) * float(m["value"]) * float(c.get("duration", 0)) / 3600.0
        return _half_up(t)
    item = data.items()[item_id]
    pos = float(item.get("premium", 0))
    neg = 0.0
    for m in item["mods"]:
        if m["op"] == "more":
            continue
        v = float(gv.get(m["stat"], {}).get(m["op"], 0)) * float(m["value"])
        if v >= 0.0:
            pos += v
        else:
            neg += v
    w = item.get("weapon")
    if w:
        pos += float(w["base_damage"]) * float(gv["attack_damage"]["flat"]) + (float(w["aps"]) - 1.0) * float(gv["attack_speed"]["add_pct"])
    if item_category(item_id) == "gear":
        return _half_up(max(pos + neg, max(0.25 * pos, 10.0)))
    return _half_up(pos + neg)
