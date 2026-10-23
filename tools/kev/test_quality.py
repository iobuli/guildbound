"""No-glitch pass, matrix-track sampling, and /api/state pelvis translation.

Run: python3 -I -m unittest discover -s tools/kev -t tools/kev
"""
import os
import sys
import unittest

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import quality.core as Q

BODY = {"up": [0, 1, 0], "forward": [0, 0, 1], "left": [1, 0, 0], "floor": 0.0, "height": 1.8}


def _studio():
    import importlib.util
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "studio", "server.py")
    spec = importlib.util.spec_from_file_location("kev_studio_server_test", path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


class MotionStateTests(unittest.TestCase):
    def test_frozen_when_the_source_moves_and_the_sample_does_not(self):
        still = np.zeros((8, 3, 3))
        self.assertEqual(Q.motion_state(still, rot_deg=40), "frozen")

    def test_static_when_neither_moves(self):
        still = np.zeros((8, 3, 3))
        self.assertEqual(Q.motion_state(still, rot_deg=0.04), "static")

    def test_ok_when_a_landmark_travels(self):
        L = np.zeros((8, 2, 3))
        L[:, 0, 0] = np.linspace(0, 0.05, 8)
        self.assertEqual(Q.motion_state(L, rot_deg=10), "ok")


class CleanupTests(unittest.TestCase):
    def test_closed_weapon_grip_is_not_a_backward_palm(self):
        hpts = {
            "hand_r": np.array([0.0, 0.0, 0.0]),
            "middle_01_r": np.array([0.0, 0.04, 0.0]),
            "middle_02_r": np.array([0.0, 0.04, -0.035]),
            "middle_03_r": np.array([0.0, 0.005, -0.035]),
        }
        self.assertTrue(Q.hand_closed("r", hpts))

    def test_knee_bent_backward_is_flagged_and_reflected(self):
        names = ["hip_l", "knee_l", "ankle_l"]
        # Knee sits behind the hip-ankle line (forward is +Z).
        frame = np.array([[0, 1.0, 0.0], [0, 0.55, -0.2], [0, 0.1, 0.1]], float)
        L = np.repeat(frame[None], 6, 0)
        lengths = {("hip_l", "knee_l"): np.linalg.norm(frame[1] - frame[0]),
                   ("knee_l", "ankle_l"): np.linalg.norm(frame[2] - frame[1])}
        issues = Q.validate(L, names, 60, 0.0, 1.8, None, BODY, lengths)
        self.assertTrue(any(i["kind"] == "knee_hyperextension" for i in issues))
        S, _, _ = Q.clean(L, names, 60, 0.0, 1.8, None, BODY, lengths, restyle=False)
        after = Q.validate(S, names, 60, 0.0, 1.8, None, BODY, lengths)
        self.assertFalse(any(i["kind"] == "knee_hyperextension" for i in after))
        self.assertGreater(S[0, 1, 2], 0.0)

    def test_stretched_segment_returns_to_rest_length(self):
        names = ["shoulder_l", "elbow_l"]
        L = np.zeros((4, 2, 3))
        L[:, 0] = [0, 1, 0]
        L[:, 1] = [0, 1, 0.5]
        lengths = {("shoulder_l", "elbow_l"): 0.28}
        before = Q.validate(L, names, 60, 0.0, 1.8, None, BODY, lengths)
        self.assertTrue(any(i["kind"] == "limb_stretch" for i in before))
        S, _, _ = Q.clean(L, names, 60, 0.0, 1.8, None, BODY, lengths, restyle=False)
        dist = np.linalg.norm(S[:, 1] - S[:, 0], axis=1)
        self.assertTrue(np.allclose(dist, 0.28, atol=0.01))

    def test_toe_through_the_floor_keeps_its_length(self):
        names = ["ankle_r", "ball_r", "toe_tip_r"]
        L = np.zeros((4, 3, 3))
        L[:, 0] = [0.0, 0.02, 0.0]
        L[:, 1] = [0.0, 0.01, 0.12]
        L[:, 2] = [0.0, -0.08, 0.20]
        lengths = {("ankle_r", "ball_r"): 0.12, ("ball_r", "toe_tip_r"): 0.06}
        S, _, _ = Q.clean(L, names, 60, 0.0, 1.8, None, BODY, lengths, restyle=False)
        toe = np.linalg.norm(S[:, 2] - S[:, 1], axis=1)
        ball = np.linalg.norm(S[:, 1] - S[:, 0], axis=1)
        self.assertTrue(np.allclose(toe, 0.06, atol=0.005))
        self.assertTrue(np.allclose(ball, 0.12, atol=0.005))
        self.assertGreaterEqual(float(S[:, :, 1].min()), -Q.FLOOR_EPS - 1e-6)

    def test_foot_below_the_floor_is_lifted(self):
        names = ["ankle_l", "ball_l"]
        L = np.zeros((4, 2, 3))
        L[:, 0] = [0.1, -0.05, 0.2]
        L[:, 1] = [0.1, -0.04, 0.35]
        S, _, _ = Q.clean(L, names, 60, 0.0, 1.8, None, BODY, {}, restyle=False)
        self.assertGreaterEqual(S[:, :, 1].min(), -Q.FLOOR_EPS - 1e-6)

    def test_final_leg_settle_keeps_ankle_inside_limit(self):
        names = ["hip_l", "knee_l", "ankle_l", "ball_l", "toe_tip_l"]
        frame = np.array([[0.0, 0.92, 0.0], [0.0, 0.50, 0.12], [0.0, 0.10, 0.0],
                          [0.0, 0.02, -0.13], [0.0, 0.01, -0.20]], float)
        L = np.repeat(frame[None], 8, 0)
        lengths = {("hip_l", "knee_l"): np.linalg.norm(frame[1] - frame[0]),
                   ("knee_l", "ankle_l"): np.linalg.norm(frame[2] - frame[1]),
                   ("ankle_l", "ball_l"): np.linalg.norm(frame[3] - frame[2]),
                   ("ball_l", "toe_tip_l"): np.linalg.norm(frame[4] - frame[3])}
        S, _, _ = Q.clean(L, names, 60, 0.0, 1.8, None, BODY, lengths, restyle=False)
        after = Q.validate(S, names, 60, 0.0, 1.8, None, BODY, lengths)
        self.assertFalse(any(i["kind"] == "ankle_limit" for i in after), after)

    def test_planted_foot_stops_sliding(self):
        names = ["ankle_l", "ball_l"]
        T = 12
        L = np.zeros((T, 2, 3))
        L[:, 0, 0] = np.linspace(0, 0.08, T)
        L[:, 0, 1] = 0.01
        L[:, 1, 0] = L[:, 0, 0]
        L[:, 1, 1] = 0.01
        L[:, 1, 2] = 0.12
        before = Q.validate(L, names, 60, 0.0, 1.8, None, BODY, {})
        self.assertTrue(any(i["kind"] == "foot_slide" for i in before))
        before_len = np.linalg.norm(L[:, 1] - L[:, 0], axis=1)
        S, _, _ = Q.clean(L, names, 60, 0.0, 1.8, None, BODY, {}, restyle=False)
        after = Q.validate(S, names, 60, 0.0, 1.8, None, BODY, {})
        self.assertFalse(any(i["kind"] == "foot_slide" for i in after))
        self.assertTrue(np.allclose(np.linalg.norm(S[:, 1] - S[:, 0], axis=1), before_len, atol=0.005))

    def test_rocking_ball_under_a_still_ankle_is_not_a_slide(self):
        names = ["ankle_l", "ball_l"]
        T = 12
        L = np.zeros((T, 2, 3))
        L[:, 0] = [0.0, 0.01, 0.0]
        L[:, 1, 0] = np.linspace(0.0, 0.06, T)
        L[:, 1, 1] = 0.01
        issues = Q.validate(L, names, 60, 0.0, 1.8, None, BODY, {})
        self.assertFalse(any(i["kind"] == "foot_slide" for i in issues))

    def test_position_pop_is_removed(self):
        names = ["wrist_r"]
        L = np.zeros((7, 1, 3))
        L[:, 0, 1] = 1.0
        L[3, 0, 1] = 1.4
        before = Q.validate(L, names, 60, 0.0, 1.8, None, BODY, {})
        self.assertTrue(any(i["kind"] == "discontinuity" for i in before))
        S, _, _ = Q.clean(L, names, 60, 0.0, 1.8, None, BODY, {}, restyle=False)
        self.assertLess(abs(S[3, 0, 1] - 1.0), 0.05)

    def test_finger_does_not_bend_away_from_the_palm(self):
        hand = np.array([0.0, 1.0, 0.0])
        middle = hand + np.array([0.0, 0.0, 0.08])
        index = hand + np.array([0.02, 0.0, 0.08])
        pinky = hand + np.array([-0.02, 0.0, 0.08])
        # index_02 curls up, opposite a downward palm.
        index2 = index + np.array([0.0, 0.04, 0.02])
        hands = [{"hand_r": hand, "middle_01_r": middle, "index_01_r": index, "pinky_01_r": pinky, "index_02_r": index2}] * 4
        names = ["wrist_r", "elbow_r", "shoulder_r"]
        L = np.zeros((4, 3, 3))
        L[:, 0] = hand
        L[:, 1] = hand + np.array([0, 0, -0.25])
        L[:, 2] = L[:, 1] + np.array([0, 0, -0.25])
        before = Q.validate(L, names, 60, 0.0, 1.8, hands, BODY, {})
        self.assertTrue(any(i["kind"] == "finger_backbend" for i in before))
        _S, hands2, _ = Q.clean(L, names, 60, 0.0, 1.8, hands, BODY, {}, restyle=False)
        after = Q.validate(L, names, 60, 0.0, 1.8, hands2, BODY, {})
        self.assertFalse(any(i["kind"] == "finger_backbend" for i in after))

    def test_quaternion_pop_is_replaced_by_the_neighbours(self):
        eye = np.array([0, 0, 0, 1.0])
        flip = np.array([1.0, 0, 0, 0])
        q = np.stack([eye, eye, flip, eye, eye])
        issues = Q.validate_quats({"hand_r": q})
        self.assertTrue(any(i["kind"] == "quat_pop" for i in issues))
        fixed = Q.repair_quats(q)
        self.assertLess(Q.quat_angle_deg(fixed[2], fixed[1]), 5.0)
        self.assertFalse(Q.validate_quats({"hand_r": fixed}))

    def test_palm_already_down_is_left_alone(self):
        forearm = np.array([0.0, 0.0, 1.0])
        down = np.array([0.0, -1.0, 0.0])
        ang = Q.palm_delta(forearm, down, np.array(BODY["up"]), np.array([1.0, 0, 0]), np.array(BODY["forward"]))
        self.assertEqual(ang, 0.0)

    def test_palm_facing_backward_on_a_hanging_arm_is_corrected(self):
        forearm = np.array([0.0, -1.0, 0.0])
        backward = np.array([0.0, 0.0, -1.0])
        ang = Q.palm_delta(forearm, backward, np.array(BODY["up"]), np.array([1.0, 0, 0]), np.array(BODY["forward"]))
        self.assertGreater(abs(ang), 0.2)

    def test_fist_weight_rises_through_a_fast_extension(self):
        ext = np.array([0.4, 0.5, 0.85, 0.98, 0.98, 0.7, 0.45])
        speed = np.array([0.1, 0.2, 1.5, 2.2, 2.0, 0.4, 0.1])
        w = Q.shape_weights(ext, speed)
        self.assertGreater(w["fist"][3], w["fist"][0])
        self.assertGreater(w["relaxed"][0], w["fist"][0])


class SamplingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.srv = _studio()

    def test_matrix_rest_decomposes(self):
        info = self.srv.model_info("kev_melee.glb")
        for bone in ("pelvis", "upperarm_r", "root"):
            i = info["index"][bone]
            nd = info["js"]["nodes"][i]
            if "matrix" not in nd:
                continue
            M = self.srv.node_matrix(nd)
            C = self.srv.compose_trs(info["rest_t"][i], info["rest_q"][i], info["rest_s"][i])
            self.assertLess(np.linalg.norm(M - C), 1e-3, bone)

    def test_kev_melee_attack_is_not_a_statue(self):
        info = self.srv.model_info("kev_melee.glb")
        tracks = self.srv.clip_tracks(info, "Attack1H01_L")
        self.assertIn(("upperarm_r", "rotation"), tracks)

        def wrist(x):
            Q, P = {}, {}
            for (bone, path), (t, v, kind) in tracks.items():
                val = self.srv.eval_track(t, v, kind, x)
                (Q if path == "rotation" else P)[bone] = val
            return np.array(self.srv.kev_read(info, Q, P)["landmarks"]["wrist_r"])

        self.assertGreater(np.linalg.norm(wrist(0.45) - wrist(0.0)), 0.05)
        rot = self.srv.clip_motion_span("kev_melee.glb", "Attack1H01_L")
        self.assertGreater(rot, 20.0)
        L = np.stack([wrist(x) for x in np.linspace(0, 1.0, 8)])[:, None, :]
        self.assertEqual(Q.motion_state(L, rot), "ok")

    def test_tpose_is_static_not_frozen(self):
        rot = self.srv.clip_motion_span("ual1.glb", "A_TPose")
        self.assertLess(rot, 2.0)
        self.assertEqual(Q.motion_state(np.zeros((4, 2, 3)), rot), "static")

    def test_b_root_maps_to_root(self):
        self.assertEqual(self.srv.canonical("B-root"), "root")
        self.assertEqual(self.srv.canonical("B-upperArm.R"), "upperarm_r")


class StatePositionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.srv = _studio()

    def test_api_state_uses_pelvis_translation(self):
        info = self.srv.model_info("ual1.glb")
        i = info["index"]["pelvis"]
        rest = info["rest_t"][i].copy()
        # Pelvis local axes are not world axes: drop the joint 3 cm along world up.
        parent = info["parent"][i]
        Wp = np.eye(4) if parent < 0 else self.srv.chain_world(info, {}, {}, parent)
        up = np.array(info["bm"]["body"]["up"], float)
        lowered = rest + (Wp[:3, :3].T @ (-0.03 * up))
        high = np.array(self.srv.kev_read(info, {})["landmarks"]["ankle_l"])
        low = np.array(self.srv.kev_read(info, {}, {"pelvis": lowered})["landmarks"]["ankle_l"])
        self.assertGreater(float((high - low) @ up), 0.02)

        class Reply:
            def reply(self, code, body, ctype="application/json"):
                self.code = code
                self.body = body

        reply = Reply()
        self.srv.Handler.telemetry(reply, {"model": "ual1.glb", "quats": {}, "pos": {"pelvis": lowered.tolist()},
                                           "t": 0.0, "playing": False, "clip": None, "duration": 0, "speed": 1})
        state_ankle = np.array(self.srv.STATE["summary"]["landmarks"]["ankle_l"])
        self.assertLess(np.linalg.norm(state_ankle - low), 1e-3)

    def test_control_and_state_agree_on_a_lowered_pelvis(self):
        srv = self.srv
        info = srv.model_info("ual1.glb")
        lm = {k: list(v["rest"]) for k, v in info["bm"]["landmarks"].items()}
        up = np.array(info["bm"]["body"]["up"], float)
        for k in lm:
            lm[k] = (np.array(lm[k]) - 0.025 * up).tolist()
        srv.COMMANDED.clear()
        srv.COMMANDED_POS.clear()
        srv.apply_frame(info, lm, inplace=False, hands={"hand_r": lm["wrist_r"]}, cleanup=False)
        control = srv.kev_read(info, srv.COMMANDED, srv.COMMANDED_POS)
        # What /api/state does once telemetry includes the posed translations.
        state = srv.kev_read(info, srv.COMMANDED, srv.COMMANDED_POS)
        dropped = srv.kev_read(info, srv.COMMANDED)
        self.assertLess(np.linalg.norm(np.array(state["landmarks"]["ankle_l"]) - np.array(control["landmarks"]["ankle_l"])), 1e-6)
        self.assertGreater(np.array(dropped["landmarks"]["ankle_l"])[1] - np.array(control["landmarks"]["ankle_l"])[1], 0.015)


if __name__ == "__main__":
    unittest.main()
