"""EXH1: stop cascade through a fresh major level, followed in its direction.

A single H1 bar that (1) is a range shock (high-low >= 3x the ATR14 known before it), (2) opens on the near side of a
major stop level (previous day high/low, previous week high/low, or the round number next to the open) that has not yet
been traded through in the current day (week for weekly levels), (3) CLOSES beyond that level and (4) closes in the outer
half of its own range, is read as a stop-loss cascade that has converted resting stops into market orders and has been
accepted beyond the level. The trade follows it: enter at the next open, stop 0.25 ATR beyond the cascade bar's opposite
extreme, exit after 24 bars. Fixed rule, constants chosen a priori (3x ATR from fact (b), 14-bar ATR, half-range close,
0.25 ATR pad and 24-bar exit as in the protocol defaults); no variant was counted or evaluated.
"""
import numpy as np
import pandas as pd

from sweep_lib import atr, levels

SHOCK = 3.0      # cascade bar range in multiples of the ATR14 known before the bar (the range-shock definition of fact (b))
CLOSE_FRAC = 0.5  # close must be in the outer half of the cascade bar's own range (midpoint, no tuning)

PREREG = dict(
    id="EXH1",
    family="stop cascade through a fresh major level (follow)",
    desc=(
        "ATR = sweep_lib.atr(df) (14-bar mean true range). Levels from sweep_lib.levels(df, name): PDH/PDL (previous "
        "completed calendar day's high/low on the bar clock), PWH/PWL (previous completed calendar week's high/low, "
        "pandas period 'W'), RNU/RND (round number just above/below the bar's own open: USDJPY whole yen, SOL multiples of "
        "5, gold multiples of 25, silver multiples of 0.5). Signal bar i, LONG cascade: (a) range shock: high[i]-low[i] >= "
        "3.0*ATR[i-1] (ATR known at the close of the previous bar); (b) for at least one upper level X in {PDH, PWH, RNU}: "
        "open[i] < X < close[i] (opened below the level, closed above it, so the level was crossed by trading inside bar i) "
        "AND the level was still intact before bar i: the highest high of the bars of the same calendar day before bar i "
        "is < X for PDH and RNU, and the highest high of the bars of the same calendar week before bar i is < X for PWH "
        "(no earlier bar of the period -> intact); (c) close in the outer half of the bar: close[i]-low[i] >= "
        "0.5*(high[i]-low[i]). SHORT cascade is the mirror image with lower levels {PDL, PWL, RND}: high-low >= "
        "3*ATR[i-1], open[i] > X > close[i], the lowest low of the same day (week for PWL) before bar i is > X, and "
        "high[i]-close[i] >= 0.5*(high[i]-low[i]). Several levels crossed by one bar give one signal. Trade: FOLLOW the "
        "cascade (LONG after a long cascade, SHORT after a short one) at the open of bar i+1; stop anchor ext = the cascade "
        "bar's opposite extreme (low[i] for longs, high[i] for shorts), stop = ext -/+ 0.25*ATR[i] (ATR known at the close "
        "of bar i); no take-profit; exit at the close of the 24th bar after entry (H=24) or at the stop. Size 1; same rule "
        "on all four instruments; overlapping signals are thinned by the harness (gap 24 bars)."
    ),
    rationale=(
        "Resting stop-loss orders cluster just beyond the most visible reference prices: yesterday's and last week's "
        "extremes and round numbers (Osler 2003, 2005, using actual stop-loss order books of a major FX dealer: stop-loss "
        "buy orders sit just above round numbers and recent highs, take-profit orders AT them). When a stop-loss cluster "
        "is triggered, the stops become market orders in the direction of the move; dealers who fill them re-hedge in the "
        "same direction, which can trigger the next cluster, so stop-loss orders produce 'price cascades' -- trends that "
        "are faster and more persistent after a level is crossed (Osler 2005, JIMF). Two conditions separate a true "
        "cascade from a sweep (the sweep fades studied in S1-S13, N4): the bar must CLOSE beyond the level and in the "
        "outer half of its range, i.e. the flow released by the stops was absorbed and price was accepted beyond the "
        "level instead of being rejected; and the bar must be a range shock (>= 3x ATR), i.e. the order flow was large "
        "relative to normal liquidity -- the size at which an imbalance is too big to be absorbed by the passive side "
        "within the hour. The 'fresh level' filter keeps only first crossings in the period, where the stop cluster is "
        "still intact (a level already traded through earlier that day has had its stops consumed). Why the move should "
        "continue past the next open: (i) the inventory of the dealers who absorbed the cascade is left on the losing "
        "side and is worked off over the following hours, (ii) a range shock often carries information (news, "
        "intervention, liquidation) to which slower participants (real-money, risk-parity, option delta hedgers short "
        "gamma) adjust gradually, (iii) the forced sellers/buyers are not finished: positions whose stops sit further "
        "away are now closer to them. Range shocks are known (fact (b), held out-of-sample) to be followed by a larger "
        "next-24h range, so the 24-bar exit sits in the window where the price will move the most; this proposal tests "
        "whether, when the shock is a fresh-level cascade with acceptance, that move is directional. Structurally the stop "
        "is the cascade bar's far extreme, at least ~1.5 ATR from entry, so the round-trip cost is a small fraction of 1R "
        "(fact (a): the 0.8-ATR wick stops made costs 0.10-0.16R)."
    ),
    novelty=(
        "No earlier method follows a single-bar stop cascade. S1-S13, N1, N4, N5, N8 all FADE bars that wick through a "
        "level and close back inside (the opposite close condition). S11 follows a FAILED SWEEP: it needs a prior sweep "
        "bar that closed back inside, then a later close beyond the sweep's extreme within 3 bars, has no size condition "
        "and no fresh-level condition; EXH1 needs no sweep, the level is crossed and closed beyond by one bar that is "
        "itself a >= 3x ATR range shock closing in its outer half. N4 fades a second sweep within 24 bars of a first one; "
        "EXH1 requires that the level was NOT traded through earlier in the day/week, and trades in the direction of the "
        "break. N6 follows a sweep near a compression release (vol-ratio trigger, sweep bar closes back inside); EXH1 has "
        "no compression condition and uses the range-shock trigger of fact (b). TSMOM1 (batch) follows the first H1 close "
        "beyond a 20-day Donchian channel with no bar-size condition and a 2N/10-day exit; VREG1 follows a close out of a "
        "12-bar compression box; SESS1 is a New-York opening-range breakout. EXH1's levels are one-day/one-week extremes "
        "and round numbers, the trigger is the size of the crossing bar, the stop is the cascade bar's opposite extreme, "
        "and the exit is 24 bars."
    ),
    data_informed=True,
    H=24,
    pad=0.25,
    notes=(
        "data_informed=True because (1) the range-shock threshold (3x prior ATR) is taken from the lead's fact (b), an "
        "empirical non-directional result, and (2) the wide stop (cascade bar's far extreme) was chosen knowing fact (a) "
        "that 0.8-ATR stops make costs 0.10-0.16R. The trade direction (follow) comes from Osler's stop-loss cascade "
        "evidence, not from inverting any failed method's result. No profit/loss, R or win rate of this rule or any "
        "variant was computed or seen; only 'eval_proposal.py check' (causality + counts) was run before registration."
    ),
)


