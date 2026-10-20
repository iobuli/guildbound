"""Tests for the evaluation's scope: only the base bodies are evaluated, never the clothing.

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev
"""
import json
import os
import tempfile
import unittest

import evaluate
from test_kev import write_glb


class BodyScopeTests(unittest.TestCase):
    def test_plan_lists_only_base_bodies(self):
        with open(os.path.join(os.path.dirname(__file__), "plan.json")) as f:
            plan = json.load(f)
        self.assertEqual(sorted(plan["body_files"]), ["female/body.glb", "male/body.glb"])

    def test_body_files_skip_clothing_beside_the_body(self):
        with tempfile.TemporaryDirectory() as root:
            for sex in ("male", "female"):
                os.makedirs(os.path.join(root, sex))
                write_glb(os.path.join(root, sex, "body.glb"), joints=65, vertices=5000)
                write_glb(os.path.join(root, sex, "shirt.glb"), joints=65, vertices=900)
                write_glb(os.path.join(root, sex, "hair_long.glb"), joints=65, vertices=300)
            plan = {"bodies": root, "body_files": ["male/body.glb", "female/body.glb"]}
            paths = [os.path.basename(p) for p in evaluate.body_files(plan)]
            self.assertEqual(paths, ["body.glb", "body.glb"])

    def test_body_entry_scores_the_body(self):
        with tempfile.TemporaryDirectory() as root:
            p = os.path.join(root, "body.glb")
            write_glb(p, joints=65, vertices=5000)
            ref = {f"bone{i}" for i in range(65)}
            entry = evaluate.body_entry(p, ref, {"height_m": [0.5, 3.0]})
            self.assertEqual(entry["bones"], 65)


if __name__ == "__main__":
    unittest.main()
