# Support / resistance level (`level`)

Takes the level the trade is built on and reconstructs its history: where it broke, how many times it was respected in its previous role ("opposite last"), the last high/low before the breakout (LXPB), and whether price is still respecting it.

Decides at **prior_bars**, step 2 of the order of operations: Is it an inverse-last level that price respected last time, and how many swing touches?. Inputs: atr. Outputs: level.

## level.inverse-last-only

Trade only levels that last did the OPPOSITE of what you are about to do. To buy below market, the level must have been RESISTANCE last; to sell above market, it must have been SUPPORT last. Levels that last acted in your own direction are a different, harder trade.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @7:47; transcript levels webinar part 1, user-pasted transcript @8:31; transcript levels webinar part 1, user-pasted transcript @14:15.

Supporting passages:
- [sources/levels-webinar-part1.md:61](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 40.18): "[7:53] levels that price last found resistance at if we were going to be below L market and we're rallying upwards we're going"
- [sources/levels-webinar-part1.md:149](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 29.89): "[20:46] levels that the inverse last we need to sell a prior support level we need to go the opposite way to trade this before"
- [sources/levels-webinar-part1.md:105](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 28.22): "[14:50] market so we need levels that will last resistance okay so how strong is this level and how can we quantify if the"

## level.look-left-at-the-last-interaction

Always look at the most recent interaction with the level. If price washed through it recently (traded in and out and ignored it), do not trade it, however well it respected the level in the past.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @6:44; transcript levels webinar part 1, user-pasted transcript @7:12; transcript levels webinar part 1, user-pasted transcript @21:39; webinar curated/levels.md (2024-09-26).

Supporting passages:
- [sources/levels-webinar-part1.md:46](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 60.93): "[6:53] level okay I look at the most recent interaction with the level so I'll always look left and see what price did there recently now if price didn't"
- [sources/course/curated/levels.md:65](../sources/course/curated/levels.md) (levels, score 22.47): "the lack of confidence comes from the red boxes, where price is repeatedly disrespecting this level... however it is not removed entirely, due to the fact that it is still a clear level, and was respected relatively recently also. [image]"
- [sources/levels-webinar-part1.md:304](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 18.06): "41:0041 minuteshere how many touches is this level well it's a very interesting one I mean I would look at"

## level.touch-is-a-swing-touch

Count swing touches, not candles. A cluster of candles on the level that produces ONE swing away is one touch (three candles touching = one). Only touches that cause a decent swing away count.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @15:39; transcript levels webinar part 1, user-pasted transcript @16:38; transcript levels webinar part 1, user-pasted transcript @17:06; transcript levels webinar part 1, user-pasted transcript @33:12.

Supporting passages:
- [sources/levels-webinar-part1.md:119](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 36.43): "[15:39] of the ways in which that happens is if each touch of the level has caused a fairly decent swing away from price"
- [sources/levels-webinar-part1.md:241](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 22.42): "[33:12] strong level well we need to count the number of touches as we said now this is interesting because at first glance you would say that's been a three touch"
- [sources/course/curated/levels.md:65](../sources/course/curated/levels.md) (levels, score 15.97): "**30/09/2024 22:14** example of a 3 touch level. as you can see, the last 2 touches were very close - but still count separately, presumably because they both had a strong impulsive reaction [image]"

## level.touches-must-be-separate-in-time

The swings should be deep and separated by time and space; the further price swings away, the longer it takes to come back. Touches that sit close together with no reaction between them count as one.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @15:39; transcript levels webinar part 1, user-pasted transcript @33:12; webinar curated/levels.md (2024-09-30).

Supporting passages:
- [sources/levels-webinar-part1.md:133](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 33.28): "[17:37] area so we come up with a number of how many touches it's had and ideally we like the swings to have been quite deep"
- [sources/levels-webinar-part1.md:119](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 31.67): "[15:55] we swing away the longer it takes to get back so I like to see level where price has hit them multiple times and each time price has swung away quite"
- [sources/course/curated/levels.md:1](../sources/course/curated/levels.md) (levels, score 21.32): "the reason why, is that the two in the blue box were very close together and the first touch in that blue box, really didnt go anywhere, it didnt have any reaction."

## level.time-matters

The longer a level takes to form, the stronger it is: a level built over a week beats one built over a trading day. A level formed over two days can still be traded if the touches are clean (discretionary).

*preference, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @17:45; transcript levels webinar part 1, user-pasted transcript @30:49.

Supporting passages:
- [sources/levels-webinar-part1.md:133](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 34.25): "18:0018 minutesthat has formed over the last trading day so it's number of touches but it's also time"
- [sources/levels-webinar-part1.md:225](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 27.3): "[30:58] actually look at this level it really only formed over two days it's an alley level that formed over two days but having said that the touches are nice"
- [sources/levels-webinar-part1.md:119](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 21.05): "[15:55] we swing away the longer it takes to get back so I like to see level where price has hit them multiple times and each time price has swung away quite"

## level.previous-role-only

