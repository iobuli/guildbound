"""The nucleus: 6 timeframes x 3 n-n-n Ichimoku lines (n = 9, 26, 52), read at every 1-minute close (kev-brain/experiments/2026-10-21-nucleus.md).

An n-n-n Ichimoku is one line: the midpoint of the highest high and lowest low of the last n bars of its timeframe. Three of them per
timeframe rebuild the classic 9-26-52 Ichimoku (conversion = mid9, base = mid26, cloud span B = mid52, span A = (mid9 + mid26) / 2).

Per line, at minute m (bar j of the line's timeframe is the one still forming; it holds the minutes up to and including m):
  forming line  HH = max(high of bars j-n+1 .. j-1, high of bar j so far), LL likewise; mid = (HH + LL) / 2, half = (HH - LL) / 2
  closed line   the same over the n closed bars j-n .. j-1 (what the line was when bar j opened)
  pos   = (price - mid) / half                 where price sits in the line's own range, -1 .. +1
  slope = sign(mid - closed mid)               the line moved up or down since the bar opened
  shift = sign(price - closed mid at bar j-n+1)  price against the line as it was n bars ago (Ichimoku's forward shift)
  lag   = sign(price - close of bar j-n)       Ichimoku's lagging line
Per timeframe: the order of mid9 / mid26 / mid52, and price against the rebuilt cloud (spans from closed lines at bar j-25, i.e. shifted 26).
Convergence (counts, no weights): the balance of lines with price above them, per timeframe and over all 18; the balance of slopes (up - down).

Prices are integer ticks (1e-4) so the JS port in web/nucleus-core.js gives identical signs and orders, and positions equal to float32 storage precision. Bars are UTC-aligned; a bar exists only if
it has at least one minute, and missing minutes are never filled.
"""
from __future__ import annotations

import sys
import warnings
from pathlib import Path

import numpy as np
import pandas as pd
from numpy.lib.stride_tricks import sliding_window_view as win

sys.path.insert(0, str(Path(__file__).resolve().parent))
import data  # noqa: E402

warnings.filterwarnings("ignore", "Mean of empty slice")  # warm-up minutes with no valid line are NaN by design

TFS = {"1m": 60_000, "5m": 300_000, "15m": 900_000, "1h": 3_600_000, "4h": 14_400_000, "1D": 86_400_000}
NS = (9, 26, 52)
TICK = 1e-4
ORDERS = ["9>26>52", "9>52>26", "26>9>52", "26>52>9", "52>9>26", "52>26>9"]  # top line first; 9>26>52 is the bullish stack
ORDER_LUT = np.full(27, -1)
for _k, _o in enumerate(ORDERS):
    _r = [NS.index(int(x)) for x in _o.split(">")]
    ORDER_LUT[_r[0] * 9 + _r[1] * 3 + _r[2]] = _k
WARMUP_DAYS = 106  # 1D n=52 shifted copy needs 2n bars back; span B needs 26 + 52


def load_m1(sym: str = "SOLUSDT") -> dict:
    z = np.load(data.MARKETS / sym / "raw" / "m1.npz")
    return {"t": z["t"].astype(np.int64), **{k: np.round(z[k] / TICK).astype(np.int64) for k in "hlc"}}


def _prev_window(x: np.ndarray, w: int, fn) -> np.ndarray:
    """fn over x[j-w .. j-1] for every j (NaN when j < w)."""
    out = np.full(x.size, np.nan)
    if x.size > w:
        out[w:] = fn(win(x[:-1].astype(float), w), axis=1)
    return out


def timeframe(m: dict, size: int) -> dict:
    """Bars of one timeframe and, per minute, its bar index and the forming bar's running high/low."""
    bid = m["t"] // size
    start = np.concatenate([[0], np.nonzero(np.diff(bid))[0] + 1])
    J = np.cumsum(np.concatenate([[0], (np.diff(bid) != 0).astype(np.int64)]))
    BH = np.maximum.reduceat(m["h"], start)
    BL = np.minimum.reduceat(m["l"], start)
    BC = m["c"][np.concatenate([start[1:] - 1, [m["t"].size - 1]])]
    g = pd.Series(J)
    runH = pd.Series(m["h"]).groupby(g).cummax().to_numpy()
    runL = pd.Series(m["l"]).groupby(g).cummin().to_numpy()
    return {"J": J, "BH": BH, "BL": BL, "BC": BC, "runH": runH, "runL": runL}


def line(tf: dict, c: np.ndarray, n: int) -> dict:
    J = tf["J"]
    pmH, pmL = _prev_window(tf["BH"], n - 1, np.max), _prev_window(tf["BL"], n - 1, np.min)
    HH = np.fmax(pmH[J], tf["runH"]) if n > 1 else tf["runH"].astype(float)
    LL = np.fmin(pmL[J], tf["runL"]) if n > 1 else tf["runL"].astype(float)
    ok = ~np.isnan(pmH[J]) if n > 1 else np.ones(J.size, bool)
    mid, half = (HH + LL) / 2, (HH - LL) / 2
    cH, cL = _prev_window(tf["BH"], n, np.max), _prev_window(tf["BL"], n, np.min)
    cmid_bar = (cH + cL) / 2  # closed line when bar j opened
    cmid = cmid_bar[J]
    pos = np.where(half > 0, (c - mid) / np.where(half > 0, half, 1), 0.0)
    pos[~ok] = np.nan
    slope = np.sign(mid - cmid)
    js = J - n + 1
    shifted = np.where(js >= 0, cmid_bar[np.clip(js, 0, None)], np.nan)
    shift = np.sign(c - shifted)
    jl = J - n
    lagc = np.where(jl >= 0, tf["BC"][np.clip(jl, 0, None)].astype(float), np.nan)
    lag = np.sign(c - lagc)
    return {"mid": np.where(ok, mid, np.nan), "pos": pos, "slope": slope, "shift": shift, "lag": lag, "cmid_bar": cmid_bar}


