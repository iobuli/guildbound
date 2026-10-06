"""Modifier stack: the one place that turns base values + modifiers into a final stat.

Pure logic, no engine types. Every effect (item, buff, race, food, debuff) is a Modifier.

Order, always the same:
    1. base
    2. + flat
    3. x (1 + sum of add_pct)
    4. x product of `more`
    5. soft cap -> hard cap
    6. slows (strongest only), applied last
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional

OPS = ("flat", "add_pct", "more", "slow")


@dataclass(frozen=True)
class Modifier:
    source: str
    stat: str
    op: str  # flat | add_pct | more | slow
    value: float
    group: Optional[str] = None  # same (group, op): only the strongest counts
    expires_at: Optional[float] = None

    def __post_init__(self):
        if self.op not in OPS:
            raise ValueError(f"unknown op {self.op!r}")


# mode "absolute": soft/hard apply to the final value.
# mode "bonus":    soft/hard apply to the bonus over base (0.6 = +60%).
# Above a soft cap the excess counts half.
CAPS = {
    "attack_speed": {"mode": "absolute", "soft": 2.5, "hard": 3.5},
    "move_speed": {"mode": "bonus", "soft": 0.6, "hard": 1.0},
    "cdr": {"hard": 0.40},
    "crit_chance": {"hard": 1.0},
    "tenacity": {"hard": 0.75},
    "lifesteal": {"hard": 0.50},
    "attunement_slots": {"hard": 8},
}


@dataclass
class StatResult:
    stat: str
    value: float
    base: float
    lines: List[str] = field(default_factory=list)  # human-readable breakdown
    note: str = ""


def _fmt(op: str, v: float) -> str:
    if op == "flat":
        return f"{v:+g}"
    if op == "add_pct":
        return f"{v * 100:+g}%"
    if op == "more":
        return f"x{v:g}"
    return f"-{v * 100:g}% slow"


def _strength(m: Modifier) -> float:
    return abs(m.value - 1.0) if m.op == "more" else abs(m.value)


def resolve(stat: str, base: float, base_label: str, mods, now: float = 0.0) -> StatResult:
    live = [m for m in mods if m.stat == stat and (m.expires_at is None or m.expires_at > now)]
    lines = [f"base {base:g}  ({base_label})"]

    # Stacking groups: strongest wins, losers are listed so the player can see why.
    best = {}
    kept: List[Modifier] = []
    for m in live:
        if m.group is None or m.op == "slow":
            kept.append(m)
            continue
        key = (m.group, m.op)
        cur = best.get(key)
        if cur is None or _strength(m) > _strength(cur):
            best[key] = m
    winners = set(best.values())
    kept += list(winners)
    for m in live:
        if m.group is not None and m.op != "slow" and m not in winners:
            win = best[(m.group, m.op)]
            lines.append(f"  (suppressed) {_fmt(m.op, m.value)} {m.source}  -- same group as {win.source}")

    flat = sum(m.value for m in kept if m.op == "flat")
    pct = sum(m.value for m in kept if m.op == "add_pct")
    mores = [m for m in kept if m.op == "more"]
    slows = [m for m in kept if m.op == "slow"]

    for m in sorted(kept, key=lambda m: OPS.index(m.op)):
        lines.append(f"  {_fmt(m.op, m.value):>10}  {m.source}")

    value = (base + flat) * (1.0 + pct)
    for m in mores:
        value *= m.value

    note = ""
    cap = CAPS.get(stat)
    if cap:
        mode = cap.get("mode")
        if mode == "absolute":
            if "soft" in cap and value > cap["soft"]:
                value = cap["soft"] + (value - cap["soft"]) * 0.5
                note = f"soft cap {cap['soft']:g} (excess halved)"
            if value > cap["hard"]:
                value = cap["hard"]
                note = f"hard cap {cap['hard']:g}"
        elif mode == "bonus" and base > 0:
            bonus = value / base - 1.0
            if bonus > cap["soft"]:
                bonus = cap["soft"] + (bonus - cap["soft"]) * 0.5
                note = f"soft cap +{cap['soft'] * 100:g}% (excess halved)"
            if bonus > cap["hard"]:
                bonus = cap["hard"]
                note = f"hard cap +{cap['hard'] * 100:g}%"
            value = base * (1.0 + bonus)
        elif "hard" in cap and value > cap["hard"]:
            value = cap["hard"]
            note = f"hard cap {cap['hard']:g}"

    if slows:
        s = max(m.value for m in slows)
        value *= 1.0 - s
        note = (note + "; " if note else "") + f"strongest slow -{s * 100:g}%"

    value = max(0.0, value)
    return StatResult(stat, value, base, lines, note)


def damage_taken_fraction(resist: float) -> float:
    """MOBA curve: 100 / (100 + resist). Never reaches immunity."""
    return 100.0 / (100.0 + max(0.0, resist))
