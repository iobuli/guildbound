"""python3 kev-lab/ingest_m1.py [SYMBOL] [FROM YYYY-MM] [TO YYYY-MM]: Binance spot 1-minute bars from the public archive.

Monthly zips from data.binance.vision go to markets/<SYM>/raw/m1/ (gitignored) and are packed into markets/<SYM>/raw/m1.npz:
t (ms, int64) and o, h, l, c, v (float64). Missing minutes stay missing (no invented prices); they are listed in the report.
The 1m bars are checked against kev's H1 file by aggregating them to hours.
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


def ingest(sym: str = "SOLUSDT", a: str = "2020-08", b: str = "2022-12") -> dict:
    raw = data.MARKETS / sym / "raw" / "m1"
    raw.mkdir(parents=True, exist_ok=True)
    parts = []
    for y, m in months(a, b):
        parts.append(read_zip(fetch(sym, y, m, raw)))
        print(f"  {y}-{m:02d}: {parts[-1].shape[0]} minutes", file=sys.stderr)
    x = np.concatenate(parts)
    x = x[np.argsort(x[:, 0], kind="stable")]
    x = x[np.concatenate([[True], np.diff(x[:, 0]) > 0])]
    t = x[:, 0].astype(np.int64)
    np.savez_compressed(data.MARKETS / sym / "raw" / "m1.npz", t=t, o=x[:, 1], h=x[:, 2], l=x[:, 3], c=x[:, 4], v=x[:, 5])
    gaps = np.diff(t) // M - 1
    report = {"minutes": int(t.size), "from": int(t[0]), "to": int(t[-1]), "missing": int(gaps[gaps > 0].sum()),
              "gaps": [(int(t[i]), int(g)) for i, g in enumerate(gaps) if g > 0][:50], "check": check_h1(sym, t, x)}
    return report


def check_h1(sym: str, t: np.ndarray, x: np.ndarray) -> dict:
    """Aggregate complete 1m hours and compare OHLC with kev's H1 bars."""
    hour = t // 3_600_000 * 3_600_000
    keys, start, cnt = np.unique(hour, return_index=True, return_counts=True)
    full = cnt == 60
    o = x[start, 1]
    end = start + cnt - 1
    c = x[end, 4]
    h = np.maximum.reduceat(x[:, 2], start)
    lo = np.minimum.reduceat(x[:, 3], start)
    H = data.h1(sym)
    idx = {int(v): i for i, v in enumerate(H["t"])}
    same = diff = missing = 0
    worst = 0.0
    for k in np.nonzero(full)[0]:
        j = idx.get(int(keys[k]))
        if j is None:
            missing += 1
            continue
        d = max(abs(o[k] - H["o"][j]), abs(h[k] - H["h"][j]), abs(lo[k] - H["l"][j]), abs(c[k] - H["c"][j])) / H["c"][j]
        worst = max(worst, d)
        if d < 1e-9:
            same += 1
        else:
            diff += 1
    return {"complete_hours": int(full.sum()), "identical": same, "differ": diff, "not_in_h1": missing, "worst_rel_diff": worst}


if __name__ == "__main__":
    import json
    args = sys.argv[1:]
    r = ingest(*(args or []))
    print(json.dumps({k: v for k, v in r.items() if k != "gaps"}, indent=1))
    print("first gaps (start ms, minutes missing):", r["gaps"][:10])
