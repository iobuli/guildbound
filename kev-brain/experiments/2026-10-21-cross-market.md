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
