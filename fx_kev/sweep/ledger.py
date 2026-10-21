"""Pre-registration ledger. A method is appended here (definition frozen, no parameters to tune) BEFORE it is ever evaluated.
run_ledger.py evaluates every registered method exactly once and appends the result; results are never overwritten.

Pass rule (fixed for every method, written before any result):
  P1  pooled pre-2026 mean net R > 0 with block-bootstrap 95% CI lower bound > 0, on BOTH instruments
  P2  mean net R > 0 in all three pre-2026 eras on USDJPY (2011-15, 2016-20, 2021-25) and in 2021-25 on SOL
  P3  Holm-corrected: P1's one-sided p (bootstrap) < 0.05 / (number of methods registered so far)
  P4  HELD-OUT 2026 (read once): mean net R > 0 on both instruments
  P5  (added before any metal data was evaluated; stricter only) mean net R > 0 in both eras 2018-21 and 2022-25 on XAUUSD and XAGUSD H1
A method is CLAIMED only if P1-P5 all hold. Held-out 2026 is unsealed only for methods passing P1, P2, P3 and P5.
"""
METHODS = [
    # id, family, description, params (fixed, canonical; never tuned)
    dict(id="S1", fam="sweep-fade", desc="Fade any sweep of PDH/PDL/PWH/PWL/Asia/swing/round level; enter next open; stop at wick+0.25ATR; 24-bar time exit",
         levels="ALL", min_levels=1, filt=None, H=24, confirm=False),
    dict(id="S2", fam="sweep-fade", desc="S1 restricted to higher-timeframe liquidity only (previous day or week high/low)",
         levels="PDH,PDL,PWH,PWL", min_levels=1, filt=None, H=24, confirm=False),
    dict(id="S3", fam="confluence", desc="S1 but the bar swept 2+ distinct levels at once (stacked liquidity)",
         levels="ALL", min_levels=2, filt=None, H=24, confirm=False),
    dict(id="S4", fam="displacement", desc="S1 but wait for next bar to close in the fade direction (confirmation), enter at the bar after",
         levels="ALL", min_levels=1, filt=None, H=24, confirm=True),
    dict(id="S5", fam="sweep+compression", desc="S1 only when volatility was compressed before the sweep (12/120 std ratio < 0.4)",
         levels="ALL", min_levels=1, filt="comp", H=24, confirm=False),
    dict(id="S6", fam="rejection-quality", desc="S1 only when the sweep bar closes in the fade-side third of its range (strong rejection wick)",
         levels="ALL", min_levels=1, filt="close_loc>=0.667", H=24, confirm=False),
    dict(id="S7", fam="with-trend sweep", desc="S1 only when the fade direction agrees with the 24-bar trend (sweep = pullback stop-run)",
         levels="ALL", min_levels=1, filt="trend24>0", H=24, confirm=False),
    dict(id="S8", fam="counter-trend sweep", desc="S1 only when the fade direction opposes the 24-bar trend (sweep = exhaustion)",
         levels="ALL", min_levels=1, filt="trend24<0", H=24, confirm=False),
]

# ---- wave 2, registered 2026-10-20 after wave 1 results (wave-1 held-out numbers were printed for all 8; from here on
#      held-out is revealed ONLY for methods that pass P1-P3, so it cannot steer later design) ----
METHODS += [
    dict(id="S9", fam="equal-highs/lows pool", kind="eqhl", desc="Two most recent confirmed swing highs (lows) within 0.15 ATR of each other and both within 48 bars form a resting-liquidity pool; fade a sweep of the pool (wick through, close back inside)",
         H=24),
    dict(id="S10", fam="sweep + market-structure shift", kind="mss", desc="After any S1 sweep, wait up to 6 bars for a close beyond the most recent opposite swing point (structure shift); enter next open in the fade direction; stop beyond sweep extreme; 48-bar exit",
         H=48),
    dict(id="S11", fam="failed sweep -> continuation", kind="failed", desc="After any S1 sweep, if one of the next 3 bars closes beyond the sweep extreme (the sweep failed), FOLLOW the break at next open; stop beyond the sweep bar's opposite extreme; 24-bar exit",
         H=24),
]

# ---- wave 3 (Kev), registered before any Kev call on sweep events ----
# K1: Kev-0.8B reads a plain-English narrative of each S1 sweep and answers Choice{reversal, continuation}.
#     Trade the fade only if P(reversal) > median P(reversal) of the previous 100 scored events of that instrument (causal, no threshold fitting).
# K2: same scores, trade the OPPOSITE (follow the break) when P(reversal) < that rolling median -> tests whether Kev's 'continuation' carries information.
# Sample: 250 thinned S1 events per instrument before 2026, evenly spaced in time (Kev costs ~1.4 s/call on CPU).
# Kev pass rule (all must hold, pre-2026): Spearman(P(reversal), fade R) > 0 on all 4 instruments AND K1 mean R > ungated S1 mean R on all 4
#     AND pooled K1 bootstrap CI lower bound > 0. Only then are 2026 events scored and the held-out unsealed.
KEV_METHODS = ["K1", "K2"]

