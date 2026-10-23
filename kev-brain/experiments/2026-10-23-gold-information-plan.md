# Gold information / raw-price and Fibonacci study: fixed plan

Written before executing the new label/outcome analysis on 2026-10-23.
User objective: test information and emergent organization, not profitability;
independently inspect the raw price path outside Kev.
Existing historical data have been used before. This is retrospective chronological
validation, not pristine unseen data or independent cross-market confirmation.

## Data and availability

Bundled 200,000 gold M30 records. No synthetic minutes. Features use observations
closed by the current timestamp. Effective existing gold override fixes in-sample
through 2018; evaluate 2019 onward, also report 2019-2022 / 2023 onward. Do not tune
any threshold/model on evaluation labels. Exclude targets whose eight-bar horizon
contains a missing M30 slot. Exclude a training outcome crossing into 2019.
Forward prices are labels only, never feature inputs. All study choices are analyst
inferences for measurement and must not modify shared strategy parameters.

## Raw-price investigation (no Kev engine inputs)

1. Raw 104-observed-M30-bar rolling extrema: measure how often the midpoint holds,
   and how often a changed midpoint occurs without a new high or low, solely
   because a prior boundary leaves the finite window. Also compare extreme
   sharing across 18/52/104-bar windows and same-side position synchronization.
2. At the first observation of each UTC Monday-aligned week, freeze an anchor
   from the PRECEDING 96 observed M30 highs/lows. The current bar cannot define
   its own prior reference. Retain that low/high throughout the week.
3. Ratios .236/.382/.618/.786 versus nearby ordinary fractions
   .25/.375/.625/.75, paired in that order. First observed contact per ratio per
   week only. Contact is M30 high/low inclusion, available at close. Approach
   sign is previous close minus fixed level (previous bar must be contiguous).
   No inferred order among multiple levels touched within one bar.
4. Outcome = approach-sign × subsequent 8-bar log move, normalized by current
   trailing-48-bar return standard deviation × sqrt(8). Positive means return
   toward the approach side AFTER contact was observable; this is not profit
   from an assumed fill at the level. Compare matched week/pair/approach-side
   Fibonacci and control contacts. Report occurrences and up/down outcomes too.

## Predictive increment

Regular non-overlapping eight-observation grid, with gap-free 8-bar target.
Predict P(next 8-observation close > current close); exact zero targets excluded.
Fixed ridge logistic regression, strength 1, training-only scaling, Newton solver.
No feature selection or hyperparameter search.
Baseline: normalized past 1/8/48-bar log moves, trailing 8/48 volatility ratio,
frozen weekly coordinate q, q², and anchor log-width / trailing-48 volatility.
Nucleus increment: 4 timeframe balances, aggregate slope balance, 12 clipped
line positions, using the existing causal 30m/1h/4h/UTC-1D definition.
Fib increment: four exp(-abs(q-ratio)/.02) proximity bases; matched ordinary
fraction increment has the same four bases and model size.
Models: baseline, baseline+Nucleus, baseline+Nucleus+Fib,
baseline+Nucleus+ordinary fractions, baseline+Fib.
Report Brier, log loss and accuracy; Brier paired differences are primary.

## Controls, uncertainty and decision rule

Five primary comparisons: Nucleus vs baseline; Nucleus+Fib vs Nucleus;
Nucleus+Fib vs Nucleus+ordinary fractions; Fib vs baseline; matched Fib vs
ordinary-fraction post-contact normalized displacement.
Use 1,000 four-calendar-week moving-block cluster bootstrap draws (seed 20261023)
and paired differences, not independent candle errors. Report 95% intervals,
centered bootstrap two-sided p estimates and BH q across all five comparisons.
A candidate must have favorable held-out effect, CI excluding zero, q<.05 and
same favorable sign in 2019-2022 and 2023 onward. This warrants further study,
not a confirmed edge or causal effect. Matched-level events at different times
can retain timing/volatility confounds.

Raw organization controls: 99 quarter-stratified return permutations and 99
quarter-stratified 48-observation block permutations. Shuffle matching H/C and
L/C geometry with returns; keep real timestamps/gaps and quarter drift. Rebuild
high/low/close paths and quantize to gold ticks. Recompute all raw-window metrics.
These controls preserve quarter drift and bar geometry, not every feature of
real volatility/calendar dependence. Report null ranges and empirical two-sided
rank p; no strong emergence claim from beating an incomplete null. BH across
all three metrics × two null types, separately from predictive comparisons.
Only call a raw-organizational lead robust to these controls if it exceeds both
null families after correction. No new metric/ratio/window is selected after
seeing these results; additional observations are explicitly exploratory.

Artifacts: reproducible Python script, JSON/Markdown results, explanatory plots.
No trades, account simulation, cost tuning, forecast deployment or parameter edits.
