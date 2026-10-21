# Stop placement

`stop` · stage **E** · decides at **prior_bars** · computed

Suggests where the stop belongs (beyond the thrust candle, never into a level, wider on crosses) and audits a stop you propose against the same rules.

## Ports

**Inputs**
- `level` (from level.level): found: boolean, role: object, center: number, zoneLo: number, zoneHi: number, breakIdx: number,null, breakT: number,null, barsSinceBreak: number, touches: number, lastTouchT: number,null, lxpbPrice: number,null, spanBars: number, chopCrossings: number, disrespected: boolean, tradable: boolean
- `atr` (from atr.atr): d1: number, h1: number, h1_24: number
- `swings` (from swing-depth.swings): h1: object[], d1: object[]
- `sfp` (from sfp.sfp): h1: object[], d1: object[]
- `tuesday` (from tuesday-rto.tuesday): valid: boolean, dir: object, entry: number,null, stop: number,null, target: number,null, rr: number,null, reasons: string[]

**Outputs**
- `stop`: stop: number,null, suggested: number,null, method: string, shifted: boolean, riskDist: number, ok: boolean, squeezeLevel: boolean

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **stop.beyond-thrust**<br>On an S/R trade the stop goes beyond the thrust candle (the candle that broke the level). | hard | stated | yes | webinar: user-pasted notes (2024-09-30)<br>webinar: curated/trade-management.md (2024-09-24) |
| **stop.never-into-a-level**<br>The stop may not sit directly in front of ATR, an S/R level or the next key fib, where price could squeeze to it and reverse. Move it to a candle further left (beyond the level).<br>*Read here as: if a swing of the right side lies within 0.3 H1 ATR beyond the stop, place the stop just beyond that swing.* | hard | stated | yes | webinar: user-pasted notes (2024-09-30)<br>webinar: curated/notes-images.md (2024-09-23)<br>journal: curated/lxpb-trades-passed-atr.md (2024-12-29) |
| **stop.daily-long-hourly-under**<br>Long off a daily level with an hourly level just underneath: the stop goes under the hourly level, so it is not hit while price finds support there. | hard | stated | no | webinar: user-pasted notes (2024-09-25) |
| **stop.second-level-squeeze**<br>He likes a second, minor level between entry and stop as a second layer of protection if price squeezes. | preference | observed | yes | webinar: curated/notes-images.md (2024-09-10) |
| **stop.crosspairs-wider**<br>On cross pairs the stop goes further out (above the S/R level rather than just above the thrust candle), because they squeeze more.<br>*The 0.25 ATR extra is the builder's number.* | hard | observed | yes | webinar: curated/notes-images.md (2024-09-23) |
| **stop.zone-needs-whole-zone**<br>If there is no clear thrust candle through the whole level zone, move the stop one candle to the left to be safe. | hard | observed | no | webinar: curated/notes-images.md (2024-10-28) |
| **stop.protected-by-htf**<br>He likes the stop to be protected by higher-timeframe levels. In one trade the stop could not go above the daily level and was protected only by the weekly one. | preference | observed | no | journal: curated/lxpb-trades-passed-atr.md (2025-04-22)<br>journal: curated/lxpb-trades-passed-atr.md (2025-11-13) |
| **stop.risky-stop-is-fine-if-idea-is-wrong**<br>A stop over double lows is "risky", but if price gets there the idea is wrong anyway. | hard | observed | no | journal: curated/lxpb-trades-htf-level.md (2026-02-19) |
| **stop.sfp-atr**<br>SFP trades: stop at the sweep extreme plus the 24-period ATR. He also uses a tighter "gap" technique when it still clears the old high.<br>*The "gap SL technique" is mentioned but never described.* | hard | stated | yes | webinar: user-pasted notes (2024-09-24)<br>webinar: curated/advanced-sfp.md (2024-09-24) |
| **stop.just-beyond-not-one-tick**<br>For failed-breakdown trades on the M5 Bund: the stop goes just under the lows, "not 1 tick under, but just under somewhere"; no ATR or thrust candle needed. | hard | stated | no | webinar: curated/notes-images.md (2024-09-25) |
| **stop.min-distance**<br>A minimum stop of 0.05% of price is used in the backtest. | hard | backtest-reported | yes | backtest: user-supplied twelve-bar page |
| **stop.grace-two-bars**<br>A 2-bar grace window at a 2x stop is now worth only about 0.05 Sharpe at a 4R target: keep it, do not lean on it. | hard | backtest-reported | no | backtest: user-supplied forensic page (2026-09-13) |
| **stop.would-i-still-be-bullish**<br>The stop question: am I stopping out into an area where I would STILL be bullish? Resistance flips to support, so look for resistance just beyond the level that price might squeeze to in the worst case, and put the stop just beneath it. Draw the stop and look at what you would be stopped out into. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @1:00:04<br>transcript: levels webinar part 1, user-pasted transcript @1:00:37<br>transcript: levels webinar part 1, user-pasted transcript @1:06:54 |
| **stop.as-tight-as-the-level-allows**<br>Keep the stop as close to market as you can, but no tighter than the squeeze point. There is usually no need for a very wide stop if the level was chosen correctly, and these trades should not show much drawdown. | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @1:00:20<br>transcript: levels webinar part 1, user-pasted transcript @1:01:42<br>transcript: levels webinar part 1, user-pasted transcript @1:01:57 |
| **stop.candle-proxy**<br>The usual stop is beneath the last candle that OPENED below and CLOSED above the level (the thrust candle). If that puts the stop into an area price could squeeze to, move to a candle further left and check again. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @1:01:00<br>transcript: levels webinar part 1, user-pasted transcript @1:01:18<br>transcript: levels webinar part 1, user-pasted transcript @1:07:09 |
| **stop.cross-definition**<br>A "cross" is any pair without the USD in it. He uses a wide stop on crosses because they are not clean, and tells people to be careful with them. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @1:06:30<br>transcript: levels webinar part 1, user-pasted transcript @1:07:39<br>transcript: levels webinar part 1, user-pasted transcript @1:08:40 |
