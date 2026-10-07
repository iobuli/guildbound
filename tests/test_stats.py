"""Run from the guildbound folder:  python -m unittest discover tests -v"""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from sim import data
from sim.character import Character, STOMACH_LIMIT, item_category, item_price
from sim.stats import Modifier, damage_taken_fraction, resolve


def kaelen():
    c = Character("Kaelen", "beastkin", level=10, calling="hunter")
    for i in ("iron_sword", "wolfhide_gloves", "swift_boots", "ring_tempest"):
        c.equip(i)
    c.restore()
    return c


class DesignDocExamples(unittest.TestCase):
    """The worked examples in docs/MAGIC_AND_STATS.md section 2.5 must be reproducible."""

    def test_beastkin_agi_25_at_level_10(self):
        self.assertAlmostEqual(kaelen().attribute("agi").value, 25.0)

    def test_attack_speed_pool_and_ring(self):
        c = kaelen()
        self.assertAlmostEqual(c.stat("attack_speed").value, 1.5 * 1.15, places=3)  # 1.725 before buff
        c.cast("gale_step")  # +30% -> pool +80%
        self.assertAlmostEqual(c.stat("attack_speed").value, 1.8 * 1.15, places=3)  # 2.07

    def test_move_speed_reaches_soft_cap_exactly(self):
        c = kaelen()
        c.cast("gale_step")
        self.assertAlmostEqual(c.stat("move_speed").value, 9.6, places=3)

    def test_armor_60_takes_62_5_percent(self):
        self.assertAlmostEqual(damage_taken_fraction(60), 0.625)
        self.assertAlmostEqual(damage_taken_fraction(100), 0.5)


class ModifierStack(unittest.TestCase):
    def test_order_flat_then_pct_then_more(self):
        m = [Modifier("a", "x", "flat", 10), Modifier("b", "x", "add_pct", 0.5), Modifier("c", "x", "more", 2.0)]
        self.assertAlmostEqual(resolve("x", 100, "t", m).value, (100 + 10) * 1.5 * 2.0)

    def test_pct_bonuses_add_not_multiply(self):
        m = [Modifier("a", "x", "add_pct", 0.2), Modifier("b", "x", "add_pct", 0.3)]
        self.assertAlmostEqual(resolve("x", 100, "t", m).value, 150)

    def test_same_group_strongest_wins(self):
        m = [Modifier("weak", "x", "add_pct", 0.15, "g"), Modifier("strong", "x", "add_pct", 0.30, "g")]
        r = resolve("x", 100, "t", m)
        self.assertAlmostEqual(r.value, 130)
        self.assertTrue(any("suppressed" in ln and "weak" in ln for ln in r.lines))

    def test_slow_strongest_only_and_last(self):
        m = [Modifier("s1", "x", "slow", 0.3), Modifier("s2", "x", "slow", 0.2), Modifier("b", "x", "flat", 100)]
        self.assertAlmostEqual(resolve("x", 100, "t", m).value, 200 * 0.7)

    def test_expired_modifiers_ignored(self):
        m = [Modifier("a", "x", "flat", 50, expires_at=5.0)]
        self.assertAlmostEqual(resolve("x", 10, "t", m, now=4.0).value, 60)
        self.assertAlmostEqual(resolve("x", 10, "t", m, now=5.0).value, 10)

    def test_attack_speed_soft_and_hard_cap(self):
        v = resolve("attack_speed", 1.0, "t", [Modifier("a", "attack_speed", "add_pct", 2.0)]).value
        self.assertAlmostEqual(v, 2.5 + 0.5 * 0.5)  # 3.0 -> 2.75
        v = resolve("attack_speed", 1.0, "t", [Modifier("a", "attack_speed", "add_pct", 20.0)]).value
        self.assertAlmostEqual(v, 3.5)

    def test_move_speed_caps(self):
        v = resolve("move_speed", 6.0, "t", [Modifier("a", "move_speed", "add_pct", 1.0)]).value
        self.assertAlmostEqual(v, 6.0 * (1 + 0.6 + 0.4 * 0.5))  # +100% -> +80%
        v = resolve("move_speed", 6.0, "t", [Modifier("a", "move_speed", "add_pct", 9.0)]).value
        self.assertAlmostEqual(v, 12.0)  # hard cap +100%

    def test_cdr_tenacity_hard_caps(self):
        self.assertAlmostEqual(resolve("cdr", 0, "t", [Modifier("a", "cdr", "flat", 0.9)]).value, 0.40)
        self.assertAlmostEqual(resolve("tenacity", 0, "t", [Modifier("a", "tenacity", "flat", 2)]).value, 0.75)


