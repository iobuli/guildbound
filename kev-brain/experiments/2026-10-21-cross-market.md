# 2026-10-21: cross-market confirmation (SOL, ETH, gold)

**Status: pre-registered before any v2 run on ETH or gold.** Written after the v2 code was finished and tested, and before
`kev study` was run. The results section below is appended by the runner and must not change anything above it.

## Setup (fixed)
- **Markets and data:** SOLUSDT (Binance H1), ETHUSD (H1 assembled from M30/M15/M5/M1; finer bars settle intrabar order),
  XAUUSD (H1; M30 settles intrabar order). Each run starts 45 days after the first bar.
- **Parser:** preset `one-hour-spec` is primary, because the leads came from that definition. `kev-default` is secondary and only a
  robustness check.
- **Orders:** every 3 hours the graph marks levels from closed bars. A resting limit order sits at each level, on the correct side of price
  (entry at the level), with a 24h cooldown per level. A fill is evaluated at the fill instant from bars before the fill bar plus the fill price.
- **Primary outcome `S0.5_T2`:** stop 0.5 x prior-day ATR(20) beyond the fill, target 2R, 120h maximum hold. The result is net of the profile's
  spread plus 0.10R. Finer bars decide intrabar order where they cover the hour; otherwise the stop wins ties and the fill bar's targets are not credited.
- **Control:** two matched placebos per real fill, with the same side, distance from price (x0.5-1.5), fill window and risk. Each stratum is compared with
  the placebos of the same real fills.
- **Statistics:** real - placebo difference with a day-cluster bootstrap 95% CI and two-sided p (mid-p), and Benjamini-Hochberg within each hypothesis.
  The in-sample/out-of-sample split comes from each profile (SOL <= 2023, ETH <= 2022, gold <= 2018).

## Hypotheses
**H1 (space).** Fills where price had moved at least 0.5 ATR clear of the level between the break and the fill, measured on bars before the fill bar
(`spaceOk`), beat their placebos.
- Found on SOL (+0.09R, not significant) with the legacy study; SOL is reported but does not count as confirmation.
- **Confirmed** if the real - placebo difference is > 0 in both ETH and gold and BH q < 0.05 for the pooled ETH + gold cell.
- **Rejected** if the pooled 95% CI lies below +0.05R.
- Otherwise **inconclusive**.
- Separately, the net mean R of space-ok fills is reported. It must be > 0 with a CI above 0 before anyone calls it tradable.

**H2 (touches).** Touch count does not change the real - placebo difference. Compare touches >= 3 against touches = 1 in each market, under both
presets (fractal and reaction counting).
- **Expected:** |difference of differences| < 0.1R.
- **Contradicted** if any market shows touches >= 3 better than touches = 1 by more than 0.1R with q < 0.05.

**H3 (lookahead demonstration).** Among random orders, the split "the fill bar closed back through the order price" (unknowable at the fill)
makes the non-closed-through group look much better.
- **Confirmed** if, in every market, placebo mean R(closed through) - placebo mean R(not) < -0.2R.

**H4 (settlement bias).** Measured on ETH and gold, the same trades: mean R (finer-bar settlement) - mean R (conservative H1 rules), for `S0.5_T2` and structural 2R.
- Descriptive, with a CI.
- **Expected:** small and positive (conservative rules understate wins).

**H5 (sessions).** Gold under NY-close sessions against the old UTC calendar days with Sunday stubs (`utc-raw`): fills, mean `S0.5_T2`, and the median
daily ATR ratio. Descriptive.
- Also SOL and ETH under NY-close against UTC days.
- **Expected:** gold fills and outcomes move by a visible amount; crypto by little.

## Not tested here
No parameter search, no new filters, no GO-gated claims beyond a descriptive row, because GO has many vetoes and few fills.

## Results (appended 2026-10-21 by `kev study`)

Primary preset one-hour-spec; outcome S0.5_T2; real vs its own matched placebos.

### Overview (all fills)

