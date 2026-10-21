"""R2C: multi-horizon time-series-momentum composite (20/60/120 trading days), daily decision, ~weekly hold,
signal-strength sizing.

Fixed rule, no tuned parameters. The look-backs 20/60/120 trading days are the canonical 1-, 3- and 6-month TSMOM
horizons (Hurst, Ooi & Pedersen combine 1/3/12-month signals; 12 months does not fit the shortest history, so 6 months
is the longest canonical horizon used), sigma over 60 trading days (~3 months, the standard ex-ante vol window of
Moskowitz, Ooi & Pedersen / Baltas & Kosowski), weight cap 2.0 (a standard winsorisation of a z-type score), N and the
2N stop are the Turtle values already used by TSMOM1, H = 120 H1 bars (~5 trading days, one week). All constants were
fixed before the first 'check'. No profit/loss, R or win rate of this rule or any variant was computed.
"""
import numpy as np
import pandas as pd

from sweep_lib import atr  # noqa: F401  (kept for protocol parity; atr column is the daily N, see desc)

LOOKBACKS = (20, 60, 120)   # 1-, 3-, 6-month TSMOM horizons in completed trading days
VOL_DAYS = 60               # ex-ante volatility window (completed daily log returns)
W_CAP = 2.0                 # cap on |S| used as position weight
ATR_DAYS = 20               # Turtle N: mean daily true range of the last 20 completed days
STOP_N = 2.0                # Turtle 2N stop

PREREG = dict(
    id="R2C",
    family="time-series momentum, multi-horizon volatility-scaled composite (follow), signal-strength sizing",
    desc=(
        "Trading days are built from H1 bars exactly as in TSMOM1: the calendar date of the bar time, except that for "
        "USDJPY, XAUUSD and XAGUSD (closed on Saturday) bars stamped on a Sunday belong to the following Monday's day; "
        "SOLUSDT uses plain calendar dates. Daily close C[d] = last H1 close of trading day d; daily high/low = max/min "
        "of the day's H1 highs/lows; daily log return r[d] = ln(C[d]/C[d-1]); daily true range TR[d] = max(H-L, "
        "|H-C[d-1]|, |L-C[d-1]|). SIGNAL BAR i = the FIRST H1 bar of each trading day d; it uses only completed days up "
        "to d-1 (computed on daily bars, then shifted one trading day). sigma = sample std (ddof=1) of the 60 daily log "
        "returns r[d-60..d-1]. For L in {20, 60, 120}: s_L = ln(C[d-1]/C[d-1-L]) / (sigma * sqrt(L)). Composite S = "
        "arithmetic mean of s20, s60, s120 (NaN if any is NaN, i.e. fewer than 121 completed days). N = mean TR over the "
        "20 completed days d-20..d-1. Skip the day if S is NaN or 0, or N is not > 0. side = sign(S) (+1 long, -1 short); "
        "weight w = min(|S|, 2.0); stop anchor ext = close[i] - side*2*N (pad = 0, so the stop is 2N from the signal "
        "close, Turtle 2N); atr = N. No take-profit. Enter at the open of bar i+1, exit at the close of the 120th bar "
        "after entry (H = 120, about one trading week) or at the stop. Signals within 120 bars of a taken signal are "
        "ignored (harness thinning), so the system holds at most one position, re-decided about every 5 (SOL, USDJPY) to 6 (metals, ~23 bars/day) trading days, "
        "always in the direction of the composite trend. Same rule and constants on all four instruments; the result "
        "is the w-weighted mean R."
    ),
    rationale=(
        "Time-series momentum is one of the best-replicated effects in futures, FX and commodities (Moskowitz, Ooi & "
        "Pedersen 2012; Hurst, Ooi & Pedersen's century of evidence; Baltas & Kosowski 2013). Mechanisms: (1) slow "
        "diffusion of and initial under-reaction to fundamental information (policy divergence and carry for USDJPY, "
        "real rates / USD and industrial demand for gold and silver, capital flows into crypto), followed by delayed "
        "over-reaction as trend followers and momentum investors join; (2) pro-cyclical hedging and risk flows (dealer "
        "short-gamma hedging, stop-outs, margin calls, CTA vol-targeted rebalancing) that push price further in the "
        "trend direction; (3) the trend premium is a compensation/behavioural effect that is broad-based across "
        "horizons rather than tied to a single look-back, so averaging the 1-, 3- and 6-month vol-scaled returns "
        "diversifies horizon risk (Hurst-Ooi-Pedersen's equal-weight 1/3/12-month blend) and reduces whipsaw at any "
        "single channel boundary. Volatility-scaling (dividing by sigma*sqrt(L)) turns each horizon's return into a "
        "t-statistic-like trend strength that is comparable across instruments and regimes; Baltas & Kosowski show "
        "continuous, strength-weighted signals beat sign-only signals because weak, noisy trends (|S| near 0) carry "
        "little expected drift but full risk, so sizing by w = min(|S|, 2) puts capital where the trend evidence is "
        "strong and caps the weight where very large scores mostly reflect a just-completed spike. Daily re-decision "
        "with a weekly hold keeps the system always aligned with the current composite trend (unlike TSMOM1's event "
        "entries), gives 3-4x as many independent trades, and the 2N daily stop keeps round-trip cost at about 0.01R "
        "(USDJPY 1 pip vs ~150+ pips of risk), removing the cost failure mode of the H1 sweep fades (fact (a))."
    ),
    novelty=(
        "All 24 REPORT.md methods react to intraday liquidity levels (PDH/PDL, PWH/PWL, Asian range, fractals, round "
        "numbers, clusters, equal highs/lows) with H1-ATR stops, 24-48 bar horizons and mostly FADE. Among the batch, "
        "TSMOM1 enters only on a fresh first H1 close beyond the 20-day Donchian channel (event-driven, sign-only, w=1, "
        "H=240); R2B uses a TSMOM direction only as a filter on FVG retests with H1 stops. R2C is the first method that "
        "(i) takes a position every trading-day decision point regardless of any price event (a continuous TSMOM "
        "rule), (ii) combines three look-back horizons into one volatility-scaled score, and (iii) uses the untried "
        "sizing modality w (signal-strength weight) instead of equal weight."
    ),
    data_informed=True,
    H=120,
    pad=0.0,
    notes=(
        "data_informed=True: the lens (multi-horizon TSMOM with weekly hold and strength sizing) was proposed by the lead "
        "after seeing round-1 results, in particular that TSMOM1 was the strongest round-1 result (gold/silver P5 true, "
        "SOL and XAU positive) while its n at H=240 could not reach P3. The constants are canonical literature values "
        "(see module docstring) and were not chosen from any result; no P&L/R/win rate of this rule or any variant was "
        "computed; only 'eval_proposal.py check' (causality + counts) was run. Known risks disclosed in advance: "
        "(1) TSMOM1 signals were 61-68% long on all four instruments, so SOL/gold gains may be drift capture; the check "
        "output reports the long share, and results should also be read by side. (2) USDJPY 2016-20 was range-bound and "
        "TSMOM1 lost there (-0.25R); that era is the likely P2 failure. Bar times are broker server time (not "
        "necessarily UTC); the trading-day construction follows TSMOM1 for comparability. R is measured in units of "
        "2N, so an equal-R trade is already inverse-volatility sized; w adds only the trend-strength tilt."
    ),
)


