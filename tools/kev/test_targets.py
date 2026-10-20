"""Kev targets: where a fist lands on a body, by the body's region spans (synthetic rigs).

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev
"""
import unittest

import targets


def rig(height_m, head=(0.86, 1.0), torso=(0.52, 0.80), legs=(0.05, 0.52)):
    regions = {"head": {"span_h": list(head)}, "torso": {"span_h": list(torso)},
               "leg_l": {"span_h": list(legs)}, "leg_r": {"span_h": list(legs)}}
    return {"height": height_m, "units_guess": "m", "regions": regions}


class LandTests(unittest.TestCase):
    def test_the_same_fist_lands_lower_on_a_bigger_body(self):
        fist = 1.40
        self.assertEqual(targets.land(fist, rig(1.55)), "head")
        self.assertEqual(targets.land(fist, rig(1.80)), "torso")
        self.assertEqual(targets.land(fist, rig(3.70)), "leg")

    def test_a_fist_between_regions_takes_the_nearest(self):
        # 0.82 of height: above the torso's top joint (0.80), below the neck (0.86).
        self.assertEqual(targets.land(0.82 * 1.6, rig(1.6)), "torso")

    def test_over_the_skull_misses(self):
        self.assertEqual(targets.land(1.05 * 1.6, rig(1.6)), "head")   # inside the skull above the head joint
        self.assertEqual(targets.land(1.20 * 1.6, rig(1.6)), "over")

    def test_centimetre_rigs_are_scaled(self):
        r = rig(180.0)
        r["units_guess"] = "cm"
        self.assertEqual(targets.land(1.40, r), "torso")


if __name__ == "__main__":
    unittest.main()
