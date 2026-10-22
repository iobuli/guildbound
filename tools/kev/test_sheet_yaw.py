"""Sheet yaw format. Spinning back kick carries a turn. The other sheets do not.

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

    def test_spinning_back_kick_yaw_and_the_other_sheets_stay_clear(self):
        sheets = os.path.join(HERE, "grid", "sheets")
        with open(os.path.join(sheets, "spinning_back_kick.json")) as f:
            spin = json.load(f)
        self.assertEqual(self.mod.pose_yaw(spin["poses"]["f1"]), (0.0, 0.0))
        self.assertEqual(self.mod.pose_yaw(spin["poses"]["f2"]), (80.0, 20.0))
        self.assertEqual(self.mod.pose_yaw(spin["poses"]["f3"]), (160.0, 10.0))
        self.assertEqual(self.mod.pose_yaw(spin["poses"]["f4"]), (180.0, 0.0))
        self.assertEqual(self.mod.pose_yaw(spin["poses"]["f6"]), (180.0, 0.0))
        self.assertEqual(self.mod.pose_yaw(spin["poses"]["f7"]), (90.0, 0.0))
        self.assertEqual(self.mod.pose_yaw(spin["poses"]["f8"]), (0.0, 0.0))
        for name in os.listdir(sheets):
            if not name.endswith(".json") or name == "spinning_back_kick.json":
                continue
            with open(os.path.join(sheets, name)) as f:
                sheet = json.load(f)
            for pose in sheet["poses"].values():
                self.assertNotIn("yaw", pose, name)
                self.assertNotIn("twist", pose, name)


if __name__ == "__main__":
    unittest.main()
