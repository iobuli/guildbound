# Predictive structure redesign: fixed development / evaluation policy

User authorizes independent structural changes to improve predictive capability.
Goal is forecast information, not assumed profitability or guaranteed direction.
Written before executing the new development/evaluation experiment. Prior gold
results and later-period data are already known: any result remains retrospective.
No additional configurations will be chosen using the 2019+ evaluation scores.

## Structure

Keep the observatory, causal reference history and previous negative results.
Add a separate predictive state layer: deduplicate identical range anchors across
30m/1h/4h/UTC-day × 9/26/52, distinguish fresh high/low breaches from retained
boundary expiry, capture close-confirmed rejection/acceptance, retained-boundary
age, cross-scale disagreement and fixed-anchor Fib contact/rejection context.
All prior ranges must be available BEFORE the current source bar; current bar's
H/L/C can describe a completed event, not define its own prior level. No path/order
is inferred inside an M30 bar. No strategy shared parameters are changed.

## Outputs (all four hours / 8 contiguous M30 observations ahead)

1. Direction: next close above current close (ties excluded).
2. Large displacement: absolute next log-close move exceeds current trailing
   48-return standard deviation × sqrt(8). Threshold is known at forecast time.
3. Terminal range exit: next close outside the PRIOR 104-observation high/low.
   This is terminal-close location, not the first intrabar break or MSB itself.

Future data are labels only. Use a fixed non-overlapping eight-observation grid.
Exclude targets with missing horizon slots. Explicitly purge training labels
whose availability is on/after the next evaluation fold's start.

## Development and locked walk-forward evaluation

Honor the effective training-era cutoff 2018. Use 2013–2018 annual validation
folds to choose one configuration per target. Each fold fits the preceding three
calendar years only. Annual refitting is the fixed adaptation policy, not a
response to evaluation performance. Probability calibration/prevalence and all
scaling are fitted on those prior rows only. Minimum 300 fit / 100 validation rows.

Five configurations, same choice set for each target:
- adaptive prior prevalence (no covariates);
- raw ridge logistic, penalty 100, half shrink toward fitting prevalence;
- raw histogram gradient boosting;
- deduplicated event-state histogram boosting;
- event-state plus fixed-Fib context histogram boosting.

Boosting: 100 iterations, learning rate .05, depth 2, max 4 leaves, min leaf 100,
L2=10, no early stopping, seed 20261023, predictions half-shrunk toward prior
prevalence. No hyperparameter search. Select by concatenated development Brier.
Then lock and evaluate 2019–2026 year by year, retraining on only the preceding
three years. Previously observed evaluation years may become training only for
later years under this already-fixed policy. Do not retune after the locked run.
Also report raw logistic, raw boosting and adaptive prior baselines throughout
holdout, even if another model was selected. Direction additionally compares
against 50/50. Terminal-exit compares with the normal-distance geometric baseline;
these are secondary diagnostics, not substitutes for the primary baselines.

## Skill criterion

Primary family: selected vs raw logistic, selected vs raw boosting and selected
vs adaptive prior for each of three targets (9 paired comparisons), plus selected
direction vs 50/50 (10 total). Four-calendar-week cluster moving-block bootstrap,
1,000 draws, seed 20261023; BH q across this family. Duplicate identical models
remain reported with zero effect rather than treated as successful comparisons.
A target is supported in this retrospective experiment only if selected forecasts
beat BOTH raw baselines AND adaptive prior (favorable lower Brier; CI excludes
zero, q<.05), retain favorable mean effects in 2019–2022 and 2023 onward, and
at least 6 of 8 calendar evaluation years improve against raw boosting. Direction
must additionally beat 50/50. Report log loss, probability calibration and outcome
frequency as well as accuracy. No high-confidence signal/abstention threshold is
chosen from evaluation labels. Outcomes failing the criterion remain research,
not promoted as validated skills.

Deliverable: a causal predictive state/forecast module with fitted-through date,
training window, target definitions, probability/baseline/skill status, reproducible
walk-forward scores and visual report. Connect it to the gold observatory without
exposing future labels in an as-of API. Preserve failed/null results. No orders,
profit simulation or automatic live feed. Independent future/other-market
validation remains necessary even if this retrospective criterion passes.
