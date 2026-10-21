"""VOLN1: first touch of the previous week's volume point of control (POC), traded as a rejection of the approach.

Level from traded activity, not from price extremes: the previous trading week's volume profile is built from its H1
bars (each bar's volume spread uniformly over its high-low range, 24 equal-height rows over the week's range, i.e. the
default 'Number of rows = 24' of the common session/visible-range volume-profile tools); the POC is the centre of the
row with the most volume. Weeks are calendar weeks Sunday..Saturday on the bar clock (pandas period 'W-SAT'), so the
Sunday-evening reopen of USDJPY / metals belongs to the new week. USDJPY and metals volume is tick volume, SOL is
exchange volume; only the relative distribution of volume across prices within a week is used, never its level.
Fixed rule, constants chosen a priori (24 rows, 1.0 ATR beyond the level, 24-bar exit, >= 24 bars in the prior week).
"""
import numpy as np
import pandas as pd

from sweep_lib import atr

ROWS = 24            # volume-profile rows over the prior week's high-low range (tool default)
MIN_PRIOR_BARS = 24  # prior week must contain at least one day of H1 bars to define a profile

PREREG = dict(
    id="VOLN1",
    family="volume-profile node (prior-week POC) first-touch rejection",
    desc=(
        "Weeks = pandas periods 'W-SAT' of the bar timestamps (Sunday..Saturday). For every week W whose immediately "
        "preceding calendar week W-1 has >= 24 H1 bars and positive total volume: build W-1's volume profile with 24 "
        "equal-height price rows spanning [min low, max high] of W-1; each W-1 bar adds its volume to the rows in "
        "proportion to the overlap of the row with the bar's [low, high] (a zero-range bar puts all its volume in the row "
        "containing its price). POC = centre of the row with the largest volume (lowest row on an exact tie). In week W: "
        "if the first bar of W has low <= POC <= high (the week opened at value) there is no trade in W. Otherwise the "
        "approach side is fixed by that first bar: from ABOVE if its low > POC, from BELOW if its high < POC. The touch "
        "bar i is the first later bar of W that reaches the POC (low[i] <= POC when approaching from above; high[i] >= POC "
        "from below). Only this first touch is considered. If the touch bar closes back on the approach side (close[i] > "
        "POC from above -> LONG; close[i] < POC from below -> SHORT), enter at the open of bar i+1; if it closes at or "
        "through the POC there is no trade in W. Stop anchor ext = POC, stop = POC -/+ 1.0*ATR14 (ATR14 known at the close "
        "of bar i), i.e. one ATR beyond the level on the far side. No take-profit; exit at the close of the 24th bar after "
        "entry (H=24) or at the stop. At most one trade per week per instrument; size 1; same rule on all instruments."
    ),
    rationale=(
        "The previous week's POC is the single price at which the most inventory changed hands last week, i.e. the modal "
        "cost basis of last week's positions. Once price has moved away from it, a large group of participants is "
        "holding positions opened there at a loss (those on the wrong side of the move) and another at a gain. The "
        "disposition effect and break-even anchoring (Shefrin & Statman 1985; Odean 1998; capital-gains-overhang and "
        "volume-weighted reference prices, Grinblatt & Han 2005) imply that the losing group is eager to exit flat when "
        "price first returns to its cost basis: shorts opened at the POC buy back there when price falls back to it from "
        "above, longs sell there when price rallies back to it from below, so the first return meets order flow opposing "
        "the approach. Auction-market (Market Profile) practice describes the same thing as the untested ('naked') POC "
        "acting as support/resistance on its first retest, because a high-volume node is where the other side previously "
        "showed up in size. The effect should be strongest on the FIRST touch (the break-even inventory is consumed by "
        "it) and only when price comes from away (a week that opens at value has no stranded inventory to release), "
        "hence those two filters; a close back on the approach side shows the opposing flow actually absorbed the touch. "
        "The stop is referenced to the level, not to the wick: if price trades a full ATR beyond the node the "
        "break-even supply/demand has been absorbed and the idea is falsified. With entry on the approach side and the "
        "stop one ATR beyond the level, 1R is typically 1-1.5 ATR, so round-trip costs are a smaller fraction of 1R than "
        "for the 0.8-ATR wick stops of the sweep methods."
    ),
    novelty=(
        "No earlier method (S1-S13, K1-K4, N1-N8) or batch proposal (FVG1, SESS1, VREG1) uses volume at all. Their levels "
        "are price extremes (previous day/week high/low, Asian range, fractal swings, equal highs/lows, clusters of "
        "daily highs), round numbers or clock ranges; VOLN1's level is the mode of last week's traded-volume distribution, "
        "which normally lies INSIDE last week's range, away from the extremes where stops cluster, so the event is a "
        "return to an acceptance price, not a liquidity sweep beyond an extreme. Only the first touch per week is used, "
        "and only when the week opened away from the node (approach from away, at most one event per week). The stop "
        "is anchored to the level (1 ATR beyond the POC) rather than 0.25 ATR beyond the touch bar's wick, and there is "
        "no take-profit at an opposite liquidity pool (unlike N1). It is not an inversion of any failed method."
    ),
    data_informed=True,
    H=24,
    pad=1.0,
    notes=(
        "data_informed=True only because the stop placement (anchored at the level with a 1-ATR pad instead of 0.25 ATR "
        "beyond a wick) was chosen knowing the lead's fact (a) that ~0.8-ATR stops make costs 0.10-0.16R; the level, "
        "the trade direction and every constant come from the cost-basis / Market-Profile argument above, not from any "
        "result. No profit/loss, R or win rate of this rule or any variant was computed or seen; only "
        "'eval_proposal.py check' (causality + counts) was run before registration. Constants: 24 rows (volume-profile "
        "tool default), 1.0 ATR (one unit of hourly noise beyond the node), H=24 (protocol default), 24 bars minimum "
        "for the prior week (one trading day)."
    ),
)


