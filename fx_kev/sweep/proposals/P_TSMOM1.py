"""TSMOM1: Donchian 20-day channel breakout (Turtle System 1 entry), 2N volatility stop, 10-day time exit.

Fixed rule, no tuned parameters. Every constant below is the textbook value from the trend-following literature
(Donchian's 4-week rule / Turtle System 1: 20-day breakout, N = 20-day ATR, stop 2N), fixed before any check was run.
"""
import numpy as np
import pandas as pd

from sweep_lib import atr

N_DAYS = 20        # Donchian 4-week rule / Turtle System 1 breakout length (completed daily bars)
ATR_DAYS = 20      # Turtle "N": 20-day average true range on daily bars
STOP_N = 2.0       # Turtle stop: 2N against the position

PREREG = dict(
    id="TSMOM1",
    family="time-series momentum / Donchian channel breakout (follow)",
    desc=(
        "Daily bars are built from H1 bars by trading day: the calendar date of the bar time, except that for USDJPY, "
        "XAUUSD and XAGUSD (markets closed on Saturday) bars stamped on a Sunday belong to the following Monday's day; "
        "SOLUSDT (24/7) uses plain calendar days. For each H1 bar i, U[i] = highest daily high and L[i] = lowest daily low "
        "of the 20 most recent COMPLETED trading days before the day of bar i (today's partial day excluded), and "
        "N[i] = mean daily true range (max(H-L, |H-Cprev|, |L-Cprev|)) of the same 20 completed days. "
        "LONG signal at bar i if close[i] > U[i] and close[i-1] <= U[i] (the first H1 close above the 20-day high); SHORT "
        "signal if close[i] < L[i] and close[i-1] >= L[i]. Enter at the open of bar i+1. Stop (ext, pad=0) = "
        "close[i] - side*2*N[i], i.e. two daily ATRs from the signal close (Turtle 2N stop). No take-profit. Exit at the "
        "close of bar i+240 (10 trading days of H1 bars for FX/metals, 10 calendar days for SOL) or at the stop. "
        "Size weight w = 1 (see notes: because R is measured in units of 2N, equal R weight IS inverse-volatility "
        "position sizing). Signals within 240 bars of a taken signal are ignored (harness thinning), so the system is "
        "flat-or-one-position, always in the direction of the most recent fresh 20-day breakout at entry time. "
        "Requires 20 completed trading days of history and N > 0."
    ),
    rationale=(
        "Time-series momentum (Moskowitz, Ooi & Pedersen 2012; Hurst, Ooi & Pedersen's century-long study; the Turtle / "
        "Donchian channel rules used by CTAs since the 1960s-80s) is one of the best-replicated out-of-sample effects in "
        "FX and commodities. Proposed mechanisms: (1) slow diffusion of and under-reaction to fundamental information "
        "(central-bank policy divergence and carry for USDJPY, real rates / USD / industrial demand for gold and silver, "
        "flows into crypto), followed by delayed over-reaction as trend followers and momentum-chasing investors join; "
        "(2) hedging and risk-management flows that are pro-cyclical (option-gamma hedging when dealers are short gamma, "
        "stop-outs and margin calls of positioned traders, portfolio insurance), which push price further in the "
        "direction of a breakout; (3) a close beyond the prior 20-day extreme means the liquidity resting beyond the most "
        "visible multi-week level was consumed AND price was accepted there (the opposite of the failed sweeps faded in "
        "S1-S13): the participants whose stops sat there are now out, and the side that absorbed them was overwhelmed, "
        "which is informative about the imbalance that will keep working over the following days. The edge, if it "
        "exists, is small per day but persistent over weeks, so it must be harvested with a wide volatility stop "
        "(2 daily ATRs, about 10x the H1 ATR) and a long hold; that also makes round-trip costs a negligible fraction "
        "of 1R (USDJPY: 1 pip vs about 100+ pips of risk), which removes the failure mode of the sweep methods, whose "
        "0.8-ATR H1 stops made costs 0.10-0.16R."
    ),
    novelty=(
        "All 24 previous methods react to intraday liquidity levels (PDH/PDL, PWH/PWL, Asian range, 5-bar fractals, "
        "round numbers, clusters, equal highs/lows) with H1-ATR-sized stops and a 24-48 bar horizon, and nearly all FADE "
        "the wick (S11/N6 follow a failed sweep for 24 bars). TSMOM1 uses a multi-week reference (the 20-day Donchian "
        "channel built from completed daily bars), requires ACCEPTANCE (an H1 close beyond the channel, not a wick that "
        "closes back inside), always trades WITH the breakout, uses a daily-volatility stop (2N, about 10 H1 ATRs) and "
        "holds 240 bars. VREG1 (compression box breakout) also follows a close beyond a range, but its box is the last "
        "12 H1 bars, it is conditioned on volatility compression, and it holds 24 bars with a box-width stop."
    ),
    data_informed=True,
    H=240,
    pad=0.0,
    notes=(
        "data_informed=True is a conservative disclosure: the choice of the trend-following lens (follow an accepted "
        "breakout, wide stop, long hold) was suggested by the lead in the light of facts (a) and (c) (sweep fades have a "
        "gross edge smaller than costs because stops are tight; fading toward the opposite liquidity pool loses). The "
        "parameters themselves (20-day channel, 20-day N, 2N stop) are the canonical Turtle System 1 / Donchian values "
        "and were not chosen from any result; H=240 is the harness maximum (about the 10-day horizon of the Turtle "
        "System 1 exit). No profit/loss, R or win rate of this rule or any variant was computed or seen; only "
        "'eval_proposal.py check' (causality and event counts) was run before registration. The Turtle 'skip a breakout "
        "if the previous one was a winner' filter is NOT used (it depends on trade outcomes). Weights: with risk = 2N, "
        "a position sized 1/N (Turtle unit, or MOP's 40%/sigma scaling) earns a dollar P&L proportional to R, so the "
        "equal-weight mean R already is the inverse-volatility-weighted result; an extra w = 1/vol would double-count. "
        "Bar times look like UTC (FX/metals weeks start with 1-3 Sunday bars at 21-23h), hence the Sunday->Monday fold "
        "so that no 1-3 bar 'day' enters the 20-day channel or N."
    ),
)


