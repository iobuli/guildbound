# Higher-timeframe bias (D1 / W1)

`daily-bias` · stage **D** · decides at **prior_bars** · computed

Reads closed daily and weekly candles for a pinbar, engulfing, SFP or inside-day failure, cancels opposing signals on the same bar, and expires the bias after a couple of days.

## Ports

**Inputs**
- `sfp` (from sfp.sfp): h1: object[], d1: object[]

**Outputs**
- `bias`: d1: object, w1: object, htf: long|short|neutral

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **daily-bias.types**<br>Daily bias comes from a pinbar, an engulfing candle, an SFP, or (in the later lessons) an inside-day failure. The 2012-era webinar names only the first three. | hard | stated | yes | webinar: user-pasted notes (2024-09-24)<br>lesson: curated/taking-a-trade-daily-biases.md (2024-10-18)<br>transcript: levels webinar part 1, user-pasted transcript @37:07 |
| **daily-bias.crossbar-cancels**<br>Two opposing biases on one bar (a bullish engulfing with a bearish SFP) cancel out and the market is neutral. | hard | stated | yes | webinar: user-pasted notes (2024-09-24)<br>webinar: curated/notes-images.md (2024-09-08) |
| **daily-bias.expires**<br>A daily-engulfing bias stops counting after a couple of days if no entry is given. | hard | stated | yes | webinar: curated/notes-images.md (2024-09-10) |
| **daily-bias.weekly-and-daily-then-h1**<br>W1/D1 give the bias, H1 gives the execution, and he never looks lower. | hard | stated | no | tweet: curated tweets (2026-09-24)<br>webinar: user-pasted notes (2024-09-24) |
| **daily-bias.only-20-percent**<br>Only about 20% of his trades have a daily bias. The other 80% go straight to H1 and are quick in-and-out trades at 0.8-1R on average. | heuristic | stated | no | webinar: user-pasted notes (2024-09-24) |
| **daily-bias.against-it**<br>You can trade against the daily bias, but the levels must be good and you must get out at the first sign of trouble. An H1 short is normally skipped if the day looks set to close as a bullish engulfing, unless the target is very close. | hard | stated | no | webinar: curated/trade-management.md (2024-10-18)<br>webinar: user-pasted notes (2024-09-25) |
| **daily-bias.reverse-trading**<br>Start from where price is going and why, then look for an entry that fits that bias. | hard | observed | no | journal: curated/lxpb-trades-what-constitutes-a-daily-bias.md (2026-02-07) |
| **daily-bias.candle-definitions**<br>The exact candle definitions (pin wick share, engulfing test) are not given in the sources. This build uses generic textbook thresholds. | heuristic | inferred | yes | lesson: curated/taking-a-trade-daily-biases.md (2024-10-18) |
| **daily-bias.unclosed-anticipation**<br>In his journal he sometimes acts on a daily or weekly candle that has not closed yet ("looks like it might close as a bullish engulfing"). | hard | observed | no | journal: curated/lxpb-trades-passed-atr.md (2024-12-21)<br>journal: curated/lxpb-trades-what-constitutes-a-daily-bias.md (2026-02-07) |
| **daily-bias.closed-bars-only**<br>This build only reads CLOSED daily and weekly bars, so an anticipated but unclosed candle is never used. | hard | inferred | yes | backtest: user-supplied forensic page (2026-09-13) |
| **daily-bias.bar-pattern-not-trend**<br>The bias must be a bar pattern on the daily chart, never just the trend: a pinbar, a swing failure (swing low broken and price closes above, or the reverse) or an engulfing. Silver example: a bullish pinbar that was also a swing failure gave "a strong directional bias" because breaking the range low and closing back inside usually takes price to the other side of the range. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @35:16<br>transcript: levels webinar part 1, user-pasted transcript @36:03<br>transcript: levels webinar part 1, user-pasted transcript @37:47 |

## Conflicts

- `daily-bias.unclosed-anticipation` vs `daily-bias.closed-bars-only`: The discretionary journal anticipates unclosed daily candles; the causal rule forbids it. A systematic version cannot act on a candle that has not printed.
