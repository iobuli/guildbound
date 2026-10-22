"""Body yaw in the solver: pivot on the planted ball, cap the step, twist the chest.

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev -p 'test_body_yaw.py'
"""
import importlib.util
import json
import math
import os
import unittest

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))


def _studio():
    path = os.path.join(HERE, "studio", "server.py")
    spec = importlib.util.spec_from_file_location("kev_studio_server_yaw", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _flat_angle(up, a, b):
    up = np.asarray(up, float)

    def flat(v):
        v = np.asarray(v, float)
        v = v - up * float(np.dot(v, up))
        n = float(np.linalg.norm(v))
        return v / max(n, 1e-9)

    fa, fb = flat(a), flat(b)
    return math.degrees(math.atan2(float(np.dot(up, np.cross(fa, fb))), float(np.dot(fa, fb))))


class BodyYawTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.srv = _studio()
        cls.info = cls.srv.model_info("ual1.glb")
        cls.stand = cls.srv.kev_read(cls.info, {}, None)["landmarks"]

    def _copy(self):
        return {k: list(v) for k, v in self.stand.items()}

    def _clear(self):
        self.info.pop("_body_yaw", None)
        self.info.pop("_body_twist", None)

    def _solve(self, lm):
        Q, P = {}, {}
        self.srv.apply_frame_into(self.info, Q, P, lm, None)
        return Q, P

    def _forward_deg(self, Q, P):
        body = self.info["bm"]["body"]
        fwd = self.srv.pelvis_axis(self.info, Q, P, body["forward"])
        return _flat_angle(body["up"], body["forward"], fwd)

    def test_missing_yaw_matches_explicit_zero(self):
        self._clear()
        q0, _ = self._solve(self._copy())
        self._clear()
        lm = self._copy()
        lm["yaw"] = 0
        lm["twist"] = 0
        q1, _ = self._solve(lm)
        for bone, quat in q0.items():
            if bone not in q1:
                continue
            a = np.asarray(quat, float)
            b = np.asarray(q1[bone], float)
            a = a / max(float(np.linalg.norm(a)), 1e-9)
            b = b / max(float(np.linalg.norm(b)), 1e-9)
            ang = math.degrees(2 * math.acos(min(1.0, abs(float(np.dot(a, b))))))
            self.assertLess(ang, 0.5, bone)

    def test_yaw_steps_twelve_degrees_and_keeps_the_planted_ball(self):
        self._clear()
        lm = self._copy()
        up = np.array(self.info["bm"]["body"]["up"], float)
        plant = min(("ball_l", "ball_r"), key=lambda k: float(np.dot(lm[k], up)))
        held = list(lm[plant])
        lm["yaw"] = 90
        self.srv._apply_body_yaw(self.info, lm)
        self.assertLess(float(np.linalg.norm(np.array(lm[plant]) - held)), 1e-6)
        self.assertAlmostEqual(self.info["_body_yaw"], 12.0, places=3)
        self._clear()
        Q, P = {}, {}
        facing = []
        for _ in range(8):
            frame = self._copy()
            frame["yaw"] = 90
            self.srv.apply_frame_into(self.info, Q, P, frame, None)
            facing.append(self._forward_deg(Q, P))
        self.assertAlmostEqual(facing[0], 12.0, delta=3.0)
        self.assertAlmostEqual(facing[-1], 90.0, delta=6.0)
        # The target ball is the pivot. The solved ball joint sits on the foot bone, which
        # is a few centimetres off that target once the ankle has orbited (ankle reach stays under 1 mm).
        solved = self.srv.kev_read(self.info, Q, P)["landmarks"][plant]
        self.assertLess(float(np.linalg.norm(np.array(solved) - held)), 0.04)

    def test_twist_is_shared_up_the_spine(self):
        self._clear()
        lm = self._copy()
        hips = np.array(lm["hip_l"]) - np.array(lm["hip_r"])
        lm["twist"] = 30
        self.srv._apply_body_yaw(self.info, lm)
        up = self.info["bm"]["body"]["up"]
        hip = np.array(lm["hip_l"]) - np.array(lm["hip_r"])
        shoulder = np.array(lm["shoulder_l"]) - np.array(lm["shoulder_r"])
        self.assertAlmostEqual(_flat_angle(up, hips, hip), 0.0, delta=1.0)
        self.assertAlmostEqual(_flat_angle(up, hip, shoulder), 30.0, delta=2.0)
        self._clear()
        frame = self._copy()
        frame["twist"] = 30
        Q, P = self._solve(frame)
        body = self.info["bm"]["body"]
        left0 = np.array(body["left"], float)

        def left_of(bone):
            rest = self.srv.rot_of(self.info["rest_world"][self.info["index"][bone]])
            world = self.srv.bone_world_rot(self.info, Q, P, bone) @ (rest.T @ left0)
            return _flat_angle(body["up"], left0, world)

        pelvis, mid, chest = left_of("pelvis"), left_of("spine_02"), left_of("spine_03")
        self.assertAlmostEqual(pelvis, 0.0, delta=4.0)
        self.assertAlmostEqual(chest, 30.0, delta=6.0)
        self.assertGreater(mid, 8.0)
        self.assertLess(mid, chest - 2.0)

    def test_sheet_without_yaw_still_faces_forward(self):
        self._clear()
        import sys
        sys.path.insert(0, os.path.join(HERE, "grid"))
        import posesheet
        path = os.path.join(HERE, "grid", "sheets", "spinning_back_kick.json")
        with open(path) as f:
            sheet = json.load(f)
        raw = posesheet.lift(sheet, sheet["poses"]["f5"], self.srv, self.info)
        Q, P = self._solve(raw)
        self.assertAlmostEqual(self._forward_deg(Q, P), 0.0, delta=20.0)


if __name__ == "__main__":
    unittest.main()