# ---- wave 4, registered after a pre-2026 gross-vs-net diagnostic (S1 gross +0.10/+0.05/-0.02/+0.04 R; cost 0.10-0.16 R because the stop is ~0.8 ATR).
#      DATA-INFORMED: chosen after seeing that cost-in-R dominates. One fixed variant each, no grid. ----
METHODS += [
    dict(id="S12", fam="sweep-fade, structural wide stop", desc="S1 with stop 1.0 ATR beyond the sweep extreme and a 48-bar time exit (cost becomes a smaller fraction of R)",
         levels="ALL", min_levels=1, filt=None, H=48, confirm=False, pad=1.0),
    dict(id="S13", fam="HTF sweep-fade, structural wide stop", desc="S2 (previous day/week high-low only) with stop 1.0 ATR beyond the sweep extreme, 48-bar exit",
         levels="PDH,PDL,PWH,PWL", min_levels=1, filt=None, H=48, confirm=False, pad=1.0),
]

# ---- wave 5, registered after K1/K2 failed. Observation (pre-2026, n=220/instrument): Spearman(P(reversal), fade R) was negative on
#      all 4 instruments (-0.07..-0.10). K1 failed. Rather than act on it, ONE confirmatory test on a FRESH sample: ----
# K3: on 250 different pre-2026 S1 events per instrument (the midpoints between the K1 sample, never scored), predict Spearman < 0 on
#     at least 3 of 4 instruments AND pooled Spearman < 0 with a permutation p < 0.05 (one-sided). If it confirms, the follow-the-break
#     trade when P(reversal) is above its causal rolling median is evaluated with real costs and stops, then 2026 is unsealed.

# ---- wave 6 (novel families), registered before evaluation ----
METHODS += [
    dict(id="N1", fam="liquidity-to-liquidity", kind="target", desc="Fade an S1 sweep only if the nearest untouched opposite-side level (PD/PW/Asia/swing high-low) is >= 2 ATR away; take profit AT that level; stop 0.25 ATR beyond the wick; 48-bar time exit",
         H=48),
]
# N2 (cross-metal SMT divergence) is evaluated by smt.py with its own fixed rule:
#   gold sweeps previous-day low (high) in a bar where silver's low (high) stays ABOVE (BELOW) silver's previous-day low (high) -> fade gold; and vice versa for silver.
#   Enter next open, stop 0.25 ATR beyond wick, 24-bar exit. Pass: pooled CI lower > 0, positive in both eras 2018-21/2022-25 for BOTH legs,
#   Bonferroni over all registered methods. 2026 unsealed only on pass.

# ---- wave 7, registered before evaluation ----
METHODS += [
    dict(id="N3", fam="London-open Asia-range sweep", kind="londonopen", desc="Sweep of today's Asian-session high/low during hours 07-08 (London open), fade; S1 trade rules (next open, wick+0.25ATR stop, 24-bar exit)",
         H=24),
]

# ---- wave 8 (unattended overnight), registered before evaluation ----
METHODS += [
    dict(id="N4", fam="second sweep (sweep of the sweep)", kind="resweep", desc="Within 24 bars after an S1 sweep, a bar wicks beyond that sweep's extreme and closes back inside it (stops above the first stop-run taken); fade at next open, stop 0.25 ATR beyond the new wick, 24-bar exit. Rationale: the second stop-run clears the late stops placed beyond the first sweep, so exhaustion should be more complete",
         H=24),
]

# ---- wave 9 (unattended overnight), registered before evaluation ----
METHODS += [
    dict(id="N5", fam="multi-day level cluster", kind="dcluster", desc="Of the previous 10 completed daily highs (lows), sort them; the highest (lowest) run of >=3 consecutive sorted values spanning <= 0.5*ATR(H1,14)*sqrt(24) is a cluster, zone = its max (min). An H1 bar that wicks through the zone and closes back inside (open inside too) is faded with S1 rules (next open, wick+0.25ATR stop, 24-bar exit). Rationale: repeatedly-respected daily extremes concentrate resting stops",
         H=24),
    dict(id="N6", fam="sweep into volatility-expansion window", kind="volrelease", desc="Release bar j: 12/120 log-return std ratio (through close of j) > 0.6, ratio at j-1 <= 0.6, and min ratio over bars j-24..j-1 < 0.4. An S1 sweep at bar i with 1 <= i-j <= 6 is FOLLOWED (side = -fade side), stop 0.25 ATR beyond the sweep bar's opposite extreme, 24-bar exit. Rationale: compression release precedes range expansion; the sweep may mark the expansion direction",
         H=24),
]
# N7 (Kev as level-quality judge), evaluated by kev_level.py with its own fixed rule, registered before any call:
#   100 evenly spaced pre-2026 thinned S1 events per instrument (kev_sweep.py sampling + thin(gap 24)); Kev gets ONLY a description of the swept
#   level (level types, bars since the level formed where computable [swing pivots: bars since pivot; PD/PW/Asia: bars since that period ended],
#   number of prior-48-bar touches within 0.1 ATR) and is asked "Is this a significant liquidity level where many stop orders are likely resting?"
#   Pass: Spearman(P(yes), fade R) > 0 on all 4 instruments AND mean R of events with P(yes) > instrument median > ungated mean R on all 4.
#   2026 never scored unless it passes.

