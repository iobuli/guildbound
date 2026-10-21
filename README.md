# kev-nodes

A trader's method, rebuilt as a graph of isolated nodes that an LLM agent can call. It is a tool for Claude to use, not an app for a person.

- **23 nodes**, one per idea (level scanner, market side, level, retest, bias, SFP, stop, target, news, risk, management, ...), each owning its rules.
- **205 rules**, each with its sources and a status (stated / observed / inferred / backtest-reported / backtest-rejected). 98 of them are tested by code. 113 cite the trader's own webinar transcript with timestamps.
- **Isolated**: a node file imports only `core/`. Nodes never name each other. All wiring is in `src/kev.ts`.
- **Typed ports**: every wire is a zod schema, checked when the graph loads and again on every run.
- **Causal by construction**: each node declares when its answer becomes knowable (`prior_bars`, `bar_open`, `fill_time`, `bar_close`, `post_fill`). The graph refuses a wire where a node needs something that is not knowable when it decides, and a node's view contains only the bars that had closed by then. This is the lesson of the "close that hadn't printed yet" forensic page: a filter that reads the fill bar's close cannot be wired to an order that fills mid-bar.
- **Follows the trader's own framework**: Direction, Objection, Location, Execution (plus Management, Risk, Context).

```
npm test        # 50 checks: each node alone, isolation, causality, lookahead audits on SOL
npm run docs    # regenerates docs/ from the node definitions (graph, per-node pages, conflicts, sources)
npm run replay  # the whole graph over SOL 2022-2026
npm run backtest  # resting-order backtest of the whole graph vs placebo and shuffled SOL (~90s) -> out/backtest.md
npm run optimize  # walk-forward search of the builder's thresholds, with the same search on shuffled SOL
npm run mcp     # MCP server: kev_levels, kev_evaluate, kev_rules, kev_node, kev_nodes, kev_graph
```

## The order it runs in
The graph evaluates, and `kev_evaluate` reports, in the trader's order of operations, and names the first step that blocks the trade:

0.5 mark the levels and count their swing touches → 1 which side of the market (buy below, sell above) → 2 inverse-last level, respected last time → 3 tier by touches (3+ blind, 2 needs one confluence, 1 needs two: 3.1 daily bias, 3.2 passed daily ATR, fib 38/50/61) → 4 space, not the immediate retest → 5 news → 6 stop → 7 target (first trouble area) → 8 risk → 9 entry → 10 management.

This covers only levels where price goes the OPPOSITE way to the last participants. The second webinar (buying support, including the SFP) is not in the sources.

## Read these first
- `docs/graph.md` the wiring and stages
- `docs/conflicts.md` where the sources disagree (kept, not resolved)
- `docs/sources.md` what the knowledge rests on
- `docs/nodes/*.md` one page per node, generated from the code so they cannot drift

## What it is not
The knowledge is a third party's notes of a discretionary forex/index trader, plus two user-supplied backtest pages that were not reproduced. Nothing was developed on crypto. Several thresholds are the builder's, marked `inferred` in `src/core/params.ts` and in the rules. The replay on SOL (`out/replay.json`) found no edge in the SFP or daily-bias signals.

## What the backtest found (SOL/USDT H1, 2021 to Oct 2026, 7,101 fills, costs 0.10R per trade)
Resting limit orders at each flipped level's last-high/low-before-breakout, filled on touch, stop from the `stop` node, stop wins same-bar ties. Day-clustered bootstrap CIs. Harness verified to have no lookahead (`npm test`).

- **Net result is negative everywhere**: about -0.18R per trade at a 2R target (-0.13R at 4R), in-sample and out-of-sample.
- **Real levels beat random orders** (same side, distance, risk, window) by about +0.11R, p < 0.01, also out-of-sample. Part of that may be the structural stop rather than the level.
- **The system's own GO verdict adds nothing** (-0.01R, p = 0.75). Touch tiers show no ordering. No single rule survives multiple-comparison correction.
- **Memoryless SOL loses about the same** (-0.17 to -0.21R), so most of the loss is cost and stop geometry.
- **Tuning the invented thresholds does not help.** Best of 18 in-sample (-0.125R) is no better than best of 18 on shuffled SOL (-0.131R) and is worse out-of-sample than the untuned default (-0.235R vs -0.195R). Defaults were left unchanged.

Limits: events overlap (several fills per day), swerve cancellation is not simulated, intrabar order is unknowable (ties go to the stop), and the same-direction half of the method (second webinar) is not in the sources.

## Gold (XAUUSD, 2010-2026): M30, H1 and H4 bases
Data: the user's MT4 exports (tab-separated, tick volume). The two H1 files are byte-identical. M30, H1, H4 and D1 agree exactly (M30 to H1, H1 to H4 and H1 to D1 match on every bar), so they are ONE market at four resolutions, not four samples. Week starts Sunday evening; costs are 1.2 bps spread + 0.10R slippage. In-sample 2010-2018, out-of-sample 2019-2026.

| Base | Fills | Mean net R at 4R | Random-order placebo | Real minus placebo | One-at-a-time Sharpe |
|---|---|---|---|---|---|
| H4 | 5,824 | -0.12 | -0.21 | +0.08 | -1.00 |
| H1 | 13,333 | -0.15 | -0.26 | +0.10 | -1.11 |
| M30 | 19,818 | -0.13 | -0.28 | +0.14 | -1.52 |

- **Costs decide it.** H1 at 4R: zero costs -0.003R, spread only -0.053R, full costs -0.153R.
- **Block-shuffled gold loses about the same** (H1 -0.16R).
- **Go/no-go and touch tiers do not help**; GO is worse than NO-GO on H1 (-0.08R, p = 0.04).
- **Failed-hold exit and 3+ touches only**, the closest this graph gets to the user's causal build, do not help (-0.18R and -0.15R at 4R).
- **Tuning the invented thresholds does not help**: best-of-18 in-sample (-0.145R) is no better than best-of-18 on shuffled gold (-0.171R); the spread of out-of-sample results across the 18 configs (-0.09 to -0.28) is as wide as the same spread on shuffled gold.
- One rule survives correction on H1 with the opposite sign: fills after price had already moved an ATR in the trade's direction did better (q = 0.018). It is a lead, not a finding.
- This does not reproduce the positive result on the user's forensic page. Differences: that engine enters at the level price, uses 3-touch clusters within 0.1 x prior-day ATR, stops at the break candle's extreme, and has a 2x stop with grace. A trade-by-trade comparison needs its trade list.

Commands: `npx tsx src/backtest/gold.ts --tf=h1|m30|h4`, `gold-gross.ts` (cost split), `gold-variants.ts`, `gold-opt-driver.ts` (parallel search). Reports are in `out/gold_*.md|log|json`. `kev_levels` and `kev_evaluate` take `symbol` = SOLUSDT or XAUUSD.
