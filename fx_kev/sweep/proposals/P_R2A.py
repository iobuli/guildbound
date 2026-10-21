"""R2A: S1 sweep fade, taken only when the market is already in a high-volatility state before the sweep bar.

Exactly S1 (fade every sweep of PDH/PDL/PWH/PWL/ASH/ASL/SWH/SWL/RN, enter next open, stop 0.25 ATR beyond the wick,
24-bar exit, no take-profit, size 1) plus ONE filter: ATR14 known before the sweep bar must be at least 1.5x its own
rolling 2400-bar median (about 100 trading days of H1 bars). Constants are canonical and were fixed before the first
'check': 14-bar ATR (the library default), 2400-bar median window (~100 trading days), min_periods 1200 (half the window),
threshold 1.5 (a round "50% above normal" level). No variant was counted or evaluated.
"""
import numpy as np
import pandas as pd

from sweep_lib import atr, events

MED_WIN = 2400        # rolling window (bars) of the ATR baseline: ~100 trading days of H1 bars
MED_MINP = 1200       # minimum observations for the baseline (half the window)
V_MIN = 1.5           # high-volatility state: prior ATR >= 1.5x its rolling median

PREREG = dict(
    id="R2A",
    family="high-volatility sweep fade (S1 + elevated-volatility state filter)",
    desc=(
        "Events E = sweep_lib.events(df, name): every bar i that wicks through one of PDH/PDL (previous completed day "
        "high/low), PWH/PWL (previous completed week high/low), ASH/ASL (today's hours 0-6 range, usable from hour 7), "
        "SWH/SWL (latest 5-bar fractal swing, usable from pivot+3) or the round number above/below the bar's open, with "
        "open and close back on the near side of the level (upper level: high > X, open < X, close < X -> SHORT; lower "
        "level: low < X, open > X, close > X -> LONG); several levels swept in the same direction by one bar give one "
        "event, bars that swept both sides are dropped. side = E.side, ext = E.ext (the sweep bar's extreme: low for "
        "longs, high for shorts), atr = E.atr (ATR14 = 14-bar mean true range known before bar i, i.e. A[i-1]). FILTER "
        "(the only change from S1): A = sweep_lib.atr(df); v[i] = A[i-1] / median(A[i-2400 .. i-1]) where the median is "
        "a rolling 2400-bar median with min_periods 1200 (no value -> no trade); keep the event only if v[i] >= 1.5. "
        "Trade: enter at the open of bar i+1 in direction side; stop = ext - side*0.25*atr; no take-profit; exit at the "
        "close of the 24th bar after entry (H=24) or at the stop; weight w = 1. Same rule on all four instruments; "
        "overlapping signals are thinned by the harness (gap 24 bars)."
    ),
    rationale=(
        "(1) Liquidity-provision premium rises with volatility. Returns to short-term reversal / liquidity provision are "
        "strongly increasing in market volatility (Nagel 2012, RFS, 'Evaporating Liquidity'): when volatility is high, "
        "liquidity suppliers face higher inventory risk and funding constraints and withdraw, so a larger share of each "
        "price move is transient price pressure rather than information, and the expected compensation for taking the "
        "other side of that pressure rises. A sweep fade is a liquidity-provision trade: it buys the forced selling that "
        "ran the stops below a visible low (sells the forced buying above a high) after the bar has already closed back "
        "inside the level, i.e. after the pressure has started to be absorbed. If the small gross edge of S1 "
        "(+0.04..+0.10R, fact (a)) is a liquidity-provision premium, it should be concentrated in elevated-volatility "
        "states, when stop runs are larger relative to the depth available to absorb them. (2) Mechanical cost relief. "
        "Round-trip cost is a fixed price amount (USDJPY) or fixed fraction of price (others), equal to about "
        "0.067/0.072/0.108/0.107 of a median H1 ATR on USDJPY/SOL/XAU/XAG, while the S1 stop distance scales with ATR "
        "(~0.8 ATR). When ATR is at least 1.5x its 100-day median, cost per R falls by a third or more, which is "
        "precisely the drag (0.10-0.16R) that erased S1's gross edge. (3) Consistency with the volatility-state "
        "evidence: S5 (sweep fades after compression) was the worst S-variant, and fact (b) says volatility state is "
        "persistent/predictable at the 24h horizon; together they suggest the fade edge is increasing in volatility "
        "state, so the opposite (elevated) state is the natural place to look. The 2400-bar median is a slow causal "
        "baseline that adapts to each instrument's secular volatility level, so the filter selects 'unusually volatile "
        "for this market' rather than 'volatile in absolute terms'."
    ),
    novelty=(
        "Every earlier volatility condition used COMPRESSION: S5 faded sweeps during 12/120 std-ratio compression, N6 "
        "followed sweep wicks 1-6 bars after a compression release, VREG1 (batch) follows a close out of a compressed "
        "12-bar box. None conditioned on an ELEVATED volatility state. EXH1 (batch) follows a single >= 3x ATR range-shock "
        "bar closing beyond a fresh level; R2A has no single-bar shock condition, uses a slow state measure (prior ATR14 "
        "vs its 2400-bar median) and FADES ordinary S1 sweeps. S12/S13 widened the stop to 1 ATR on all events; R2A "
        "keeps S1's stop and lowers cost/R only through the state selection. S2-S11 filter by level type, multiplicity, "
        "confirmation, wick shape, trend or structure, never by volatility regime."
    ),
    data_informed=True,
    H=24,
    pad=0.25,
    notes=(
        "Round-2 lens. data_informed=True because the idea was shaped by fact (a) (S1's gross edge is smaller than "
        "costs), fact (b) (volatility state predicts the next-24h range), S5's result (compression fades were the worst "
        "S-variant) and the cost diagnostic (cost as a fraction of median ATR). Known risk: EXH1, which followed >= 3 ATR "
        "range-shock bars, was about 0 on USDJPY and the metals and +0.26 on SOL, so fading violent moves on SOL may "
        "lose. Constants (14, 2400, 1200, 1.5, H=24, pad=0.25, w=1) were fixed before the first 'check'; no profit/loss, "
        "R or win rate of this rule or any variant was computed or seen. Only 'eval_proposal.py check' (causality and "
        "counts) was run before registration."
    ),
)


def generate(df, name):
    cols = ["i", "side", "ext", "atr", "w"]
    E = events(df, name)
    if E is None or len(E) == 0:
        return pd.DataFrame(columns=cols)

    A = atr(df)                                            # A[j] = ATR14 known at the close of bar j
    A_prev = A.shift(1)                                    # A[i-1]: known before bar i opens
    base = A_prev.rolling(MED_WIN, min_periods=MED_MINP).median()   # median(A[i-2400 .. i-1])
    v = (A_prev / base.where(base > 0)).values             # NaN when the baseline is undefined

    i = E["i"].values.astype(int)
    keep = np.isfinite(v[i]) & (v[i] >= V_MIN)
    E = E[keep]
    if len(E) == 0:
        return pd.DataFrame(columns=cols)

    out = pd.DataFrame({
        "i": E["i"].values.astype(int),
        "side": E["side"].values.astype(int),
        "ext": E["ext"].values.astype(float),
        "atr": E["atr"].values.astype(float),             # ATR14 known before bar i (as in S1)
        "w": 1.0,
    }, index=E.index)
    out.index.name = "t"
    return out.dropna(subset=["atr", "ext"])
