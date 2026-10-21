"""Reads what kev writes (markets/*, out/signals/*) and runs kev when signals need refreshing. Live: new SOL hours from Binance."""
from __future__ import annotations

import json
import subprocess
import urllib.request
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent  # kev-nodes/
MARKETS, SIGNALS = ROOT / "markets", ROOT / "out" / "signals"
LIVE = {"SOLUSDT": "SOLUSDT"}  # markets whose own feed is Binance spot, so new hours can be appended without splicing feeds
H = 3_600_000


def markets() -> list[str]:
    return sorted(p.name for p in MARKETS.iterdir() if (p / "profile.json").exists())


def h1(sym: str) -> dict:
    """Columnar H1 candles as numpy arrays (t, o, h, l, c, v)."""
    j = json.loads((MARKETS / sym / "h1.json").read_text())
    t = j["t0"] + np.cumsum(np.array(j["dt"], float)) * j["step"]
    return {"t": t.astype(np.int64), **{k: np.array(j[k], float) for k in "ohlcv"}}


def signals(sym: str) -> dict:
    return json.loads((SIGNALS / f"{sym}.json").read_text())


def kev(*args: str) -> str:
    """Runs a kev CLI command (TypeScript) and returns its output."""
    r = subprocess.run(["npx", "tsx", "src/cli.ts", *args], cwd=ROOT, capture_output=True, text=True, timeout=1800)
    if r.returncode:
        raise RuntimeError(r.stderr[-2000:])
    return r.stdout


def append_live(sym: str) -> int:
    """Appends newly closed Binance hours to markets/<sym>/h1.json. Returns how many were added."""
    path = MARKETS / sym / "h1.json"
    j = json.loads(path.read_text())
    t = j["t0"] + sum(j["dt"]) * j["step"]
    url = f"https://data-api.binance.vision/api/v3/klines?symbol={LIVE[sym]}&interval=1h&startTime={int(t + H)}&limit=1000"
    rows = json.loads(urllib.request.urlopen(url, timeout=30).read())
    now_ms = rows[-1][6] if rows else 0
    added = 0
    for k in rows:
        open_t, close_t = int(k[0]), int(k[6])
        if close_t > now_ms or open_t <= t:  # skip the hour still forming and anything already present
            continue
        if close_t >= int(__import__("time").time() * 1000):
            continue
        j["dt"].append((open_t - t) / j["step"])
        for key, idx in zip("ohlc", (1, 2, 3, 4)):
            j[key].append(float(k[idx]))
        j["v"].append(float(k[5]))
        t = open_t
        added += 1
    if added:
        path.write_text(json.dumps(j))
    return added
