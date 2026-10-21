# Three-confluence rule (`confluence`)

Every S/R trade needs 3 confluences. Each respected touch is one; the rest come from the daily bias, a passed daily ATR, or a fib retracement that lines up with the level.

Decides at **fill_time**, step 3 of the order of operations: Tier by touches: 3+ blind, 2 needs one confluence, 1 needs two (daily bias, fib, ATR). Inputs: level, bias, gate, atr, swings. Outputs: confluence.

## confluence.three-needed

Every support/resistance trade needs 3 confluences.

*hard, stated, tested by code.* Sources: lesson curated/taking-a-trade-1-touch-level.md (2024-10-21); lesson curated/taking-a-trade-2-touch-level.md (2024-10-21); lesson curated/taking-a-trade-3-touch-level.md (2024-10-21).

Supporting passages:
- [sources/course/curated/taking-a-trade-2-touch-level.md:1](../sources/course/curated/taking-a-trade-2-touch-level.md) (2-touch-level, score 22.89): "**21/10/2024 15:39** every support / resistance trade needs 3 confluences. but not every S/R level will be a ."
- [sources/course/curated/taking-a-trade-3-touch-level.md:1](../sources/course/curated/taking-a-trade-3-touch-level.md) (3-touch-level, score 21.96): "**21/10/2024 15:33** every support / resistance trade needs 3 confluences."
- [sources/course/curated/taking-a-trade-1-touch-level.md:1](../sources/course/curated/taking-a-trade-1-touch-level.md) (1-touch-level, score 19.6): "**21/10/2024 15:42** every support / resistance trade needs 3 confluences. but not every S/R level will be a ."

## confluence.touch-tiers

3 or more touches: trade the level blind. 2 touches: one more confluence. 1 touch: two more.

*hard, stated, tested by code.* Sources: lesson curated/taking-a-trade-1-touch-level.md (2024-10-21); webinar user-pasted notes (2024-09-24).

Supporting passages:
- [sources/levels-webinar-part1.md:181](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 16.85): "[23:44] I will trade it blind now what I mean by blind is I will literally just leave an order in at that level with a stop and a"
- [sources/levels-webinar-part1.md:552](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 15.66): "[1:15:16] three touches plus the inverse then I will trade it blind what blind means is just bang in a limit order okay I'm just"
- [sources/user-research-pages.md:303](../sources/user-research-pages.md) (Rebuild it yourself, score 15.33): "4. **mhm — 24/09/2024 20:57** 3 touches or more (of a flipped level), he will trade the level blind. 2 touches he wants 1 more factor of confluence. 1 touch he needs lots (at least 2 factors) of confluence. factors of confluence:"

## confluence.the-extra-factors

The extra confluences are: daily bias, a surpassed daily ATR, or a fib retracement.

*hard, stated, tested by code.* Sources: lesson curated/taking-a-trade-2-touch-level.md (2024-10-21).

Supporting passages:
- [sources/course/curated/taking-a-trade-2-touch-level.md:1](../sources/course/curated/taking-a-trade-2-touch-level.md) (2-touch-level, score 24.6): "**21/10/2024 15:56** surpassed Daily ATR"
- [sources/course/curated/taking-a-trade-1-touch-level.md:1](../sources/course/curated/taking-a-trade-1-touch-level.md) (1-touch-level, score 23.35): "these extra confluences could be two of the following: (edited)"
- [sources/levels-webinar-part1.md:552](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 16.91): "[1:16:11] what those confluences are they are a D1 bias and if you're unsure of the D1 bias you'll want to look at the last webinar I did on D1"

## confluence.fib-within-ten-percent-atr

A fib counts only if it is within 10% of the day's ATR from the level (ATR 100: within 10 pips; ATR 50: within 5).

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-30).

Supporting passages:
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 27.79): "so if the D1 ATR = 100, and the fib is ≤10 pips away from your level, then it counts as a confluence."
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 17.98): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"
- [sources/levels-webinar-part1.md:337](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 16.2): "[45:24] only on average moves 100 Pips on a day and you get in it rallies 100 Pips you're looking for a place to take profits you're probably thinking well it's done what it does on average I'm"

## confluence.fib-removed-past-618

If price pulls back further than the 61.8% fib, remove the fib: it no longer counts.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-10-05).

Supporting passages:
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 21.46): "* If price pulls back beyond 61.8% fib level, remove the fib (no longer valid confluence)"
- [sources/user-research-pages.md:474](../sources/user-research-pages.md) (Rebuild it yourself, score 17.37): "if we pull back further than the 61.8% fib level, he removes the fib and they can no longer be used as confluence."

