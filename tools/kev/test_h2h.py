"""Tests for chaining rules (combos.py) and the hand-to-hand planner (h2h.py), on synthetic data.

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev
"""
import math
import unittest

import numpy as np

import combos
import h2h

IDENT = np.array([0.0, 0.0, 0.0, 1.0])
STILL = np.zeros(3)


def pose(arm_motion_deg=0.0):
    """A (first, last)-style joint dict: arm joints moving at `arm_motion_deg` per key."""
    v = np.array([math.radians(arm_motion_deg), 0.0, 0.0])
    return {"upperarm_r": (IDENT, v), "lowerarm_r": (IDENT, v), "hand_r": (IDENT, v), "spine_01": (IDENT, STILL)}


class ChainRuleTests(unittest.TestCase):
    def test_mid_swing_is_arm_motion_at_the_end(self):
        self.assertFalse(combos.mid_swing(pose(0.2)))
        self.assertTrue(combos.mid_swing(pose(4.0)))

    def test_a_chain_stops_on_a_mid_swing_clip(self):
        names = ["a", "b", "c"]
        score = {(x, y): 1.0 for x in names for y in names if x != y}
        self.assertEqual(combos.chain("a", names, score, 3, stops=frozenset({"a"}))["clips"], ["a"])
        self.assertEqual(len(combos.chain("a", names, score, 3)["clips"]), 3)


class PlannerTests(unittest.TestCase):
    RULES = {
        "same_hand_penalty": 20.0, "crossfade_hides_deg": 15.0, "facing_weight": 0.5,
        "chain_length_by_windup": [[0.30, 2], [0.42, 3], [9.0, 4]],
        "windup_weight_fast_enemy": 60.0, "windup_weight_slow_enemy": 60.0, "fast_enemy_windup": 0.35,
        "impact_weight_fast": 0.2, "impact_weight_slow": 1.5,
    }

    def timing(self, hand, windup_s, mid=False, yaw=0.0):
        return {"hand": hand, "windup_s": windup_s, "mid_swing": mid, "yaw_start": yaw, "yaw_end": yaw}

    def setUp(self):
        self.ends = {t: (pose(), pose()) for t in ("jab", "cross", "hook")}
        self.by_track = {"jab": {"poise": 8}, "cross": {"poise": 12}, "hook": {"poise": 16}}

    def test_punch_rule_refuses_hand_strikes_from_other_libraries(self):
        with self.assertRaises(SystemExit):
            h2h.check_punch_rule([{"track": "sekiro/Zombie Punch", "kind": "light"}], ["ual1", "ual2", "kev"])
        h2h.check_punch_rule([{"track": "ual1/Punch_Jab", "kind": "light"}], ["ual1", "ual2", "kev"])
        # A kick on a foot bone from another library is not a hand strike.
        h2h.check_punch_rule([{"track": "sekiro/Zombie Kick", "kind": "heavy", "bone": "foot_r"}], ["ual1", "ual2", "kev"])

    def test_chain_length_follows_enemy_tempo(self):
        timing = {"jab": self.timing("l", 0.1), "cross": self.timing("r", 0.1), "hook": self.timing("r", 0.1)}
        light = ["jab", "cross", "hook"]
        _, fast = h2h.best_chain(light, self.ends, timing, self.by_track, self.RULES, {"windup_s": 0.2})
        _, slow = h2h.best_chain(light, self.ends, timing, self.by_track, self.RULES, {"windup_s": 0.6})
        self.assertEqual((len(fast), len(slow)), (2, 4))

    def test_a_mid_swing_strike_only_closes_a_chain(self):
        timing = {"jab": self.timing("l", 0.1), "cross": self.timing("r", 0.1), "hook": self.timing("r", 0.1, mid=True)}
        _, seq = h2h.best_chain(["jab", "cross", "hook"], self.ends, timing, self.by_track, self.RULES, {"windup_s": 0.6})
        self.assertNotIn("hook", seq[:-1])

    def test_a_turn_across_the_cut_costs(self):
        timing = {"jab": self.timing("l", 0.1, yaw=0.0), "cross": self.timing("r", 0.1, yaw=90.0)}
        straight = {"jab": self.timing("l", 0.1, yaw=0.0), "cross": self.timing("r", 0.1, yaw=0.0)}
        turned = h2h.chain_cost(["jab", "cross"], self.ends, timing, self.by_track, self.RULES, 0.0, 0.0, 0.5)
        flat = h2h.chain_cost(["jab", "cross"], self.ends, straight, self.by_track, self.RULES, 0.0, 0.0, 0.5)
        self.assertAlmostEqual(turned - flat, 45.0)

    def test_slow_strikes_cost_only_past_the_enemy_windup(self):
        timing = {"jab": self.timing("l", 0.5), "cross": self.timing("r", 0.5)}
        vs_slow = h2h.chain_cost(["jab", "cross"], self.ends, timing, self.by_track, self.RULES, 60.0, 0.0, 0.6)
        vs_fast = h2h.chain_cost(["jab", "cross"], self.ends, timing, self.by_track, self.RULES, 60.0, 0.0, 0.2)
        self.assertAlmostEqual(vs_slow, 0.0)
        self.assertAlmostEqual(vs_fast, 60.0 * 0.3 * 2)


if __name__ == "__main__":
    unittest.main()
