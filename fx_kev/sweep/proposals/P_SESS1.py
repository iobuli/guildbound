"""SESS1: New York opening-range breakout (08:00-10:00 ET range, first close beyond it 10:00-16:00 ET, follow it).

Session clock (fixed assumption, verified only from timestamps, never from prices/returns): bar times are UTC.
Evidence: for USDJPY/XAUUSD/XAGUSD the last bar of the trading week opens at 20:00 UTC in New-York-DST weeks and
21:00 UTC in New-York-standard weeks (= 16:00 ET, i.e. the 17:00 ET Friday close) in all but a handful of holiday
weeks; USDJPY reopens Sunday 21:00/22:00 UTC (= 17:00 ET) and the metals Sunday 22:00/23:00 UTC (= 18:00 ET);
SOLUSDT is Binance data (UTC). Bars are therefore mapped to New York local time
with the IANA tz database (America/New_York, DST-aware). This is a deterministic calendar mapping: causal.
"""
import numpy as np
import pandas as pd

from sweep_lib import atr

PREREG = dict(
    id="SESS1",
    family="session opening-range breakout (New York open)",
    desc=(
        "Treat bar times as UTC and convert them to New York local time (America/New_York, DST-aware). Only Monday-Friday "
        "New York dates are used. For each New York date, the opening range (OR) is the high/low of the two H1 bars that "
        "open at 08:00 and 09:00 ET (i.e. 08:00-10:00 ET); both bars must exist, otherwise no trade that day. Then scan, in "
        "time order, the bars of the same date that open at 10:00, 11:00, ..., 15:00 ET (closing by 16:00 ET). The FIRST "
        "such bar whose close is above the OR high is a long signal; the FIRST whose close is below the OR low is a short "
        "signal; only the first close outside the OR counts (at most one signal per date, none if every bar closes inside). "
        "Enter at the open of the next bar. Stop anchor ext = the opposite side of the OR (OR low for longs, OR high for "
        "shorts), stop = ext -/+ 0.25*ATR14 (ATR14 of H1 bars, known at the close of the signal bar). No take-profit; "
        "time exit at the close of the 24th bar after entry (H=24). Same rule and clock on all four instruments."
    ),
    rationale=(
        "All four instruments are USD-quoted, and 08:00-10:00 ET concentrates the main USD information events of the day: "
        "US scheduled macro releases at 08:30 ET (payrolls, CPI, retail sales, claims), the COMEX metals open (08:20 ET) "
        "and the US cash-equity open (09:30 ET), which since 2020 is also the most active period for crypto. Evans & Lyons "
        "(2008) show that a large share of the price effect of macro news reaches FX through order flow in the hours after "
        "the release, so price discovery is incomplete when the opening window ends; Gao, Han, Li & Zhou (2018, JFE) show "
        "the early part of the trading day predicts the rest of the day (late-informed and infrequent-rebalancing traders), "
        "with similar intraday momentum reported for crypto (Shen, Urquhart & Wang 2022) and commodity futures; the opening-"
        "range breakout is the classic implementation (Crabel 1990; Zarattini & Aziz 2023). A full H1 close outside the "
        "range after the window means post-window flow is confirming the opening reaction. The stop sits at the far side of "
        "the whole 2-hour range (typically 2-3 H1 ATR from entry), so the trade is wrong only if the entire opening reaction "
        "is reversed, and round-trip costs are a much smaller fraction of 1R than for 0.8-ATR sweep stops. Parameters are "
        "canonical, not tuned: the OR is the two clock hours containing 08:30 and 09:30 ET, the breakout window is the rest "
        "of the US cash session to 16:00 ET, pad 0.25 ATR and H=24 are the protocol defaults, ATR14 is standard."
    ),
    novelty=(
        "None of S1-S13, K1-K4 or N1-N8 uses a clock-anchored session range or trades a full close beyond a level in the "
        "breakout direction: those methods fade or follow wick-sweeps (wick through, close back inside) of previous "
        "day/week extremes, the 00-07h Asian range, fractal swings or round numbers, with stops 0.25 ATR beyond the wick. "
        "SESS1's level is the same-day New York 08:00-10:00 ET range (DST-correct), the trigger is a close outside it, and "
        "the stop is the opposite side of that range. It is not an inversion of any failed method."
    ),
    data_informed=False,   # designed from the cited literature and the session clock only; the designer had read the
                           # REPORT (sweep fades: small gross edge erased by costs; fades toward the opposite pool lose),
                           # but no parameter or direction was chosen from any price/return result.
    H=24,
    pad=0.25,
)

OR_HOURS = (8, 9)              # ET bar-open hours forming the opening range (08:00-10:00 ET)
BRK_FIRST, BRK_LAST = 10, 15   # ET bar-open hours in which a breakout close is accepted (bars closing 11:00..16:00 ET)


def generate(df, name):
    if len(df) == 0:
        return pd.DataFrame(columns=["i", "side", "ext", "atr"])
    et = df.index.tz_localize("UTC").tz_convert("America/New_York")
    hr = np.asarray(et.hour)
    wd = np.asarray(et.dayofweek)
    dkey = np.asarray(et.year) * 10000 + np.asarray(et.month) * 100 + np.asarray(et.day)   # New York calendar date
    h, l, c = df.high.values, df.low.values, df.close.values
    a = atr(df).values                                    # ATR14 including bar i: known at the close of bar i
    rows = []
    cur = None
    for i in range(len(df)):                              # single forward pass: state at i uses bars 0..i only
        if dkey[i] != cur:
            cur = dkey[i]; or_hi = -np.inf; or_lo = np.inf; seen = set(); done = False
        if wd[i] >= 5:
            continue
        hh = hr[i]
        if hh in OR_HOURS:
            or_hi = max(or_hi, h[i]); or_lo = min(or_lo, l[i]); seen.add(hh)
            continue
        if done or not (BRK_FIRST <= hh <= BRK_LAST) or len(seen) < len(OR_HOURS):
            continue
        if c[i] > or_hi:
            side, ext = 1, or_lo
        elif c[i] < or_lo:
            side, ext = -1, or_hi
        else:
            continue
        done = True                                       # only the first close outside the OR counts
        if np.isfinite(a[i]) and a[i] > 0:
            rows.append((df.index[i], i, side, ext, a[i]))
    E = pd.DataFrame(rows, columns=["t", "i", "side", "ext", "atr"])
    if E.empty:
        return pd.DataFrame(columns=["i", "side", "ext", "atr"])
    return E.set_index("t")