class CharacterModel(unittest.TestCase):
    def test_derived_stats_are_recomputed_not_stored(self):
        c = Character("t", "human", level=5)
        hp1 = c.stat("max_hp").value
        c.equip("hunter_jerkin")
        self.assertAlmostEqual(c.stat("max_hp").value, hp1 + 60)
        c.unequip("body")
        self.assertAlmostEqual(c.stat("max_hp").value, hp1)

    def test_level_up_raises_hp(self):
        c = Character("t", "ironkin", level=1)
        a = c.stat("max_hp").value
        c.level = 2
        self.assertGreater(c.stat("max_hp").value, a)

    def test_races_are_distinct(self):
        hp = {r: Character("t", r, level=10).stat("max_hp").value for r in data.races()}
        self.assertEqual(max(hp, key=hp.get), "ironkin")
        self.assertEqual(min(hp, key=hp.get), "wisp")
        mana = {r: Character("t", r, level=10).stat("max_mana").value for r in data.races()}
        self.assertEqual(max(mana, key=mana.get), "sylph")

    def test_weapon_scaling_attribute(self):
        c = Character("t", "beastkin", level=10)
        c.equip("windfang_daggers")
        ad = c.stat("attack_damage").value
        self.assertAlmostEqual(ad, 10 + 17 + c.attribute("agi").value)

    def test_encumbrance_slows(self):
        c = Character("t", "wisp", level=1)
        ms = c.stat("move_speed").value
        for _ in range(6):
            c.add_item("hide_bundle")
        self.assertLess(c.stat("move_speed").value, ms)

    def test_damage_cannot_be_negative_or_immune(self):
        c = kaelen()
        self.assertGreater(c.take_physical(100), 0)


class MagicRules(unittest.TestCase):
    def test_attunement_slots_limit(self):
        c = Character("t", "human", level=1)  # 3 slots
        for sp in ("quick_step", "strength_surge", "stone_skin"):
            self.assertTrue(c.cast(sp)[0], sp)
        ok, msg = c.cast("second_wind")
        self.assertFalse(ok)
        self.assertIn("attunement slot", msg)

    def test_sustained_buff_drains_mana_and_collapses(self):
        c = Character("t", "human", level=1)
        c.cast("gale_step")  # upkeep 2.5/s is more than a level-1 human regenerates (~1.65/s)
        c.mana = 2.6
        events = c.tick(1.0)
        self.assertEqual(events, [])
        events = c.tick(1.0)
        self.assertTrue(any("out of mana" in e for e in events))
        self.assertIsNone(c._find("gale_step"))

    def test_dark_leaves_backlash_with_traceable_cause(self):
        c = kaelen()
        c.cast("shadowrush")
        c.tick(2.0)
        c.dismiss("shadowrush")
        r = c.stat("move_speed")
        self.assertTrue(any("Backlash" in ln and "Shadowrush" in ln for ln in r.lines))
        for _ in range(21):
            c.tick(1.0)
        self.assertFalse(any(e.kind == "backlash" for e in c.effects))  # route out: it wears off

    def test_dark_drain_never_kills(self):
        c = kaelen()
        c.cast("shadowrush")
        for _ in range(200):
            c.tick(1.0)
            self.assertGreater(c.hp, 0)

    def test_light_and_dark_same_family_do_not_stack(self):
        c = kaelen()
        c.cast("gale_step")
        c.cast("shadowrush")
        ms = c.stat("move_speed")
        self.assertEqual(sum(1 for ln in ms.lines if "suppressed" in ln), 1)

    def test_stomach_limit(self):
        c = Character("t", "human", level=1)
        results = [c.consume(i)[0] for i in ("hearty_stew", "swift_tonic", "ironbark_draught")]
        self.assertEqual(results, [True, True, True])
        ok, msg = c.consume("keen_eye_tea")
        self.assertFalse(ok)
        self.assertIn("stomach", msg)
        self.assertEqual(STOMACH_LIMIT, 3)

    def test_racial_awaken_uses_more_multiplier_and_expires(self):
        c = Character("t", "drakari", level=10)
        ad = c.stat("attack_damage").value
        self.assertTrue(c.cast("awaken")[0])
        self.assertAlmostEqual(c.stat("attack_damage").value, ad * 1.25)
        for _ in range(11):
            c.tick(1.0)
        self.assertAlmostEqual(c.stat("attack_damage").value, ad)
        self.assertFalse(c.cast("awaken")[0])  # on cooldown


