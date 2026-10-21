# Levels + liquidity-sweep system with Kev: research log

All methods were pre-registered in `ledger.py` before they were run, with fixed parameters and no grid search.
`run_ledger.py` evaluates each method once and appends the result to `results_v2.jsonl`; results are never overwritten.

## Setup
- **Instruments:**
  - Discovery: USDJPY H1 (2011–2025) and SOLUSDT H1 (2020–2025).
  - Replication (never looked at before evaluation): XAUUSD and XAGUSD H1, 2018–2025, resampled from M15.
  - Held-out: all four, 2026-01 to 2026-09. Sealed unless a method passes every other test.
- **Levels:** previous day high/low, previous week high/low, Asian-session high/low, the latest 5-bar fractal swing high/low (confirmed 2 bars later), and round numbers.
- **Sweep:** a bar wicks through a level and closes back inside it. The default trade fades the sweep: enter at the next open, stop 0.25 ATR beyond the wick, exit after 24 bars.
- **Costs:** USDJPY 1 pip, SOL 0.1%, gold 0.025%, silver 0.05%, all round trip.
- **Lookahead check:** every bar after t is overwritten with noise and the events at t must not change. 0 failures out of 10 on each instrument, for every run.
- **Pass rule (fixed before any results):**
  - P1: bootstrap CI above 0 on both discovery instruments.
  - P2: positive in every era.
  - P3: Holm/Bonferroni correction over all registered methods.
  - P5: positive in both eras on both metals.
  - P4: 2026 positive on all four instruments.

## Results: 13 rule methods, 0 pass
| id | idea | USDJPY R [95% CI] | SOL R | Gold R | Silver R |
|---|---|---|---|---|---|
| S1 | fade any sweep | +0.006 [-0.09,+0.11] | -0.065 | -0.179* | -0.118 |
| S2 | previous day/week levels only | +0.036 | +0.127 | -0.038 | -0.200* |
| S3 | 2+ levels swept at once | -0.101 | +0.038 | -0.071 | -0.198* |
| S4 | wait for a confirmation bar | +0.094 [-0.01,+0.20] | +0.022 | -0.113 | -0.028 |
| S5 | sweep after volatility compression | -0.159 | -0.271 | -0.076 | -0.048 |
| S6 | strong rejection wick | -0.008 | +0.043 | -0.044 | -0.131* |
| S7 | sweep with the 24h trend | -0.074 | -0.059 | -0.132 | -0.253* |
| S8 | sweep against the 24h trend | +0.036 | +0.069 | -0.133* | -0.130 |
| S9 | equal-highs/lows liquidity pool | -0.058 | +0.216 | -0.132 | -0.027 |
| S10 | sweep, then market-structure shift | +0.090 [-0.01,+0.20] | -0.018 | -0.093 | -0.136 |
| S11 | failed sweep, then follow | +0.025 | +0.004 | -0.068 | -0.175* |
| S12 | S1 with a 1 ATR stop, 48h exit (data-informed) | -0.034 | -0.002 | -0.053 | -0.124 |
| S13 | S2 with a 1 ATR stop, 48h exit (data-informed) | +0.061 | +0.029 | -0.074 | -0.139* |

`*` marks a 95% CI entirely below zero. R is net mean R per non-overlapping trade, before 2026.

**Diagnostic (S1, gross vs net):** gross R is +0.10 / +0.05 / −0.02 / +0.04 on USDJPY / SOL / gold / silver, and costs are 0.10–0.16R per trade. The stop sits about 0.8 ATR away and 77–80% of trades hit it. The gross edge exists but is smaller than the cost.

## Kev-0.8B (wave 3)
Kev reads each sweep as a plain-English narrative and chooses between reversal and continuation. I sampled 250 events per instrument before 2026.
- **Probabilities are compressed:** P(reversal) ranges only from 0.43 to 0.63.
- **Spearman(P, R)** is −0.098 / −0.095 / −0.072 / −0.070 across the four instruments.
- **K1** trades only when P is above its causal rolling median. Pooled R is −0.046, CI [−0.33, +0.16]. It fails, and 2026 stays sealed.
- **K3:** the consistently negative sign was retested as one confirmatory test on a fresh, disjoint sample of 996 events.
  - Spearman by instrument: −0.129 / +0.040 / −0.053 / −0.053.
  - Pooled −0.049, one-sided permutation p = 0.069.
  - It needed p < 0.05, so it is **not confirmed**. Kev-0.8B has no usable signal on sweeps, and 2026 stays sealed.

