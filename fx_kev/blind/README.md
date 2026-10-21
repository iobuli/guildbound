# Blind pattern-discovery workspace

## Task
Find **novel, non-Ichimoku patterns that indicate a change**: trend start/end, reversal, breakout vs fake-out, volatility-regime shift,
momentum exhaustion, mean-reversion onset, session/calendar effects, cross-timeframe disagreement that resolves, etc.
Instruments: USDJPY (D1, H1, M15, M5) and SOLUSDT (H1). Anything that predicts a *change* in direction, volatility or regime
over a stated horizon is in scope. Ichimoku is already studied (see results/); do not just re-derive it.

## Data (data/*.csv, comma-separated, header: time,open,high,low,close,volume)
| file | span | bars |
|---|---|---|
| USDJPY_D1 | 2010-08 to 2025-12-31 | 4,808 |
| USDJPY_H1 | 2010-08 to 2025-12-31 | 95,807 |
| USDJPY_M15 | 2022-08 to 2025-12-31 | 83,297 |
| USDJPY_M5 | 2025-05 to 2025-12-31 | 49,892 |
| SOLUSDT_H1 | 2020-08 to 2025-12-31 | 47,200 |
| ichimoku_paper.md | background paper (Indonesian), optional | |
`time` is the bar OPEN time. A bar is only known at open_time + its duration. Volume is tick volume for USDJPY (not true volume).
All times are broker server time, not necessarily UTC. Crypto trades 24/7; FX has weekend gaps.

## Previous results (results/*.csv), all built causally, for reference and as extra data
- `ichimoku_crosses_features_outcomes.csv`: 4,369 USDJPY H1 Tenkan/Kijun crosses, 2011 to 2025: features (signed by trade side), outcomes
  (R in H1-ATR units, mfe, mae, t_peak, stopped), and a `split` label. `side` 1=long, -1=short.
- `ichimoku_exit_rule_R.csv`: net R per trade under 13 exit rules, same trades.
- `kev08b_answers.csv`: Kev-0.8B yes-probabilities for 3 questions per cross (daily/hourly/15m text snapshots). Weak, near-chance.
- RESULTS_SO_FAR.md: what was found and what failed.

## Kev (optional tool)
A Kev-0.8B decision model runs at http://127.0.0.1:8008 (TypeSafe System One API; python `typesafe_sdk`, `TypeSafeClient(api_key="local", base_url=..., model="kev-latest")`,
`client.system_one(state=<text or dict>, questions={"q": Noul(instructions="...")})` -> `.nouls["q"].noul` is P(yes); also `Choice(instructions, criteria={name: description})`).
CPU only, ~1.4 s per call. It was never trained on prices and scored near chance on rendered indicator text, so use it only where it can
plausibly add value (e.g. labelling a regime from a compact description) and always compare against a non-Kev baseline. It is optional.

## Rules (non-negotiable; findings that break them are discarded)
1. **Causal only.** A feature at decision time t may use only bars with open_time + duration <= t. Entry fills at the first available bar OPEN after t.
   Write and run a corruption test (overwrite all bars after t with noise; features must not change) for every feature family you use.
2. **Time-ordered discovery.** Use data only from this directory. Do not search elsewhere on disk for more data (no /root/.claude/uploads, no other paths).
   Inside it, split chronologically: explore on the first ~60% of each series, confirm on the next ~40% ONCE per frozen hypothesis. Embargo overlapping trades at boundaries.
3. **Pre-register and count.** Write each hypothesis (definition, horizon, direction) BEFORE you test it, and keep a running count of every hypothesis tested,
   including failures. Report the count. Apply a multiple-comparison correction (Bonferroni/Holm or a permutation/shuffled-label null) to anything you claim.
4. **Costs.** USDJPY about 1 pip round trip; SOLUSDT 0.1% round trip. Report net.
5. **Evidence bar.** Effect size with bootstrap CI (block bootstrap or resample non-overlapping events), stable sign across at least 3 sub-periods,
   and, where possible, replication on a second instrument or timeframe. Non-overlapping events only for significance counts.
6. **No peeking at the future of your own sample** through parameter tuning on the confirm segment. If you tune, tune on the discovery segment.
7. Say plainly when you found nothing. A null result with a good audit trail is a valid and valuable outcome.

## Deliverables (write into results_agent/ inside this workspace)
- `FINDINGS.md`: ranked list of patterns, each with: plain definition, why it might exist, horizon, discovery stats, confirm stats with CI, number of hypotheses tested
  before it, costs, failure modes. Plus a section "what did NOT work".
- `rules.py`: for every pattern you claim, a self-contained, FROZEN function
  `def signal_<name>(d: dict[str, pandas.DataFrame], t: pandas.Timestamp) -> int` returning +1 (expect up), -1 (expect down) or 0 (no signal), where `d` maps
  names like "USDJPY_H1" to the full OHLCV frame (index = bar open time) and the function must only read bars closed by `t`; plus a `HORIZON_BARS`, `TIMEFRAME`
  and the exit rule as module-level constants. These will be run, unchanged, on an 8-month held-out period you have not seen.
- All code you wrote, in code/.
