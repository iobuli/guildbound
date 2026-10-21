"""MAG1: draw-on-liquidity approach trade. When price comes within one ATR of a still-untaken prior-day/prior-week
high (low), trade TOWARD it with the take-profit exactly at that pool and a symmetric one-ATR stop.

Fixed rule, no tuned parameters. Constants are round, canonical choices fixed before any check was run:
  ZONE = 1.0 ATR   the magnet must be within one typical hourly range of the signal close
  STOP = 1.0 ATR   stop distance mirrors the zone (stop one ATR on the other side of the signal close)
  H    = 24 bars   one trading day
"""
import numpy as np
import pandas as pd

from sweep_lib import atr

ZONE = 1.0
STOP = 1.0

PREREG = dict(
    id="MAG1",
    family="liquidity magnet: approach to an untaken prior-day/prior-week extreme (tp at the pool)",
    desc=(
        "Trading day: for USDJPY, XAUUSD, XAGUSD the FX convention, the day rolls at 17:00 New York time (bar open times are "
        "treated as UTC, converted to America/New_York DST-aware, shifted +7h, date taken; so Sunday-evening bars belong to "
        "Monday); for SOLUSDT the UTC calendar date. Trading week = pandas period 'W' (Mon..Sun) of the trading day. "
        "PDH/PDL = high/low of the previous trading day present in the data; PWH/PWL = high/low of the previous trading week "
        "present in the data (both fully completed before the current day/week starts). At the close of bar i: PDH is "
        "UNTAKEN if every bar of the current trading day up to and including bar i has high < PDH; PDL untaken if every such "
        "bar has low > PDL; PWH (PWL) untaken if every bar of the current trading week up to and including i has high < PWH "
        "(low > PWL). U[i] = the nearest untaken upper pool = min of the untaken values among {PDH, PWH} (none -> no upper "
        "magnet); D[i] = the nearest untaken lower pool = max of the untaken values among {PDL, PWL}. A = ATR14 = "
        "sweep_lib.atr(df) at the close of bar i. Upper zone flag zu[i] = U exists and U[i] - close[i] <= 1.0*A[i]; lower "
        "zone flag zd[i] = D exists and close[i] - D[i] <= 1.0*A[i]. LONG signal at bar i if zu[i] and not zu[i-1] (the close "
        "has just come within one ATR of an untaken upper pool, or a new untaken pool has just appeared within one ATR) and "
        "not zd[i]; SHORT signal if zd[i] and not zd[i-1] and not zu[i]. If price is within one ATR of untaken pools on both "
        "sides there is no trade. Enter at the open of bar i+1. Take-profit tp = U[i] for longs, D[i] for shorts (exactly at "
        "the pool). Stop: ext = close[i] - side*1.0*A[i], pad = 0, i.e. one ATR beyond the signal close on the side away "
        "from the magnet. Exit at tp, at the stop, or at the close of the 24th bar after entry (H=24). Size 1. Same rule on "
        "all four instruments; overlapping signals are thinned by the harness (gap 24 bars)."
    ),
    rationale=(
        "Prior-day and prior-week highs/lows are where stop-loss orders of positioned traders and breakout entry stops "
        "cluster (Osler 2003, 2005: stop-loss orders in FX cluster just beyond salient prior extremes and round numbers, and "
        "their execution produces fast, self-reinforcing price cascades). Such an untaken pool is a source of liquidity: a "
        "large participant who needs to buy (sell) size fills best against the burst of sell (buy) orders the triggered "
        "stops release, and dealers who see the clustered orders in their books, as well as short-horizon traders who "
        "anticipate the cascade, profit from pushing price the final stretch into the pool. That incentive exists only "
        "while the pool is still untaken (its orders have not been consumed) and is strongest when it is close, i.e. within "
        "one typical hourly range. So the hypothesis is that once price is within one ATR of an untaken pool, the "
        "probability of touching the pool before moving one ATR the other way is higher than the martingale value implied "
        "by the distances. The trade harvests only this run-in leg and exits AT the pool, before the partial reversal that "
        "the sweep-fade literature (fact a: small gross reversal after a sweep) documents on the other side of the level; "
        "both facts are consistent with the same stop-hunt cycle (attraction -> run -> exhaustion)."
    ),
    novelty=(
        "None of the 24 methods in REPORT.md (S1-S13, K1-K3, N1-N8) nor the registered batch proposals trades the APPROACH "
        "to a level: S1-S13, N4, N5, N8 fade a wick through a level after it was swept; S11 and EXH1 follow after the level "
        "has broken; N1 fades a sweep toward the OPPOSITE pool. MAG1 differs from N1 on every axis: (1) there is no sweep "
        "or reversal trigger, the event is price entering a one-ATR zone below (above) an untaken pool, so the trade is "
        "usually WITH the recent move rather than against an extreme; (2) the target is the NEAREST untaken pool and must be "
        "within 1 ATR, whereas N1 required the opposite pool to be at least 2 ATR away (low hit rate); (3) the stop is a "
        "fixed one ATR from the signal close (symmetric to the zone) instead of a ~0.8 ATR wick stop, and the exit is at the "
        "pool before any sweep happens. It also differs from VOLN1 (fades a first touch of a volume node, no tp) and TSMOM1 "
        "(20-day breakout follow)."
    ),
    data_informed=True,
    H=24,
    pad=0.0,
    notes=(
        "data_informed=True: choosing a NEAR target and an approach (non-reversal) entry was shaped by knowing that N1 "
        "(sweep fade toward a far opposite pool) lost about 0.2R everywhere and that sweep fades have only a small gross "
        "edge. No P/L of this rule or any variant was computed; only eval_proposal.py check (causality + counts) was run "
        "before registration. Day roll at 17:00 New York is the standard FX/metals trading-day convention (bar times look "
        "like UTC: FX Sunday bars at 21-23h, metals at 22-23h, last Friday bar 21h); SOL uses the standard UTC daily candle."
    ),
)


