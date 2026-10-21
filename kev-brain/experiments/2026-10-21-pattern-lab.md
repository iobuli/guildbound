# Pattern lab: what price does after 3-touch levels, sweeps and structure breaks (pre-registered 2026-10-21, before any run)

## Why
The trade method loses because its frame is tight: a limit order at the level with a 0.5-ATR stop. The user asked to relax it first and then refine:
- start from 3-touch level bases;
- watch volatility, liquidity sweeps and market-structure breaks (MSB);
- learn, in fine detail, what price does after each pattern in 2020-2022.

This is a descriptive event study, not a trading rule. Any rule it suggests is checked once on 2023 onward.

## Events (H1, all known at the event bar's close; `kev events`, `kev signals`)
- **T3:** a kev fill (one-hour-spec, flipped level) at a level with 3+ touches. Relaxed: no space or GO requirement. T1-2 fills are kept for comparison.
- **Sweep:** the first bar to trade beyond a confirmed swing (kev's 3-bar fractals) that closes back inside, i.e. an SFP.
  - Relaxed: no visit-count or age veto. Swings older than 120 bars are ignored.
  - Recorded with each sweep:
    - the swing's depth (kev's swing-in/out, in daily ATR), with "deep" meaning 1+ ATR as in kev;
    - its age;
    - where the bar closed in its range;
    - the bar's size.
- **Break:** the first close beyond the most recent confirmed swing high or low. **MSB** is a break against the prior structure: a close above a lower high, or below a higher low.
- **Combos:**
  - Sweep then MSB in the same direction within 24 bars, timed at the MSB bar.
  - T3 fill with a same-direction sweep in the 6 bars up to the fill.
- **Volatility monitor:** the percentile of the prior-day ATR within the trailing year, split into terciles. Also recorded: the trading session (UTC hour) and whether the trade is with or against the 50-day return.

## Measures (signed in the trade's direction, in prior-day ATR units, entry at the event bar's close)
- Return after 1, 4, 12, 24 and 72 bars.
- Best and worst excursion within 24 bars (MFE/MAE).
- First-passage R for stop/target pairs in ATR: 0.5/0.5, 0.5/1, 1/1 and 1/2, with a 72-bar time-out.
  - Same-bar ties count as the stop.
  - Costs are not included; the cost per trade in R is shown beside the results.
- **Lift:** each event minus the average of all bars in the same month and direction (removes drift).
- **CIs:** day-cluster bootstrap. BH q across all groups in the table.

## Refinement rule (fixed now)
- A pattern or split goes forward to 2023 onward only if, in 2020-2022:
  - the pooled 24-bar lift has q < 0.05;
  - its CI excludes 0;
  - it has the same sign in at least 2 of the 3 markets;
  - it has 100+ events.
- At most the 5 strongest by |t| are carried forward, together with their best stop/target pair from 2020-2022.
- They are checked once on 2023 onward, with costs.

## Results (appended 2026-10-21 by kev-lab/patterns.py; full table in out/lab/patterns.json and on the kev-lab page)
12,327 events in 2020-2022 across SOL, ETH and gold. The 24h lift is in daily ATR, against all hours of the same month and direction.

| pattern | n | 24h lift [95% CI] | 72h raw |
|---|---|---|---|
| level fill, any touches | 3828 | -0.00 [-0.04, 0.03] | +0.02 |
| T3: level fill, 3+ touches | 380 | -0.04 [-0.11, 0.03] | -0.24 |
| T3 + space | 88 | +0.07 [-0.07, 0.20] | -0.32 |
| sweep (SFP), any | 4043 | -0.01 [-0.03, 0.01] | -0.05 |
| sweep of a deep swing | 246 | -0.02 [-0.13, 0.10] | -0.17 |
| MSB | 2391 | -0.01 [-0.03, 0.02] | +0.01 |
| sweep then MSB within 24 bars | 1417 | +0.01 [-0.02, 0.05] | +0.03 |

- **Refinements:** volatility tercile, session and with/against the 50-day move gave 70 groups. **None passed** the pre-registered bar, so nothing was carried to 2023 onward.
- **Most consistent hints (q 0.12-0.47, not real by the rule; leads only):**
  - sweeps taken *with* the 50-day move: -0.05, the same sign in all 3 markets;
  - sweep → MSB *against* the 50-day move: +0.06, in all 3 markets.
  - Both say "fade the 50-day move a little". That is plausible mean reversion, but the size is below costs: about 0.1 ATR per trade with a 1-ATR stop.
- **3-touch levels are not better:** fills at 3+ touch levels did slightly worse than 1-touch fills at every horizon, especially 72h (-0.24 ATR). This matches H2.
- **Quarter by quarter:** every pattern's lift flips sign from quarter to quarter (for example, deep sweeps go +0.35 then -0.45). Nothing is stable enough to trade.
- **Comparison with the earlier limit-order results:** taken at the bar's close, kev's level fills have no edge at all. The earlier +0.05 to +0.25R over random orders came from the resting limit order at the level, not from what price does after it is seen.

**Verdict:** on H1, after the bar closes, none of these patterns (3-touch levels, sweeps/SFPs, MSB, sweep+MSB) predicts the next 1-72 hours beyond what an ordinary hour of the same month does.
