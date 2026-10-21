"""MTF1: daily-trend pullback that sweeps the prior day's extreme on H1 and reclaims it (with-daily-trend stop-run fade).

Multi-timeframe rule. Daily (D1) structure from CLOSED daily bars built from the H1 data: the 20-day trend (last completed
daily close vs its 20-day SMA), the prior day's high/low, and the 14-day daily ATR. H1 trigger: the FIRST breach of the
prior day's low (uptrend) / high (downtrend) in the current trading day, by a bar that wicks through it and closes back
on the trend side. Stop buffer is scaled to the daily timeframe (0.25 x daily ATR beyond the sweep wick).

Clock (fixed assumption, from timestamps only, never from prices/returns): bar times are UTC (USDJPY reopens Sunday
21:00/22:00 UTC = 17:00 New York, metals Sunday 22:00/23:00 UTC, last bar of the week opens 16:00 New York; SOLUSDT is
Binance data in UTC). FX and metals daily bars use the market's New York 17:00 close (DST-aware, America/New_York), SOL
uses the UTC calendar day (Binance daily candle). Deterministic calendar mapping: causal.

Fixed rule; every constant is a textbook/protocol default chosen before any check was run (20-day SMA trend, 14-day ATR,
0.25 ATR pad, 24-bar exit, >= 12 H1 bars for a daily bar to count). No variant was counted or evaluated.
"""
import numpy as np
import pandas as pd

TREND_DAYS = 20     # daily trend: completed close vs 20-day SMA of completed daily closes (one trading month)
ATR_DAYS = 14       # daily ATR (Wilder's default length), simple mean of daily true range
MIN_BARS = 12       # a trading day needs >= 12 H1 bars to count as a daily bar (drops holiday / data-gap stubs)
NY_SHIFT_H = 7      # New York 17:00 close -> 00:00 of the next trading day

PREREG = dict(
    id="MTF1",
    family="multi-timeframe: daily-trend pullback, H1 sweep-and-reclaim of the prior-day extreme",
    desc=(
        "Trading day of each H1 bar: USDJPY/XAUUSD/XAGUSD: convert the bar open time from UTC to America/New_York "
        "(DST-aware), add 7 hours, take the date (day runs 17:00-17:00 New York; Sunday-evening bars belong to Monday). "
        "SOLUSDT: UTC calendar date. Daily bar of a trading day = open of its first H1 bar, max high, min low, close of its "
        "last H1 bar; only days with >= 12 H1 bars are valid daily bars. For an H1 bar i in trading day d, 'prior day' P = "
        "the most recent VALID trading day strictly before d (always fully closed before bar i opens). From the valid "
        "daily series up to and including P: PDH = high[P], PDL = low[P]; SMA20 = mean of the 20 valid daily closes ending "
        "at P; ATRd = mean over the 14 valid days ending at P of the daily true range max(H-L, |H-Cprev|, |L-Cprev|) "
        "(Cprev = previous valid daily close). Daily uptrend if close[P] > SMA20, downtrend if close[P] < SMA20 (needs 20 "
        "and 15 valid days of history). LONG signal at H1 bar i: daily uptrend AND open[i] > PDL AND low[i] < PDL AND "
        "close[i] > PDL (the bar wicked through the prior-day low and closed back above it) AND no earlier H1 bar of the "
        "same trading day d traded below PDL (min low of bars of day d before i >= PDL; the first bar of the day passes), "
        "i.e. only the first breach of the prior-day low in the day. SHORT signal: mirror image in a daily downtrend with "
        "PDH (open[i] < PDH, high[i] > PDH, close[i] < PDH, max high of earlier bars of day d <= PDH). Only the trend side "
        "is ever traded (PDL sweeps in uptrends, PDH sweeps in downtrends), so there is at most one signal per trading "
        "day. Enter at the open of bar i+1. Stop anchor ext = the sweep wick (low[i] for longs, high[i] for shorts); "
        "atr column = ATRd (daily ATR known at the close of bar i); pad = 0.25, so stop = wick -/+ 0.25 x daily ATR14. "
        "No take-profit; exit at the close of the 24th bar after entry (H=24) or at the stop. Size 1. Same rule, clock "
        "and constants on all four instruments; signals within 24 bars of a taken signal are dropped by harness thinning."
    ),
    rationale=(
        "Two well-documented horizon effects line up in this setup: time-series momentum at daily-to-monthly horizons "
        "(Moskowitz, Ooi & Pedersen 2012; Menkhoff, Sarno, Schmeling & Schrimpf 2012 for FX) and short-horizon reversal "
        "of liquidity-driven moves (Lehmann 1990; Jegadeesh 1990; Grossman & Miller 1988 temporary price impact). A drop "
        "to the prior day's low while the daily trend is up is a one-day counter-move against the slower trend, the "
        "classic 'buy the pullback in a trend' configuration. The prior day's high/low is the most widely watched "
        "intraday reference on every daily chart; protective sell stops of existing longs and breakout-sell orders "
        "cluster just beyond it (Osler 2003, 2005: stop-loss orders cluster beyond salient price levels and execute as "
        "market orders). The first breach of the day converts that resting liquidity into market sells; when the same "
        "hour closes back above the level the triggered supply was absorbed without acceptance below, and inside a daily "
        "uptrend the absorbing side is the slower trend-following / dip-buying money whose demand persists for days. "
        "With the stop-loss supply exhausted the next day should drift with the daily trend. The trade is a daily-"
        "timeframe idea (daily trend, daily level), so its invalidation buffer is a quarter of a DAILY ATR beyond the "
        "wick: an hourly-scaled buffer sits inside one hour's noise, and the hold is one trading day (24 H1 bars)."
    ),
    novelty=(
        "S1-S13/N-series used H1-only context. S2 used previous-day/week levels with no trend condition, any breach, "
        "UTC-calendar days and a 0.25 x H1-ATR stop; S7/S8 conditioned on the 24-bar H1 return. MTF1 conditions on the "
        "DAILY trend from closed D1 bars (close vs 20-day SMA), trades only the trend-side level (PDL in uptrends, PDH in "
        "downtrends), only the first breach of that level in the trading day (stops still resting), builds FX/metal "
        "daily bars on the New York 17:00 close rather than UTC midnight, and scales the stop buffer to the daily ATR "
        "(0.25 x daily ATR is about 1.4 H1 ATRs, ~6x the 0.25 x H1-ATR pad), with no take-profit at the opposite pool (unlike N1). A daily-uptrend PDL "
        "sweep is usually AGAINST the 24h move, so it is neither S7 nor a plain S8 subset."
    ),
    data_informed=True,
    data_informed_note=(
        "Disclosed: the daily-ATR stop buffer was chosen knowing fact (a) (sweep fades have a small gross edge erased by "
        "costs because 0.25 x H1-ATR stops are ~0.8 ATR away and 77-80% are hit), and the designer had seen that S8 "
        "(against the 24h trend) printed less negative than S7 (with it). No result of this rule or any variant was "
        "computed; constants are defaults fixed before the first check."
    ),
    H=24,
    pad=0.25,
)


