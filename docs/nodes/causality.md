# Causality lessons from the backtest

`causality` · stage **X** · decides at **prior_bars** · rule card (nothing to compute)

What the user-supplied backtest forensics established. These are the reasons the graph forbids any node from using information that did not exist at the moment of the fill.

## Ports

**Inputs**
- none

**Outputs**
- none

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **causality.fill-time-test**<br>Every condition must answer: at the exact bar and price where the order fills, is this computable from bars strictly before it, or from that bar's open and the traded price alone? The fill bar's high, low and close are forbidden at fill time.<br>*This is what the graph's decideAt check and the view slicing enforce.* | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |
| **causality.level-held-is-an-exit**<br>The "bar must close back on the break side" rule is an EXIT, not a filter. As a filter it showed Sharpe 5.13; used as an exit at the close it is 0.84, about six times smaller. | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |
| **causality.reclaim-gate-rejected**<br>Requiring the bar to close 0.75R past the level was rejected: a 90.5% win rate at Sharpe 6.71 described bars that had already gone the right way. Filled at the close it loses money. | hard | backtest-rejected | no | backtest: user-supplied forensic page (2026-09-13) |
| **causality.phantom-fill**<br>An earlier bug booked entries at the level whether or not price traded there (about 60% of fills). It was caught only when TradingView returned 41.84% against 61.5%. | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |
| **causality.direction-from-cluster-kind**<br>Direction from the cluster kind (a broken high means go long on the retest) stands. It was measured on both sides of every setup independently. | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |
| **causality.space-rule-stands**<br>The space rule (price travels 0.5 x ATR clear before a retest counts) stands and is described as the single most important rule in the system. | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |
| **causality.twelve-bar-blind-spot**<br>Refusing any retest within 12 bars of the break threw away the sharpest setups; waiting only 1 bar won both halves on gold, EURUSD and USDJPY.<br>*The page itself says every figure in the audit used wait 12 and needs rerunning, and it pre-dates the lookahead fix. Do not treat as settled.* | hard | backtest-reported | no | backtest: user-supplied twelve-bar page |
| **causality.sol-transfer**<br>SOL was never used to fit a parameter: the configuration was set on gold and two FX pairs and dropped on SOL unchanged. | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |
| **causality.unmeasured-costs**<br>Slippage is a flat 0.10R everywhere and has not been measured; funding and depth limits on a crypto venue are not modelled. | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |

## Conflicts

- `causality.twelve-bar-blind-spot` vs `causality.space-rule-stands`: One page says wait 1 bar after the break, the other says price must travel 0.5 ATR clear first. They can both hold (distance, not time) but the time claim is untested after the lookahead fix.
