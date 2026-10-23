"""The nucleus: 6 timeframes x 3 n-n-n Ichimoku lines (n = 9, 26, 52), read at every 1-minute close (kev-brain/experiments/2026-10-21-nucleus.md).

An n-n-n Ichimoku is one line: the midpoint of the highest high and lowest low of the last n bars of its timeframe. Three of them per
timeframe rebuild the classic 9-26-52 Ichimoku (conversion = mid9, base = mid26, cloud span B = mid52, span A = (mid9 + mid26) / 2).

Per line, at minute m (bar j of the line's timeframe is the one still forming; it holds the minutes up to and including m):
  forming line  HH = max(high of bars j-n+1 .. j-1, high of bar j so far), LL likewise; mid = (HH + LL) / 2, half = (HH - LL) / 2
  closed line   the same over the n closed bars j-n .. j-1 (what the line was when bar j opened)
  pos   = (price - mid) / half                 where price sits in the line's own range, -1 .. +1
  slope = sign(mid - closed mid)               the line moved up or down since the bar opened
  shift = sign(price - closed mid at bar j-n+1)  price against the line as it was n bars ago (Ichimoku's forward shift).
        Unknown until that bar's closed mid exists (J >= 2n-1); stored as -9, never as a fake 0.
  lag   = sign(price - close of bar j-n)       Ichimoku's lagging line. Same -9 until the bar exists.
  cmid, cpos  the closed line at this minute (the line when bar j opened) and price's position in its range.
Per timeframe: the order of mid9 / mid26 / mid52. Equal mids are not a strict order (order -2); they are not filed with either stack.
Price against the rebuilt cloud (spans from closed lines at bar j-25, i.e. shifted 26; -9 until both spans exist).
A minute is valid only when every line's pos, slope, shift and lag are known and every cloud is known.
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
MISSING = -9  # not knowable yet. A real tie stays 0. The cloud uses the same sentinel.
TIE = -2  # two or more of mid9 / mid26 / mid52 are equal
WARMUP_DAYS = 106  # existing UTC days, not calendar days. 1D n=52 shift needs J >= 103; 106 leaves a short gap margin.


def load_m1(sym: str = "SOLUSDT") -> dict:
    """Prices in ticks. Zero-volume flat minutes stay in the series (flagged as zv by ingest); dropping them would move 2020 highs and lows."""
    z = np.load(data.MARKETS / sym / "raw" / "m1.npz")
    return {"t": z["t"].astype(np.int64), **{k: np.round(z[k] / TICK).astype(np.int64) for k in "hlc"}}


def _as_sign(x: np.ndarray) -> np.ndarray:
    """-1, 0, 1, or MISSING where x is NaN. 0 is a real tie, not a stand-in for 'not yet'."""
    out = np.full(x.shape, MISSING, np.int8)
    ok = ~np.isnan(x)
    out[ok] = x[ok]
    return out


def warmup_index(t: np.ndarray, hour_t: int, days: int = WARMUP_DAYS) -> int:
    """First minute of the UTC day `days` existing days before hour_t's day.

    An existing day is a UTC day with at least one minute at or before that day.
    If fewer than `days` such days exist, the first minute of the series.
    """
    end = int(np.searchsorted(t, (int(hour_t) // 86_400_000 + 1) * 86_400_000, side="left"))
    if end <= 0:
        return 0
    _, first = np.unique(t[:end] // 86_400_000, return_index=True)
    if first.size > days:
        return int(first[-(days + 1)])
    return int(first[0])


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
    if n > 1:
        pmH, pmL = _prev_window(tf["BH"], n - 1, np.max), _prev_window(tf["BL"], n - 1, np.min)
        ok = ~np.isnan(pmH[J])
        HH, LL = np.fmax(pmH[J], tf["runH"]), np.fmin(pmL[J], tf["runL"])
    else:
        ok = np.ones(J.size, bool)
        HH, LL = tf["runH"].astype(float), tf["runL"].astype(float)
    mid = np.where(ok, (HH + LL) / 2, np.nan)
    half = np.where(ok, (HH - LL) / 2, np.nan)
    cH, cL = _prev_window(tf["BH"], n, np.max), _prev_window(tf["BL"], n, np.min)
    cmid_bar = (cH + cL) / 2  # closed line when bar j opened
    cmid = cmid_bar[J]
    chalf = ((cH - cL) / 2)[J]
    pos = np.where(half > 0, (c - mid) / np.where(half > 0, half, 1), 0.0)
    pos[~ok] = np.nan
    cpos = np.where(chalf > 0, (c - cmid) / np.where(chalf > 0, chalf, 1), 0.0)
    cpos[np.isnan(cmid)] = np.nan
    slope = np.sign(mid - cmid)
    js = J - n + 1
    shifted = np.where(js >= 0, cmid_bar[np.clip(js, 0, None)], np.nan)
    shift = np.sign(c - shifted)
    jl = J - n
    lagc = np.where(jl >= 0, tf["BC"][np.clip(jl, 0, None)].astype(float), np.nan)
    lag = np.sign(c - lagc)
    return {"mid": mid, "pos": pos, "slope": slope, "shift": shift, "lag": lag, "cmid_bar": cmid_bar, "cmid": cmid, "cpos": cpos}


@np.errstate(all="ignore")
def nucleus(m: dict, tfs: dict | None = None) -> dict:
    """Every reading at every minute. Returns float32/int8 arrays keyed '<tf>.<n>.<reading>', '<tf>.order', '<tf>.cloud', 'bal', ..."""
    tfs = TFS if tfs is None else tfs
    c = m["c"].astype(float)
    out: dict[str, np.ndarray] = {"t": m["t"], "c": m["c"]}
    signs, slopes = [], []
    for name, size in tfs.items():
        tf = timeframe(m, size)
        L = {n: line(tf, c, n) for n in NS}
        for n in NS:
            out[f"{name}.{n}.pos"] = L[n]["pos"].astype(np.float32)
            out[f"{name}.{n}.cpos"] = L[n]["cpos"].astype(np.float32)
            out[f"{name}.{n}.cmid"] = L[n]["cmid"]
            for k in ("slope", "shift", "lag"):
                out[f"{name}.{n}.{k}"] = _as_sign(L[n][k])
            signs.append(np.sign(L[n]["pos"]))
            slopes.append(np.where(np.isnan(L[n]["mid"]), np.nan, L[n]["slope"]))
        mids = np.stack([L[n]["mid"] for n in NS], 1)
        valid = ~np.isnan(mids).any(1)
        rank = np.argsort(-np.nan_to_num(mids, nan=0.0), axis=1, kind="stable")
        code = ORDER_LUT[rank[:, 0] * 9 + rank[:, 1] * 3 + rank[:, 2]]
        tied = (mids[:, 0] == mids[:, 1]) | (mids[:, 0] == mids[:, 2]) | (mids[:, 1] == mids[:, 2])
        out[f"{name}.order"] = np.where(~valid, -1, np.where(tied, TIE, code)).astype(np.int8)
        jc = tf["J"] - 25
        okc = jc >= 0
        jcc = np.clip(jc, 0, None)
        A = (L[9]["cmid_bar"][jcc] + L[26]["cmid_bar"][jcc]) / 2
        B = L[52]["cmid_bar"][jcc]
        cl = np.where(c > np.fmax(A, B), 1, np.where(c < np.fmin(A, B), -1, 0))
        out[f"{name}.cloud"] = np.where(okc & ~np.isnan(A) & ~np.isnan(B), cl, MISSING).astype(np.int8)
        out[f"{name}.bal"] = np.nanmean(np.stack(signs[-3:], 1), 1).astype(np.float32)
    S = np.stack(signs, 1)
    out["bal"] = np.nanmean(S, 1).astype(np.float32)
    out["slope_bal"] = np.nanmean(np.stack(slopes, 1), 1).astype(np.float32)
    known = ~np.isnan(S).any(1) & np.all([out[f"{k}.cloud"] != MISSING for k in tfs], 0)
    known &= np.all([~np.isnan(out[f"{k}.{n}.pos"]) for k in tfs for n in NS], 0)
    for name in tfs:
        for n in NS:
            for k in ("slope", "shift", "lag"):
                known &= out[f"{name}.{n}.{k}"] != MISSING
    out["valid"] = known
    return out


def frames(m: dict, hour_t: int) -> dict:
    """The minutes of one hour (what the page shows), from WARMUP_DAYS existing days before that day."""
    a = warmup_index(m["t"], hour_t)
    b = int(np.searchsorted(m["t"], hour_t + 3_600_000, side="left"))
    sub = {k: v[a:b] for k, v in m.items()}
    nu = nucleus(sub)
    k0 = int(np.searchsorted(sub["t"], hour_t, side="left"))
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