def _trading_day(idx, name):
    """Naive Timestamp (midnight) labelling the trading day of each bar. Calendar mapping only (causal)."""
    if name == "SOLUSDT_H1":
        return idx.normalize()
    ny = idx.tz_localize("UTC").tz_convert("America/New_York") + pd.Timedelta(hours=NY_SHIFT_H)
    return ny.normalize().tz_localize(None)


def generate(df, name):
    o, h, l, c = df.open, df.high, df.low, df.close
    day = pd.Series(_trading_day(df.index, name), index=df.index)

    # daily bars from H1 (aggregates of a day are only ever used for days strictly before the bar's own day)
    D = df.groupby(day.values).agg(open=("open", "first"), high=("high", "max"), low=("low", "min"),
                                   close=("close", "last"), n=("close", "size")).sort_index()
    V = D[D.n >= MIN_BARS].copy()
    prev_c = V.close.shift(1)
    tr = pd.concat([V.high - V.low, (V.high - prev_c).abs(), (V.low - prev_c).abs()], axis=1).max(axis=1)
    tr[prev_c.isna()] = np.nan
    V["atrd"] = tr.rolling(ATR_DAYS).mean()
    V["sma"] = V.close.rolling(TREND_DAYS).mean()
    V["up"] = V.close > V.sma
    V["dn"] = V.close < V.sma

    # for every trading day d (valid or not) take the most recent VALID day strictly before d
    days = D.index.values
    pos = np.searchsorted(V.index.values, days, side="left") - 1      # index into V of last valid day < d
    ok = pos >= 0
    pidx = np.where(ok, pos, 0)
    def prior(col, fill):
        v = V[col].values[pidx].astype(float)
        return pd.Series(np.where(ok, v, fill), index=D.index)
    P = pd.DataFrame({"pdh": prior("high", np.nan), "pdl": prior("low", np.nan), "atrd": prior("atrd", np.nan),
                      "sma": prior("sma", np.nan), "up": prior("up", 0.0), "dn": prior("dn", 0.0)})
    Pb = P.reindex(day.values)
    Pb.index = df.index
    sma_ok = Pb.sma.notna() & Pb.atrd.notna()
    up = (Pb.up > 0.5) & sma_ok
    dn = (Pb.dn > 0.5) & sma_ok

    # running extremes of EARLIER bars of the same trading day (causal: cummin/cummax shifted by one bar within the day)
    first = day.ne(day.shift(1))
    prev_min = l.groupby(day.values).cummin().shift(1).where(~first, np.inf)
    prev_max = h.groupby(day.values).cummax().shift(1).where(~first, -np.inf)

    pdl, pdh = Pb.pdl, Pb.pdh
    long_ = up & (o > pdl) & (l < pdl) & (c > pdl) & (prev_min >= pdl)
    short_ = dn & (o < pdh) & (h > pdh) & (c < pdh) & (prev_max <= pdh)
    long_, short_ = long_.fillna(False).astype(bool), short_.fillna(False).astype(bool)

    rows = []
    for mask, side in ((long_, 1), (short_, -1)):
        ii = np.flatnonzero(mask.values)
        if len(ii) == 0:
            continue
        ext = (l.values[ii] if side == 1 else h.values[ii])
        rows.append(pd.DataFrame({"i": ii, "side": side, "ext": ext, "atr": Pb.atrd.values[ii]}, index=df.index[ii]))
    if not rows:
        return pd.DataFrame(columns=["i", "side", "ext", "atr"])
    E = pd.concat(rows).sort_index()
    E = E[np.isfinite(E.atr) & (E.atr > 0)]
    return E
