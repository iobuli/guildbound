"""VREG1: compression-box breakout, traded in the break direction with the stop beyond the far side of the box.

Turns the confirmed non-directional effect "after a 12-bar/120-bar return-std ratio < 0.4 the next 24h range is larger"
into a directional rule by letting the first close outside the compressed 12-bar box choose the direction.
Fixed rule, no tuned parameters: 12 / 120 / 0.4 are the windows and threshold of the confirmed volatility effect, the box
uses the same 12 bars as the short window, H=24 matches the 24h horizon of that effect, pad=0.25 is the protocol default.
"""
import numpy as np
import pandas as pd

from sweep_lib import atr

SHORT = 12        # short realised-vol window and box length (bars)
LONG = 120        # long realised-vol window (bars)
THRESH = 0.4      # compression threshold on std(12)/std(120)

PREREG = dict(
    id="VREG1",
    family="volatility-compression box breakout (follow)",
    desc=(
        "r = log(close).diff(). Compression ratio at bar j: q[j] = std(r[j-11..j]) / std(r[j-119..j]) (rolling sample std, "
        "known at the close of bar j). Signal bar i requires q[i-1] < 0.4 (the market was compressed at the close of the "
        "previous bar). Box = [min(low[i-12..i-1]), max(high[i-12..i-1])] (the 12 compressed bars before i). The break must "
        "happen by trading, not by a gap: boxLow <= open[i] <= boxHigh. If close[i] > boxHigh -> LONG at the open of bar "
        "i+1 with stop anchor ext = boxLow (far side of the box); if close[i] < boxLow -> SHORT at the open of bar i+1 with "
        "ext = boxHigh. Stop = ext -/+ 0.25*ATR14 (ATR known at the close of bar i). Exit at the close of bar i+24 or at the "
        "stop; no take-profit; size 1. Later signals within 24 bars of a taken signal are ignored (harness thinning), so in "
        "practice this is the first close-breakout of a compression episode."
    ),
    rationale=(
        "Realised volatility clusters and mean-reverts toward its recent norm, so a period whose 12-bar volatility is under "
        "40% of its 120-bar volatility is followed by range expansion (confirmed non-directional effect, held out-of-sample "
        "including 2026). Compression is a low-participation equilibrium: dealers hold little inventory, liquidity is thin "
        "and both breakout-entry orders and protective stops cluster just beyond the obvious, tight box edges. When a bar "
        "CLOSES outside the box (acceptance, not a wick-and-reject), the stops and entry orders on that side have been "
        "triggered and the new information or session flow that broke the balance is being priced; with order flow "
        "autocorrelated over hours and the expected expansion still ahead, the expansion should be more likely to extend in "
        "the break direction than to travel back through the whole box. The stop beyond the far side of the box is the "
        "natural falsification point (a full traverse against the break means the box was a range, not a launch pad). "
        "Because the box height is several ATR, the round-trip cost is a small fraction of 1R, addressing the main failure "
        "mode of the sweep methods (stops ~0.8 ATR, costs 0.10-0.16R)."
    ),
    novelty=(
        "No reference level (PDH/PDL, PWH/PWL, Asian range, fractal, round number) and no sweep is involved. S5 faded "
        "sweeps that happened during compression; N6 followed the wick direction of a sweep (bar closed back INSIDE a level) "
        "1-6 bars after a compression release. VREG1 requires the opposite bar type: a close OUTSIDE the compressed 12-bar "
        "box (acceptance), entered while the compression is still in force, traded in the break direction with the stop at "
        "the far side of the box (multi-ATR risk) instead of 0.25 ATR beyond a wick. It is a direction rule layered on the "
        "confirmed volatility-expansion effect rather than a level-reaction rule."
    ),
    data_informed=True,
    H=24,
    pad=0.25,
    notes=(
        "data_informed=True only because the conditioning (12/120 std ratio < 0.4, 24h horizon) is taken from the confirmed "
        "NON-directional range-expansion result (b) supplied by the lead; no profit/loss, R or win rate of this rule or any "
        "variant was computed or seen. Constants were fixed before the first 'check'; only causality and event counts were "
        "inspected before registration. The gap filter (open[i] inside the box) mirrors FVG1's exclusion of session/weekend "
        "gaps so that the break is made by trading."
    ),
)


def generate(df, name):
    o = df.open.astype(float); h = df.high.astype(float); l = df.low.astype(float); c = df.close.astype(float)
    n = len(df)
    cols = ["i", "side", "ext", "atr"]
    if n < LONG + 2:
        return pd.DataFrame(columns=cols)

    r = np.log(c).diff()
    s_short = r.rolling(SHORT).std()
    s_long = r.rolling(LONG).std()
    q = s_short / s_long.where(s_long > 0)                 # NaN when the long std is zero/undefined
    comp_prev = (q < THRESH).shift(1, fill_value=False).values.astype(bool)   # compressed at close of bar i-1

    box_hi = h.rolling(SHORT).max().shift(1).values         # max(high[i-12..i-1])
    box_lo = l.rolling(SHORT).min().shift(1).values         # min(low[i-12..i-1])
    ov, cv = o.values, c.values
    A = atr(df).values                                      # ATR14 known at the close of bar i

    ok = comp_prev & np.isfinite(box_hi) & np.isfinite(box_lo) & np.isfinite(A) & (A > 0)
    ok &= (ov >= box_lo) & (ov <= box_hi)                   # break made by trading, not by a gap
    up = ok & (cv > box_hi)
    dn = ok & (cv < box_lo)

    idx = np.flatnonzero(up | dn)
    if len(idx) == 0:
        return pd.DataFrame(columns=cols)
    side = np.where(up[idx], 1, -1)
    ext = np.where(side == 1, box_lo[idx], box_hi[idx])
    E = pd.DataFrame({"i": idx, "side": side, "ext": ext, "atr": A[idx]}, index=df.index[idx])
    E.index.name = "t"
    return E
