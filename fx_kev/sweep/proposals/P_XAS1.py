"""XAS1: cross-asset residual reversal (single-leg pairs trade on a hedge-regression residual).

Fixed rule, constants chosen a priori (see PREREG notes). The second instrument is loaded inside generate() and only
partner bars that have CLOSED by the close of the signal bar are used (see _partner_close).
"""
import numpy as np
import pandas as pd

import sweep_lib
from sweep_lib import atr

PARTNER = {                      # economic link, fixed before any check was run
    "XAUUSD_H1": "XAGUSD_H1",    # gold  <- silver   (same macro drivers: USD, real yields, risk sentiment)
    "XAGUSD_H1": "XAUUSD_H1",    # silver <- gold
    "USDJPY_H1": "XAUUSD_H1",    # USDJPY <- gold    (US real yields / USD, safe-haven flows; negative beta)
    "SOLUSDT_H1": "XAGUSD_H1",   # SOL   <- silver   (the pro-cyclical / risk-sensitive metal; weak link, see notes)
}
K = 24          # divergence window: one day of H1 bars
W = 480         # hedge-regression / residual-volatility window: about 20 trading days of H1 bars
Z_IN = 2.0      # entry: standardized 24-bar residual crosses +-2 sigma (Gatev-Goetzmann-Rouwenhorst 2-sigma rule)
Z_STOP = 1.0    # stop: a further 1 sigma of 24-bar residual beyond the signal close (residual 2 -> 3 sigma)

