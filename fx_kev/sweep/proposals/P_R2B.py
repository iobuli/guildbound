"""R2B: trend-aligned displacement retest. FVG1 (registered, sha256 cd1f4a60...) signals kept only when they agree with
the sign of the 20-day (480 H1 bar) time-series momentum.

Fixed rule, no tuned parameters. FVG1's event definition and its constants (0.5 ATR gap, 120-bar gap life, first-retest
rejection, ext, tie rules, H=24, pad=0.25) are copied unchanged; the only addition is the canonical one-month (20 trading
day) time-series-momentum sign of Moskowitz, Ooi & Pedersen (2012), measured on H1 bars as a 480-bar log return.
"""
import numpy as np
import pandas as pd

from sweep_lib import atr

MIN_GAP_ATR = 0.5   # (FVG1) gap height must be at least half of the pre-displacement ATR(14)
MAX_AGE = 120       # (FVG1) a gap stays live for one trading week of H1 bars; untouched older gaps are dropped
TREND_BARS = 480    # 20 trading days x 24 H1 bars (FX/metals); 20 calendar days for 24/7 SOLUSDT

PREREG = dict(
    id="R2B",
    family="fair-value-gap retest x time-series momentum (trend-aligned continuation)",
    desc=(
        "Step 1 (FVG1, unchanged): generate the registered FVG1 signals exactly as in P_FVG1.py (sha256 cd1f4a60...). "
        "Bullish FVG formed at bar k: low[k] > high[k-2]; middle bar k-1 bullish (close>open) and spanning the gap "
        "(low[k-1] <= high[k-2] and high[k-1] >= low[k]); gap height low[k]-high[k-2] >= 0.5*ATR14 at the close of bar k-2. "
        "Track for at most 120 bars from k+1; the FIRST bar i>k with low[i] <= low[k] is the only retest: if close[i] > low[k] "
        "-> LONG candidate at the open of bar i+1, otherwise the gap is mitigated and dropped. ext = min(low[k-2], low[k-1], "
        "low[i]). Bearish mirror (high[k] < low[k-2], bearish spanning middle bar, first retest high[i] >= high[k] with "
        "close[i] < high[k] -> SHORT candidate, ext = max(high[k-2], high[k-1], high[i])). Several same-side retests on one "
        "bar: the most recently formed gap defines ext; long and short retests on the same bar: no trade. "
        "Step 2 (trend filter, new): trend[i] = sign(ln close[i] - ln close[i-480]) computed on H1 bar positions (480 H1 "
        "bars = about 20 trading days for USDJPY/XAUUSD/XAGUSD and 20 calendar days for SOLUSDT). Keep the FVG1 signal at "
        "bar i only if side == trend[i]; drop it if trend[i] is 0 or undefined (i < 480 or non-finite closes). "
        "Trade: enter at the open of bar i+1, stop = ext - side*0.25*ATR14(at close of i), exit at the close of bar i+24 "
        "or at the stop; no take-profit; weight w = 1. Harness thinning (signals within 24 bars of a taken one ignored) "
        "applies as for FVG1."
    ),
    rationale=(
        "A three-bar fair-value gap is the footprint of an aggressive parent order (meta-order) that consumed liquidity "
        "faster than it was replenished; a first retest that wicks into the gap and closes back outside it shows that the "
        "same participant (or the passive interest it left behind) is still defending price. Meta-order persistence is "
        "not homogeneous: order flow that is aligned with the slow, multi-week direction of the market is more likely to "
        "come from slow-moving capital (real-money rebalancing, macro/carry positioning, CTA and other trend-following "
        "flows, hedgers adjusting over days) whose execution is spread over many hours and days, so the unfilled "
        "remainder keeps working after the retest. Time-series momentum itself is attributed to under-reaction and to "
        "this gradual capital deployment (Moskowitz, Ooi & Pedersen 2012; Hurst, Ooi & Pedersen). Counter-trend "
        "displacements, by contrast, are more often liquidity events (stop runs, short-covering, news spikes against the "
        "prevailing flow) whose impact decays and reverts, so their retests should hold less often. Conditioning a "
        "short-horizon continuation entry on the sign of the slow trend therefore combines two weakly informative, "
        "largely independent signals (a three-bar intraday imbalance and a 20-day return) that point the same way, which "
        "should raise the per-trade edge of the continuation over the following 24 hours."
    ),
    novelty=(
        "S1-S13, K1-K3 and N1-N8 fade or follow wicks through reference liquidity levels or use Kev; none uses imbalances "
        "or multi-week momentum. S7 (sweep with the 24h trend) and round-1 MTF1 combine a trend with a REVERSAL entry (a "
        "sweep fade) and both failed. R2B combines a slow trend (20-day TSMOM sign, not a 24h trend) with a CONTINUATION "
        "entry (first rejected retest of a displacement gap), i.e. both components point in the direction of the "
        "prevailing order flow. Round-1 FVG1 is the unfiltered event set; round-1 TSMOM1 trades Donchian breakouts with a "
        "2N stop and 240-bar hold, whereas R2B uses the momentum sign only as a filter on 24-bar FVG trades."
    ),
    data_informed=True,
    H=24,
    pad=0.25,
    notes=(
        "Round-2 lens, data-informed: chosen because round-1 FVG1 and TSMOM1 were each reported positive on 3 of 4 "
        "instruments (FVG1: USDJPY, SOL, XAU; TSMOM1: SOL, XAU, XAG). No profit/loss, R or win rate of this rule or any "
        "variant was computed by the designer; only 'eval_proposal.py check' (causality, counts, long share) was run. "
        "Parameters are not tuned: everything from FVG1 is copied verbatim and the lookback is the canonical one-month "
        "(20 trading day) TSMOM horizon expressed in H1 bars (480 = 20 x 24); no alternative lookback was examined. "
        "Bar positions are used instead of calendar time, so weekends and holidays are skipped for FX/metals (metals "
        "trade about 23 bars a day, so 480 bars is about 21 trading days). Expected risk: FVG1's tight stop is kept "
        "(cost/R about 0.08-0.11), so the filter must raise the per-trade gross edge substantially for P1/P3."
    ),
)


def generate(df, name):
    E = _fvg1_generate(df, name)
    if len(E) == 0:
        return pd.DataFrame(columns=["i", "side", "ext", "atr"])
    c = df.close.values.astype(float)
    n = len(c)
    with np.errstate(divide="ignore", invalid="ignore"):
        lc = np.log(c)
        mom = np.full(n, np.nan)
        if n > TREND_BARS:
            mom[TREND_BARS:] = lc[TREND_BARS:] - lc[:-TREND_BARS]    # ln close[i] - ln close[i-480], causal
    tr = np.where(np.isfinite(mom), np.sign(mom), 0.0)
    ii = E["i"].values.astype(int)
    keep = (tr[ii] != 0) & (E["side"].values.astype(int) == tr[ii])
    return E.loc[keep].copy()


def _fvg1_generate(df, name):
    """Verbatim copy of the registered FVG1 generate() (P_FVG1.py, sha256 cd1f4a60...)."""
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