def _trading_day(idx, name):
    if name == "SOLUSDT_H1":
        return idx.normalize()
    ny = idx.tz_localize("UTC").tz_convert("America/New_York") + pd.Timedelta(hours=7)
    return pd.DatetimeIndex(ny.tz_localize(None).normalize())


def _prev_period_extremes(h, l, key):
    """For each bar: high/low of the previous period (by sorted key) present in the data, and running high/low of the
    current period up to and including the bar. All causal."""
    g_hi = h.groupby(key).max(); g_lo = l.groupby(key).min()
    prev_hi = g_hi.shift(1).reindex(key).values
    prev_lo = g_lo.shift(1).reindex(key).values
    run_hi = h.groupby(key).cummax().values
    run_lo = l.groupby(key).cummin().values
    return prev_hi, prev_lo, run_hi, run_lo


def generate(df, name):
    o = df.open.astype(float); h = df.high.astype(float); l = df.low.astype(float); c = df.close.astype(float)
    n = len(df)
    if n < 30:
        return pd.DataFrame(columns=["i", "side", "ext", "atr", "tp"])
    A = atr(df).values

    day = _trading_day(df.index, name)
    week = day.to_period("W").start_time
    hh = pd.Series(h.values, index=np.arange(n)); ll = pd.Series(l.values, index=np.arange(n))
    dkey = pd.Series(day.values, index=np.arange(n)); wkey = pd.Series(week.values, index=np.arange(n))
    pdh, pdl, dhi, dlo = _prev_period_extremes(hh, ll, dkey)
    pwh, pwl, whi, wlo = _prev_period_extremes(hh, ll, wkey)

    up_d = np.where(np.isfinite(pdh) & (dhi < pdh), pdh, np.inf)
    up_w = np.where(np.isfinite(pwh) & (whi < pwh), pwh, np.inf)
    U = np.minimum(up_d, up_w)
    dn_d = np.where(np.isfinite(pdl) & (dlo > pdl), pdl, -np.inf)
    dn_w = np.where(np.isfinite(pwl) & (wlo > pwl), pwl, -np.inf)
    D = np.maximum(dn_d, dn_w)

    cv = c.values
    okA = np.isfinite(A) & (A > 0)
    zu = okA & np.isfinite(U) & ((U - cv) <= ZONE * np.where(okA, A, np.nan))
    zd = okA & np.isfinite(D) & ((cv - D) <= ZONE * np.where(okA, A, np.nan))
    zu_prev = np.concatenate([[True], zu[:-1]])     # first bar: treated as already in zone -> no signal
    zd_prev = np.concatenate([[True], zd[:-1]])
    long_sig = zu & ~zu_prev & ~zd
    short_sig = zd & ~zd_prev & ~zu

    rows = []
    for i in np.flatnonzero(long_sig | short_sig):
        side = 1 if long_sig[i] else -1
        tp = U[i] if side == 1 else D[i]
        ext = cv[i] - side * STOP * A[i]
        rows.append((df.index[i], int(i), side, float(ext), float(A[i]), float(tp)))
    E = pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr", "tp"]).set_index("t")
    return E