PREREG = dict(
    id="XAS1",
    family="cross-asset relative value / residual (pairs) reversal, single leg",
    desc=(
        "Each instrument has one fixed partner: gold->silver, silver->gold, USDJPY->gold, SOLUSDT->silver. "
        "Partner alignment: for main bar i (open time t_i, closes at t_i+1h) the partner price Pc[i] is the close of the "
        "LAST partner bar whose open time is <= t_i, i.e. a partner bar that has closed at or before the close of bar i "
        "(as-of / backward merge on bar open times; partner bars after t_i are never used). Bar i is 'fresh' only if the "
        "partner has a bar with open time exactly t_i (partner market open at the signal hour). "
        "r[i] = log(close[i]/close[i-1]) of the main instrument, f[i] = log(Pc[i]/Pc[i-1]) (0 when the partner did not trade). "
        "At every bar j, over the W=480 main bars j-479..j: beta[j] = cov(r,f)/var(f) (OLS slope of r on f; the "
        "intercept/drift is not used) and residual vol s[j] = sqrt(var(r) - cov(r,f)^2/var(f)) (std of r - beta*f; sample "
        "moments). The estimation window is lagged by K=24 bars so it does not overlap the divergence window: "
        "b[i] = beta[i-24], sd[i] = s[i-24]. 24-bar residual: e[i] = log(close[i]/close[i-24]) - b[i]*log(Pc[i]/Pc[i-24]); "
        "z[i] = e[i] / (sd[i]*sqrt(24)). "
        "SHORT signal at bar i if z[i] >= 2 and z[i-1] < 2 (first bar whose 1-day move is 2 sigma richer than the partner "
        "implies); LONG signal if z[i] <= -2 and z[i-1] > -2. Bar i must be fresh and all quantities finite with var(f) > 0. "
        "Enter at the open of bar i+1. Stop (ext, pad=0) = close[i]*exp(-side*1.0*sd[i]*sqrt(24)), i.e. one further "
        "24-bar residual sigma against the position measured from the signal close (the residual going from 2 to 3 sigma "
        "if the partner stays put). No take-profit. Exit at the close of bar i+24 or at the stop. Weight w = 1. "
        "atr = ATR14 of the main instrument at the close of bar i (used only by the harness's minimum-risk check). "
        "Signals within 24 bars of a taken signal are ignored (harness thinning)."
    ),
    rationale=(
        "Gold and silver share their main drivers (USD, US real yields, risk appetite; H1 return correlation about 0.74), "
        "and USDJPY and gold are linked through the same US real-yield/USD channel and opposite safe-haven roles "
        "(correlation about -0.35). A one-day move of one market that its linked market does NOT confirm is, on average, "
        "less likely to be news about the shared fundamental and more likely to be market-specific liquidity demand: a "
        "large parent order or ETF creation/redemption flow in one metal, option-hedging or stop cascades in one "
        "contract, positioning squeezes in one futures market. Liquidity providers and relative-value desks that trade "
        "the gold/silver ratio or gold-in-yen absorb that flow only at a price concession, which is repaid as the "
        "deviation decays (Grossman & Miller 1988 liquidity provision; Gatev, Goetzmann & Rouwenhorst 2006 pairs "
        "trading with a 2-sigma open rule; Avellaneda & Lee 2010 residual s-scores; Da, Liu & Schaumburg 2014 show the "
        "short-term reversal sits in the residual, not the factor component). The part of the move explained by the "
        "partner is treated as information and is NOT faded, because the residual removes it via the rolling hedge ratio. "
        "Trading one leg keeps the common-factor exposure unhedged; that adds noise but no expected loss. The stop is "
        "set in units of the residual's own daily volatility (median about 1.8-2.5 H1 ATR14, versus about 0.8 ATR for "
        "the sweep stops), so the round-trip cost is a smaller fraction of 1R, avoiding the failure mode of the sweep methods (0.8-ATR stops, costs 0.10-0.16R)."
    ),
    novelty=(
        "Only N2 (gold/silver SMT divergence) of the 24 prior methods used a second instrument, and it was a liquidity-"
        "level event: one metal swept its previous-day high/low while the other did not, faded with a stop 0.25 ATR "
        "beyond the wick. XAS1 uses no level, sweep or wick at all: the event is a statistical one, a 2-sigma 24-bar "
        "residual from a rolling 20-day hedge regression on a linked market, applied to all four instruments (USDJPY "
        "hedged with gold, SOL with silver), with a residual-volatility stop (median 1.8-2.5 ATR14 from the signal close) and a 24-bar time exit. "
        "It is not an inversion of any failed method (it trades the same convergence direction as N2, chosen from the "
        "pairs-trading prior, not from N2's result)."
    ),
    data_informed=False,
    H=24,
    pad=0.0,
    notes=(
        "Constants are canonical, not tuned: K=24 (one day), W=480 (about 20 trading days, the standard 1-month window), "
        "2-sigma entry (Gatev et al.), stop at a further 1 sigma (pairs 'spread at 3 sigma' stop), H=24 = K. Before "
        "writing the rule I looked only at INPUT properties of pre-2026 data, never at any forward return, R, P/L or win "
        "rate: (1) cross-correlations of H1 log returns at lags -3..+3 peak at lag 0 for every pair and are ~0 at +-1 "
        "(e.g. gold/silver 0.739 at lag 0, |r|<0.02 at +-1; USDJPY/gold -0.351 vs |r|<0.015), so the four series share "
        "one clock and the same open-time labelling; a partner bar with the same open time as the main bar closes at "
        "the same instant as the main bar, so using it at the signal close is causal. (2) SOL's correlation with "
        "silver/gold/USDJPY is 0.11/0.08/-0.02; silver was the a-priori choice (pro-cyclical metal) and is also the "
        "largest. Consequence: for SOL the hedge removes only ~1% of variance, so the SOL leg is close to a plain 24-bar "
        "own-move fade; USDJPY removes ~12%; the metals ~55%. CAUSALITY of the cross-asset input: the harness's "
        "corruption test only perturbs the main instrument, so the partner path is guarded by construction: partner "
        "bars are taken by searchsorted(partner open times, t_i, side='right')-1 (no partner bar with open time > t_i "
        "can be selected), all rolling windows are trailing, shifts are positive, and the partner file is loaded with "
        "include_holdout only when the main df itself extends into 2026 (pre-2026 rows are identical either way). "
        "STRUCTURAL LIMIT (please read): every partner series starts in 2018-03 (metals) or 2020-08 (SOL), so the "
        "USDJPY leg can only fire from about 2018-04 onward; the USDJPY 2011-2015 era has 0 events and P2 cannot be "
        "met by any cross-asset rule on this data. The USDJPY 2016-2020 era contains only 2018-2020 events. Extra causality test run by the designer (input-only, no "
        "outcomes): for 10 random signals per instrument, partner bars with open time > t were replaced by noise, and "
        "separately the partner series was truncated at t; signals at t were unchanged in 80/80 cases; truncating the "
        "main series mid-sample left all earlier signals identical. Check counts (non-overlapping, pre-2026): USDJPY 308 "
        "(2018-2025), SOL 233, gold 293, silver 269. Stop distance |close-ext|/ATR14 median: USDJPY 2.40, SOL 2.45, "
        "gold 1.86, silver 1.75; median rolling beta: USDJPY on gold -0.22, SOL on silver 0.34, gold on silver 0.39, "
        "silver on gold 1.51."
    ),
)