| group | n | mean R [95% CI] | placebo | real - placebo [95% CI] | p | q |
|---|---|---|---|---|---|---|
| SOLUSDT all fills | 3222 | -0.07 [-0.13, -0.01] | -0.16 | 0.09 [0.07, 0.11] | 0.003 |  |
| ETHUSD all fills | 3875 | -0.10 [-0.16, -0.04] | -0.15 | 0.05 [0.03, 0.07] | 0.003 |  |
| XAUUSD all fills | 5633 | -0.13 [-0.17, -0.08] | -0.19 | 0.06 [0.05, 0.08] | 0.003 |  |
| SOLUSDT GO at fill (descriptive) | 208 | 0.20 [-0.05, 0.45] | n/a | n/a [n/a, n/a] | n/a |  |
| ETHUSD GO at fill (descriptive) | 258 | -0.10 [-0.31, 0.12] | n/a | n/a [n/a, n/a] | n/a |  |
| XAUUSD GO at fill (descriptive) | 389 | -0.26 [-0.43, -0.05] | n/a | n/a [n/a, n/a] | n/a |  |
| SOLUSDT all fills (kev-default parser) | 7721 | -0.08 [-0.12, -0.05] | -0.13 | 0.05 [0.04, 0.07] | 0.003 |  |
| ETHUSD all fills (kev-default parser) | 10327 | -0.13 [-0.16, -0.10] | -0.17 | 0.04 [0.03, 0.05] | 0.003 |  |
| XAUUSD all fills (kev-default parser) | 15543 | -0.10 [-0.12, -0.07] | -0.14 | 0.05 [0.04, 0.06] | 0.003 |  |

### H1 space

| group | n | mean R [95% CI] | placebo | real - placebo [95% CI] | p | q |
|---|---|---|---|---|---|---|
| SOLUSDT space ok | 554 | 0.03 [-0.12, 0.17] | -0.14 | 0.17 [0.12, 0.23] | 0.003 | 0.003 |
| ETHUSD space ok | 697 | -0.15 [-0.28, -0.01] | -0.30 | 0.15 [0.09, 0.20] | 0.003 | 0.003 |
| XAUUSD space ok | 1063 | -0.20 [-0.30, -0.09] | -0.30 | 0.10 [0.05, 0.14] | 0.003 | 0.003 |
| ETH + gold pooled, space ok | 1760 | -0.18 [-0.26, -0.09] | -0.30 | 0.12 [0.08, 0.15] | 0.003 | 0.003 |

**Verdict: CONFIRMED** (rule: real - placebo > 0 in ETH and gold, pooled q < 0.05; rejected if the pooled CI is below +0.05R).

Net mean R of space-ok fills: SOLUSDT 0.03 [-0.12, 0.18] (n 554); ETHUSD -0.15 [-0.28, -0.01] (n 697); XAUUSD -0.20 [-0.30, -0.09] (n 1063).

Split and robustness (not part of the decision):

| group | n | mean R [95% CI] | placebo | real - placebo [95% CI] | p | q |
|---|---|---|---|---|---|---|
| SOLUSDT space ok, in-sample <= 2023 | 339 | 0.03 [-0.15, 0.24] | -0.15 | 0.19 [0.11, 0.28] | 0.003 |  |
| SOLUSDT space ok, out-of-sample | 215 | 0.02 [-0.21, 0.24] | -0.13 | 0.15 [0.06, 0.24] | 0.003 |  |
| ETHUSD space ok, in-sample <= 2022 | 364 | -0.14 [-0.32, 0.03] | -0.27 | 0.13 [0.05, 0.20] | 0.003 |  |
| ETHUSD space ok, out-of-sample | 333 | -0.15 [-0.34, 0.07] | -0.32 | 0.17 [0.10, 0.25] | 0.003 |  |
| XAUUSD space ok, in-sample <= 2018 | 536 | -0.13 [-0.26, 0.02] | -0.22 | 0.09 [0.03, 0.14] | 0.005 |  |
| XAUUSD space ok, out-of-sample | 527 | -0.28 [-0.42, -0.12] | -0.38 | 0.11 [0.05, 0.16] | 0.003 |  |
| SOLUSDT space ok (kev-default parser) | 2617 | -0.08 [-0.15, -0.02] | -0.14 | 0.06 [0.03, 0.09] | 0.003 |  |
| ETHUSD space ok (kev-default parser) | 4003 | -0.15 [-0.21, -0.10] | -0.20 | 0.05 [0.03, 0.06] | 0.003 |  |
| XAUUSD space ok (kev-default parser) | 5869 | -0.12 [-0.16, -0.07] | -0.18 | 0.07 [0.05, 0.08] | 0.003 |  |