def generate(df, name):
    n = len(df)
    cols = ["i", "side", "ext", "atr", "w"]
    if n < 3:
        return pd.DataFrame(columns=cols)
    idx = df.index
    day = idx.normalize()
    if name != "SOLUSDT_H1":                                   # FX / metals: Sunday session belongs to Monday
        day = day + pd.to_timedelta((idx.dayofweek == 6).astype(int), unit="D")
    day = pd.DatetimeIndex(day)

    D = df.groupby(day).agg(high=("high", "max"), low=("low", "min"), close=("close", "last"))
    C = D.close.astype(float)
    pc = C.shift(1)
    tr = pd.concat([D.high - D.low, (D.high - pc).abs(), (D.low - pc).abs()], axis=1).max(axis=1)
    lr = np.log(C).diff()
    sigma = lr.rolling(VOL_DAYS).std()                          # std of the 60 returns ending at day d (ddof=1)
    s = [np.log(C / C.shift(L)) / (sigma * np.sqrt(L)) for L in LOOKBACKS]
    S_raw = (s[0] + s[1] + s[2]) / 3.0                          # NaN if any horizon is NaN
    # values for day d use only completed days up to d-1
    S_d = S_raw.shift(1)
    N_d = tr.rolling(ATR_DAYS).mean().shift(1)

    dv = day.values
    first = np.r_[True, dv[1:] != dv[:-1]]                     # first H1 bar of each trading day
    S = S_d.reindex(day).values.astype(float)
    N = N_d.reindex(day).values.astype(float)
    c = df.close.values.astype(float)

    with np.errstate(invalid="ignore"):
        ok = first & np.isfinite(S) & (S != 0) & np.isfinite(N) & (N > 0) & np.isfinite(c)
    pos = np.flatnonzero(ok)
    if len(pos) == 0:
        return pd.DataFrame(columns=cols)
    side = np.sign(S[pos]).astype(int)
    E = pd.DataFrame(
        {
            "i": pos.astype(int),
            "side": side,
            "ext": c[pos] - side * STOP_N * N[pos],
            "atr": N[pos],
            "w": np.minimum(np.abs(S[pos]), W_CAP),
        },
        index=idx[pos],
    )
    E.index.name = "t"
    return E
