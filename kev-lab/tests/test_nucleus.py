"""Nucleus checks: brute force, no lookahead, forming vs closed lines, n-n-n collapse, 1m vs H1, and JS/Python parity on real hours."""
import datetime as dt
import gzip
import json
import shutil
import subprocess
import sys
import tempfile
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
        # 2020-08..2026-09 archive vs this h1.json: 209 hours differ, 207 of them only because v=0 flats move open/high/low.
        self.assertEqual(r["differ"], r["differ_zero_volume"] + r["differ_other"])
        self.assertEqual((r["differ"], r["differ_zero_volume"], r["differ_other"]), (209, 207, 2))
        self.assertEqual(r["unexplained_hours"], [1619139600000, 1649826000000])
        self.assertEqual(r["zero_volume_minutes"], int((m["v"] == 0).sum()))
        self.assertGreater(r["zero_volume_minutes"], 0)

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
                    self.assertAlmostEqual(float(nu[f"{tf}.bal"][i]), v["bal"], places=6, msg=(h, tf))
                for tf, n, pos, slope, shift, lag, cmid, cpos in f["lines"]:
                    p = float(nu[f"{tf}.{n}.pos"][i])
                    self.assertTrue((pos is None and np.isnan(p)) or abs(p - pos) < 1e-6, (h, tf, n, p, pos))  # float32 storage
                    self.assertEqual((int(nu[f"{tf}.{n}.slope"][i]), int(nu[f"{tf}.{n}.shift"][i]), int(nu[f"{tf}.{n}.lag"][i])), (slope, shift, lag), (h, tf, n))
                    got = float(nu[f"{tf}.{n}.cmid"][i])
                    self.assertTrue((cmid is None and np.isnan(got)) or abs(got - cmid) < 1e-6, (h, tf, n, got, cmid))
                    gp = float(nu[f"{tf}.{n}.cpos"][i])
                    self.assertTrue((cpos is None and np.isnan(gp)) or abs(gp - cpos) < 1e-6, (h, tf, n, gp, cpos))
                self.assertAlmostEqual(float(nu["bal"][i]), f["bal"], 6)
                self.assertAlmostEqual(float(nu["slope_bal"][i]), f["slopeBal"], 6)
                compared += 1
        self.assertGreater(compared, 1000)


DAY = 86_400_000
START = int(dt.datetime(2020, 8, 11, tzinfo=dt.timezone.utc).timestamp() * 1000)  # SOLUSDT listing day
MISSING = -9  # same sentinel as an unready cloud: not a real sign of 0
TIE = -2


def ymd(ms) -> str:
    return dt.datetime.fromtimestamp(int(ms) / 1000, dt.timezone.utc).date().isoformat()


def daily(n, price, drop=()):
    """One minute at each UTC midnight, so every timeframe has one bar per kept day."""
    t = START + np.arange(n, dtype=np.int64) * DAY
    keep = np.ones(n, bool)
    keep[list(drop)] = False
    c = np.asarray(price, np.int64)
    return {"t": t[keep], "h": c[keep].copy(), "l": c[keep].copy(), "c": c[keep].copy()}