def generate(df, name):
    o, h, l, c = df.open, df.high, df.low, df.close
    L = levels(df, name)
    a = atr(df)                       # ATR14 known at the close of bar i
    a_prev = a.shift(1)               # ATR14 known before bar i opens
    rng = h - l
    shock = rng >= SHOCK * a_prev

    day = pd.Series(df.index.normalize(), index=df.index)
    wk = pd.Series(df.index.to_period("W").start_time, index=df.index)
    same_day = day.eq(day.shift(1))
    same_wk = wk.eq(wk.shift(1))
    # extremes of the bars of the same day / week strictly BEFORE bar i (NaN for the first bar of the period)
    day_hi_prior = h.groupby(day.values).cummax().shift(1).where(same_day)
    day_lo_prior = l.groupby(day.values).cummin().shift(1).where(same_day)
    wk_hi_prior = h.groupby(wk.values).cummax().shift(1).where(same_wk)
    wk_lo_prior = l.groupby(wk.values).cummin().shift(1).where(same_wk)

    up = pd.Series(False, index=df.index)
    for col, prior in (("PDH", day_hi_prior), ("RNU", day_hi_prior), ("PWH", wk_hi_prior)):
        x = L[col]
        intact = ~(prior >= x)        # no earlier bar of the period traded at/above the level (NaN prior -> intact)
        up |= ((o < x) & (c > x) & intact).fillna(False)
    dn = pd.Series(False, index=df.index)
    for col, prior in (("PDL", day_lo_prior), ("RND", day_lo_prior), ("PWL", wk_lo_prior)):
        x = L[col]
        intact = ~(prior <= x)
        dn |= ((o > x) & (c < x) & intact).fillna(False)

    up &= shock & ((c - l) >= CLOSE_FRAC * rng)
    dn &= shock & ((h - c) >= CLOSE_FRAC * rng)
    up = up.fillna(False).values.astype(bool)
    dn = dn.fillna(False).values.astype(bool)
    sig = up | dn                     # mutually exclusive: up needs close > open, dn needs close < open

    i = np.flatnonzero(sig)
    side = np.where(up[i], 1, -1)
    E = pd.DataFrame({
        "i": i,
        "side": side,
        "ext": np.where(side == 1, l.values[i], h.values[i]),
        "atr": a.values[i],
    }, index=df.index[i])
    return E.dropna(subset=["atr", "ext"])
