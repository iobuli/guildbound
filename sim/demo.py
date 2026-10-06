"""Run:  python -m sim.demo   (from the guildbound folder)"""
from __future__ import annotations

import sys

from . import data
from .character import Character
from .render import render_sheet


def build_hunter() -> Character:
    c = Character("Kaelen", "beastkin", level=10, calling="hunter", sidelines=["merchant", "smith"], gold=1250)
    for item in ("iron_sword", "oak_buckler", "leather_cap", "hunter_jerkin", "wolfhide_gloves", "swift_boots",
                 "ring_tempest", "amulet_vigil"):
        c.equip(item)
    c.add_item("hide_bundle")
    c.proficiency.update({"hunter": 52, "merchant": 18, "smith": 8})
    c.restore()
    return c


def section(title: str) -> None:
    print("\n" + "#" * 62 + f"\n# {title}\n" + "#" * 62)


def main() -> None:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    c = build_hunter()
    section("1. THE CHARACTER MODEL  (Beastkin Hunter, level 10)")
    print(render_sheet(c))

    section("2. WHERE EACH NUMBER COMES FROM")
    print(c.explain("attack_speed"))
    print()
    print(c.explain("move_speed"))
    print()
    print(c.explain("max_hp"))

    section("3. BUFFS: cast Gale Step (Light), then Quick Step on top")
    for sp in ("gale_step", "quick_step"):
        ok, msg = c.cast(sp)
        print(f"  {msg}")
    print()
    print(c.explain("move_speed"))
    print(f"\n  Attack speed now {c.stat('attack_speed').value:.2f}/s, DPS {c.dps():.1f}")

    section("4. DARK vs LIGHT: swap to Shadowrush, run 8 seconds")
    c.dismiss("gale_step"); c.dismiss("quick_step")
    c.cooldowns.clear()
    print(f"  before: HP {c.hp:.0f}, Mana {c.mana:.0f}, MS {c.stat('move_speed').value:.2f}, "
          f"AS {c.stat('attack_speed').value:.2f}")
    print(" ", c.cast("shadowrush")[1])
    for _ in range(8):
        c.tick(1.0)
    print(f"  8s in:  HP {c.hp:.0f}, Mana {c.mana:.0f}, MS {c.stat('move_speed').value:.2f}, "
          f"AS {c.stat('attack_speed').value:.2f}  (soft cap kicks in)")
    c.dismiss("shadowrush")
    print(f"  dismissed -> Backlash: MS {c.stat('move_speed').value:.2f}")
    print(c.explain("move_speed"))

    section("5. ARMOR CURVE  (damage taken = 100 / (100 + armor))")
    d = build_hunter()
    arm = d.stat("armor").value
    print(f"  armor {arm:.1f}: a 100-damage hit deals {d.take_physical(100):.1f}")
    d.restore()
    d.cast("stone_skin")
    arm = d.stat("armor").value
    print(f"  + Stone Skin, armor {arm:.1f}: a 100-damage hit deals {d.take_physical(100):.1f}")
    d.restore()
    d.dismiss("stone_skin"); d.cast("aegis")
    arm = d.stat("armor").value
    print(f"  + Aegis (Light), armor {arm:.1f}: a 100-damage hit deals {d.take_physical(100):.1f}")
    d.restore()
    d.dismiss("aegis"); d.cooldowns.clear(); d.cast("bone_carapace")
    arm = d.stat("armor").value
    print(f"  + Bone Carapace (Dark), armor {arm:.1f}: a 100-damage hit deals {d.take_physical(100):.1f}, "
          f"but MS {d.stat('move_speed').value:.2f}")

    section("6. RACE TILT AT LEVEL 10 (same naked build)")
    print(f"  {'Race':<10}{'HP':>6}{'Mana':>6}{'AD':>6}{'AS':>6}{'Armor':>7}{'MR':>5}{'MS':>6}{'Slots':>6}")
    for rid, race in data.races().items():
        r = Character(race["name"], rid, level=10)
        print(f"  {race['name']:<10}{r.stat('max_hp').value:>6.0f}{r.stat('max_mana').value:>6.0f}"
              f"{r.stat('attack_damage').value:>6.0f}{r.stat('attack_speed').value:>6.2f}"
              f"{r.stat('armor').value:>7.1f}{r.stat('magic_resist').value:>5.0f}"
              f"{r.stat('move_speed').value:>6.2f}{r.stat('attunement_slots').value:>6.0f}")

    section("7. JOBS: 'you can do everything, it is just harder'")
    for who in (build_hunter(), Character("Mira", "human", level=10, calling="merchant")):
        print(f"  {who.name} ({who.race}), calling={who.calling}")
        for job in ("hunter", "merchant", "rogue"):
            st = who.job_standing(job)
            print(f"    {job:<9}{st['kind']:<9}pay x{st['pay']:.2f}  xp x{st['xp']:.2f}  "
                  f"failure +{st['failure'] * 100:.1f}%")


if __name__ == "__main__":
    main()
