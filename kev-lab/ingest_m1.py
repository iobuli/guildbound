"""python3 kev-lab/ingest_m1.py [SYMBOL] [FROM YYYY-MM] [TO YYYY-MM]: Binance spot 1-minute bars from the public archive.

Monthly zips from data.binance.vision go to markets/<SYM>/raw/m1/ (gitignored) and are packed into markets/<SYM>/raw/m1.npz:
t (ms, int64), o, h, l, c, v (float64), and zv (bool, volume == 0). Missing minutes stay missing (no invented prices); they are listed in the report.
Binance writes a no-trade minute as a flat bar at the previous close with v=0. Those rows are kept (they are in the archive) and flagged in zv.
The 1m bars are checked against kev's H1 file by aggregating them to hours. Hours that differ only because those flats move the open, high or low are counted apart from any other mismatch.
"""
from __future__ import annotations

import csv
import io
import sys
import urllib.request
import zipfile
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import data  # noqa: E402

M = 60_000
URL = "https://data.binance.vision/data/spot/monthly/klines/{s}/1m/{s}-1m-{y}-{m:02d}.zip"


def months(a: str, b: str):
    y, m = map(int, a.split("-"))
    y1, m1 = map(int, b.split("-"))
    while (y, m) <= (y1, m1):
        yield y, m
        y, m = (y + 1, 1) if m == 12 else (y, m + 1)


def fetch(sym: str, y: int, m: int, raw: Path) -> Path:
    p = raw / f"{sym}-1m-{y}-{m:02d}.zip"
    if not p.exists() or p.stat().st_size == 0:
        p.write_bytes(urllib.request.urlopen(URL.format(s=sym, y=y, m=m), timeout=120).read())
    return p


def read_zip(p: Path) -> np.ndarray:
    with zipfile.ZipFile(p) as z:
        rows = list(csv.reader(io.TextIOWrapper(z.open(z.namelist()[0]))))
    if rows and not rows[0][0].isdigit():
        rows = rows[1:]  # some archives carry a header
    a = np.array([[float(r[0]), *map(float, r[1:6])] for r in rows])
    if a.size and a[0, 0] > 1e14:  # microsecond timestamps (archives from 2025 on)
        a[:, 0] //= 1000
    return a


def assemble(parts: list[np.ndarray]) -> dict[str, np.ndarray]:
    """Stack monthly rows, drop duplicate timestamps, keep v=0 flats, flag them in zv."""
    x = np.concatenate(parts)
    x = x[np.argsort(x[:, 0], kind="stable")]
    x = x[np.concatenate([[True], np.diff(x[:, 0]) > 0])]
    v = x[:, 5]
    return {"t": x[:, 0].astype(np.int64), "o": x[:, 1], "h": x[:, 2], "l": x[:, 3], "c": x[:, 4], "v": v, "zv": v == 0}


def ingest(sym: str = "SOLUSDT", a: str = "2020-08", b: str = "2022-12") -> dict:
    raw = data.MARKETS / sym / "raw" / "m1"
    raw.mkdir(parents=True, exist_ok=True)
    parts = []
    for y, m in months(a, b):
        parts.append(read_zip(fetch(sym, y, m, raw)))
        print(f"  {y}-{m:02d}: {parts[-1].shape[0]} minutes", file=sys.stderr)
    d = assemble(parts)
    np.savez_compressed(data.MARKETS / sym / "raw" / "m1.npz", **d)
    t = d["t"]
    x = np.column_stack([d["t"], d["o"], d["h"], d["l"], d["c"], d["v"]])
    gaps = np.diff(t) // M - 1
    report = {"minutes": int(t.size), "from": int(t[0]), "to": int(t[-1]), "missing": int(gaps[gaps > 0].sum()),
              "zero_volume_minutes": int(d["zv"].sum()),
              "gaps": [(int(t[i]), int(g)) for i, g in enumerate(gaps) if g > 0][:50], "check": check_h1(sym, t, x)}
    return report


def check_h1(sym: str, t: np.ndarray, x: np.ndarray, H: dict | None = None) -> dict:
    """Aggregate complete 1m hours and compare OHLC with kev's H1 bars.

    v=0 minutes stay in the raw aggregate. A differing hour that matches once those minutes are ignored
    for the open, high and low (the close is the last minute either way) is differ_zero_volume.
    Anything still off is differ_other, with its hour timestamps in unexplained_hours.
    """
    if H is None:
        H = data.h1(sym)
    t = np.asarray(t)
    x = np.asarray(x, float)
    v = x[:, 5]
    hour = t // 3_600_000 * 3_600_000
    keys, start, cnt = np.unique(hour, return_index=True, return_counts=True)
    full = cnt == 60
    idx = {int(v_): i for i, v_ in enumerate(H["t"])}
    same = zv = other = missing = 0
    unexplained = []
    worst = worst_other = 0.0
    for k in np.nonzero(full)[0]:
        j = idx.get(int(keys[k]))
        if j is None:
            missing += 1
            continue
        s, n = int(start[k]), int(cnt[k])
        sl = slice(s, s + n)
        o, h, lo, cl = x[s, 1], x[sl, 2].max(), x[sl, 3].min(), x[s + n - 1, 4]
        d = max(abs(o - H["o"][j]), abs(h - H["h"][j]), abs(lo - H["l"][j]), abs(cl - H["c"][j])) / H["c"][j]
        worst = max(worst, float(d))
        if d < 1e-9:
            same += 1
            continue
        traded = v[sl] != 0
        if traded.any():
            ii = np.flatnonzero(traded)
            o2, h2, l2 = x[sl, 1][ii[0]], x[sl, 2][traded].max(), x[sl, 3][traded].min()
            d2 = max(abs(o2 - H["o"][j]), abs(h2 - H["h"][j]), abs(l2 - H["l"][j]), abs(cl - H["c"][j])) / H["c"][j]
        else:
            d2 = 1.0
        if d2 < 1e-9:
            zv += 1
        else:
            other += 1
            unexplained.append(int(keys[k]))
            worst_other = max(worst_other, float(d2))
    return {"complete_hours": int(full.sum()), "identical": same, "differ": zv + other,
            "differ_zero_volume": zv, "differ_other": other, "unexplained_hours": unexplained,
            "zero_volume_minutes": int((v == 0).sum()), "not_in_h1": missing,
            "worst_rel_diff": worst, "worst_unexplained_rel_diff": worst_other}


if __name__ == "__main__":
    import json
    args = sys.argv[1:]
    r = ingest(*(args or []))
    print(json.dumps({k: v for k, v in r.items() if k != "gaps"}, indent=1))
    print("first gaps (start ms, minutes missing):", r["gaps"][:10])