class DesignRules(unittest.TestCase):
    """Guardrails from the design doc, enforced against the data."""

    PRIMARY = {"swiftness": "move_speed", "might": "attack_damage", "ward": "armor"}

    def _peak(self, spell, stat):
        return next(m["value"] for m in spell["mods"] if m["stat"] == stat)

    def test_dark_peak_is_about_1_4x_light(self):
        sp = data.spells()
        for fam, stat in self.PRIMARY.items():
            light = next(s for s in sp.values() if s["family"] == fam and s["aspect"] == "light")
            dark = next(s for s in sp.values() if s["family"] == fam and s["aspect"] == "dark")
            self.assertAlmostEqual(self._peak(dark, stat) / self._peak(light, stat), 1.4, delta=0.01, msg=fam)

    def test_every_dark_spell_has_a_cost_beyond_mana(self):
        for sid, s in data.spells().items():
            if s["aspect"] != "dark":
                continue
            has_cost = (s.get("hp_drain") or s.get("backlash") or s.get("corruption")
                        or any(m["value"] < 0 for m in s["mods"]))
            self.assertTrue(has_cost, sid)

    def test_light_has_no_hidden_costs(self):
        for sid, s in data.spells().items():
            if s["aspect"] == "light":
                self.assertFalse(s.get("hp_drain") or s.get("backlash") or s.get("corruption"), sid)
                self.assertFalse(any(m["value"] < 0 for m in s["mods"]), sid)

    def test_every_family_has_all_three_aspects(self):
        fams = {}
        for s in data.spells().values():
            if s["school"] == "boon":
                fams.setdefault(s["family"], set()).add(s["aspect"])
        for fam, aspects in fams.items():
            self.assertEqual(aspects, {"neutral", "light", "dark"}, fam)


if __name__ == "__main__":
    unittest.main()


class EconomyValues(unittest.TestCase):
    """Item values. The game checks the same numbers (game/selftest.gd EXPECTED_VALUES)."""

    EXPECTED = {"ring_ogre": 495, "brawler_wraps": 580, "sprinter_greaves": 650, "hearty_stew": 67, "swift_tonic": 37, "keen_eye_tea": 18, "ironbark_draught": 28, "slime_gel": 6, "hide_bundle": 30, "core_common": 40, "leather_cap": 120, "iron_sword": 770}

    def test_values_match_the_game(self):
        for item_id, want in self.EXPECTED.items():
            self.assertEqual(item_price(item_id), want, item_id)

    def test_every_item_is_worth_something(self):
        for item_id in data.items():
            self.assertGreater(item_price(item_id), 0, item_id)

    def test_consumables_cost_at_least_ten(self):
        for item_id in data.consumables():
            self.assertGreaterEqual(item_price(item_id), 10, item_id)

    def test_categories(self):
        self.assertEqual(item_category("chain_vest"), "gear")
        self.assertEqual(item_category("hearty_stew"), "consumable")
        self.assertEqual(item_category("slime_gel"), "material")
        self.assertEqual(item_category("core_rare"), "core")
