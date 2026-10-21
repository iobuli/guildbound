# Causality lessons from the backtest (`causality`)

What the user-supplied backtest forensics established. These are the reasons the graph forbids any node from using information that did not exist at the moment of the fill.

Decides at **prior_bars**. Inputs: none. Outputs: none.

## causality.fill-time-test

Every condition must answer: at the exact bar and price where the order fills, is this computable from bars strictly before it, or from that bar's open and the traded price alone? The fill bar's high, low and close are forbidden at fill time.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13). Note: This is what the graph's decideAt check and the view slicing enforce.

Supporting passages:
- [sources/user-research-pages.md:127](../sources/user-research-pages.md) (How to not do this a third time, score 81.51): "Every condition in the engine now has to answer one question before it is allowed in: **at the exact bar and price where the order fills, is this quantity computable from bars strictly before it, or from that bar’s open and the traded price"
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 24.54): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"
- [sources/user-research-pages.md:36](../sources/user-research-pages.md) (Experiment one — the reclaim gate, score 22.34): "If the condition reads the close, then the honest entry price **is** the close. Run it that way: the stop stays where it was, so risk is wider and the fill is worse. That is the trade you could actually have got."

## causality.level-held-is-an-exit

The "bar must close back on the break side" rule is an EXIT, not a filter. As a filter it showed Sharpe 5.13; used as an exit at the close it is 0.84, about six times smaller.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:50](../sources/user-research-pages.md) (Experiment two — the level-held rule, score 59.62): "Here there is a third option, and it matters. You cannot **filter** on the close, but you can **exit** on it. Take every retest fill; if the bar ends up closing on the wrong side, flatten at that close and wear the small loss. That is fully"
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 38.89): "| Rejected setups win 13.1% — the hold rule is a filter worth keeping | The Level-Held Filter | Half right. The 13.1% is real, but you cannot know which setups belong to that group until the bar has ended. It becomes an **exit**, not a filt"
- [sources/user-research-pages.md:13](../sources/user-research-pages.md) (A bar has four prices and only one of them is the last, score 23.35): "**The level-held rule** — take the trade only if the bar *closes* back on the break side.\"

## causality.reclaim-gate-rejected

Requiring the bar to close 0.75R past the level was rejected: a 90.5% win rate at Sharpe 6.71 described bars that had already gone the right way. Filled at the close it loses money.

*hard, backtest-rejected, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:36](../sources/user-research-pages.md) (Experiment one — the reclaim gate, score 73.89): "Verdict: rejected outright A 90.5% win rate at Sharpe 6.71 was the tell. Nothing in an hourly retest strategy wins nine times in ten. The gate does not predict the bar; it *is* the bar."
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 50.77): "| The reclaim gate — require the bar to close 0.75R past the level | never published | Rejected. It was caught before it reached a page, which is the only reason it is a footnote here rather than a sixth correction. |"
- [sources/user-research-pages.md:13](../sources/user-research-pages.md) (A bar has four prices and only one of them is the last, score 32.22): "**The reclaim gate** — take it only if the bar closes at least 0.75R past the level."

## causality.phantom-fill

An earlier bug booked entries at the level whether or not price traded there (about 60% of fills). It was caught only when TradingView returned 41.84% against 61.5%.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:26](../sources/user-research-pages.md) (One real hour, in the order it happened, score 80.92): "Why this is the same bug as the phantom fill Earlier in this project, entries were booked *at the level* whether or not price ever traded there — roughly 60% of fills at prices that never printed. That was caught only because the Pine scrip"
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 19.66): "| Rejected setups win 13.1% — the hold rule is a filter worth keeping | The Level-Held Filter | Half right. The 13.1% is real, but you cannot know which setups belong to that group until the bar has ended. It becomes an **exit**, not a filt"
- [sources/user-research-pages.md:13](../sources/user-research-pages.md) (A bar has four prices and only one of them is the last, score 19.51): "The system buys at a level. The level is a price, price reaches it mid-bar, the order fills. That part is sound. The problem is what happened next."

## causality.direction-from-cluster-kind

