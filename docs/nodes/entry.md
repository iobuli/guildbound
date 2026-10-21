# Entry decision

`entry` · stage **E** · decides at **fill_time** · computed

Combines the location, retest, confluence, ATR gate, news and bias into a go / no-go for each entry style, with every veto spelled out and the order price.

## Ports

**Inputs**
- `side` (from market-side.side): relation: below|above|unknown, requiredDir: long|short|neutral, valid: boolean, detail: string
- `level` (from level.level): found: boolean, role: object, center: number, zoneLo: number, zoneHi: number, breakIdx: number,null, breakT: number,null, barsSinceBreak: number, touches: number, lastTouchT: number,null, lxpbPrice: number,null, spanBars: number, chopCrossings: number, disrespected: boolean, tradable: boolean
- `retest` (from retest.retest): retested: boolean, barsBreakToRetest: number,null, excursionAtr: number, spaceOk: boolean, shape: rounded|sharp|none, approachAtr: number, momentum: hard|drift|none, tooSoon: boolean, rallyAway: boolean,null, swerves: number,null, cancelOrder: boolean
- `confluence` (from confluence.confluence): touches: number, factors: object, fibDetail: string, score: number, needed: number, meets: boolean, blind: boolean
- `gate` (from atr-gate.gate): movedAtr: number, fromHighAtr: number, fromLowAtr: number, passedUp: boolean, passedDown: boolean, blocks: object, supports: object
- `news` (from news.news): redNow: boolean, minutesToNextRed: number,null, minutesSinceLastRed: number,null, noNewFills: boolean, action: ok|pull-limit|keep-limit|reinsert-later, holdIfInTrade: boolean
- `sfp` (from sfp.sfp): h1: object[], d1: object[]
- `bias` (from daily-bias.bias): d1: object, w1: object, htf: long|short|neutral
- `tuesday` (from tuesday-rto.tuesday): valid: boolean, dir: object, entry: number,null, stop: number,null, target: number,null, rr: number,null, reasons: string[]
- `context` (from context.context): weekday: number, hourUtc: number, session: asian|london|ny|off, fridayLate: boolean, flattenBy: none|end-of-week|end-of-day, mode: with-bias|no-bias, conservativeTargets: boolean

**Outputs**
- `entry`: candidates: object[], primary: object

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **entry.limit-at-lxpb**<br>S/R entries are limit orders placed at the last high/low before the breakout, in the previous role's direction: bid the LHPB after an upside break, offer the LLPB after a downside break. | hard | stated | yes | webinar: curated/levels.md (2024-09-24)<br>lesson: curated/taking-a-trade-3-touch-level.md (2024-10-21) |
| **entry.blind-with-three-touches**<br>A level with three or more touches is traded blind, with no extra confluence. | hard | stated | yes | webinar: user-pasted notes (2024-09-24) |
| **entry.vetoes-stack**<br>All of these must clear before an S/R entry: the level is on the right side of the market, it is tradeable, it has its confluences, there was space and it is not the immediate retest, it has not been swerved, the market has not already passed its daily ATR in the trade direction, and it is not inside red news. | hard | inferred | yes | lesson: curated/taking-a-trade-1-touch-level.md (2024-10-21)<br>webinar: user-pasted notes (2024-10-03) |
| **entry.prefer-the-level-before-the-trap**<br>Between two levels, prefer the one that comes in before the level price is trapped at, and the one closer to the great higher-timeframe level. This also avoids swerves. | preference | observed | no | journal: curated/lxpb-trades-pattern-failure.md (2025-01-06)<br>journal: curated/lxpb-trades-return-to-hammer.md (2025-01-03) |
| **entry.limit-after-acceptance**<br>Newer practice: enter by limit after a close and acceptance beyond the prior level, at the nearest naked level to the SFP low. Do not blindly sell prior highs; wait to see the price action. | hard | observed | no | tweet: curated tweets (2026-06-03)<br>tweet: curated tweets (2022-10-21) |
| **entry.offer-the-retest-of-the-spike**<br>After a spike, wait for a daily close back inside, offer the retest of the spike high if it stays naked on H1, and bid the spike low for the exit. | hard | observed | no | tweet: curated tweets (2026-05-19) |
| **entry.blind-bid-then-add**<br>Blind-bid the D1 level, then add on the H1 price action. The first position then has a near-breakeven stop and total risk stays at 1R. | hard | observed | no | tweet: curated tweets (2026-05-01) |
| **entry.re-sell-the-same-level**<br>Selling a flipped level again on the retest after taking profits there means never being offside during the whole trending move. | heuristic | stated | no | webinar: curated/tp-selection.md (2024-09-24) |
| **entry.entry-types-when-no-lxpb**<br>When there is no LXPB the entry can be an H1 pin or a spike candle at the higher-timeframe level. | hard | observed | no | journal: curated/lxpb-trades-passed-atr.md (2024-12-16)<br>journal: curated/lxpb-trades-return-to-hammer.md (2024-12-22) |
| **entry.pull-on-drift-of-doom**<br>Pull a limit when price stair-steps down to it ("drift of doom"), even when the first trouble area is not too close. | hard | observed | no | journal: curated/lxpb-trades-engulfing.md (2026-02-26)<br>journal: curated/lxpb-trades-return-to-hammer.md (2025-05-02) |
| **entry.counter-bias-warning**<br>Trading against a fresh higher-timeframe bias is allowed only with good levels and a quick exit. | hard | stated | yes | webinar: curated/trade-management.md (2024-10-18) |
