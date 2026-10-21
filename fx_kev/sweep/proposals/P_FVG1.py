"""FVG1: first-retest rejection of an H1 fair-value gap (ICT three-bar imbalance), traded in the displacement direction.

Fixed rule, no tuned parameters (constants below are round, canonical choices fixed before any check was run).
"""
import numpy as np
import pandas as pd

from sweep_lib import atr

MIN_GAP_ATR = 0.5   # gap height must be at least half of the pre-displacement ATR(14)
MAX_AGE = 120       # a gap stays live for one trading week of H1 bars; untouched older gaps are dropped

PREREG = dict(
    id="FVG1",
    family="fair-value-gap retest",
    desc=(
        "Bullish FVG formed at bar k: low[k] > high[k-2]; the middle bar k-1 is a bullish candle (close>open) that itself "
        "traded through the whole gap (low[k-1] <= high[k-2] and high[k-1] >= low[k], so session/weekend gaps are excluded); "
        "gap height low[k]-high[k-2] >= 0.5*ATR14 measured at the close of bar k-2 (before the displacement). Gap zone = "
        "[high[k-2], low[k]]. Track the gap from bar k+1 for at most 120 bars. The FIRST bar i>k whose low trades into the "
        "gap (low[i] <= low[k]) is the only retest considered: if that bar closes back above the gap top (close[i] > low[k]), "
        "i.e. it wicked into the imbalance and was rejected from it, go LONG at the open of bar i+1; if it closes inside or "
        "below the gap, the gap is mitigated and there is no trade. Stop anchor ext = min(low[k-2], low[k-1], low[i]) (the "
        "origin of the displacement and the retest wick), stop = ext - 0.25*ATR14(at close of i). Exit at the close of bar "
        "i+24 or at the stop; no take-profit. Bearish FVG is the exact mirror (high[k] < low[k-2], bearish middle bar spanning "
        "the gap, first retest high[i] >= high[k] with close[i] < high[k] -> SHORT, ext = max(high[k-2], high[k-1], high[i])). "
        "If several gaps get their first retest on the same bar in one direction, the most recently formed one defines ext; "
        "if long and short retests coincide on the same bar, no trade."
    ),
    rationale=(
        "A three-bar imbalance marks a price range that traded only during one fast, one-directional bar: an aggressor "
        "(typically a large parent order or a forced/informed flow) lifted offers faster than liquidity replenished, so "
        "passive interest inside that range was never filled. Order flow is strongly autocorrelated (meta-orders are split "
        "over hours), so the same participant tends to keep working the remainder of the order passively at better prices; "
        "the first pullback into the unfilled range is where that resting/re-entering demand meets price. A retest bar that "
        "wicks into the gap and closes back outside it shows that this demand is still present and absorbed the pullback, so "
        "continuation in the displacement direction is expected over the following day. The stop sits beyond the origin of "
        "the displacement: if price returns there the imbalance is fully repaired and the order-flow story is falsified. "
        "Requiring the middle bar to span the gap ensures the imbalance came from trading, not from a session/weekend gap."
    ),
    novelty=(
        "None of S1-S13, K1-K4, N1-N8 uses imbalances: they all fade (or follow failed) wicks through reference levels "
        "(PDH/PDL, PWH/PWL, Asian range, fractals, round numbers, clusters) or score sweeps with Kev. FVG1 needs no reference "
        "level at all; the event is defined by a three-bar displacement structure, the trade is WITH the displacement "
        "(a pullback continuation, not a reversal of an extension), the entry is on the first retest of the gap rather than "
        "on the bar that makes an extreme, and the stop is anchored at the displacement origin (wider than a wick stop)."
    ),
    data_informed=False,
    H=24,
    pad=0.25,
    notes=(
        "Direction (with displacement), retest = first touch only, rejection = close back outside the gap, stop beyond the "
        "formation origin are the textbook ICT definitions. Constants 0.5 ATR and 120 bars were fixed a priori as round "
        "values; only causality/event counts (eval_proposal.py check) were looked at before registration."
    ),
)


def generate(df, name):
    o = df.open.values.astype(float); h = df.high.values.astype(float)
    l = df.low.values.astype(float); c = df.close.values.astype(float)
    n = len(o)
    A = atr(df).values            # A[j] = ATR14 known at the close of bar j
    if n < 4:
        return pd.DataFrame(columns=["i", "side", "ext", "atr"])

    k = np.arange(2, n)
    a0 = A[k - 2]
    ok_a = np.isfinite(a0) & (a0 > 0)
    bull = ok_a & (l[k] > h[k - 2]) & (c[k - 1] > o[k - 1]) & (l[k - 1] <= h[k - 2]) & (h[k - 1] >= l[k]) \
        & ((l[k] - h[k - 2]) >= MIN_GAP_ATR * np.where(ok_a, a0, np.inf))
    bear = ok_a & (h[k] < l[k - 2]) & (c[k - 1] < o[k - 1]) & (h[k - 1] >= l[k - 2]) & (l[k - 1] <= h[k]) \
        & ((l[k - 2] - h[k]) >= MIN_GAP_ATR * np.where(ok_a, a0, np.inf))

    sig = {}   # i -> {side: (k, ext)}  (keeps the most recently formed gap per side)
    for kk in k[bull]:
        top = l[kk]
        for i in range(kk + 1, min(n, kk + 1 + MAX_AGE)):
            if l[i] <= top:                       # first return into the gap
                if c[i] > top:                    # rejected: closed back above the gap
                    ext = min(l[kk - 2], l[kk - 1], l[i])
                    d = sig.setdefault(i, {})
                    if 1 not in d or kk > d[1][0]:
                        d[1] = (kk, ext)
                break
    for kk in k[bear]:
        bot = h[kk]
        for i in range(kk + 1, min(n, kk + 1 + MAX_AGE)):
            if h[i] >= bot:
                if c[i] < bot:
                    ext = max(h[kk - 2], h[kk - 1], h[i])
                    d = sig.setdefault(i, {})
                    if -1 not in d or kk > d[-1][0]:
                        d[-1] = (kk, ext)
                break

    rows = []
    for i in sorted(sig):
        d = sig[i]
        if len(d) != 1:
            continue                              # long and short retests on the same bar: ambiguous
        side, (kk, ext) = next(iter(d.items()))
        if not np.isfinite(A[i]):
            continue
        rows.append((df.index[i], i, side, ext, A[i]))
    E = pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"]).set_index("t")
    return E