def js_frames(m, hours):
    n = m["t"].size
    body = np.concatenate([[n], m["t"] // 60_000, m["h"], m["l"], m["c"]]).astype("<i4")
    fd, name = tempfile.mkstemp(suffix=".bin.gz")
    p = Path(name)
    try:
        with open(fd, "wb") as f:
            f.write(gzip.compress(body.tobytes(), 6))
        out = subprocess.run(["node", str(HERE / "nucleus_parity.cjs"), str(p), ",".join(map(str, hours))], capture_output=True, text=True, check=True)
    finally:
        p.unlink(missing_ok=True)
    return json.loads(out.stdout)


class Review(unittest.TestCase):
    def test_daily_shift_is_missing_until_the_shifted_line_exists(self):
        # Rising prices: a stored 0 cannot be a real tie. Cloud is ready 26 days before n=52 shift.
        m = daily(120, 100_000 + np.arange(120) * 1000)
        nu = N.nucleus(m)
        shift, cloud = nu["1D.52.shift"], nu["1D.cloud"]
        bad = np.nonzero((cloud != -9) & (shift == MISSING))[0]
        self.assertEqual(bad.size, 26)
        self.assertEqual((ymd(m["t"][bad[0]]), ymd(m["t"][bad[-1]])), ("2020-10-27", "2020-11-21"))
        self.assertFalse(bool(nu["valid"][bad].any()))
        self.assertIn(int(nu["1D.52.slope"][bad[0]]), (-1, 0, 1))  # slope is already real; only shift is early
        i = int(bad[-1] + 1)
        self.assertEqual(ymd(m["t"][i]), "2020-11-22")
        self.assertEqual(int(shift[i]), 1)
        self.assertTrue(bool(nu["valid"][i]))

    def test_equal_mids_are_not_filed_as_the_bull_stack(self):
        price = np.full(120, 100_000)
        price[59] = 0
        price[85:] = 100_000
        m = daily(120, price)
        nu = N.nucleus(m)
        i = 110
        self.assertTrue(bool(nu["valid"][i]))
        self.assertEqual(int(nu["1D.52.shift"][i]), 0)  # price really equals the shifted mid
        self.assertEqual(int(nu["1D.order"][i]), TIE)  # mid9 == mid26 > mid52, not 9>26>52
        trend = N.nucleus(daily(120, 100_000 + np.arange(120) * 1000))
        self.assertEqual(int(trend["1D.order"][110]), 0)  # a real bull stack stays a bull stack

    def test_closed_line_level_and_position_are_exported(self):
        m, n = synthetic(), 26
        nu = N.nucleus(m)
        tf = N.timeframe(m, N.TFS["1h"])
        ks = np.nonzero(tf["J"] >= n)[0]
        k = int(ks[len(ks) // 2])
        j = int(tf["J"][k])
        hh, ll = tf["BH"][j - n:j].max(), tf["BL"][j - n:j].min()
        cmid = (hh + ll) / 2
        half = (hh - ll) / 2
        cpos = 0.0 if half == 0 else (m["c"][k] - cmid) / half
        self.assertAlmostEqual(float(nu[f"1h.{n}.cmid"][k]), cmid)
        self.assertAlmostEqual(float(nu[f"1h.{n}.cpos"][k]), float(np.float32(cpos)), places=5)

    def test_warmup_counts_existing_days(self):
        m = daily(200, 100_000 + np.arange(200) * 1000, drop=range(100, 110))
        nu = N.nucleus(m)
        hour = int(START + 180 * DAY)
        i = int(np.where(m["t"] == hour)[0][0])
        fr = N.frames(m, hour)
        for key in ("1D.52.shift", "1D.52.slope", "1D.cloud", "1D.order", "1D.9.cmid", "valid"):
            self.assertEqual(np.asarray(fr[key][0]).item(), np.asarray(nu[key][i]).item(), key)
        self.assertEqual(int(nu["1D.52.shift"][i]), 1)
        cal_gap = hour // DAY * DAY - 106 * DAY
        self.assertLess(int(m["t"][N.warmup_index(m["t"], hour)]), cal_gap)  # gaps push the slice further back
        # no gaps: 106 existing days is the same midnight the calendar offset used
        plain = daily(150, 100_000 + np.arange(150) * 1000)
        hour = int(plain["t"][140])
        cal = hour // DAY * DAY - 106 * DAY
        self.assertEqual(int(plain["t"][N.warmup_index(plain["t"], hour)]), cal)

    @unittest.skipUnless(shutil.which("node"), "needs node")
    def test_js_matches_python_on_the_fixes(self):
        gappy = daily(200, 100_000 + np.arange(200) * 1000, drop=range(100, 110))
        price = np.full(120, 100_000)
        price[59] = 0
        price[85:] = 100_000
        tied = daily(120, price)
        for m, hour in ((gappy, int(START + 180 * DAY)), (tied, int(tied["t"][110]))):
            nu = N.nucleus(m)
            i = int(np.where(m["t"] == hour)[0][0])
            js = js_frames(m, [hour])[str(hour)][0]
            self.assertEqual(bool(nu["valid"][i]), js["valid"])
            self.assertAlmostEqual(float(nu["bal"][i]), js["bal"], 6)
            self.assertAlmostEqual(float(nu["slope_bal"][i]), js["slopeBal"], 6)
            self.assertEqual(int(nu["1D.order"][i]), js["tf"]["1D"]["order"])
            self.assertEqual(int(nu["1D.cloud"][i]), js["tf"]["1D"]["cloud"])
            row = next(x for x in js["lines"] if x[0] == "1D" and x[1] == 52)
            _, _, pos, slope, shift, lag, cmid, cpos = row
            self.assertEqual((int(nu["1D.52.slope"][i]), int(nu["1D.52.shift"][i]), int(nu["1D.52.lag"][i])), (slope, shift, lag))
            self.assertAlmostEqual(float(nu["1D.52.cmid"][i]), cmid, places=6)
            self.assertAlmostEqual(float(nu["1D.52.cpos"][i]), cpos, places=6)
            p = float(nu["1D.52.pos"][i])
            self.assertTrue((pos is None and np.isnan(p)) or abs(p - pos) < 1e-6)
        plain = daily(150, 100_000 + np.arange(150) * 1000)
        hour = int(plain["t"][140])
        script = r"""
const path = require("path");
require(path.join(process.argv[1], "web", "nucleus-core.js"));
const t = new Float64Array(JSON.parse(process.argv[2]));
const hour = Number(process.argv[3]);
const b = Nucleus.hourBounds({ t: new Float64Array([t[0], t[t.length - 1] + 3599999]) });
if (b.t0 !== t[0] || b.t1 !== Math.floor((t[t.length - 1] + 3599999) / 3600000) * 3600000) process.exit(2);
process.stdout.write(String(Nucleus.warmupIndex(t, hour, 106)));
"""
        r = subprocess.run(["node", "-e", script, str(HERE.parent), json.dumps(plain["t"].tolist()), str(hour)], capture_output=True, text=True, check=True)
        self.assertEqual(int(r.stdout), N.warmup_index(plain["t"], hour), r.stderr)


class IngestFlags(unittest.TestCase):
    def test_zero_volume_minutes_are_kept_and_the_h1_check_names_them(self):
        import ingest_m1
        M, H = 60_000, 3_600_000
        base = 1_600_000_000_000 // H * H
        rows = []
        for hour, close_bias in ((0, 0), (1, 0), (2, 1)):
            for i in range(60):
                stale = hour == 0 and i < 5
                px = 50 if stale else 100
                rows.append([base + hour * H + i * M, px, px, px, px, 0 if stale else 1])
        x = np.array(rows, float)
        kept = x.copy()
        h1 = {
            "t": np.array([base, base + H, base + 2 * H]),
            "o": np.array([100.0, 100.0, 100.0]),
            "h": np.array([100.0, 100.0, 100.0]),
            "l": np.array([100.0, 100.0, 100.0]),
            "c": np.array([100.0, 100.0, 101.0]),
            "v": np.zeros(3),
        }
        r = ingest_m1.check_h1("SOLUSDT", x[:, 0], x, H=h1)
        np.testing.assert_array_equal(x, kept)  # flag and report; do not rewrite the bars
        self.assertEqual(r["zero_volume_minutes"], 5)
        self.assertEqual(r["identical"], 1)
        self.assertEqual(r["differ_zero_volume"], 1)
        self.assertEqual(r["differ_other"], 1)
        self.assertEqual(r["differ"], 2)
        self.assertEqual(r["unexplained_hours"], [base + 2 * H])
        packed = ingest_m1.assemble([x, np.array([[rows[5][0], 9, 9, 9, 9, 7]], float)])
        self.assertEqual(packed["o"][5], 100)  # the later duplicate (price 9) is dropped
        self.assertEqual(int((packed["v"] == 0).sum()), 5)  # v=0 rows stay
        np.testing.assert_array_equal(packed["zv"], packed["v"] == 0)


class TraceWindow(unittest.TestCase):
    def test_periods_default_to_the_published_window_and_can_move(self):
        import inspect
        import trace as T
        self.assertEqual(T.SPLIT, 1_640_995_200_000)  # 2022-01-01
        self.assertEqual(T.END, 1_672_531_200_000)  # 2023-01-01
        self.assertEqual(T.parse_utc_date("2022-01-01"), T.SPLIT)
        self.assertEqual(T.parse_utc_date("2023-01-01"), T.END)
        self.assertEqual(inspect.signature(T.main).parameters["split"].default, T.SPLIT)
        self.assertEqual(inspect.signature(T.main).parameters["end"].default, T.END)
        sept = T.parse_utc_date("2026-09-01")
        hours = np.array([T.SPLIT - 3_600_000, T.SPLIT, T.END - 3_600_000, T.END, sept])
        d, c = T.period_masks(hours, first=0, split=T.SPLIT, end=T.END)
        np.testing.assert_array_equal(d, [True, False, False, False, False])
        np.testing.assert_array_equal(c, [False, True, True, False, False])
        _, later = T.period_masks(hours, first=0, split=T.SPLIT, end=T.parse_utc_date("2026-10-01"))
        self.assertTrue(bool(later[-1]))
        self.assertIsNone(T.period_bounds(hours, np.zeros(hours.size, bool)))
        self.assertEqual(T.hours_in_window(np.array([T.END - 3_600_000, T.END, T.END + 3_600_000]), T.END), 1)
        self.assertEqual(T.period_bounds(hours, later), [int(T.SPLIT), int(sept + 3_599_999)])


class BuildExe(unittest.TestCase):
    def test_host_node_does_not_have_to_be_the_pinned_runtime(self):
        text = (HERE.parent / "build-exe.sh").read_text()
        html = (HERE.parent / "web" / "nucleus.html").read_text()
        page = (HERE.parent / "web" / "nucleus.js").read_text()
        self.assertNotIn("need node $NODE_V", text)
        self.assertIn("NODE_V=v22.22.0", text)
        self.assertIn("FUSE=NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2", text)
        self.assertIn("host node", text)
        r = subprocess.run(["bash", "-n", str(HERE.parent / "build-exe.sh")], capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        # the page had the same hard stop as the trace (2022-12-31) and a fixed warmup floor
        self.assertNotIn('max="2022-12-31"', html)
        self.assertNotIn('min="2020-11-26"', html)
        self.assertNotIn("Date.UTC(2022, 11, 31", page)
        self.assertNotIn("Date.UTC(2020, 10, 26)", page)
        self.assertIn("hourBounds", page)


if __name__ == "__main__":
    unittest.main()