## Wave 6: novel families
- **N1, liquidity-to-liquidity:** fade a sweep only when the opposite liquidity level is at least 2 ATR away, and take profit at that level. Net R is −0.16 / −0.26 / −0.20 / −0.21 on USDJPY / SOL / gold / silver, with every CI below 0. Fail.
- **N2, gold/silver SMT divergence:** one metal sweeps its previous-day low or high while the other doesn't, and the sweeping metal is faded. Gold −0.19R, silver −0.25R, pooled −0.22R with CI [−0.31, −0.13]. Fail.
- **N4, second sweep (sweep of the sweep):** within 24 bars of a sweep, a bar wicks past that sweep's extreme and closes back inside it, and the move is faded. Net R is −0.044 / −0.106 / −0.065 / −0.231 on USDJPY / SOL / gold / silver; no CI lower bound is above 0, and silver's CI is entirely below 0. Fail, held-out still sealed.
- **N5, multi-day level cluster:** 3+ of the previous 10 daily highs (lows) within 0.5·ATR·√24 form a zone; a wick through it that closes back inside is faded with S1 rules. Net R is −0.026 / −0.146 / −0.151 / −0.196 on USDJPY / SOL / gold / silver; no CI lower bound is above 0. Fail, held-out still sealed.
- **N6, sweep into a volatility-expansion window:** an S1 sweep 1–6 bars after a compression release (12/120 std ratio went from below 0.4 to above 0.6) is followed in the wick's direction. Net R is −0.161 / −0.116 / −0.190 / −0.050; no CI lower bound is above 0. Fail, held-out still sealed.
- **N7, Kev as level-quality judge:** Kev-0.8B sees only a description of the swept level (type, age, touches in the prior 48 bars) and answers a yes/no question on whether it is a significant liquidity level. 100 pre-2026 events per instrument. Spearman(P(yes), R) is +0.024 / −0.173 / −0.128 / −0.074. Mean R above the median P(yes) vs ungated: +0.09 vs −0.28 / −0.15 vs +0.24 / −0.10 vs −0.33 / −0.52 vs −0.22. P(yes) only spans 0.36–0.59. Fail, and 2026 was never scored.
- **N8, weekly-open sweep of the previous week's high/low (Monday 00:00–05:59):** net R −0.084 / +0.190 / −0.177 / −0.443 on USDJPY / SOL / gold / silver, with only 31–82 trades each. Fail. This was the scheduled task's 04:11 UTC run.
- **Takeaway:** fading toward the opposite liquidity pool loses consistently, about 0.2R per trade.

## Overnight iteration
An hourly scheduled task resumes this session. Each run designs and pre-registers one new method family, evaluates it, and logs it in `OVERNIGHT_RUNS.txt` and here. It stops itself after 8 lines. It fired at 04:11 UTC, the 8th line was written by that run (N8), and the task is now disabled.

## Conclusion
- 24 pre-registered methods (S1–S13, K1–K3, N1–N8) were tested across 4 instruments, and none passed.
- Rule-based level and liquidity-sweep fades do have a small gross edge on some instruments, but it is smaller than realistic costs.
- Kev adds nothing. Its probabilities sit around 0.5 and do not rank outcomes.
- No method qualifies for live use, so the 2026 held-out data remains unused and available for a future candidate.

## How to iterate
1. Add a new dict to `METHODS` in `ledger.py`, with a fixed definition and no parameters to tune.
2. Run `./run_all.sh`.
3. The runner evaluates only new ids, runs the lookahead check first, applies the same P1–P5 bar, and unseals 2026 only for a method that passes everything else.

## Disclosures
- **Wave-1 held-out:** the 2026 numbers for S1–S8 on USDJPY and SOL were printed by mistake. After that, held-out data was sealed for every method that did not pass, and no later design used it.
- **Data-informed methods:** S12, S13 and K3 were designed after seeing pre-2026 results, and are labelled as such.
- **Hypotheses tested:**
  - 13 rule methods × 4 instruments
  - K1 and K2
  - K3
  - an earlier blind agent's 280 discovery tests on overlapping data
