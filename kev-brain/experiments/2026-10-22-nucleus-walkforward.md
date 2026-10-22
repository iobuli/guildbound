# Nucleus walk-forward: fit on 2024, then 2025 and 2026 Jan-Sep

Pre-registered 2026-10-22, before any fit is scored on 2025 or 2026. The unlearned 2023-2026 trace already exists. This study does not retune it and does not use those years to choose a model.

## Question
Do weights fit only on 2024 SOLUSDT nucleus readings predict the rest of the hour, and the next hour, in 2025 and in 2026 Jan-Sep? Comparison is the plain unlearned fade of the 18-line balance.

## Data
`markets/SOLUSDT/raw/m1.npz` from `kev-lab/ingest_m1.py` (Binance spot 1m, 2020-08 through 2026-09). Readings are `nucleus.nucleus` as it stands. No new Ichimoku parameter.

A training label is legal only if its exit is still inside 2024. Rest-of-hour exit is minute 59 of the same hour. Next-hour exit is minute 59 of the following hour, so the 2024-12-31 23:00 hour is not a next-hour training row. 2025 and 2026 rows are not passed to selection or fitting.

## Decision times and features
Minutes of the hour, fixed: 0, 15, 30, 45. An hour is used only when all four minutes are valid and the target outcome is finite.

38 features, this order, from the minute itself:

- `bal`, `slope_bal`
- `{1m,5m,15m,1h,4h,1D}.bal`
- `{tf}.slope_bal`: mean of the stored slopes of n = 9, 26, 52 on that timeframe
- `{tf}.{9,26,52}.pos`: position in that line's range
- `{tf}.cloud`: -1, 0, or 1

No shift, lag, or line-order dummies. No feature is added or dropped after the run.

## Model
L2 logistic regression. Label y = 1 when the trace outcome z is positive (up), else 0. Hours with z = 0 are dropped.

Features are z-scored with the training fold's mean and standard deviation (a zero-variance column is left at 0). The intercept is not penalised.

Objective, y in {-1, +1} after the usual map from {0, 1}, linear score f = b + x·w:

mean logistic loss + (λ / 2) · ||w||^2

Newton, 30 steps. The traded position is the sign of f. That is the probability threshold 0.5. The threshold is not searched.

Two models, fit separately: rest of the hour, and the next hour.

## What 2024 is allowed to choose
Time-ordered folds, UTC calendar quarters of 2024:

- Train Q1, validate Q2
- Train Jan-Jun, validate Q3
- Train Jan-Sep, validate Q4

Grid: minute in {0, 15, 30, 45} and λ in {100, 10, 1, 0.1}. Selection score is the mean validation AUC of the linear score for y = 1. A fold counts only if it has at least 200 training hours, 200 validation hours, and 50 of each class. A cell needs at least two such folds. Tie: higher λ, then earlier minute.

The winning minute and λ are refit on every legal 2024 hour. Coefficients, means, and scales are then frozen.

## Test, once each
- 2025: hour open 2025-01-01 through 2025-12-31
- 2026: hour open 2026-01-01 through 2026-09-30 (latest ingested month)

Nothing in the test windows is used to change features, λ, minute, threshold, or sign.

On each window, for the learned score and for the comparisons below:

- AUC of the score for z > 0
- Hit rate: share of nonzero positions whose sign matches the simple return
- Spearman rho of the score against z, and a permutation band (95th percentile of |rho|, 100 shuffles of hours, seed 7)
- Gross bps: mean of position times the simple return from the decision price to the exit, in basis points
- Net of the repo's Binance spot cost, and net of a flat 5 bps round trip

The score is oriented so that higher means "up". For the learned model that is the linear score. For the fade it is minus `bal`. A useful rule has AUC above 0.5 and rho above 0. This is the opposite sign convention from the raw `bal` rho in the 2026-10-21 trace, where a negative rho meant the contrarian fade.

## Comparisons, fixed now
1. Learned rule at its chosen minute.
2. Unlearned fade of `bal` at that same minute: position = -sign(bal), flat when bal is 0.
3. Unlearned fade of `bal` at minute 0, the clock used in the previous cost table.

No other feature is used as a baseline.