Do not count the touches from before the level flipped. A level that was resistance, then support once, then comes back as resistance is a ONE-touch level.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @1:13:06; transcript levels webinar part 1, user-pasted transcript @1:14:37; webinar curated/notes-images.md (2024-09-10).

Supporting passages:
- [sources/levels-webinar-part1.md:535](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 29.51): "[1:12:59] um good question from uh Jeremy if the level is flip from support to resistance do you also count the touches of the"
- [sources/course/curated/taking-a-trade-1-touch-level.md:1](../sources/course/curated/taking-a-trade-1-touch-level.md) (1-touch-level, score 19.43): "**21/10/2024 16:34** this is a 1 touch level of resistance flipped into support: [image]"
- [sources/user-research-pages.md:323](../sources/user-research-pages.md) (Rebuild it yourself, score 19.39): "* 3+ touches of a flipped level: can trade “blind” (no additional confluence needed)"

## level.broken-level-is-still-a-breakout

A level that was broken and has not been revisited is still a breakout to trade, even if you have forgotten about it. It does not matter how long it takes to come back.

*hard, observed, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @18:40; transcript levels webinar part 1, user-pasted transcript @19:17; transcript levels webinar part 1, user-pasted transcript @20:14.

Supporting passages:
- [sources/levels-webinar-part1.md:133](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 26.37): "[19:03] matter how long it takes we're attempting to trade a level that was at one point a breakout and we're just"
- [sources/course/curated/trade-management.md:93](../sources/course/curated/trade-management.md) (trade-management, score 16.27): ""i feel like i overlooked these two areas that came very close and then bounced. i feel like the marked should have really gone [especially off here (arrow)] and it didnt. it came back, why is it repeatedly coming back to this level? twice "
- [sources/levels-webinar-part1.md:288](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 15.99): "[38:58] got a good two touch level and you've got a daily bias I'm not obsessed with the amount of space that it takes back there but I still you"

## level.lxpb

Place the order at the last high (low) before the breakout, the LHPB / LLPB. Price tends to gravitate to it. With several candidate lines, take the one at the most recent area.

*hard, stated, tested by code.* Sources: webinar curated/levels.md (2024-09-24); webinar curated/levels.md (2024-09-30); webinar curated/notes-images.md (2024-09-23).

Supporting passages:
- [sources/course/curated/tp-selection.md:56](../sources/course/curated/tp-selection.md) (tp-selection, score 27.48): ""price often tends to gravitate to the last area before the breakout" [image]"
- [sources/course/curated/taking-a-trade-targets.md:1](../sources/course/curated/taking-a-trade-targets.md) (targets, score 26.96): "in general, price tends to gravitate to the most recent level. so even though price will run into the upper support level first - it is still a safe bet to target the lower (more recent) one. [image]"
- [sources/course/curated/levels.md:1](../sources/course/curated/levels.md) (levels, score 23.91): ""price tends to gravitate towards the last low pre-breakout." [image]"

## level.recent-price-action-wins

Recent price action outweighs older price action when drawing the line: the line sits where the recent touches are.

*hard, stated, not coded.* Sources: webinar user-pasted notes (2024-09-24); webinar curated/levels.md (2024-09-25).

Supporting passages:
- [sources/user-research-pages.md:323](../sources/user-research-pages.md) (Rebuild it yourself, score 20.96): "* When drawing at THE high/low point, consider moving the line to S/R above/below (since price often probes beyond exact highs/lows)"
- [sources/user-research-pages.md:303](../sources/user-research-pages.md) (Rebuild it yourself, score 19.08): "2. **mhm — 24/09/2024 20:39** line goes approx in the middle of the 'SR zone' and is then refined to the area with the most touches, but *heavily* bias more recent candles over older ones."
- [sources/course/curated/tp-selection.md:19](../sources/course/curated/tp-selection.md) (tp-selection, score 14.35): "entry would be on the break of this support line here (upper red) - despite the fact that its messy on the left, that doesnt really matter, as the recent PA is super clearly showing it was support. and recent PA > older PA. [image]"

## level.untradeable-when-disrespected

A level becomes untradeable when the latest touch is not respected by price action.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-25); transcript levels webinar part 1, user-pasted transcript @7:02.

Supporting passages:
- [sources/course/curated/tp-selection.md:92](../sources/course/curated/tp-selection.md) (tp-selection, score 25.79): "reminds me of: 'a level becomes untradeable when the latest touch is not respected by PA'. (edited) [image]"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 21.33): "* A level becomes untradeable when the latest touch isn’t respected by price action"
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 17.27): "a level becomes untradeable when the latest touch is not respected by PA."

## level.chopping-through

Do not draw a level that price is currently chopping through, even if it was a good level further left. If it is disrespected repeatedly but was respected recently, keep it but be cautious.

*hard, stated, tested by code.* Sources: webinar curated/levels.md (2024-09-26); webinar curated/levels.md (2024-09-27). Note: Computed as a count of crossings in the last 40 bars (4 or more = chopping).

Supporting passages:
- [sources/course/curated/levels.md:43](../sources/course/curated/levels.md) (levels, score 23.38): "the reason for this is because even though it was a good level on the left, price over on the right (where we currently are) is not respecting it at all - and is in fact, just chopping through it (red box). [image]"
- [sources/course/curated/levels.md:65](../sources/course/curated/levels.md) (levels, score 22.32): "the lack of confidence comes from the red boxes, where price is repeatedly disrespecting this level... however it is not removed entirely, due to the fact that it is still a clear level, and was respected relatively recently also. [image]"
- [sources/levels-webinar-part1.md:46](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 20.67): "[6:53] level okay I look at the most recent interaction with the level so I'll always look left and see what price did there recently now if price didn't"

## level.trendlines-are-levels

Trend lines are treated as levels, with the same premise (hit multiple times, breaks, retests), not as confluence. He does not have a great love of them.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @1:20:40.

Supporting passages:
- [sources/levels-webinar-part1.md:584](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 42.39): "[1:21:08] lines I would trade a level with a trend line with the same premise as the levels if it's been hit multiple times breaks"
- [sources/user-research-pages.md:323](../sources/user-research-pages.md) (Rebuild it yourself, score 15.62): "Drawing S/R Lines"
- [sources/levels-webinar-part1.md:31](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 15.26): "[3:47] beginning of January so multiple times price has traded into that level sers have hit it um and uh you know the"

## level.crumbles-if-retested

A level that keeps being tested is likely to crumble. On a successful trade it often shifts quickly after the second touch.

*heuristic, observed, not coded.* Sources: webinar curated/levels.md (2024-09-25).

Supporting passages:
- [sources/course/curated/levels.md:43](../sources/course/curated/levels.md) (levels, score 35.82): "sometimes it will come back for a second touch, but on a successful trade, it will often shift pretty quickly after this touch. [image]"
- [sources/levels-webinar-part1.md:535](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 14.04): "[1:12:35] uh this uh this level here wasn't really really retested late in the session this was 1 p.m that this tested"
- [sources/levels-webinar-part1.md:519](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 12.69): "[1:11:32] uh quite quickly you often find if it trades into a good level that it can have the elastic band effect if it's drifting into that level it worries me"

## level.zone-not-line

A level is a zone. Price has only broken it when it breaks the whole zone, not the first line.

*hard, observed, not coded.* Sources: webinar curated/notes-images.md (2024-10-28).

Supporting passages:
- [sources/course/curated/trade-management.md:93](../sources/course/curated/trade-management.md) (trade-management, score 14.81): "**24/09/2024 20:05** here price breaks down underneath his level on the first retest. on the next candle, when it pushes back above the level - he counts this as a thrust candle, and moves his SL to underneath it."
- [sources/course/curated/taking-a-trade-3-touch-level.md:1](../sources/course/curated/taking-a-trade-3-touch-level.md) (3-touch-level, score 13.77): "which means that a level that was supporting price, once that level is broken, we expect it to act as the opposite - aka resistance [image]"
- [sources/course/curated/notes-images.md:114](../sources/course/curated/notes-images.md) (notes-images, score 12.37): "**28/10/2024 17:58** in this case, when price broke under the red line (where you would have your limit) - this is not actually considered price thrusting through the level - as 'the level' included the entire blue area."

## level.how-far-back

He looks far back for levels, in one case using a level from February 2022 for a trade in September 2024.

*heuristic, observed, not coded.* Sources: webinar curated/levels.md (2024-09-24); journal curated/lxpb-trades-passed-atr.md (2025-01-06).

Supporting passages:
- [sources/course/curated/levels.md:19](../sources/course/curated/levels.md) (levels, score 16.14): "**24/09/2024 19:10** an example of how far back he looks for his levels: [image]"
- [sources/course/curated/lxpb-trades-passed-atr.md:29](../sources/course/curated/lxpb-trades-passed-atr.md) (passed-atr, score 14.39): "note: this is a level from february 2022, used for a trade in september 2024. [image]"
- [sources/course/general/general_keywords.md:34](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 12.35): "- 31/05/2022 | trade would have won"

## level.pick-llpb-when-stacked

With a weekly resistance and a second resistance slightly above it, put the order at the LLPB.

*hard, stated, not coded.* Sources: webinar curated/levels.md (2024-09-28).

Supporting passages:
- [sources/course/curated/levels.md:65](../sources/course/curated/levels.md) (levels, score 19.83): "**28/09/2024 13:38** scenario: a weekly resistance, with a bearish resistance just slightly above it... which one to put your short order in at?"
- [sources/course/general/general_keywords.md:898](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 13.52): "- 27/03/2026 | i could have put my stop much tighter, above the H1 hammer thats above the green zone."
- [sources/course/general/general_full.md:14629](../sources/course/general/general_full.md) (take notes, score 13.02): "i could have put my stop much tighter, above the H1 hammer thats above the green zone. but i was cautious of getting stopped out right into the S/R just above it. i would feel like an idiot if price wicked my stop into the resistance above "
