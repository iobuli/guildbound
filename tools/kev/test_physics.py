"""Physics pass: feet, balance, landings, and the hooks a combo can call.

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev
"""
import copy
import math
import unittest

import numpy as np

from physics import PhysicsPass, body, correct
from physics.metrics import measure
from physics.sample import clip_landmarks, rest_landmarks
from physics.walk import frames


def _dist(a, b):
    return math.dist(a, b)


class PhysicsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rest = rest_landmarks("ual1.glb")
        cls.walk = frames(cls.rest, seconds=2.2, hz=30)
        cls.dt = 1.0 / 30.0
        cls.out = correct(cls.walk, dt=cls.dt, rest=cls.rest)

    def test_walk_feet_stick_and_clear_the_floor(self):
        before, after = self.out["diagnostics"]["before"], self.out["diagnostics"]["after"]
        self.assertLess(after["slip_mps"], before["slip_mps"] * 0.25)
        self.assertLess(after["slip_max_mps"], before["slip_max_mps"])
        self.assertLess(after["penetration_mm"], 0.1)
        self.assertGreater(before["penetration_mm"], 5.0)
        self.assertGreater(after["pelvis_travel_m"], 0.5)
        self.assertLess(before["pelvis_travel_m"], 0.05)

    def test_walk_legs_stay_near_the_authored_length(self):
        for src, dst in zip(self.walk, self.out["frames"]):
            for s in "lr":
                ak, hk = f"ankle_{s}", f"hip_{s}"
                self.assertLess(abs(_dist(dst[ak], dst[hk]) - _dist(src[ak], src[hk])), 0.08)

    def test_heel_stays_down_when_the_toes_are_up(self):
        stand = copy.deepcopy(self.walk[0])
        stand["ball_l"] = [stand["ball_l"][0], stand["ball_l"][1] + 0.05, stand["ball_l"][2]]
        out = correct([stand, stand], dt=self.dt, rest=self.rest)
        lm = out["frames"][-1]
        floor = out["floor"]
        posed, _ = body.enrich(lm, self.rest)
        self.assertGreaterEqual(posed["heel_l"][1], floor - 1e-3)
        self.assertGreater(posed["ball_l"][1], floor + 0.01)

    def test_off_balance_pose_is_pulled_back(self):
        # Bent stance leg (a straight leg has no reach left to shift). Body leaned off that foot.
        seq = []
        for _ in range(18):
            fr = copy.deepcopy(self.walk[8])
            for k in ("pelvis", "hip_l", "hip_r", "neck", "shoulder_l", "shoulder_r", "head_joint", "crown", "nose",
                      "wrist_l", "wrist_r", "elbow_l", "elbow_r"):
                fr[k] = [fr[k][0] - 0.10, fr[k][1] - 0.04, fr[k][2]]
            seq.append(fr)
        out = correct(seq, dt=self.dt, rest=self.rest)
        self.assertGreater(out["diagnostics"]["before"]["com_off_mean_m"], 0.02)
        self.assertLess(out["diagnostics"]["after"]["com_off_mean_m"], out["diagnostics"]["before"]["com_off_mean_m"])
        self.assertGreater(out["diagnostics"]["pass"]["pelvis_shift_peak_m"], 0.0)

    def test_landing_compresses_and_releases(self):
        air = copy.deepcopy(self.walk[8])
        for k in air:
            air[k] = [air[k][0], air[k][1] + 0.25, air[k][2]]
        down = copy.deepcopy(self.walk[8])
        seq = [air] * 3 + [down] * 20
        out = correct(seq, dt=self.dt, rest=self.rest)
        peak = out["diagnostics"]["pass"]["compression_peak_m"]
        self.assertGreater(peak, 0.01)
        self.assertLess(out["diagnostics"]["frames"][-1]["compression_m"], peak * 0.5)

    def test_impulse_travels_then_friction_stops_it(self):
        stand = self.walk[8]
        sim = PhysicsPass(rest=self.rest)
        sim.step(stand, self.dt)
        sim.impulse([0.0, 0.0, 4.0])
        zs = []
        for _ in range(45):
            zs.append(sim.step(stand, self.dt)["landmarks"]["pelvis"][2])
        self.assertGreater(max(zs) - zs[0], 0.05)
        self.assertLess(abs(zs[-1] - zs[-2]), 0.004)

    def test_hit_stop_holds_the_pose(self):
        out = correct(self.walk[:8], dt=self.dt, rest=self.rest, events=[{"frame": 3, "hit_stop": 0.1}])
        held = out["frames"][3:6]
        for fr in held[1:]:
            self.assertEqual(fr["pelvis"], held[0]["pelvis"])
            self.assertTrue(out["diagnostics"]["frames"][4]["hit_stop"])

    def test_wrist_lags_then_settles_on_the_forearm(self):
        stand = copy.deepcopy(self.walk[8])
        sim = PhysicsPass(rest=self.rest, config={"contact": False, "balance": False, "momentum": False})
        sim.step(stand, self.dt)
        arm = [stand["wrist_l"][i] - stand["elbow_l"][i] for i in range(3)]
        lat = [-arm[2], 0.0, arm[0]]
        n = math.hypot(lat[0], lat[2]) or 1.0
        jumped = copy.deepcopy(stand)
        jumped["wrist_l"] = [stand["wrist_l"][0] + lat[0] / n * 0.06, stand["wrist_l"][1] + 0.02,
                             stand["wrist_l"][2] + lat[2] / n * 0.06]
        first = sim.step(jumped, self.dt)["landmarks"]
        self.assertGreater(_dist(first["wrist_l"], jumped["wrist_l"]), 0.01)
        self.assertAlmostEqual(_dist(first["wrist_l"], first["elbow_l"]), _dist(jumped["wrist_l"], jumped["elbow_l"]), delta=0.002)
        last = first
        for _ in range(16):
            last = sim.step(jumped, self.dt)["landmarks"]
        self.assertLess(_dist(last["wrist_l"], jumped["wrist_l"]), 0.005)

    def test_determinism_and_flags_and_wrapped_frames(self):
        again = correct(self.walk, dt=self.dt, rest=self.rest)
        self.assertEqual(again["frames"], self.out["frames"])
        bare = correct(self.walk[:5], dt=self.dt, rest=self.rest,
                       config={"contact": False, "balance": False, "momentum": False, "secondary": False})
        for src, dst in zip(self.walk[:5], bare["frames"]):
            for k, v in src.items():
                self.assertTrue(np.allclose(dst[k], v))
        wrapped = [{"t": i * self.dt, "landmarks": fr} for i, fr in enumerate(self.walk[:4])]
        packed = correct(wrapped, dt=self.dt, rest=self.rest)
        self.assertEqual(packed["frames"][2]["t"], wrapped[2]["t"])
        self.assertIn("pelvis", packed["frames"][2]["landmarks"])

    def test_catalogue_attacks_and_landings(self):
        rest = self.rest
        for clip, pen_before in (("Punch_Cross", 10.0), ("Jump_Start", 50.0), ("Jump_Land", 10.0)):
            seq, dt, _ = clip_landmarks("ual1.glb", clip, fps=30)
            out = correct(seq, dt=dt, rest=rest)
            self.assertGreater(out["diagnostics"]["before"]["penetration_mm"], pen_before)
            self.assertLess(out["diagnostics"]["after"]["penetration_mm"], 1.0)
        jab, dt, _ = clip_landmarks("ual1.glb", "Punch_Jab", fps=30)
        jab_out = correct(jab, dt=dt, rest=rest)
        self.assertLess(jab_out["diagnostics"]["after"]["slip_mps"], 0.01)
        self.assertLessEqual(jab_out["diagnostics"]["after"]["jerk"], jab_out["diagnostics"]["before"]["jerk"])
        loop, dt, _ = clip_landmarks("ual1.glb", "Walk_Loop", fps=30)
        loop_out = correct(loop, dt=dt, rest=rest)
        self.assertLess(loop_out["diagnostics"]["after"]["slip_mps"], loop_out["diagnostics"]["before"]["slip_mps"] * 0.5)
        self.assertLess(loop_out["diagnostics"]["after"]["balance_violations"], loop_out["diagnostics"]["before"]["balance_violations"])
        self.assertEqual(measure(self.out["frames"], self.dt, self.out["floor"], rest)["penetration_mm"],
                         self.out["diagnostics"]["after"]["penetration_mm"])


if __name__ == "__main__":
    unittest.main()
