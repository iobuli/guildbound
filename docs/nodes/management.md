# Trade management

`management` · stage **M** · decides at **post_fill** · computed

Once a position is open, tests the exit and trailing rules against the bars since the fill: evolving R, broken-entry retests, closes beyond the first test, thrust-candle trails, and whether adding is allowed.

## Ports

**Inputs**
- `stop` (from stop.stop): stop: number,null, suggested: number,null, method: string, shifted: boolean, riskDist: number, ok: boolean, squeezeLevel: boolean
- `target` (from target.target): candidates: object[], chosen: object, minRr: number
- `context` (from context.context): weekday: number, hourUtc: number, session: asian|london|ny|off, fridayLate: boolean, flattenBy: none|end-of-week|end-of-day, mode: with-bias|no-bias, conservativeTargets: boolean

**Outputs**
- `mgmt`: evolvingR: number,null, canAdd: boolean, actions: object[]

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **management.evolving-r**<br>RR evolves through a trade. A 1R trade that is 0.5R half way through is still held; only when it gets "ridiculously low" (under 0.5R) does he consider closing. "Don't be a prick for a tick." | hard | stated | yes | webinar: curated/trade-management.md (2024-09-24) |
| **management.broken-entry-retest-exit**<br>If price closes under your entry, retests the entry from below as resistance, and then closes beneath the initial low, exit the trade. | hard | stated | yes | webinar: curated/trade-management.md (2024-09-24) |
| **management.close-beyond-first-test**<br>If the trade is a short and an H1 close comes above the highs of the first test of your resistance, you must close it. The same logic applies to a long. | hard | stated | yes | webinar: curated/trade-management.md (2024-09-24) |
| **management.trail-beyond-thrust**<br>When price breaks a level in your favour, trail the stop beyond the thrust candle that caused the break. These stops still have to make sense: no trailing directly into a level. If it fails, go back another candle. | heuristic | stated | yes | webinar: curated/trade-management.md (2024-09-24)<br>webinar: curated/trade-management.md (2024-11-05) |
| **management.squeeze-point**<br>After the second return to the entry level and price curving away, he trails beyond the "squeeze point": the high (low) price would reach if it reacted off the level again. A third comeback is unlikely to go well. | hard | observed | no | webinar: curated/trade-management.md (2024-09-23)<br>webinar: curated/trade-management.md (2024-09-25) |
| **management.is-this-trade-done**<br>"Is this trade done?" If price came very close to target and then fell back to the entry, it is over: take what is there. If TP is hit without a fill, close the order at once. | hard | stated | no | webinar: curated/trade-management.md (2024-09-23)<br>webinar: curated/trade-management.md (2024-09-24) |
| **management.rejection-at-fta-exit**<br>Exit on a rejection candle at the first trouble area. Take the profits you have and do not hang around hoping the market comes back to your levels. | hard | observed | no | webinar: curated/trade-management.md (2024-09-24) |
| **management.trapped-exit-at-level**<br>If you are trapped in a trade that clearly will not reach target, get out at the level that is trapping you, with a limit for as small a loss as possible. | hard | stated | no | webinar: curated/trade-management.md (2024-11-04) |
| **management.opposite-sweep-exit**<br>Get out at a bullish sweep when you are short from a three-tap level and the S/R of the sweep is high: why stay when you are about 80% sure price will pump. | hard | observed | no | webinar: curated/trade-management.md (2024-10-29) |
| **management.flatten-by-close**<br>Forex positions are closed at the end of the week; FTSE and Bund trades at the end of the day, partly to avoid being gapped out. | hard | stated | yes | webinar: user-pasted notes (2024-09-24)<br>webinar: curated/trade-management.md (2024-09-24) |
| **management.add-when-confirming**<br>Add to a winner only when the market is confirming, the addition fits the plan, the upside is still asymmetric and risk stays controlled.<br>*Coded as: at least 1R in profit and evolving R still at least 1.* | hard | stated | yes | tweet: curated tweets (2026-06-03)<br>tweet: curated tweets (2026-05-01) |
| **management.macd-divergence-exit**<br>He has closed a trade on MACD divergence above a key level.<br>*MACD is not computed in this build.* | hard | observed | no | webinar: curated/trade-management.md (2024-09-23) |
| **management.hold-through-news**<br>Hold open trades through news; do not cut or lighten. | hard | stated | no | webinar: user-pasted notes (2024-10-03) |
