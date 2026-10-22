"""Sheet yaw format. The solver does not read these yet.

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev -p 'test_sheet_yaw.py'
"""
import importlib.util
import json
import os
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))


def _sheets():
    path = os.path.join(HERE, "grid", "build_sheets.py")
    spec = importlib.util.spec_from_file_location("build_sheets", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class SheetYawTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.mod = _sheets()

    def test_pair_round_trips(self):
        pose = {"hip": [1.0, 2.0]}
        self.mod.with_yaw(pose, (90, 20))
        self.assertEqual(self.mod.pose_yaw(pose), (90.0, 20.0))
        self.assertEqual(pose["hip"], [1.0, 2.0])

    def test_zero_pair_adds_no_keys(self):
        pose = {"hip": [1.0, 2.0]}
        self.mod.with_yaw(pose, (0, 0))
        self.assertNotIn("yaw", pose)
        self.assertNotIn("twist", pose)
        self.assertEqual(self.mod.pose_yaw(pose), (0.0, 0.0))

    def test_shipped_spinning_back_kick_has_no_yaw_yet(self):
        path = os.path.join(HERE, "grid", "sheets", "spinning_back_kick.json")
        with open(path) as f:
            sheet = json.load(f)
        yaw, twist = self.mod.pose_yaw(sheet["poses"]["f1"])
        self.assertEqual((yaw, twist), (0.0, 0.0))
        for pose in sheet["poses"].values():
            self.assertNotIn("yaw", pose)
            self.assertNotIn("twist", pose)


if __name__ == "__main__":
    unittest.main()