Direction from the cluster kind (a broken high means go long on the retest) stands. It was measured on both sides of every setup independently.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 50.72): "| Direction from the cluster kind — a broken high means go long | What Direction Is Actually Right | Stands. Unaffected by the fill question, because it was measured on both sides of every setup independently. |"
- [sources/course/curated/taking-a-trade-1-touch-level.md:1](../sources/course/curated/taking-a-trade-1-touch-level.md) (1-touch-level, score 13.13): "this adds our third and final confluence to the long setup [image]"
- [sources/levels-webinar-part1.md:257](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 13.11): "[33:54] to me because I found they have the the best chance of success in the long term is that if it's a three touch I'll trade"

## causality.space-rule-stands

The space rule (price travels 0.5 x ATR clear before a retest counts) stands and is described as the single most important rule in the system.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 38): "| The space rule — price must travel 0.5×ATR clear before a retest counts | the source webinar | Stands, and is the single most important rule in the system. It uses only bars that closed before the fill. |"
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 16.41): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"
- [sources/user-research-pages.md:179](../sources/user-research-pages.md) (Does it survive out of sample, score 14.63): "The published system audit and every figure in it used wait 12, so all of it needs rerunning before wait 1 becomes the default. And the **same-bar cluster collision** — one price carrying both a long and a short signal — has not been rechec"

## causality.twelve-bar-blind-spot

Refusing any retest within 12 bars of the break threw away the sharpest setups; waiting only 1 bar won both halves on gold, EURUSD and USDJPY.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied twelve-bar page. Note: The page itself says every figure in the audit used wait 12 and needs rerunning, and it pre-dates the lookahead fix. Do not treat as settled.

Supporting passages:
- [sources/user-research-pages.md:136](../sources/user-research-pages.md) (The Twelve-Bar Blind Spot, score 49.05): "The engine refused any retest arriving within `12` bars of a break. Those are the sharpest setups — price closes through the level, snaps straight back, and you fill at the level with the break candle as your stop. **It was throwing away it"
- [sources/user-research-pages.md:179](../sources/user-research-pages.md) (Does it survive out of sample, score 24.77): "**What changed.** One parameter: the minimum number of bars between a break and a qualifying retest. Everything else is untouched — 3-touch levels clustered within 0.1× the prior day's ATR, entry at the level price, stop at the break candle"
- [sources/user-research-pages.md:158](../sources/user-research-pages.md) (Does it survive out of sample, score 23.53): "Seven values were swept, so the result is worthless unless it holds in data nothing was fitted to. Each record split in half: **wait 1 wins both halves on all three long-history instruments**, and decays less doing it."

## causality.sol-transfer

SOL was never used to fit a parameter: the configuration was set on gold and two FX pairs and dropped on SOL unchanged.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:98](../sources/user-research-pages.md) (Per instrument, score 56.46): "SOL was never used to fit a single parameter — the whole configuration was set on gold and the two FX pairs, then dropped on SOL unchanged."
- [sources/user-research-pages.md:228](../sources/user-research-pages.md) (Rebuild it yourself, score 13.62): "**Method.** Pool universe sampled across new, trending (5m/1h/6h/24h) and top-volume endpoints so the set is not one cohort. For each pool, the recent trade tape keeps tx_from_address. That builds a wallet→pool bipartite graph, projected to"
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 12.46): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"

## causality.unmeasured-costs

Slippage is a flat 0.10R everywhere and has not been measured; funding and depth limits on a crypto venue are not modelled.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:98](../sources/user-research-pages.md) (Per instrument, score 62.07): "Still unmeasured Slippage is charged at a flat 0.10R everywhere. On gold in thin hours, and on a crypto venue with funding and real depth limits, that number needs measuring rather than assuming. Nothing here should be sized until it is."

## Conflicts

- `causality.twelve-bar-blind-spot` vs `causality.space-rule-stands`: One page says wait 1 bar after the break, the other says price must travel 0.5 ATR clear first. They can both hold (distance, not time) but the time claim is untested after the lookahead fix.