_CACHE = {}


def _partner_close(df, name):
    """Partner close as of each main bar: last partner bar with open time <= t_i (closed by the close of main bar i)."""
    other = PARTNER[name]
    hold = bool(df.index[-1] >= pd.Timestamp("2026-01-01"))
    key = (other, hold)
    if key not in _CACHE:
        _CACHE[key] = sweep_lib.load(other, include_holdout=hold)["close"].astype(float)
    p = _CACHE[key]
    pidx = p.index.values
    t = df.index.values
    pos = np.searchsorted(pidx, t, side="right") - 1          # partner open time <= t_i only
    ok = pos >= 0
    pc = np.full(len(t), np.nan)
    pc[ok] = p.values[pos[ok]]
    fresh = np.zeros(len(t), dtype=bool)
    fresh[ok] = pidx[pos[ok]] == t[ok]
    return pc, fresh


def generate(df, name):
    cols = ["i", "side", "ext", "atr"]
    if len(df) < W + 2 * K + 2:
        return pd.DataFrame(columns=cols)
    pc, fresh = _partner_close(df, name)
    c = df.close.values.astype(float)
    lc = pd.Series(np.log(c), index=df.index)
    lp = pd.Series(np.log(pc), index=df.index)
    r = lc.diff()
    f = lp.diff()

    cov = r.rolling(W).cov(f)
    vf = f.rolling(W).var()
    vr = r.rolling(W).var()
    beta = cov / vf.where(vf > 0)
    s2 = (vr - cov * cov / vf.where(vf > 0)).clip(lower=0)
    b = beta.shift(K)                                            # estimation window ends 24 bars before bar i
    sd = np.sqrt(s2).shift(K)

    e = (lc - lc.shift(K)) - b * (lp - lp.shift(K))
    z = (e / (sd * np.sqrt(K))).where(sd > 0)
    zp = z.shift(1)

    zv, zpv = z.values, zp.values
    fin = np.isfinite(zv) & np.isfinite(zpv) & fresh
    with np.errstate(invalid="ignore"):
        short = fin & (zv >= Z_IN) & (zpv < Z_IN)
        long_ = fin & (zv <= -Z_IN) & (zpv > -Z_IN)

    A = atr(df).values.astype(float)
    sdv = sd.values
    rows = []
    for i in np.flatnonzero(long_ | short):
        side = 1 if long_[i] else -1
        ext = c[i] * np.exp(-side * Z_STOP * sdv[i] * np.sqrt(K))
        rows.append((df.index[i], int(i), side, ext, A[i], zv[i], b.values[i]))
    E = pd.DataFrame(rows, columns=["t"] + cols + ["z", "beta"]).set_index("t")
    return E
