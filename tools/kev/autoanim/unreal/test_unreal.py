"""Retarget basis and compact GLB regressions; no game install/key required."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest

import numpy as np
import retarget_glb as R


class RetargetTests(unittest.TestCase):
    def test_game_forward_and_left_survive_axis_conversion(self):
        np.testing.assert_allclose(R.BASIS @ [1, 0, 0], [1, 0, 0])
        np.testing.assert_allclose(R.BASIS @ [0, 1, 0], [0, 0, 1])
        np.testing.assert_allclose(R.BASIS @ [0, 0, 1], [0, 1, 0])

    def test_quaternion_round_trip_including_half_turn(self):
        for angle in [0, 0.01, 1.2, np.pi]:
            q = np.array([np.sin(angle/2), 0, 0, np.cos(angle/2)])
            matrix = R.statue.trs({"rotation": q})
            recovered = R.quaternion(matrix)
            self.assertAlmostEqual(abs(float(q @ recovered)), 1.0, places=6)

    def test_compact_animation_matches_full_export(self):
        base = Path(__file__).resolve().parents[4] / 'assets/anim/ual2.glb'
        poses = [dict(q=[0, 0, 0, 1], p=[0, 0, 100], s=[1, 1, 1]),
                 dict(q=[0, 0, np.sin(0.2), np.cos(0.2)], p=[5, 0, 100], s=[1, 1, 1])]
        data = dict(source='test', name='test', duration=0.1,
                    bones=[dict(name='WAIST', parent=-1, rest=poses[0])],
                    frames=[dict(t=0, pose=[poses[0]]), dict(t=0.1, pose=[poses[1]])])
        with tempfile.TemporaryDirectory() as temp:
            source, full, compact = [Path(temp) / name for name in ['clip.json', 'full.glb', 'compact.glb']]
            source.write_text(json.dumps(data))
            with contextlib.redirect_stdout(io.StringIO()):
                R.convert(base, full, [source])
                R.convert(base, compact, [source], animation_only=True)
            f, fb = R.statue.read_glb(full)
            c, cb = R.statue.read_glb(compact)
            self.assertNotIn('meshes', c)
            self.assertNotIn('skins', c)
            self.assertLess(compact.stat().st_size, full.stat().st_size / 10)
            for fs, cs in zip(f['animations'][0]['samplers'], c['animations'][0]['samplers']):
                np.testing.assert_allclose(R.statue.accessor_view(f, fb, fs['output'])[0],
                                           R.statue.accessor_view(c, cb, cs['output'])[0])


if __name__ == '__main__':
    unittest.main()
