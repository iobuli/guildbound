# Three-confluence rule

`confluence` · stage **L** · decides at **fill_time** · computed

Every S/R trade needs 3 confluences. Each respected touch is one; the rest come from the daily bias, a passed daily ATR, or a fib retracement that lines up with the level.

## Ports

**Inputs**
- `level` (from level.level): found: boolean, role: object, center: number, zoneLo: number, zoneHi: number, breakIdx: number,null, breakT: number,null, barsSinceBreak: number, touches: number, lastTouchT: number,null, lxpbPrice: number,null, spanBars: number, chopCrossings: number, disrespected: boolean, tradable: boolean
- `bias` (from daily-bias.bias): d1: object, w1: object, htf: long|short|neutral
- `gate` (from atr-gate.gate): movedAtr: number, fromHighAtr: number, fromLowAtr: number, passedUp: boolean, passedDown: boolean, blocks: object, supports: object
- `atr` (from atr.atr): d1: number, h1: number, h1_24: number
- `swings` (from swing-depth.swings): h1: object[], d1: object[]

**Outputs**
- `confluence`: touches: number, factors: object, fibDetail: string, score: number, needed: number, meets: boolean, blind: boolean

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **confluence.three-needed**<br>Every support/resistance trade needs 3 confluences. | hard | stated | yes | lesson: curated/taking-a-trade-1-touch-level.md (2024-10-21)<br>lesson: curated/taking-a-trade-2-touch-level.md (2024-10-21)<br>lesson: curated/taking-a-trade-3-touch-level.md (2024-10-21) |
| **confluence.touch-tiers**<br>3 or more touches: trade the level blind. 2 touches: one more confluence. 1 touch: two more. | hard | stated | yes | lesson: curated/taking-a-trade-1-touch-level.md (2024-10-21)<br>webinar: user-pasted notes (2024-09-24) |
| **confluence.the-extra-factors**<br>The extra confluences are: daily bias, a surpassed daily ATR, or a fib retracement. | hard | stated | yes | lesson: curated/taking-a-trade-2-touch-level.md (2024-10-21) |
| **confluence.fib-within-ten-percent-atr**<br>A fib counts only if it is within 10% of the day's ATR from the level (ATR 100: within 10 pips; ATR 50: within 5). | hard | stated | yes | webinar: user-pasted notes (2024-09-30) |
| **confluence.fib-removed-past-618**<br>If price pulls back further than the 61.8% fib, remove the fib: it no longer counts. | hard | stated | yes | webinar: user-pasted notes (2024-10-05) |
| **confluence.fib-from-daily-swings**<br>Fibs are drawn from daily swing points, only "deep key levels". He likes fibs from different areas to line up.<br>*This build uses the latest D1 swing with at least 1 ATR of depth as "major".* | hard | stated | yes | webinar: user-pasted notes (2024-10-05)<br>webinar: user-pasted notes (2024-10-06) |
| **confluence.bias-expires**<br>A daily-engulfing confluence lapses if no entry is given within a couple of days. It reverts to a standard level that needs its other confluence. | hard | stated | yes | webinar: curated/notes-images.md (2024-09-10) |
| **confluence.bias-loosens-levels**<br>With a daily bias the hourly level does not have to be great: "I just want in", but still a decent level. | preference | stated | no | webinar: curated/notes-images.md (2024-09-08)<br>webinar: curated/tp-selection.md (2024-09-24) |
| **confluence.tiers-from-the-transcript**<br>The summary he gives twice: 3 or more touches, trade it blind (just a limit order with a stop and a target). 2 touches, one factor of confluence. 1 touch, two factors. The less significant the level, the more you need to tell you price will bounce. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @23:35<br>transcript: levels webinar part 1, user-pasted transcript @24:25<br>transcript: levels webinar part 1, user-pasted transcript @56:12<br>transcript: levels webinar part 1, user-pasted transcript @1:15:09<br>transcript: levels webinar part 1, user-pasted transcript @1:16:03 |
| **confluence.bias-is-a-bar-pattern**<br>The daily bias used as a confluence must be a bar pattern on the daily chart: a pinbar, a swing failure (breaks a swing low and closes above, or breaks a swing high and closes below) or an engulfing candle. "It's in a strong uptrend" is not enough. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @35:16<br>transcript: levels webinar part 1, user-pasted transcript @36:03<br>transcript: levels webinar part 1, user-pasted transcript @37:47 |
| **confluence.only-three-factors**<br>There are three factors of confluence: a daily bias, a Fibonacci level, and the market at or past its ATR. A moving average was mentioned in passing as an example of "something else" but is not one of the three. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @25:07<br>transcript: levels webinar part 1, user-pasted transcript @34:09<br>transcript: levels webinar part 1, user-pasted transcript @1:16:11 |
| **confluence.fib-38-50-61-only**<br>Only the 38%, 50% and 61.8% fibs are used. The 78% is not. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @1:20:30<br>transcript: levels webinar part 1, user-pasted transcript @1:19:02 |
| **confluence.fib-from-major-daily-swing**<br>Draw the fib from a major swing point, preferably on the daily chart rather than the hourly, from the low to the highest point before the market began its turn. | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @1:16:35<br>transcript: levels webinar part 1, user-pasted transcript @47:52 |
| **confluence.level-gets-the-order**<br>When the fib and the ATR line sit close to the level, the order goes at the LEVEL, because he attaches the most importance to levels. The stop goes beyond the fib and the ATR. | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @49:09 |
| **confluence.worked-examples**<br>Silver: two-touch level + daily bias (pinbar that is also a swing failure) = tradeable. EURCHF: two-touch level + ATR (16 pips below) + 61% fib on the same spot = "perfect confluence". Silver neckline retest: two-touch + 38% fib = tradeable, but skipped because price bounced just before it. | hard | observed | no | transcript: levels webinar part 1, user-pasted transcript @33:03<br>transcript: levels webinar part 1, user-pasted transcript @38:11<br>transcript: levels webinar part 1, user-pasted transcript @48:27<br>transcript: levels webinar part 1, user-pasted transcript @57:37 |
