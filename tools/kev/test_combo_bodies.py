"""Acceptance per combo: every generated unarmed_vs_* chain, checked against every body it was planned on.

Reads the committed h2h report (tools/kev/results/h2h/report.json, written by `h2h.py --write`) and the movesets.
Run: python3 -I -m unittest discover -s tools/kev -t tools/kev
"""
import glob
import json
import os
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
REPORT = os.path.join(HERE, "results", "h2h", "report.json")
# Human-sized bodies: no punch goes over the head, and at least half of every chain lands on the head or torso
# (a hook to a long-legged elf's hip is a body shot, not a miss).
HUMAN = ["body", "elf", "knight", "mage"]
# The 3.7 m mech: the allowed punches (UAL, Kevin) all strike at human chest height, so they reach its legs only.
GIANT = "Mech_Char_Mesh"


class ComboOnBodiesTests(unittest.TestCase):
    def setUp(self):
        with open(REPORT) as f:
            self.plans = json.load(f)["plans"]

    def test_every_moveset_is_the_planned_chain(self):
        for p in self.plans:
            with self.subTest(enemy=p["enemy"]["name"]):
                with open(os.path.join(REPO, "assets", "anim", "movesets", f"unarmed_vs_{p['enemy']['name']}.json")) as f:
                    m = json.load(f)
                self.assertEqual([a["name"] for a in m["light"]], p["chain"])
        files = glob.glob(os.path.join(REPO, "assets", "anim", "movesets", "unarmed_vs_*.json"))
        self.assertEqual(len(files), len(self.plans))

    def test_chains_land_on_human_bodies(self):
        checked = 0
        for p in self.plans:
            for rig in HUMAN:
                if rig not in p.get("on_rigs", {}):
                    continue
                with self.subTest(enemy=p["enemy"]["name"], rig=rig):
                    lands = p["on_rigs"][rig]["lands"]
                    self.assertEqual(len(lands), len(p["on_rigs"][rig]["chain"]))
                    self.assertNotIn("over", lands)
                    self.assertGreaterEqual(sum(x in ("head", "torso") for x in lands) * 2, len(lands), lands)
                    checked += 1
        self.assertGreaterEqual(checked, len(self.plans), "no human body in the report")

    def test_on_the_planned_body_each_chain_reaches_the_head(self):
        for p in self.plans:
            with self.subTest(enemy=p["enemy"]["name"]):
                self.assertIn("head", p["lands"])

    def test_the_giant_is_out_of_punch_reach(self):
        """A known limit, recorded so it is noticed when a rising strike joins the pool."""
        for p in self.plans:
            if GIANT in p.get("on_rigs", {}):
                with self.subTest(enemy=p["enemy"]["name"]):
                    self.assertEqual(set(p["on_rigs"][GIANT]["lands"]), {"leg"})

    def test_a_jump_punch_reaches_the_giant_and_overshoots_humans(self):
        """targets.py: a punch at the top of UAL's NinjaJump_Start (both allowed clips, layered)."""
        path = os.path.join(HERE, "results", "h2h", "targets.json")
        with open(path) as f:
            jump = json.load(f).get("jump")
        if not jump:
            self.skipTest("targets.json has no jump section")
        lands = jump["lands"]
        if GIANT in lands["jab"]:
            self.assertIn(lands["jab"][GIANT], {"torso", "head"})
        for rig in HUMAN:
            if rig in lands["jab"]:
                self.assertEqual(lands["jab"][rig], "over", rig)


if __name__ == "__main__":
    unittest.main()