@np.errstate(all="ignore")
def nucleus(m: dict) -> dict:
    """Every reading at every minute. Returns float32/int8 arrays keyed '<tf>.<n>.<reading>', '<tf>.order', '<tf>.cloud', 'bal', ..."""
    c = m["c"].astype(float)
    out: dict[str, np.ndarray] = {"t": m["t"], "c": m["c"]}
    signs, slopes = [], []
    for name, size in TFS.items():
        tf = timeframe(m, size)
        L = {n: line(tf, c, n) for n in NS}
        for n in NS:
            for k in ("pos",):
                out[f"{name}.{n}.{k}"] = L[n][k].astype(np.float32)
            for k in ("slope", "shift", "lag"):
                out[f"{name}.{n}.{k}"] = np.nan_to_num(L[n][k], nan=0).astype(np.int8)
            signs.append(np.sign(L[n]["pos"]))
            slopes.append(np.where(np.isnan(L[n]["mid"]), np.nan, L[n]["slope"]))
        mids = np.stack([L[n]["mid"] for n in NS], 1)
        valid = ~np.isnan(mids).any(1)
        rank = np.argsort(-np.nan_to_num(mids, nan=0), axis=1, kind="stable")  # ties: the shorter line first
        out[f"{name}.order"] = np.where(valid, ORDER_LUT[rank[:, 0] * 9 + rank[:, 1] * 3 + rank[:, 2]], -1).astype(np.int8)
        jc = tf["J"] - 25
        okc = jc >= 0
        jcc = np.clip(jc, 0, None)
        A = (L[9]["cmid_bar"][jcc] + L[26]["cmid_bar"][jcc]) / 2
        B = L[52]["cmid_bar"][jcc]
        cl = np.where(c > np.fmax(A, B), 1, np.where(c < np.fmin(A, B), -1, 0))
        out[f"{name}.cloud"] = np.where(okc & ~np.isnan(A) & ~np.isnan(B), cl, -9).astype(np.int8)
        out[f"{name}.bal"] = np.nanmean(np.stack(signs[-3:], 1), 1).astype(np.float32)
    S = np.stack(signs, 1)
    out["bal"] = np.nanmean(S, 1).astype(np.float32)
    out["slope_bal"] = np.nanmean(np.stack(slopes, 1), 1).astype(np.float32)
    out["valid"] = ~np.isnan(S).any(1) & np.all([out[f"{k}.cloud"] != -9 for k in TFS], 0) & np.all([~np.isnan(out[f"{k}.{n}.pos"]) for k in TFS for n in NS], 0)
    return out


def frames(m: dict, hour_t: int) -> dict:
    """The 60 minute-frames of one hour (what the page shows), computed on the minutes from WARMUP_DAYS before that day."""
    day0 = hour_t // 86_400_000 * 86_400_000 - WARMUP_DAYS * 86_400_000
    a, b = np.searchsorted(m["t"], [day0, hour_t + 3_600_000])
    sub = {k: v[a:b] for k, v in m.items()}
    nu = nucleus(sub)
    k0 = np.searchsorted(sub["t"], hour_t)
    return {k: v[k0:] for k, v in nu.items()}


def snapshots(sym: str = "SOLUSDT") -> Path:
    """Every 5th minute of the full nucleus, plus each hour's close, to out/nucleus/<sym>.npz (gitignored)."""
    m = load_m1(sym)
    nu = nucleus(m)
    keep = ((nu["t"] // 60_000) % 5 == 0) | ((nu["t"] // 60_000) % 60 == 59)  # every 5th minute and each hour's last
    out = data.ROOT / "out" / "nucleus"
    out.mkdir(parents=True, exist_ok=True)
    p = out / f"{sym}.npz"
    np.savez_compressed(p, **{k.replace(".", "__"): v[keep] for k, v in nu.items()})
    return p


def export_bin(sym: str = "SOLUSDT") -> Path:
    """1m data for the browser / exe: gzip of little-endian int32 [n, minute(n), h(n), l(n), c(n)] (minute = t / 60000, prices in ticks)."""
    import gzip
    m = load_m1(sym)
    n = m["t"].size
    body = np.concatenate([[n], m["t"] // 60_000, m["h"], m["l"], m["c"]]).astype("<i4")
    out = data.ROOT / "out" / "nucleus"
    out.mkdir(parents=True, exist_ok=True)
    p = out / f"{sym}-m1.bin.gz"
    p.write_bytes(gzip.compress(body.tobytes(), 6))
    return p


if __name__ == "__main__":
    if sys.argv[1:] == ["export"]:
        print(export_bin(), snapshots())
        sys.exit()
    import time
    t0 = time.time()
    m = load_m1()
    nu = nucleus(m)
    print(f"{nu['t'].size} minutes, {int(nu['valid'].sum())} with every line valid, {time.time() - t0:.1f}s")
    print({k: round(float(np.nanmean(v)), 3) for k, v in nu.items() if k.endswith("bal") or k == "slope_bal"})
