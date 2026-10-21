# FINDINGS (independent blind study)

## Headline
**No tradable directional pattern survived.** Total hypotheses tested in discovery: **280** (174 wave-1 H1 [87 per instrument x USDJPY, SOL];
106 wave-2: vol 10, D1 28, session-range 8, climax 4, cross-asset 4, M15 USDJPY 52 [86 minus calendar families], rest counted in the table files `disc_h1.pkl`, `disc_wave2.pkl`),
plus **7 one-shot confirmations** (3 directional, 4 volatility). Multiple-testing: Bonferroni over 280 in discovery; over 7 in confirm (alpha 0.05/7 = 0.007).
Wave 2 was written after seeing wave-1 discovery results (not wave-2 results). Direction choices (follow vs fade) are all registered as separate hypotheses in `code/hyp.py`, `code/wave2.py`.
Costs: USDJPY 1 pip round trip, SOL 0.1%. Splits: first 60% discovery, last 40% confirm with 50-bar embargo; non-overlapping events only.
Causality: `code/corrupt_test.py` (overwrite all bars after t with noise; all features incl. D1 context unchanged; 0 failures, both instruments).

## Claimed (non-directional, volatility only)
1. **USDJPY volatility compression -> larger next-24h range than a typical day.** Definition: std of last 12 H1 log returns < 0.4 x std of last 120. Outcome y = log(next-24-bar range / (ATR14*sqrt24)), event mean minus thinned baseline.
   Discovery: +0.234 (t=9.1, n=360). Confirm: **+0.232, CI [0.17, 0.29], n=269, t=7.2**; thirds 0.19/0.24/0.27. Passes Bonferroni. Why: vol mean-reversion/clustering, the ATR14 denominator is short-window so a compressed state reverts to the longer-run level. Well known, not new alpha, not directional, does not give a P&L by itself (would need an options/straddle or breakout structure; breakout follow/fade direction tests were null).
   **Failed to replicate on SOLUSDT H1** (confirm +0.038, CI [-0.07, 0.15]).
2. **USDJPY bar with range > 3x prior ATR -> elevated next-24h range** (vol clustering). Confirm +0.085, CI [0.037, 0.135], n=489, t=3.4; thirds 0.093/0.091/0.071. SOL: discovery +0.126 (t 4.2) but confirm +0.017 (CI [-0.05, 0.09]) - did not hold.
   Also in discovery: range after the first bar following a weekend gap is lower (t=-11) but this is an artefact (the gap enters ATR via true range); unconfirmed, not claimed.
`rules.py` contains `vol_compression_flag_usdjpy` (bool, not a +/-1 signal) and `signal_null` (returns 0). There is no directional rule to freeze.

## Directional candidates confirmed once (all failed)
| frozen hypothesis | disc net | confirm n | confirm net (95% CI) | thirds |
|---|---|---|---|---|
| SOL D1-trend vs 24h H1 counter-move resolves to D1 trend, H24 | t=2.0 | 166 | +0.10% (-0.50, 0.74), t=0.3 | +0.18/-0.31/+0.44 |
| USDJPY M15 close breaks 20-day high/low, follow, H96 bars | +22 pips t=2.0 | 73 | -23.9 pips (-47, -0.5) | all negative |
| USDJPY M15 efficiency ratio48>0.25, follow, H96 bars | t=2.1 | 248 | -6.6 pips (-18.5, +4.7), t=-1.1 | -1.4/-14.8/-3.5 |
Discovery effects reversed sign: classic regime luck (2022-24 USDJPY trend).

## What did NOT work (discovery, net of cost; none had Holm-adjusted p<0.5 on a tradable net basis)
Families (follow and fade each): compression-then-breakout; plain 24-bar close breakout; 6-run of same-direction closes; >6 ATR drawdown / run-up from 120-bar extreme;
volume (tick/real) z>2 bar; autocorrelation regime (+-0.08) with 6-bar momentum; efficiency ratio trending/choppy; >3 ATR exhaustion bar; prior-day high/low fake-out and breakout;
20-day-high breakout; D1-trend vs H1 counter-move (both resolutions); weekend gap fade/follow; round-number rejection/break (1 yen; SOL half power-of-ten);
24 hour-of-day long-only (some gross edges ~1 pip at USDJPY hour 01/20 server time = equal to the cost, net ~0); day-of-week; 8-hour session momentum/fade; Asian/London range breakout at next session open;
D1 RSI2, 3-day streak, 20d breakout, NR7 close-location, 60d momentum, 5d reversal (USDJPY and SOL); SOL liquidation climax; USDJPY<->SOL 24h lead-lag; M15 replication of the H1 families (H 12h/24h).
Best tradable discovery net t was ~2.0 (p~0.04, ~0.9 after Bonferroni). Many "significant" discovery rows were just the cost drag on long-only hour tests (negative net, mirror gross near zero).
## Failure modes / caveats
Server-time hours unknown; M15 only 3.3y, M5 not used (49k bars, too short; no hypothesis tested on it). SOL H1 split period dominated by one huge bull run. Bootstrap iid on non-overlapping events.
Honest conclusion: null for direction; only known volatility mean-reversion/clustering on USDJPY.
