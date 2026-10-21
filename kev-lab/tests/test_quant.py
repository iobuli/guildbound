"""Known-value checks for quant.py and the paper broker. Run: python3 -m unittest discover -s kev-lab/tests"""
import math
import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import quant  # noqa: E402
from paper import KEY, Book  # noqa: E402


class Quant(unittest.TestCase):
    def test_sharpe(self):
        d = np.array([0.01, -0.01, 0.02, 0.0])
        self.assertAlmostEqual(quant.sharpe(d, 1), d.mean() / d.std(ddof=1))
        self.assertEqual(quant.sharpe(np.zeros(5)), 0.0)

    def test_drawdown(self):
        mdd, dur, _ = quant.drawdown(np.array([100, 120, 90, 95, 130, 117]))
        self.assertAlmostEqual(mdd, -0.25)
        self.assertEqual(dur, 2)

    def test_psr_dsr(self):
        rng = np.random.default_rng(1)
        good, noise = rng.normal(0.002, 0.01, 1000), rng.normal(0, 0.01, 1000)
        self.assertGreater(quant.psr(good), 0.95)
        self.assertLess(abs(quant.psr(noise) - 0.5), 0.45)
        trials = [rng.normal(0, 0.01, 1000) for _ in range(20)]
        self.assertLessEqual(quant.dsr(good, trials), quant.psr(good))

    def test_hurst_random_walk(self):
        h = quant.hurst(np.random.default_rng(3).normal(0, 1, 20000))
        self.assertLess(abs(h - 0.5), 0.1)

    def test_variance_ratio(self):
        vr, z = quant.variance_ratio(np.random.default_rng(4).normal(0, 1, 20000))
        self.assertLess(abs(vr - 1), 0.15)

    def test_kelly(self):
        self.assertEqual(quant.kelly_fraction(np.full(20, -1.0)), 0.0)
        r = np.array([2.0, -1.0] * 10)
        self.assertAlmostEqual(quant.kelly_fraction(r), 0.5 / 2.5)


class Broker(unittest.TestCase):
    def row(self, mkt, t, xt, R):
        return {"mkt": mkt, "t": t, "dir": 1, "fill": 1.0, "r": {KEY: R}, "xt": {KEY: xt}}

    def test_accounting(self):
        b = Book().run([self.row("A", 0, 10, 2.0), self.row("A", 5, 20, -1.0), self.row("B", 12, 30, -1.0)])
        self.assertEqual(b.skipped["market busy"], 1)
        self.assertAlmostEqual(b.ledger[0]["pnl"], 1000.0)  # 0.5% of 100k x 2R
        self.assertAlmostEqual(b.ledger[1]["pnl"], -505.0)  # sized from 101k
        self.assertAlmostEqual(b.equity, 100_000 + sum(x["pnl"] for x in b.ledger))

    def test_risk_cap(self):
        rows = [self.row(m, k, 100, 1.0) for k, m in enumerate("ABCD")]
        b = Book().run(rows)
        self.assertEqual(b.skipped["risk cap"], 1)
        self.assertTrue(math.isclose(b.equity, 101_500.0))


if __name__ == "__main__":
    unittest.main()