def _poc(h, l, v):
    """Centre of the max-volume row of a ROWS-row profile over [min l, max h]; volume spread uniformly over [l, h]."""
    lo, hi = float(np.min(l)), float(np.max(h))
    if not hi > lo:
        return np.nan
    edges = lo + (hi - lo) * np.arange(ROWS + 1) / ROWS
    rng = h - l
    ov = np.clip(np.minimum(h[:, None], edges[None, 1:]) - np.maximum(l[:, None], edges[None, :-1]), 0.0, None)
    share = ov / np.where(rng > 0, rng, 1.0)[:, None]
    z = np.flatnonzero(rng <= 0)
    if len(z):
        share[z, :] = 0.0
        share[z, np.clip(((l[z] - lo) / (hi - lo) * ROWS).astype(int), 0, ROWS - 1)] = 1.0
    prof = (share * v[:, None]).sum(axis=0)
    j = int(np.argmax(prof))
    return 0.5 * (edges[j] + edges[j + 1])


def generate(df, name):
    cols = ["i", "side", "ext", "atr"]
    n = len(df)
    if n < 2 * MIN_PRIOR_BARS:
        return pd.DataFrame(columns=cols)
    h = df.high.values.astype(float); l = df.low.values.astype(float)
    c = df.close.values.astype(float); v = df.volume.values.astype(float)
    A = atr(df).values                                        # ATR14 known at the close of bar i
    wk = np.asarray(df.index.to_period("W-SAT").asi8)         # week ordinal (Sunday..Saturday)
    starts = np.flatnonzero(np.r_[True, wk[1:] != wk[:-1]])   # first bar of each week (index is sorted)
    ends = np.r_[starts[1:], n]

    rows = []
    for k in range(1, len(starts)):
        ps, pe, s, e = starts[k - 1], ends[k - 1], starts[k], ends[k]
        if wk[s] - wk[ps] != 1 or pe - ps < MIN_PRIOR_BARS:
            continue                                          # prior calendar week missing or too short
        vv = v[ps:pe]
        if not np.all(np.isfinite(vv)) or vv.sum() <= 0:
            continue
        poc = _poc(h[ps:pe], l[ps:pe], vv)                    # uses only bars of week W-1 (all closed before W opens)
        if not np.isfinite(poc):
            continue
        if l[s] <= poc <= h[s]:
            continue                                          # week opened at value: no approach from away
        from_above = l[s] > poc
        for i in range(s + 1, e):                             # forward scan: first touch only
            if from_above and l[i] <= poc:
                side = 1 if c[i] > poc else 0
            elif (not from_above) and h[i] >= poc:
                side = -1 if c[i] < poc else 0
            else:
                continue
            if side != 0 and np.isfinite(A[i]) and A[i] > 0:
                rows.append((df.index[i], i, side, poc, A[i]))
            break
    if not rows:
        return pd.DataFrame(columns=cols)
    E = pd.DataFrame(rows, columns=["t"] + cols).set_index("t")
    return E
