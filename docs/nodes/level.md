# Support / resistance level

`level` · stage **L** · decides at **prior_bars** · computed

Takes the level the trade is built on and reconstructs its history: where it broke, how many times it was respected in its previous role ("opposite last"), the last high/low before the breakout (LXPB), and whether price is still respecting it.

## Ports

**Inputs**
- `atr` (from atr.atr): d1: number, h1: number, h1_24: number

**Outputs**
- `level`: found: boolean, role: object, center: number, zoneLo: number, zoneHi: number, breakIdx: number,null, breakT: number,null, barsSinceBreak: number, touches: number, lastTouchT: number,null, lxpbPrice: number,null, spanBars: number, chopCrossings: number, disrespected: boolean, tradable: boolean

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **level.inverse-last-only**<br>Trade only levels that last did the OPPOSITE of what you are about to do. To buy below market, the level must have been RESISTANCE last; to sell above market, it must have been SUPPORT last. Levels that last acted in your own direction are a different, harder trade. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @7:47<br>transcript: levels webinar part 1, user-pasted transcript @8:31<br>transcript: levels webinar part 1, user-pasted transcript @14:15 |
| **level.look-left-at-the-last-interaction**<br>Always look at the most recent interaction with the level. If price washed through it recently (traded in and out and ignored it), do not trade it, however well it respected the level in the past. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @6:44<br>transcript: levels webinar part 1, user-pasted transcript @7:12<br>transcript: levels webinar part 1, user-pasted transcript @21:39<br>webinar: curated/levels.md (2024-09-26) |
| **level.touch-is-a-swing-touch**<br>Count swing touches, not candles. A cluster of candles on the level that produces ONE swing away is one touch (three candles touching = one). Only touches that cause a decent swing away count. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @15:39<br>transcript: levels webinar part 1, user-pasted transcript @16:38<br>transcript: levels webinar part 1, user-pasted transcript @17:06<br>transcript: levels webinar part 1, user-pasted transcript @33:12 |
| **level.touches-must-be-separate-in-time**<br>The swings should be deep and separated by time and space; the further price swings away, the longer it takes to come back. Touches that sit close together with no reaction between them count as one. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @15:39<br>transcript: levels webinar part 1, user-pasted transcript @33:12<br>webinar: curated/levels.md (2024-09-30) |
| **level.time-matters**<br>The longer a level takes to form, the stronger it is: a level built over a week beats one built over a trading day. A level formed over two days can still be traded if the touches are clean (discretionary). | preference | stated | yes | transcript: levels webinar part 1, user-pasted transcript @17:45<br>transcript: levels webinar part 1, user-pasted transcript @30:49 |
| **level.previous-role-only**<br>Do not count the touches from before the level flipped. A level that was resistance, then support once, then comes back as resistance is a ONE-touch level. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @1:13:06<br>transcript: levels webinar part 1, user-pasted transcript @1:14:37<br>webinar: curated/notes-images.md (2024-09-10) |
| **level.broken-level-is-still-a-breakout**<br>A level that was broken and has not been revisited is still a breakout to trade, even if you have forgotten about it. It does not matter how long it takes to come back. | hard | observed | no | transcript: levels webinar part 1, user-pasted transcript @18:40<br>transcript: levels webinar part 1, user-pasted transcript @19:17<br>transcript: levels webinar part 1, user-pasted transcript @20:14 |
| **level.lxpb**<br>Place the order at the last high (low) before the breakout, the LHPB / LLPB. Price tends to gravitate to it. With several candidate lines, take the one at the most recent area. | hard | stated | yes | webinar: curated/levels.md (2024-09-24)<br>webinar: curated/levels.md (2024-09-30)<br>webinar: curated/notes-images.md (2024-09-23) |
| **level.recent-price-action-wins**<br>Recent price action outweighs older price action when drawing the line: the line sits where the recent touches are. | hard | stated | no | webinar: user-pasted notes (2024-09-24)<br>webinar: curated/levels.md (2024-09-25) |
| **level.untradeable-when-disrespected**<br>A level becomes untradeable when the latest touch is not respected by price action. | hard | stated | yes | webinar: user-pasted notes (2024-09-25)<br>transcript: levels webinar part 1, user-pasted transcript @7:02 |
| **level.chopping-through**<br>Do not draw a level that price is currently chopping through, even if it was a good level further left. If it is disrespected repeatedly but was respected recently, keep it but be cautious.<br>*Computed as a count of crossings in the last 40 bars (4 or more = chopping).* | hard | stated | yes | webinar: curated/levels.md (2024-09-26)<br>webinar: curated/levels.md (2024-09-27) |
| **level.trendlines-are-levels**<br>Trend lines are treated as levels, with the same premise (hit multiple times, breaks, retests), not as confluence. He does not have a great love of them. | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @1:20:40 |
| **level.crumbles-if-retested**<br>A level that keeps being tested is likely to crumble. On a successful trade it often shifts quickly after the second touch. | heuristic | observed | no | webinar: curated/levels.md (2024-09-25) |
| **level.zone-not-line**<br>A level is a zone. Price has only broken it when it breaks the whole zone, not the first line. | hard | observed | no | webinar: curated/notes-images.md (2024-10-28) |
| **level.how-far-back**<br>He looks far back for levels, in one case using a level from February 2022 for a trade in September 2024. | heuristic | observed | no | webinar: curated/levels.md (2024-09-24)<br>journal: curated/lxpb-trades-passed-atr.md (2025-01-06) |
| **level.pick-llpb-when-stacked**<br>With a weekly resistance and a second resistance slightly above it, put the order at the LLPB. | hard | stated | no | webinar: curated/levels.md (2024-09-28) |