## confluence.fib-from-daily-swings

Fibs are drawn from daily swing points, only "deep key levels". He likes fibs from different areas to line up.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-10-05); webinar user-pasted notes (2024-10-06). Note: This build uses the latest D1 swing with at least 1 ATR of depth as "major".

Supporting passages:
- [sources/user-research-pages.md:474](../sources/user-research-pages.md) (Rebuild it yourself, score 41.65): "he likes fibs (from different areas) to line up with each other."
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 23.81): "* Fibs are drawn from daily swing points (“deep key levels”)"
- [sources/user-research-pages.md:303](../sources/user-research-pages.md) (Rebuild it yourself, score 17.08): "* 2). fib levels from major swings (he likes to draw these from the daily)"

## confluence.bias-expires

A daily-engulfing confluence lapses if no entry is given within a couple of days. It reverts to a standard level that needs its other confluence.

*hard, stated, tested by code.* Sources: webinar curated/notes-images.md (2024-09-10).

Supporting passages:
- [sources/course/curated/notes-images.md:1](../sources/course/curated/notes-images.md) (notes-images, score 31.79): "however, if entry isnt given after a couple of days, you can no longer use the D1 engulfing as a factor of confluence - and this goes back to being just a standard 2 touch level (that still requires 1 more factor of confluence). (edited)"
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 29.21): "this can be (slightly) mitigated if we have good confluence. the example given was an MSB that looks reasonably small. definitely multiple pips, but not a standard large breakout with rounded retest that we typically see. but this breakout "
- [sources/user-research-pages.md:323](../sources/user-research-pages.md) (Rebuild it yourself, score 18.6): "* 3+ touches of a flipped level: can trade “blind” (no additional confluence needed)"

## confluence.bias-loosens-levels

With a daily bias the hourly level does not have to be great: "I just want in", but still a decent level.

*preference, stated, not coded.* Sources: webinar curated/notes-images.md (2024-09-08); webinar curated/tp-selection.md (2024-09-24).

Supporting passages:
- [sources/course/curated/tp-selection.md:19](../sources/course/curated/tp-selection.md) (tp-selection, score 22.07): ""now this (mouse) isnt a great level. but when you have a daily bias, your hourly levels dont have to be great [it just needs to be fairly good / decent].""
- [sources/course/curated/notes-images.md:1](../sources/course/curated/notes-images.md) (notes-images, score 15.84): ""if ive got a daily bias, i basically just want in. i still want at least a decent level, dont get me wrong. but i just want in.""
- [sources/levels-webinar-part1.md:288](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 13.06): "[38:58] got a good two touch level and you've got a daily bias I'm not obsessed with the amount of space that it takes back there but I still you"

## confluence.tiers-from-the-transcript

The summary he gives twice: 3 or more touches, trade it blind (just a limit order with a stop and a target). 2 touches, one factor of confluence. 1 touch, two factors. The less significant the level, the more you need to tell you price will bounce.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @23:35; transcript levels webinar part 1, user-pasted transcript @24:25; transcript levels webinar part 1, user-pasted transcript @56:12; transcript levels webinar part 1, user-pasted transcript @1:15:09; transcript levels webinar part 1, user-pasted transcript @1:16:03.

Supporting passages:
- [sources/levels-webinar-part1.md:181](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 40.79): "[23:44] I will trade it blind now what I mean by blind is I will literally just leave an order in at that level with a stop and a"
- [sources/levels-webinar-part1.md:416](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 38.6): "[56:20] Confluence to tell me to take that trade so I might need I might need a fib level and an at"
- [sources/levels-webinar-part1.md:552](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 32.3): "[1:15:16] three touches plus the inverse then I will trade it blind what blind means is just bang in a limit order okay I'm just"

## confluence.bias-is-a-bar-pattern

The daily bias used as a confluence must be a bar pattern on the daily chart: a pinbar, a swing failure (breaks a swing low and closes above, or breaks a swing high and closes below) or an engulfing candle. "It's in a strong uptrend" is not enough.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @35:16; transcript levels webinar part 1, user-pasted transcript @36:03; transcript levels webinar part 1, user-pasted transcript @37:47.

