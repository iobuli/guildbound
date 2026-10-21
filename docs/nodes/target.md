# Targets and first trouble area

`target` · stage **O** · decides at **prior_bars** · computed

Finds where the trade is likely to meet trouble (the FTA, the most recent respected level, the higher-timeframe target, or the other side of a range) and checks the reward-to-risk.

## Ports

**Inputs**
- `stop` (from stop.stop): stop: number,null, suggested: number,null, method: string, shifted: boolean, riskDist: number, ok: boolean, squeezeLevel: boolean
- `level` (from level.level): found: boolean, role: object, center: number, zoneLo: number, zoneHi: number, breakIdx: number,null, breakT: number,null, barsSinceBreak: number, touches: number, lastTouchT: number,null, lxpbPrice: number,null, spanBars: number, chopCrossings: number, disrespected: boolean, tradable: boolean
- `swings` (from swing-depth.swings): h1: object[], d1: object[]
- `bias` (from daily-bias.bias): d1: object, w1: object, htf: long|short|neutral
- `atr` (from atr.atr): d1: number, h1: number, h1_24: number
- `tuesday` (from tuesday-rto.tuesday): valid: boolean, dir: object, entry: number,null, stop: number,null, target: number,null, rr: number,null, reasons: string[]

**Outputs**
- `target`: candidates: object[], chosen: object, minRr: number

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **target.fta**<br>Take profit at the First Trouble Area: the first place on the chart where price might meet trouble. Closing there is more consistent than trying to run every trade. In the 2012 webinar the example target was "the next trouble area, about a 30 pip move". | hard | stated | yes | lesson: curated/taking-a-trade-targets.md (2024-10-22)<br>transcript: levels webinar part 1, user-pasted transcript @53:38 |
| **target.most-recent-over-first**<br>Price tends to gravitate to the most recent level, so the first level in the way is not always the FTA. He often targets recent price action over older, more obvious levels. | heuristic | observed | yes | lesson: curated/taking-a-trade-targets.md (2024-10-22)<br>webinar: curated/tp-selection.md (2024-09-24)<br>journal: curated/lxpb-trades-passed-atr.md (2025-04-03) |
| **target.ignore-tiny-swings**<br>A tiny one-touch swing high is not the FTA. If it were larger or had more taps it would be.<br>*Swings under 0.5 H1 ATR deep are ignored here; the source gives no number.* | hard | observed | yes | webinar: curated/tp-selection.md (2024-09-24) |
| **target.htf-bias-htf-target**<br>With a daily bias the target is on the daily too, and an H1 level in the way is not worried about. "D1 for bias, D1 for TP." | hard | stated | yes | webinar: curated/tp-selection.md (2024-09-24) |
| **target.no-bias-fta-or-range**<br>With no higher-timeframe bias, take profit at the FTA; with no FTA either, aim for the other side of the range. After a false break of a range, price usually goes to the other side. | hard | stated | yes | webinar: user-pasted SFP notes (2024-09-24)<br>webinar: curated/tp-selection.md (2024-09-24) |
| **target.inverse-last**<br>Target the inverse-last level: where the market last found support (for a short) or resistance (for a long). | hard | stated | no | webinar: curated/tp-selection.md (2024-09-24) |
| **target.sections**<br>With a daily bias he splits the take-profit across the hourly levels in the way (five levels: sell a fifth at each) and re-enters each fifth when the level flips and retests. | hard | observed | no | webinar: curated/tp-selection.md (2024-10-18) |
| **target.minimum-rr**<br>Minimum reward-to-risk is about 0.8R, and sometimes 0.5R in a high-probability setup. If the evolving R gets "ridiculously low" he closes. | hard | stated | yes | webinar: user-pasted SFP notes (2024-09-24)<br>webinar: curated/trade-management.md (2024-09-24) |
| **target.take-the-sure-thing**<br>Take a smaller target when it has confluence on the left and the RR is already good enough: "take the sure bet". | preference | observed | no | journal: curated/lxpb-trades-passed-atr.md (2025-01-03)<br>webinar: curated/tp-selection.md (2024-09-24) |
| **target.look-further-left**<br>Look further left than you think before picking an FTA: an older level that is still being respected can be the real one. | hard | observed | no | webinar: curated/tp-selection.md (2024-09-24) |
| **target.far-targets-pay**<br>Causal backtest: with the lookahead filters removed, win rate is about 29% and the system wants a far target, 3-5R, with a broad plateau. 1.0R loses money. | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |

## Conflicts

- `target.minimum-rr` vs `target.far-targets-pay`: The teacher's discretionary style is quick in-and-out trades near the first trouble area (0.8-1.2R). The causal backtest says the same entries only pay at 3-5R. They are different styles; do not mix their stops and targets.