### H2 touches

| market | parser | n touches>=3 | n touches=1 | (real-placebo | >=3) - (real-placebo | =1) [95% CI] | p | q |
|---|---|---|---|---|---|---|
| SOLUSDT | one-hour-spec | 370 | 2228 | -0.00 [-0.07, 0.07] | 0.897 | 0.953 |
| SOLUSDT | kev-default | 1906 | 3752 | -0.02 [-0.06, 0.01] | 0.200 | 0.953 |
| ETHUSD | one-hour-spec | 450 | 2677 | 0.01 [-0.05, 0.08] | 0.708 | 0.953 |
| ETHUSD | kev-default | 2246 | 5438 | -0.01 [-0.03, 0.02] | 0.752 | 0.953 |
| XAUUSD | one-hour-spec | 555 | 3832 | -0.00 [-0.05, 0.05] | 0.952 | 0.953 |
| XAUUSD | kev-default | 3343 | 8133 | -0.01 [-0.03, 0.02] | 0.488 | 0.953 |

**Verdict: as expected, no touch-count effect beyond 0.1R with q < 0.05.**

### H3 lookahead demonstration (random orders only)

| market | placebo fills | closed through | mean R closed through | mean R not | difference [95% CI] |
|---|---|---|---|---|---|
| SOLUSDT | 5423 | 2844 | -0.31 | 0.01 | -0.31 [-0.40, -0.21] |
| ETHUSD | 6540 | 3451 | -0.30 | 0.03 | -0.33 [-0.41, -0.25] |
| XAUUSD | 9617 | 5130 | -0.35 | -0.00 | -0.35 [-0.42, -0.28] |

**Verdict: CONFIRMED** (rule: difference < -0.2R in every market). Random orders whose fill bar closed through the price average SOLUSDT -0.31, ETHUSD -0.30, XAUUSD -0.35 R; the ones that did not average SOLUSDT 0.01, ETHUSD 0.03, XAUUSD -0.00 R.

### H4 settlement bias (finer bars minus conservative H1 rules, same trades)

| market | outcome | trades | settled with finer bars | outcomes changed | mean R finer | mean R conservative | bias [95% CI] |
|---|---|---|---|---|---|---|---|
| ETHUSD | S0.5_T2 | 3875 | 100% | 0 | -0.10 | -0.10 | 0.000 [0.000, 0.000] |
| ETHUSD | structural 2R | 3006 | 100% | 68 | -0.32 | -0.38 | 0.068 [0.053, 0.085] |
| XAUUSD | S0.5_T2 | 5633 | 100% | 2 | -0.13 | -0.13 | 0.001 [0.000, 0.003] |
| XAUUSD | structural 2R | 3645 | 100% | 70 | -0.23 | -0.28 | 0.058 [0.044, 0.074] |

### H5 sessions

Gold daily bars: 4333 NY-close days (3 with under 6 hours of bars) vs 5216 UTC days (886 under 6 hours); median daily ATR(20) 21.33 vs 18.43 (15.7% larger once Sunday stubs are gone).

| run | fills | mean R S0.5_T2 [95% CI] | placebo | structural 2R |
|---|---|---|---|---|
| XAUUSD NY-close (v2) | 5633 | -0.13 [-0.17, -0.08] | -0.19 | -0.23 |
| XAUUSD UTC days with Sunday stubs (old) | 6623 | -0.13 [-0.17, -0.08] | -0.18 | -0.22 |
| SOLUSDT UTC days (v2) | 3222 | -0.07 [-0.13, -0.01] | -0.16 | -0.25 |
| SOLUSDT NY-close days | 3251 | -0.07 [-0.13, -0.01] | -0.15 | -0.24 |
| ETHUSD UTC days (v2) | 3875 | -0.10 [-0.16, -0.04] | -0.15 | -0.32 |
| ETHUSD NY-close days | 3924 | -0.11 [-0.16, -0.05] | -0.16 | -0.32 |