Supporting passages:
- [sources/levels-webinar-part1.md:272](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 44.06): "[37:23] here so the market breaks a swing low closes above or breaks a swing High closes below or there will be a bullish"
- [sources/user-research-pages.md:323](../sources/user-research-pages.md) (Rebuild it yourself, score 29.82): "* When drawing at THE high/low point, consider moving the line to S/R above/below (since price often probes beyond exact highs/lows)"
- [sources/course/general/general_keywords.md:918](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 28.56): "- 21/04/2026 | skipped trade: engulfing failure price above the weekly S/Rdaily closed above the engulfing KPI (low of the candle prior to the bearish engulfing)hourly LXPB right on top of the weekly zoneSL well below the weekly zone to be "

## confluence.only-three-factors

There are three factors of confluence: a daily bias, a Fibonacci level, and the market at or past its ATR. A moving average was mentioned in passing as an example of "something else" but is not one of the three.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @25:07; transcript levels webinar part 1, user-pasted transcript @34:09; transcript levels webinar part 1, user-pasted transcript @1:16:11.

Supporting passages:
- [sources/levels-webinar-part1.md:257](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 36.74): "[34:41] level and the third is going to be the market trading at or over its ATR which is its average true range and I'm going"
- [sources/levels-webinar-part1.md:552](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 27.72): "[1:16:27] ATR if the market is at or past its ATR coming into a level that's Confluence the third one is a fib level if we've"
- [sources/user-research-pages.md:323](../sources/user-research-pages.md) (Rebuild it yourself, score 27.16): "* 3+ touches of a flipped level: can trade “blind” (no additional confluence needed)"

## confluence.fib-38-50-61-only

Only the 38%, 50% and 61.8% fibs are used. The 78% is not.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @1:20:30; transcript levels webinar part 1, user-pasted transcript @1:19:02.

Supporting passages:
- [sources/user-research-pages.md:474](../sources/user-research-pages.md) (Rebuild it yourself, score 19.46): "if we pull back further than the 61.8% fib level, he removes the fib and they can no longer be used as confluence."

## confluence.fib-from-major-daily-swing

Draw the fib from a major swing point, preferably on the daily chart rather than the hourly, from the low to the highest point before the market began its turn.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @1:16:35; transcript levels webinar part 1, user-pasted transcript @47:52.

Supporting passages:
- [sources/levels-webinar-part1.md:552](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 24.04): "[1:16:35] got a strong FIB level taken from a major swing point and preferably on The Daily I like to FIB I like to do draw my"
- [sources/levels-webinar-part1.md:354](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 22.07): "[48:12] the market if you take a fib from low to the highest point before the market began its turn back so we have to go up"
- [sources/user-research-pages.md:323](../sources/user-research-pages.md) (Rebuild it yourself, score 18.82): "2. Fibonacci levels from major swings (preferably drawn from daily)"

## confluence.level-gets-the-order

When the fib and the ATR line sit close to the level, the order goes at the LEVEL, because he attaches the most importance to levels. The stop goes beyond the fib and the ATR.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @49:09.

Supporting passages:
- [sources/levels-webinar-part1.md:354](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 24.07): "[49:09] right in the same area what I will tend to do is I will tend to buy at the level I always attach the most importance to the levels so in this case I'm going to"
- [sources/user-research-pages.md:303](../sources/user-research-pages.md) (Rebuild it yourself, score 23.82): "5. **mhm — 24/09/2024 21:08** not super applicable: if drawing a (weekly/daily i think this was) line at THE high/low, move it to S/R above/below (what are the odds price will stop at THE high (assume this high is the only touch on your lin"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 18.36): "* If price pulls back beyond 61.8% fib level, remove the fib (no longer valid confluence)"

## confluence.worked-examples

Silver: two-touch level + daily bias (pinbar that is also a swing failure) = tradeable. EURCHF: two-touch level + ATR (16 pips below) + 61% fib on the same spot = "perfect confluence". Silver neckline retest: two-touch + 38% fib = tradeable, but skipped because price bounced just before it.

*hard, observed, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @33:03; transcript levels webinar part 1, user-pasted transcript @38:11; transcript levels webinar part 1, user-pasted transcript @48:27; transcript levels webinar part 1, user-pasted transcript @57:37.

Supporting passages:
- [sources/levels-webinar-part1.md:416](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 37.7): "[56:05] daily bias it might be our ATR it might be our FIB level if it's a one touch last and we"
- [sources/levels-webinar-part1.md:432](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 32.02): "[58:20] but what we really need is a factor of Confluence if only a two touch level so we would start looking I would"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 31.17): "* If price pulls back beyond 61.8% fib level, remove the fib (no longer valid confluence)"
