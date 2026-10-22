"""Spinning back kick yaw reaches the solver as a scalar and turns the pelvis.

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev -p 'test_spin_yaw.py'
"""
import math
import os
import sys
import unittest

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from combo.bake import solve, studio
from combo.compile import compile_combo
from combo.metrics import measure
from combo.rig import Rig


def _combo(name, plant):
    return {
        "name": name, "model": "ual1.glb", "fps": 60,
        "moves": [{"id": name, "do": "sheet", "sheet": name, "plant": plant,
                   "hands": {"l": "guard", "r": "guard"},
                   "dur": {"antic": 0.14, "strike": 0.16, "hold": 0.05, "overshoot": 0.04, "follow": 0.16}}],
    }


def _flat(up, a, b):
    def flat(v):
        v = np.asarray(v, float)
        v = v - up * float(np.dot(v, up))
        n = float(np.linalg.norm(v))
        return v / max(n, 1e-9)

    fa, fb = flat(a), flat(b)
    return math.degrees(math.atan2(float(np.dot(up, np.cross(fa, fb))), float(np.dot(fa, fb))))


class SpinYawTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.srv = studio()
        cls.rig = Rig(cls.srv, "ual1.glb")
        cls.compiled = compile_combo(cls.rig, _combo("spinning_back_kick", "l"))
        cls.solved, _ms = solve(cls.srv, cls.rig.info, cls.compiled["samples"])
        cls.report = measure(cls.rig.info, cls.compiled["samples"], cls.solved, cls.compiled["fps"])

    def _at(self, tag_suffix):
        keys = [k for k in self.compiled["keys"] if k["tag"].endswith(tag_suffix)]
        self.assertTrue(keys, tag_suffix)
        key = keys[0]
        samples = self.compiled["samples"]
        i = min(range(len(samples)), key=lambda n: abs(samples[n]["t"] - key["t"]))
        return i, samples[i]

    def _facing(self, i):
        body = self.rig.info["bm"]["body"]
        fr = self.solved[i]
        fwd = self.srv.pelvis_axis(self.rig.info, fr["quats"], {"pelvis": fr["pelvis"]}, body["forward"])
        return _flat(np.asarray(body["up"], float), body["forward"], fwd)

    def test_yaw_is_a_scalar_and_the_pose_stays_in_the_side_plane(self):
        i1, f1 = self._at(":f1")
        i4, f4 = self._at(":f4")
        self.assertAlmostEqual(f1["landmarks"]["yaw"], 0.0, delta=1.0)
        self.assertAlmostEqual(f4["landmarks"]["yaw"], 180.0, delta=5.0)
        up = np.asarray(self.rig.info["bm"]["body"]["up"], float)
        hip1 = np.asarray(f1["landmarks"]["hip_l"]) - np.asarray(f1["landmarks"]["hip_r"])
        hip4 = np.asarray(f4["landmarks"]["hip_l"]) - np.asarray(f4["landmarks"]["hip_r"])
        self.assertLess(abs(_flat(up, hip1, hip4)), 15.0)
        roundhouse = compile_combo(self.rig, _combo("roundhouse_kick", "l"))
        for s in roundhouse["samples"]:
            self.assertNotIn("yaw", s["landmarks"])

    def test_solved_turn_and_gates(self):
        face = {tag: self._facing(self._at(tag)[0]) for tag in (":f1", ":f3", ":f4", ":f6", ":f8")}
        self.assertAlmostEqual(face[":f1"], 0.0, delta=20.0)
        self.assertAlmostEqual(face[":f3"], 160.0, delta=20.0)
        self.assertAlmostEqual(face[":f4"], 180.0, delta=20.0)
        self.assertAlmostEqual(face[":f6"], 180.0, delta=20.0)
        self.assertAlmostEqual(face[":f8"], 0.0, delta=20.0)
        self.assertTrue(self.report["ok"], self.report.get("fails"))
        self.assertLessEqual(self.report["jerk"], 1700.0)
        self.assertLessEqual(self.report["max_joint_step_deg"], 52.0)
        self.assertLessEqual(self.report["plant_slip_cm"], 1.5)

    def test_pivot_foot_turns_instead_of_skidding(self):
        i1, _f1 = self._at(":f1")
        i6, _f6 = self._at(":f6")
        up = np.asarray(self.rig.info["bm"]["body"]["up"], float)
        a = self.solved[i1]["landmarks"]
        b = self.solved[i6]["landmarks"]

        def horiz(p):
            p = np.asarray(p, float)
            return p - up * float(np.dot(p, up))

        side = "l"
        sample = self.compiled["samples"]
        target = float(np.linalg.norm(horiz(sample[i6]["landmarks"][f"ball_{side}"]) - horiz(b[f"ball_{side}"])))
        self.assertLess(target, 0.04, side)
        foot1 = np.asarray(a[f"ball_{side}"]) - np.asarray(a[f"ankle_{side}"])
        foot6 = np.asarray(b[f"ball_{side}"]) - np.asarray(b[f"ankle_{side}"])
        turned = _flat(up, foot1, foot6)
        facing = self._facing(i6) - self._facing(i1)
        self.assertAlmostEqual(turned, facing, delta=25.0)


if __name__ == "__main__":
    unittest.main()
