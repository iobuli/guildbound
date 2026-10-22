"""Guards for the 2024-only walk-forward. No market file is read."""
import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import walkforward as W  # noqa: E402


def hours_in_2024(per_quarter: int = 400, seed: int = 0) -> np.ndarray:
    rng = np.random.default_rng(seed)
    parts = []
    for a, b in zip(W.QUARTERS[:-1], W.QUARTERS[1:]):
        span = (b - a) // 3_600_000
        pick = rng.choice(span, size=per_quarter, replace=False)
        parts.append(a + np.sort(pick) * 3_600_000)
    return np.sort(np.concatenate(parts))


class Folds(unittest.TestCase):
    def test_expanding_quarters_do_not_see_the_future(self):
        hour = hours_in_2024()
        folds = W.expanding_folds(hour)
        self.assertEqual(len(folds), 3)
        for train, val in folds:
            self.assertGreater(train.sum(), 200)
            self.assertGreater(val.sum(), 200)
            self.assertLess(hour[train].max(), hour[val].min())

    def test_a_2025_hour_is_rejected(self):
        hour = np.concatenate([hours_in_2024(per_quarter=50), [W.Y2025]])
        with self.assertRaises(ValueError):
            W.expanding_folds(hour)


class Model(unittest.TestCase):
    def test_feature_list_is_the_registered_38(self):
        names = W.feature_names()
        self.assertEqual(len(names), 38)
        self.assertEqual(names[:2], ["bal", "slope_bal"])
        self.assertEqual(names[-1], "1D.cloud")

    def test_separable_points_get_a_positive_weight(self):
        X = np.array([[-2.0], [-1.0], [1.0], [2.0]])
        y = np.array([0.0, 0.0, 1.0, 1.0])
        w = W.fit_logistic(X, y, lam=0.1)
        self.assertGreater(w[1], 0)
        score = w[0] + X[:, 0] * w[1]
        self.assertEqual(W.auc_score(score, y), 1.0)

    def test_zscore_uses_the_training_moments(self):
        train = np.array([[0.0], [2.0], [4.0]])
        mu, sd = W.zstats(train)
        got = W.apply_z(np.array([[10.0]]), mu, sd)
        self.assertAlmostEqual(got[0, 0], (10 - 2) / train.std())

    def test_slip_cost_is_five_hundredths_of_atr(self):
        got = W.slip_cost_bps(np.array([100.0]), np.array([10_000.0]))
        self.assertAlmostEqual(got[0], 5.0)

    def test_selection_stays_on_the_signalled_minute_and_inside_2024(self):
        rng = np.random.default_rng(1)
        hour = hours_in_2024(per_quarter=300, seed=1)
        n, p = hour.size, 3
        signal = rng.normal(size=n)
        y = (signal > 0).astype(float)
        X_by, y_by = {}, {}
        for mh in W.MH_GRID:
            X = rng.normal(size=(n, p))
            if mh == 15:
                X[:, 0] = signal
            X_by[mh] = X
            y_by[mh] = y
        chosen, table = W.select(X_by, y_by, hour)
        self.assertEqual(chosen["mh"], 15)
        self.assertGreater(chosen["cv_auc"], 0.9)
        on_minute = [r for r in table if r["mh"] == 15 and r["cv_auc"] is not None]
        winner = max(on_minute, key=lambda r: (r["cv_auc"], r["lam"]))
        self.assertEqual(chosen["lam"], winner["lam"])
        others = [r["cv_auc"] for r in table if r["mh"] != 15]
        self.assertGreater(chosen["cv_auc"], max(others))
        self.assertEqual(len(table), len(W.MH_GRID) * len(W.LAM_GRID))
        self.assertLess(int(hour.max()), W.Y2025)


if __name__ == "__main__":
    unittest.main()
