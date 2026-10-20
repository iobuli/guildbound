"""Kev nodes: the same checks run on every rig in the repo, plus naming rules per rig family.

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev
"""
import os
import time
import unittest

import nodes

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
RIGS = [
    "assets/anim/ual2.glb",
    "assets/anim/ual1.glb",
    "assets/anim/sekiro.glb",
    "assets/anim/rokoko.glb",
    "assets/anim/kev_melee.glb",
    "assets/characters/hero/male/body.glb",
    "assets/characters/hero/female/body.glb",
    "assets/packs/actionforge/clips/idle_loop.glb",
    # Rigify characters from the uploaded company pack (local only, gitignored).
    "assets/packs/company_characters/glb/elf.glb",
    "assets/packs/company_characters/glb/knight.glb",
    "assets/packs/company_characters/glb/mage.glb",
]
# Not human-proportioned: checked for limbs and symmetry, not human ranges.
CREATURES = ["assets/packs/company_characters/glb/dragon.glb"]
REQUIRED = {"head", "torso", "arm_l", "arm_r", "hand_l", "hand_r", "leg_l", "leg_r", "foot_l", "foot_r"}


class NamingTests(unittest.TestCase):
    def setUp(self):
        self.rules = nodes.load_rules()

    def check(self, name, role, side):
        self.assertEqual(nodes.classify(name, self.rules), (role, side), name)

    def test_unreal_names(self):
        self.check("upperarm_l", "upperarm", "l")
        self.check("lowerarm_r", "forearm", "r")
        self.check("calf_twist_01_l", "twist", "l")
        self.check("ik_hand_gun", "helper", "")

    def test_mixamo_names(self):
        self.check("mixamorig:LeftArm", "upperarm", "l")
        self.check("mixamorig:RightForeArm", "forearm", "r")
        self.check("mixamorig:LeftUpLeg", "thigh", "l")
        self.check("mixamorig:RightLeg", "shin", "r")
        self.check("mixamorig:HeadTop_End", "helper", "")

    def test_kevin_names(self):
        self.check("B-upperArm.L", "upperarm", "l")
        self.check("B-forearm.R", "forearm", "r")
        self.check("B-shoulder.L", "clavicle", "l")
        self.check("B-toe.R", "toe", "r")

    def test_rigify_names(self):
        self.check("DEF-upper_arm.L.001", "upperarm", "l")
        self.check("DEF-forearm.R.001", "forearm", "r")
        self.check("DEF-thigh.L", "thigh", "l")
        self.check("DEF-palm.03.L", "finger", "l")
        # Unsided segment numbers stay: spine.004-.006 are the neck and head.
        self.check("DEF-spine.005", "neck", "")
        self.check("DEF-spine.006", "head", "")
        self.check("DEF-spine.002", "spine", "")


class EveryRigTests(unittest.TestCase):
    """One body checklist, run on each rig that is present."""

    def test_every_rig_parses_into_a_human_body(self):
        rules = nodes.load_rules()
        checked = 0
        for rel in RIGS:
            path = os.path.join(REPO, rel)
            if not os.path.exists(path):
                continue
            with self.subTest(rig=rel):
                rep = nodes.parse(path, rules)
                self.assertNotIn("error", rep)
                self.assertTrue(REQUIRED <= set(rep["regions"]), set(REQUIRED) - set(rep["regions"]))
                for side in ("l", "r"):
                    self.assertTrue(0.25 <= rep["limbs"][side]["arm"] <= 0.42, rep["limbs"][side])
                    self.assertTrue(0.45 <= rep["limbs"][side]["leg"] <= 0.60, rep["limbs"][side])
                self.assertLess(rep["asymmetry"]["arm"], 0.02)
                self.assertLess(rep["asymmetry"]["leg"], 0.02)
                # Every node's parent comes before it can be resolved, and no node is its own parent.
                for k, _, parent, *_ in rep["nodes"]:
                    self.assertNotEqual(k, parent)
                checked += 1
        self.assertGreaterEqual(checked, 3, "fewer than three rigs found to check")

    def test_creature_rigs_have_symmetric_limbs(self):
        rules = nodes.load_rules()
        found = [os.path.join(REPO, r) for r in CREATURES if os.path.exists(os.path.join(REPO, r))]
        if not found:
            self.skipTest("no creature rigs present")
        for path in found:
            with self.subTest(rig=path):
                rep = nodes.parse(path, rules)
                self.assertNotIn("error", rep)
                for side in ("l", "r"):
                    self.assertIsNotNone(rep["limbs"][side]["arm"])
                    self.assertIsNotNone(rep["limbs"][side]["leg"])
                self.assertLess(rep["asymmetry"]["arm"], 0.02)
                self.assertLess(rep["asymmetry"]["leg"], 0.02)

    def test_parse_is_fast(self):
        path = os.path.join(REPO, "assets/characters/hero/male/body.glb")
        if not os.path.exists(path):
            self.skipTest("hero body not present")
        rules = nodes.load_rules()
        nodes.parse(path, rules)  # warm the file cache
        t0 = time.perf_counter()
        for _ in range(10):
            nodes.parse(path, rules)
        self.assertLess((time.perf_counter() - t0) / 10, 0.05, "parse slower than 50 ms")


if __name__ == "__main__":
    unittest.main()