# ---- wave 10 (scheduled overnight run fired 04:11 UTC), registered before evaluation ----
METHODS += [
    dict(id="N8", fam="weekly-open PW sweep", kind="weekopen", desc="Monday 00:00-05:59: sweep of the previous week's high/low, fade with S1 rules. Rationale: stops beyond last week's range get run at the weekly open", H=24),
]

# ---- wave 11, registered 2026-10-21 before any training data was built ----
# K4 "Kev fine-tuned on real sweep outcomes": init_from jaredpalmer/kev-0.8b, LoRA fine-tune (kev.train --data, lr 2e-5, 1 epoch, defaults otherwise)
#   on the S1 sweep narratives (kev_sweep.narrative) of ALL four instruments with event time < 2021-01-01, label = "reversal" if the S1 fade's net R > 0
#   else "continuation" (Choice, same question text as K1). No hyperparameter search: one run, settings fixed here.
#   Evaluation (read once): S1 events 2021-01-01..2025-12-31, up to 300 evenly spaced per instrument. Pass if Spearman(P(reversal), fade R) > 0 on all 4
#   AND mean fade R of events with P above the causal rolling median (previous 100) beats ungated mean R on all 4 AND pooled gated block-bootstrap CI lower > 0.
#   Only on pass are 2026 events scored.

# ---- wave 12, registered 2026-10-21 BEFORE any of this data was downloaded. DATA-INFORMED (chosen from round-1/2 near-misses). ----
# C1 "crypto continuation replication". Observation (pre-2026, drift-adjusted): follow/continuation rules showed a residual edge on SOL only
#   (FVG1 +0.14R, EXH1 +0.26R, TSMOM1 +0.32R, R2C +0.07R after subtracting SOL's own drift; SOL shorts also positive for FVG1/EXH1).
# Test on 8 coins never used in this project, Binance spot H1 (data.binance.vision), from first available month (>= 2018-01) to 2025-12-31:
#   BTCUSDT ETHUSDT BNBUSDT XRPUSDT ADAUSDT LTCUSDT LINKUSDT DOGEUSDT. 2026 bars of these coins are NOT downloaded unless C1 passes.
# The four REGISTERED proposal files are run UNCHANGED (sha256 in proposals/registry.jsonl). Cost 0.10% round trip (same as SOL).
#   Calendar-dependent generators (TSMOM1, R2C) get name="SOLUSDT_H1" (crypto: plain UTC dates). EXH1's round-number step per coin =
#   10**(round(log10(median close of the coin's first 365 days)) - 1) (price-scale only, no outcomes), passed via sweep_lib.RN_STEP.
# PRIMARY: FVG1. Pass if, pooled over the 8 coins: mean net R > 0 with a calendar-WEEK cluster-bootstrap 95% CI lower bound > 0 (trades in
#   the same week resampled together, which respects cross-coin correlation), AND mean net R > 0 on at least 6 of 8 coins, AND the pooled
#   drift-adjusted mean (each coin's own mean H-bar log return removed, as in drift_diag.py) > 0.
# SECONDARY: EXH1, TSMOM1, R2C with the same rule at a Bonferroni level of 0.05/4 (one-sided cluster-bootstrap p).
# If anything passes: adversarial review, then ONE final confirmation on 2026 data (SOL + the 8 coins), read once.

# ---- wave 12b, registered after C1 pre-2026 results and BEFORE any 2026 crypto data is downloaded or read ----
# C1 result (pre-2026, 0.10% RT cost): FVG1 FAIL (primary). EXH1, TSMOM1, R2C PASS (8/8 coins positive, week-cluster CI > 0, drift-adj > 0).
# Final confirmation (read once, after the adversarial review): 2026-01-01..2026-09-30 for the 8 coins (downloaded fresh) + SOL (sealed file).
#   Each surviving method passes the confirmation if, at 0.20% round-trip cost (Binance spot taker 0.10% per side, stricter than C1's 0.10%):
#   pooled mean net R over the 9 coins > 0 AND mean net R > 0 on at least 5 of 9 coins. Also reported, not used for the decision: 0.10% cost,
#   per-coin table, week-cluster CI. With ~9 months of data the test has low power; a fail is informative, a pass is supportive, not proof.
