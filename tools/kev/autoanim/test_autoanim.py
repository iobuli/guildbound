import json
import os
import tempfile
import unittest

import numpy as np

from core import CameraIntrinsics, MotionConfig, MotionPipeline, attach_depth, robust_depth, synthetic_frames, unproject
from reference import ReferenceRecorder


class DepthTests(unittest.TestCase):
    def test_depth_median_rejects_holes_and_edge_outlier(self):
        depth = np.full((9, 9), 2.0, dtype=float)
        depth[3:6, 3:6] = [[2.0, 0.0, 8.0], [2.01, np.nan, 2.0], [1.99, 2.0, 2.01]]
        value, confidence = robust_depth(depth, 4, 4, radius=1)
        self.assertAlmostEqual(value, 2.0, places=2)
        self.assertGreater(confidence, 0.4)

    def test_unproject_camera_centre(self):
        k = CameraIntrinsics(500, 500, 320, 240, 640, 480)
        np.testing.assert_allclose(unproject(320, 240, 2.0, k), [0, 0, 2])

    def test_attach_depth_preserves_tracker_confidence(self):
        k = CameraIntrinsics(500, 500, 4, 4, 9, 9)
        out = attach_depth({"nose": {"x": 0.5, "y": 0.5, "confidence": 0.8}}, np.full((9, 9), 1.5), k)
        self.assertAlmostEqual(out["nose"]["z"], 1.5)
        self.assertLessEqual(out["nose"]["confidence"], 0.8)


class PipelineTests(unittest.TestCase):
    def test_mapping_derives_kev_body_targets(self):
        pipe = MotionPipeline()
        out = pipe.process(next(iter(synthetic_frames(1, noise=0))))
        required = {"pelvis", "neck", "head_joint", "crown", "nose", "shoulder_l", "shoulder_r",
                    "elbow_l", "elbow_r", "wrist_l", "wrist_r", "hip_l", "hip_r", "knee_l",
                    "knee_r", "ankle_l", "ankle_r", "ball_l", "ball_r"}
        self.assertTrue(required.issubset(out["landmarks"]))

    def test_dropout_and_spikes_stay_finite(self):
        pipe = MotionPipeline(MotionConfig(max_speed_m_s=8.0))
        outputs = [pipe.process(f) for f in synthetic_frames(1200, dropout=0.2, spikes=0.02)]
        outputs = [f for f in outputs if f]
        self.assertGreaterEqual(len(outputs), 1190)
        values = [x for f in outputs for p in f["landmarks"].values() for x in p]
        self.assertTrue(np.isfinite(values).all())
        self.assertGreater(pipe.filter.clamped, 0)
        self.assertGreater(pipe.filter.held, 0)

    def test_metric_input_can_preserve_scale(self):
        raw = next(iter(synthetic_frames(1, noise=0)))
        # Convert only enough direct points to a synthetic +Y-up Kev frame.
        raw["space"] = "kev"
        raw["landmarks"] = {k: {"x": v["x"], "y": 1 - v["y"], "z": v["z"], "confidence": 1}
                            for k, v in raw["landmarks"].items()}
        pipe = MotionPipeline(MotionConfig(input_space="kev", preserve_metric_scale=True))
        out = pipe.process(raw)
        self.assertIsNotNone(out)
        self.assertEqual(pipe.scale, 1.0)


class ReferenceTests(unittest.TestCase):
    def test_reference_round_trip_shape(self):
        pipe = MotionPipeline()
        rec = ReferenceRecorder("test")
        for frame in synthetic_frames(8, noise=0):
            rec.add(pipe.process(frame))
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "test.json")
            rec.write(path)
            with open(path, encoding="utf-8") as f:
                data = json.load(f)
        self.assertEqual(data["name"], "test")
        self.assertEqual(len(data["samples"]), 8)
        self.assertGreater(data["duration"], 0)


if __name__ == "__main__":
    unittest.main()
