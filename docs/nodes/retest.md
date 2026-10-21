# Retest quality and space

`retest` · stage **L** · decides at **prior_bars** · computed

Judges the return to a broken level: was there space after the break (and not the immediate retest), did price come in with momentum, and has the order been swerved (price ran away without filling it).

## Ports

**Inputs**
- `level` (from level.level): found: boolean, role: object, center: number, zoneLo: number, zoneHi: number, breakIdx: number,null, breakT: number,null, barsSinceBreak: number, touches: number, lastTouchT: number,null, lxpbPrice: number,null, spanBars: number, chopCrossings: number, disrespected: boolean, tradable: boolean
- `atr` (from atr.atr): d1: number, h1: number, h1_24: number
- `bias` (from daily-bias.bias): d1: object, w1: object, htf: long|short|neutral

**Outputs**
- `retest`: retested: boolean, barsBreakToRetest: number,null, excursionAtr: number, spaceOk: boolean, shape: rounded|sharp|none, approachAtr: number, momentum: hard|drift|none, tooSoon: boolean, rallyAway: boolean,null, swerves: number,null, cancelOrder: boolean

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **retest.needs-space**<br>There must be space between the break and the retest: price breaks, closes beyond the level on the hourly, and KEEPS GOING. The higher it keeps going, the more space; the example shown is "about the minimum" he wants.<br>*He says he cannot quantify it ("never been able to say you need 10 bars"). The two coded tests below are proxies.* | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @25:30<br>transcript: levels webinar part 1, user-pasted transcript @28:00 |
| **retest.never-immediate**<br>Never trade the immediate retest (the "buck kiss": the first close beyond the level followed by a bid). It is not reliable on the hourly, and he does not take it even when there is a daily bias. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @26:03<br>transcript: levels webinar part 1, user-pasted transcript @39:16 |
| **retest.needs-space-in-time**<br>As a rough proxy for space: 5 bars is not enough and an 11-candle retest is "just about enough", roughly half a day before an H1 entry. | heuristic | observed | yes | webinar: curated/notes-images.md (2024-09-16) |
| **retest.needs-space-in-price**<br>Price must travel at least 0.5 x ATR clear of the level before a retest counts. Described as the single most important rule in the system, and it uses only bars that closed before the fill. | hard | backtest-reported | yes | backtest: user-supplied forensic page (2026-09-13) |
| **retest.bias-relaxes-space**<br>With a daily bias you do not need to be obsessed with the amount of space, but you still want the break to have happened and price to round back to the area. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @38:49<br>webinar: curated/notes-images.md (2024-09-08) |
| **retest.clean-break-and-go**<br>The nicest trades break through and just keep going. He does not like any interaction with the level after the break: not break, retest, go, and come back. | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @29:30 |
| **retest.bounce-just-before-the-level**<br>If price bounced just before the level (missed by a few ticks), did not fill the order, and came straight back, do not take the trade: the market is not done with that level. | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @28:49<br>transcript: levels webinar part 1, user-pasted transcript @59:00 |
| **retest.momentum-into-level**<br>He likes momentum INTO the level. Buying a level the market is falling into hard is safer than buying one it drifts into slowly, though it is not a must. Reasons: pros take profits quickly after a fast drop, amateurs hold, and a sharp move can bounce like an elastic band. | preference | stated | yes | transcript: levels webinar part 1, user-pasted transcript @26:41<br>transcript: levels webinar part 1, user-pasted transcript @1:09:15<br>transcript: levels webinar part 1, user-pasted transcript @1:10:37 |
| **retest.drift-in-asia-is-fine**<br>A drift into the level is not a problem in the Asian session, where price drifts anyway. In London or the US session a drift would concern him. | preference | stated | no | transcript: levels webinar part 1, user-pasted transcript @1:12:04 |
| **retest.rounded-over-sharp**<br>A rounded retest is preferred over a sharp V-shaped one. | preference | stated | yes | webinar: user-pasted notes (2024-09-24)<br>lesson: curated/taking-a-trade-3-touch-level.md (2024-10-21) |
| **retest.missed-the-level-half-way**<br>If the order is missed and price bounces more than half way to the target, pull the order (a 10-15 pip bounce alone does not). The bounce creates a swing point, stops accumulate beyond it, and buying back there means buying as people stop out. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @53:21<br>transcript: levels webinar part 1, user-pasted transcript @54:04<br>webinar: curated/trade-management.md (2024-09-24) |
| **retest.swerve-exception**<br>Exception: with a three-touch level and a daily bias he leaves the order in through a swerve, because the daily target is far away. | hard | observed | yes | webinar: curated/trade-management.md (2024-09-24) |
| **retest.four-swerves**<br>Four swerves of the level should cause the long to be cancelled. | hard | observed | yes | webinar: curated/trade-management.md (2024-09-30) |
| **retest.naked**<br>A level is "naked" while it has not been retested on the H1 since the break. | hard | stated | yes | tweet: curated tweets (2022-10-21)<br>tweet: curated tweets (2026-05-19) |
| **retest.it-can-take-a-long-time**<br>The market sometimes takes a long while to come back. It is still the same breakout; the level does not expire because time has passed. | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @18:40<br>transcript: levels webinar part 1, user-pasted transcript @19:17 |
| **retest.one-in-ten-misses**<br>With pinpoint levels, about 1 order in 10 never fills because the level was too accurate. That is accepted. | heuristic | stated | no | webinar: curated/trade-management.md (2024-09-24) |

## Conflicts

- `retest.momentum-into-level` vs `retest.rounded-over-sharp`: The transcript says he wants price to come into the level HARD; the 2024 notes say he prefers a ROUNDED retest over a sharp V. A hard move in and a rounded retest pull opposite ways. Both are preferences, not vetoes, so the node reports each and decides nothing.
- `retest.needs-space` vs `retest.needs-space-in-price`: The trader says space cannot be quantified and shows a chart. The 11-bar and 0.5 ATR figures are proxies from a note-taker and a backtest page; the same backtest material also reports that waiting 1 bar beat waiting 12. Treat the time proxy as unproven.
