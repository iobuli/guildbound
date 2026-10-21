# 2026-10-21: the $100k paper book

**Status: pre-registered before any candidate was scored.** The results are appended below by kev-lab and must not change anything above them.

## Rules (fixed)
- **Signals:** `kev signals` with preset `one-hour-spec`: every causal fill in SOLUSDT, ETHUSD and XAUUSD. Each fill uses a limit at the level, a stop 0.5 x prior-day ATR beyond the fill, and a 2R target (`S0.5_T2`). R is net of the profile's spread and 0.10R slippage.
- **Candidates:** four trials, all counted in the deflated Sharpe ratio:
  - A: every fill.
  - B: space ok (price moved 0.5 ATR clear of the level before the return, earlier bars only). This is the confirmed H1 result.
  - C: B, plus the percentage trend at the fill agrees with the trade (long in an up trend, short in a down trend). This is the audit's fix.
  - D: GO (the graph's own verdict).
- **Book:**
  - $100,000 start, risking 0.5% of current equity per trade;
  - at most one open position per market, and at most 1.5% of equity at risk across open positions;
  - a fill is skipped if it breaks either limit;
  - profit or loss is booked at the exit time.
- **Selection window:** fills from 2020-01-01 to 2022-12-31, all three markets. The candidate with the highest deflated Sharpe ratio (4 trials, daily P&L) is chosen and frozen.
- **Forward window:** 2023-01-01 to the latest bar. The chosen candidate runs once on it, with no changes. The same filter applied to the matched random orders gives a placebo book for comparison.
- **Honesty notes:**
  - SOL 2024+ and ETH 2023+ were seen as out-of-sample in earlier studies. Those were descriptive comparisons; nothing was tuned on them.
  - Gold's latest bar is 2026-07-27.
  - The result is reported whatever it is.

## Results (appended 2026-10-21 by kev-lab/run.py)

Selection on 2020-2022 (all candidates lost money; D had the least-bad deflated Sharpe):

| candidate | trades | mean R | final | Sharpe | DSR |
|---|---|---|---|---|---|
| A every fill | 1216 | -0.10 | $51,411 | -1.47 | 0.00 |
| B space ok | 388 | -0.12 | $77,910 | -0.99 | 0.01 |
| C space ok + trend agrees | 179 | -0.15 | $87,439 | -0.74 | 0.02 |
| D GO (chosen) | 167 | -0.10 | $91,769 | -0.51 | 0.05 |

Forward 2023-01-01 to now, D GO, untouched:
- $100,000 became **$83,079**: 239 trades, mean -0.15R, 32% wins, CAGR -4.8%, Sharpe -0.79, max drawdown -24.2%, PSR 0.06.
- By market: ETH -$1,649 (-0.03R), SOL -$2,428 (-0.06R), gold -$12,844 (-0.40R).
- Placebo book (the same GO filter on matched random orders): $65,610, mean -0.40R. So kev's levels beat random by about 0.25R per trade, but the book still loses.
- Stationary bootstrap (600 paths): P(ending below $100k) 94%; final 5/50/95% $69k / $83k / $100k; max drawdown median -21%.

**Verdict:** no tradable edge. This is the same conclusion as every earlier test. The relative edge over random orders is real; the absolute edge after costs is not. Gold drives most of the loss.

Hurst (R/S) and variance ratio (24h) per market-year are all about 0.55 and about 1: hourly returns are close to a random walk in every year. There is no persistent trending or mean-reverting regime to exploit.
