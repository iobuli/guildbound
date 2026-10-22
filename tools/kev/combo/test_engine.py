"""Combo engine: hermite joins, planted feet, and a solved clip that clears the glitch gates.

Run with the rest of Kev: python3 -I -m unittest discover -s tools/kev/combo -t tools/kev
"""
import os
import unittest

import numpy as np

from combo.compile import compile_combo, load_combo
from combo.geom import hermite
from combo.rig import Rig


def _srv():
    from combo.bake import studio
    return studio()


class HermiteTests(unittest.TestCase):
    def test_endpoints_and_flat_hold(self):
        p = hermite([0, 0, 0], [1, 0, 0], [0, 0, 0], [0, 0, 0], 0.2, 0.0)
        self.assertAlmostEqual(float(p[0]), 0.0)
        p = hermite([0, 0, 0], [1, 0, 0], [0, 0, 0], [0, 0, 0], 0.2, 1.0)
        self.assertAlmostEqual(float(p[0]), 1.0, places=5)
        mid = hermite([0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], 0.1, 0.5)
        self.assertAlmostEqual(float(np.linalg.norm(mid)), 0.0, places=6)


class CompileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.srv = _srv()
        cls.rig = Rig(cls.srv, "ual1.glb")

    def test_four_combos_plant_and_finite(self):
        for name in ("jab_cross_hook_roundhouse", "rush_uppercut", "spin_back_jump_kick", "brawler_mage"):
            compiled = compile_combo(self.rig, load_combo(name))
            self.assertGreater(compiled["duration"], 1.0)
            for s in compiled["samples"]:
                for v in s["landmarks"].values():
                    self.assertTrue(np.isfinite(v).all())
            # a hit-stop keeps the striking pose: consecutive :hit samples match
            hits = [s for s in compiled["samples"] if s["tag"].endswith(":hit")]
            self.assertGreaterEqual(len(hits), 4)
            # planted targets do not move inside a lock
            for s in "lr":
                joint = f"ankle_{s}"
                run = []

                def close(run):
                    if len(run) < 5:
                        return
                    pts = np.array([compiled["samples"][i]["landmarks"][joint] for i in run])
                    drift = np.linalg.norm(pts - np.median(pts, axis=0), axis=1).max()
                    self.assertLess(drift, 0.004, f"{name} {joint} target moved {drift:.4f} m inside a plant")

                for i, sample in enumerate(compiled["samples"]):
                    if joint in sample["plant"]:
                        run.append(i)
                    else:
                        close(run)
                        run = []
                close(run)


class SolveTests(unittest.TestCase):
    def test_jab_cross_clears_gates(self):
        from combo.bake import bake_combo
        row = bake_combo(_srv(), "jab_cross_hook_roundhouse", render=False)
        self.assertTrue(row["report"]["ok"], row["report"]["fails"])
        self.assertTrue(os.path.isfile(row["baked"]))
