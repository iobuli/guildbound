"""Nucleus checks: brute force, no lookahead, forming vs closed lines, n-n-n collapse, 1m vs H1, and JS/Python parity on real hours."""
import json
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import data  # noqa: E402
import nucleus as N  # noqa: E402


def synthetic(n=6000, seed=5):
    rng = np.random.default_rng(seed)
    t = 1_600_000_000_000 // 86_400_000 * 86_400_000 + np.arange(n, dtype=np.int64) * 60_000
    keep = np.ones(n, bool)
    keep[[700, 701, 702, 2500] + list(range(4000, 4065))] = False  # missing minutes, one whole hour
    c = 100_000 + np.cumsum(rng.integers(-30, 31, n))
    h = c + rng.integers(0, 20, n)
    lo = c - rng.integers(0, 20, n)
    return {"t": t[keep], "h": h[keep], "l": lo[keep], "c": c[keep]}


class Lines(unittest.TestCase):
    def setUp(self):
        self.m = synthetic()
        self.nu = N.nucleus(self.m)

    def test_forming_line_brute_force(self):
        m, size, n = self.m, N.TFS["5m"], 9
        tf = N.timeframe(m, size)
        L = N.line(tf, m["c"].astype(float), n)
        bars = m["t"] // size
        for k in np.random.default_rng(1).integers(200, m["t"].size, 60):
            ub = np.unique(bars[: k + 1])[-n:]  # the forming bar and the n-1 bars before it that exist
            sel = np.isin(bars[: k + 1], ub)
            HH, LL = m["h"][: k + 1][sel].max(), m["l"][: k + 1][sel].min()
            mid = (HH + LL) / 2
            self.assertAlmostEqual(L["mid"][k], mid)
            pos = (m["c"][k] - mid) / ((HH - LL) / 2) if HH > LL else 0.0
            self.assertAlmostEqual(L["pos"][k], pos)

    def test_no_lookahead(self):
        k = 3000
        cut = {key: v[: k + 1] for key, v in self.m.items()}
        changed = {key: v.copy() for key, v in self.m.items()}
        changed["h"][k + 1 :] += 5000
        changed["l"][k + 1 :] -= 5000
        changed["c"][k + 1 :] += 777
        for other in (N.nucleus(cut), N.nucleus(changed)):
            for key, v in self.nu.items():
                np.testing.assert_array_equal(v[: k + 1], other[key][: k + 1], err_msg=key)

    def test_forming_at_bar_end_equals_closed_at_next_open(self):
        m, n = self.m, 26
        tf = N.timeframe(m, N.TFS["15m"])
        L = N.line(tf, m["c"].astype(float), n)
        last = np.nonzero(np.diff(tf["J"]))[0]  # last minute of each bar
        nxt = tf["J"][last + 1]
        ok = ~np.isnan(L["mid"][last]) & ~np.isnan(L["cmid_bar"][nxt])
        np.testing.assert_allclose(L["mid"][last][ok], L["cmid_bar"][nxt][ok])
        j = 100  # closed line = rolling n over the bar series
        self.assertEqual(L["cmid_bar"][j], (tf["BH"][j - n : j].max() + tf["BL"][j - n : j].min()) / 2)

    def test_n_n_n_collapses(self):
        tf = N.timeframe(self.m, N.TFS["1h"])
        L = N.line(tf, self.m["c"].astype(float), 9)
        a = (L["cmid_bar"] + L["cmid_bar"]) / 2  # span A of a 9-9-9 Ichimoku
        np.testing.assert_array_equal(a[~np.isnan(a)], L["cmid_bar"][~np.isnan(a)])  # = span B: zero-thickness cloud


M1 = data.MARKETS / "SOLUSDT" / "raw" / "m1.npz"
BIN = data.ROOT / "out" / "nucleus" / "SOLUSDT-m1.bin.gz"


@unittest.skipUnless(M1.exists() and BIN.exists() and shutil.which("node"), "needs SOL 1m data (ingest_m1.py, nucleus.py export) and node")
class RealData(unittest.TestCase):
    def test_m1_matches_kev_h1(self):
        import ingest_m1
        m = np.load(M1)
        x = np.stack([m["t"], m["o"], m["h"], m["l"], m["c"], m["v"]], 1)
        r = ingest_m1.check_h1("SOLUSDT", m["t"], x)
        self.assertGreater(r["identical"] / r["complete_hours"], 0.98)

    def test_js_equals_python(self):
        m = N.load_m1()
        nu = N.nucleus(m)
        hours = np.unique(m["t"] // 3_600_000 * 3_600_000)
        pick = np.random.default_rng(3).choice(hours[hours > m["t"][0] + 110 * 86_400_000], 20, replace=False)
        js = json.loads(subprocess.run(["node", str(HERE / "nucleus_parity.cjs"), str(BIN), ",".join(map(str, pick))], capture_output=True, text=True, check=True).stdout)
        idx = {int(t): i for i, t in enumerate(m["t"])}
        compared = 0
        for h, fr in js.items():
            for f in fr:
                i = idx[int(f["t"])]
                self.assertEqual(bool(nu["valid"][i]), f["valid"])
                for tf, v in f["tf"].items():
                    self.assertEqual(int(nu[f"{tf}.order"][i]), v["order"], (h, tf))
                    self.assertEqual(int(nu[f"{tf}.cloud"][i]), v["cloud"], (h, tf))
                for tf, n, pos, slope, shift, lag in f["lines"]:
                    p = float(nu[f"{tf}.{n}.pos"][i])
                    self.assertTrue((pos is None and np.isnan(p)) or abs(p - pos) < 1e-6, (h, tf, n, p, pos))  # float32 storage
                    self.assertEqual((int(nu[f"{tf}.{n}.slope"][i]), int(nu[f"{tf}.{n}.shift"][i]), int(nu[f"{tf}.{n}.lag"][i])), (slope, shift, lag), (h, tf, n))
                self.assertAlmostEqual(float(nu["bal"][i]), f["bal"], 6)
                compared += 1
        self.assertGreater(compared, 1000)


if __name__ == "__main__":
    unittest.main()