## Costs
`markets/SOLUSDT/override.json`: spread 0, slippage 0.10R. The paper-book stop is 0.5 daily ATR, so 0.10R is 0.05 times ATR.

ATR is Wilder 20 on UTC daily true range (`atr.d1-period-20`), taken from the previous completed day. Cost in bps = 0.05 * ATR / decision price * 10000. The decision price is the minute of the trade, which is known then. The same hours are used for gross and net. The 5 bps case subtracts 5 from each trade and does not add the ATR slippage.

Uncertainty on gross bps and on the 0.10R net is a day-cluster bootstrap (`patterns.boot`, 600 resamples, seed 11).

## What will not be done
No second model, no threshold search, no new minute, no refit after seeing 2025 or 2026. A crash fix that leaves this grid, this feature list, and this cost formula unchanged is allowed. The grid will not move after the test numbers exist.

## Results (scored once, 2026-10-22, after the section above was committed)

`python3 kev-lab/walkforward.py`. Training never saw an hour open in 2025. Rest-of-hour training ends 2024-12-31 23:00 (8,784 hours). Next-hour training ends 2024-12-31 22:00 (8,783 hours), which is the last hour whose next-hour exit is still in 2024.

2024 chose the same cell for both targets: minute 0, λ = 1. Across λ the validation AUC barely moved (rest-of-hour minute 0 spans 0.546 to 0.548). Later minutes were worse (rest-of-hour minute 45 spans 0.523 to 0.528). Because minute 0 won, the two fade baselines are the same row.

The frozen weights are small, about 0.01, and almost all negative on position and slope. The fit is a soft version of the contrarian fade, not a different pattern. Full grid and coefficients: `out/lab/walkforward.json`.

Scores below are oriented so that higher means up. The fade score is minus `bal`, so its rho is the negative of the raw balance rho from the earlier trace. AUC 0.50 and rho 0 are the empty results. The fade drops hours where `bal` is exactly 0, which is why its n is lower.

| Window | Target | Rule | n | AUC | Hit | Rho | Band | Gross bps | Gross CI | Net 0.10R | Net 5 bps |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 2025 | rest of hour | learned | 8725 | 0.525 | 0.517 | +0.032 | 0.019 | +0.54 | -1.29 to +2.26 | -35.5 | -4.46 |
| 2025 | rest of hour | fade bal | 8202 | 0.517 | 0.511 | +0.024 | 0.018 | +0.59 | -1.28 to +2.38 | -35.4 | -4.41 |
| 2025 | next hour | learned | 8729 | 0.522 | 0.516 | +0.031 | 0.022 | +0.03 | -3.20 to +3.11 | -36.0 | -4.97 |
| 2025 | next hour | fade bal | 8206 | 0.521 | 0.513 | +0.028 | 0.021 | +0.61 | -2.44 to +3.80 | -35.4 | -4.39 |
| 2026 Jan-Sep | rest of hour | learned | 6476 | 0.531 | 0.519 | +0.036 | 0.023 | -1.58 | -3.17 to -0.02 | -28.2 | -6.58 |
| 2026 Jan-Sep | rest of hour | fade bal | 6072 | 0.521 | 0.518 | +0.023 | 0.023 | -1.08 | -2.72 to +0.80 | -27.7 | -6.08 |
| 2026 Jan-Sep | next hour | learned | 6510 | 0.535 | 0.525 | +0.039 | 0.022 | -0.97 | -3.82 to +1.72 | -27.5 | -5.97 |
| 2026 Jan-Sep | next hour | fade bal | 6101 | 0.529 | 0.527 | +0.029 | 0.022 | -0.84 | -3.75 to +2.06 | -27.4 | -5.84 |

Mean 0.10R cost was 36.0 bps in 2025 and 26.6 bps in 2026 Jan-Sep.

Reading: the ranking carries a little. Learned AUC is 0.52 to 0.53 and the rho clears its band in every learned cell. That is a small step above the fade's AUC, about one point. It does not carry as money. Every gross interval covers zero or sits below it. In 2026 the rest-of-hour learned trade hit 52% of the time and still lost 1.6 bps a trade, so the losers were larger than the winners. The best gross point in the table is +0.6 bps. A 5 bps round trip is already past that, and 0.10R is about 30 bps further. The learned rule does not beat the unlearned fade on bps in any cell.