def generate(df, name):
    n = len(df)
    if n < 3:
        return pd.DataFrame(columns=["i", "side", "ext", "atr"])
    idx = df.index
    day = idx.normalize()
    if name != "SOLUSDT_H1":                                   # FX / metals: Sunday session belongs to Monday
        day = day + pd.to_timedelta((idx.dayofweek == 6).astype(int), unit="D")
    day = pd.DatetimeIndex(day)

    D = df.groupby(day).agg(high=("high", "max"), low=("low", "min"), close=("close", "last"))
    pc = D.close.shift(1)
    tr = pd.concat([D.high - D.low, (D.high - pc).abs(), (D.low - pc).abs()], axis=1).max(axis=1)
    # values for day d use only days strictly before d (completed days)
    U_d = D.high.rolling(N_DAYS).max().shift(1)
    L_d = D.low.rolling(N_DAYS).min().shift(1)
    N_d = tr.rolling(ATR_DAYS).mean().shift(1)

    U = U_d.reindex(day).values.astype(float)
    L = L_d.reindex(day).values.astype(float)
    N = N_d.reindex(day).values.astype(float)
    c = df.close.values.astype(float)
    A = atr(df).values.astype(float)                            # H1 ATR14 known at the close of bar i

    cp = np.r_[np.nan, c[:-1]]                                  # close of bar i-1
    ok = np.isfinite(U) & np.isfinite(L) & np.isfinite(N) & (N > 0) & np.isfinite(A) & np.isfinite(cp)
    with np.errstate(invalid="ignore"):
        long_ = ok & (c > U) & (cp <= U)
        short = ok & (c < L) & (cp >= L)

    rows = []
    for i in np.flatnonzero(long_ | short):
        side = 1 if long_[i] else -1
        ext = c[i] - side * STOP_N * N[i]
        rows.append((idx[i], int(i), side, ext, A[i]))
    E = pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"]).set_index("t")
    return E
