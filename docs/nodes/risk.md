# Risk and sizing

`risk` · stage **R** · decides at **fill_time** · computed

Turns the stop distance into a position size for a fixed percentage risk. The rules about adding and not cutting size live here as rules.

## Ports

**Inputs**
- `stop` (from stop.stop): stop: number,null, suggested: number,null, method: string, shifted: boolean, riskDist: number, ok: boolean, squeezeLevel: boolean

**Outputs**
- `risk`: riskAmount: number, riskDistance: number, size: number, ok: boolean

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **risk.always-one-r**<br>"Always 1R. I never cut size." Cutting size right after a loss makes no sense. 1R can shrink as a share of the account in a drawdown because risk is a fixed percentage. | hard | stated | no | tweet: curated tweets (2026-09-29) |
| **risk.size-from-stop**<br>Position size = account risk / stop distance, with risk a fixed percentage of the account.<br>*The backtest used 0.5% risk per trade; the percentage in live use is not stated.* | hard | inferred | yes | tweet: curated tweets (2026-09-29)<br>backtest: user-supplied twelve-bar page |
| **risk.adding-keeps-risk-controlled**<br>Add to a trade that is confirming your thesis if the addition fits the plan, the upside is still asymmetric and total risk stays controlled. Adding on H1 price action to a blind D1 bid leaves risk at 1R overall because the first stop is near breakeven. | hard | stated | no | tweet: curated tweets (2026-06-03)<br>tweet: curated tweets (2026-05-01) |
| **risk.review-in-drawdown**<br>Reviewing strategies in a -10R or -20R drawdown. | heuristic | observed | no | webinar: curated/notes-images.md (2024-09-30) |
| **risk.correlated-setups**<br>Rule 4, "know when to break the rules": he took a second highly correlated setup (breaking his usual rule) only because he had seen exactly one loss under that setup. | heuristic | stated | no | webinar: user-pasted notes (2024-09-24) |
| **risk.slippage-unmeasured**<br>The backtest charged a flat 0.10R of slippage. That figure is unmeasured, especially for gold in thin hours and a crypto venue with funding and depth limits. "Nothing here should be sized until it is." | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |
| **risk.long-losing-runs**<br>A 29% win rate means long stretches of being wrong; the worst losing run is the price of the far target. | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |
