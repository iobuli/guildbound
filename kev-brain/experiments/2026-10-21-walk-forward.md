# Iteration 1: kev picks its own rules, walk-forward (pre-registered 2026-10-21, before any run)

## Why
The frozen paper book (2026-10-21-paper-book.md) lost money: $100k became $83,079. Its random-order book did worse ($65,610).
2023 onward has now been seen, so tuning rules on it would overfit. Instead, the iteration is a *process*. Each year, kev
chooses per market from a fixed menu using only earlier data, and may choose "do not trade". The walk-forward result is an honest
estimate of how that process does on data it has not seen. The truly fresh test is the live book from today onward.

## Menu (fixed now; 10 filters x 5 exits = 50 configs per market)
- Filters:
  - all;
  - space ok;
  - GO;
  - trendPct agrees with side;
  - daily bias (htf) agrees;
  - space + trendPct;
  - space + htf;
  - GO + trendPct;
  - volatility rank < 0.5;
  - volatility rank >= 0.5.
- Exits:
  - stop 0.5 ATR with targets 1R, 2R or 4R;
  - stop 1 ATR with targets 2R or 4R.
  - The structural stop (`fta`) was dropped before the run: random orders have no structure, so it has no fair placebo.
- Costs as in kev's profiles. The fill set is the one-hour-spec preset; nothing in the TypeScript changes.

## Process
- **Years traded:** 2021 to 2026.
- **Selection:** for each year Y and market M, every config is scored on M's fills before Y-01-01 (expanding window, from M's first bar).
  - Only configs with at least 40 training trades count.
  - Score: deflated Sharpe ratio of per-trade R, with the trial count set to the configs that count, so a lucky pick from 50 is penalised.
- **Primary rule:** trade the top config in year Y only if its DSR >= 0.90; otherwise M sits out year Y.
- **Sensitivity (not the verdict):** the same with DSR >= 0.50.
- **Book:**
  - one $100k account across markets, 0.5% risk per trade;
  - at most one position per market, and 1.5% total open risk;
  - the same choices are applied to matched random orders (the placebo book).
- **Verdict:**
  - The process works if its 2021-2026 book ends above $100k AND above its placebo book.
  - A book that sits out everything is reported as "kev correctly found nothing to trade", not as a success.
- **After the run:** freeze the choice made with all data up to today as `live-v1` and paper-trade it on new SOL hours (the only live feed). That is the only clean out-of-sample test left.

## Results (appended 2026-10-21 by kev-lab/run.py)
- **Primary (DSR >= 0.90):** no market cleared the bar in any year, so the book never traded and stayed at $100,000. This is reported as "kev correctly found nothing to trade", not as a success.
- **Sensitivity (DSR >= 0.50):** only SOL traded, with GO (+trend in 2023) and exits at stop 0.5 ATR / 1R target, in 2022-2025.
  - 77 trades; $100k became $98,118 (mean -0.05R, max drawdown -6%).
  - Random orders under the same choices: $95,813 (-0.13R).
  - SOL's training mean was +0.20 to +0.47R. What followed, year by year:

| year | trades | mean R |
|---|---|---|
| 2022 | 18 | +0.41 |
| 2023 | 21 | -0.15 |
| 2024 | 24 | +0.07 |
| 2025 | 14 | -0.67 |

- **ETH and gold:** the best of the 50 configs had a negative or zero training mean in almost every year (DSR about 0). Nothing on the menu works there.
- **Live choice (all data to today, DSR >= 0.90):** sit out every market. The best is SOL GO+trend at 0.5 ATR / 2R (training +0.27R, n 115, DSR 0.59), below the bar.

**Verdict: the process does not work.** It never ends above $100k. It does lose less than random orders, which is the same relative edge seen before.
- The selection step behaves as designed: it removed ETH and gold, the main source of the frozen book's loss.
- SOL's in-sample "best of 50" edges shrink to about 0 out of sample, which is the decay the deflated Sharpe warns about.
- What kev's levels add is real but small: +0.05 to +0.25R relative to random. That is not enough to pay for costs and 0.5-ATR stops.
