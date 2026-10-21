# Entry decision (`entry`)

Combines the location, retest, confluence, ATR gate, news and bias into a go / no-go for each entry style, with every veto spelled out and the order price.

Decides at **fill_time**, step 9 of the order of operations: Entry: every veto clears, then the order goes in. Inputs: side, level, retest, confluence, gate, news, sfp, bias, tuesday, context. Outputs: entry.

## entry.limit-at-lxpb

S/R entries are limit orders placed at the last high/low before the breakout, in the previous role's direction: bid the LHPB after an upside break, offer the LLPB after a downside break.

*hard, stated, tested by code.* Sources: webinar curated/levels.md (2024-09-24); lesson curated/taking-a-trade-3-touch-level.md (2024-10-21).

Supporting passages:
- [sources/course/tweets/tweets_clean.md:50](../sources/course/tweets/tweets_clean.md) (2026, score 26.1): "Wait for a daily close back below the spike high, offer the retest of that high if it remains naked on H1, and bid the spike low for the exit."
- [sources/course/general/general_keywords.md:933](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 17.74): "- 29/04/2026 | CHFJPY Direction:W1 bearish pinbar x SFPW1 Low of HammerD1 S/RObjection:the weekly technically hasnt broken under the bearish hammer, as it hasnt yet closed.Location:D1 LXPBExecution:H1 LXPB with TP @FTA [image]"
- [sources/course/curated/notes-images.md:59](../sources/course/curated/notes-images.md) (notes-images, score 17.32): "so if price is hovering near his level before NFP for example, he will remove the limit - and if applicable - reinsert it about 10 minutes after the news release. [image]"

## entry.blind-with-three-touches

A level with three or more touches is traded blind, with no extra confluence.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24).

Supporting passages:
- [sources/levels-webinar-part1.md:181](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 16.88): "[24:33] of thumb if I've got three touches or more as I say I will trade it blind if I've got two touches last then I'm going"
- [sources/course/curated/taking-a-trade-2-touch-level.md:1](../sources/course/curated/taking-a-trade-2-touch-level.md) (2-touch-level, score 15.88): "**21/10/2024 16:16** because we already have 2 touches on our level, only +1 extra confluence is needed. any more is a bonus, but only 1 is mandatory."
- [sources/levels-webinar-part1.md:241](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 15.73): "[32:55] okay okay so we want to look at next is what happens if you get a level that hasn't had three touches because we know those are the"

## entry.vetoes-stack

All of these must clear before an S/R entry: the level is on the right side of the market, it is tradeable, it has its confluences, there was space and it is not the immediate retest, it has not been swerved, the market has not already passed its daily ATR in the trade direction, and it is not inside red news.

*hard, inferred, tested by code.* Sources: lesson curated/taking-a-trade-1-touch-level.md (2024-10-21); webinar user-pasted notes (2024-10-03).

Supporting passages:
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 33.53): "you do NOT trade (in the same direction) if the market has passed its daily ATR. "if we're 70 points down on the day, and the ATR is 70. im not selling. I don't care if its doing a rounded retest on the mother of all levels. I am not sellin"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 32.66): "* Never trade in same direction if market has passed daily ATR"
- [sources/course/curated/notes-images.md:33](../sources/course/curated/notes-images.md) (notes-images, score 27.63): "he pointed out another entry on a different chart with an 11 candle retest and said ''i would say this is just about enough space''. so it seems like he wants at least ~half a day of time to pass before his H1 entries. (edited) [image]"

## entry.prefer-the-level-before-the-trap

Between two levels, prefer the one that comes in before the level price is trapped at, and the one closer to the great higher-timeframe level. This also avoids swerves.

*preference, observed, not coded.* Sources: journal curated/lxpb-trades-pattern-failure.md (2025-01-06); journal curated/lxpb-trades-return-to-hammer.md (2025-01-03).

Supporting passages:
- [sources/course/curated/lxpb-trades-return-to-hammer.md:1](../sources/course/curated/lxpb-trades-return-to-hammer.md) (return-to-hammer, score 14.98): "note: despite there being a lower level for entry, the upper one is used in this case (the true LXPB). not only is this fairly common just to avoid swerves in general, but the trader also said that we would prefer the upper level anyway, be"
- [sources/levels-webinar-part1.md:181](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 12.82): "[24:10] okay fairly straightforward now the question comes into play when you've got levels that have less than"
- [sources/user-research-pages.md:179](../sources/user-research-pages.md) (Does it survive out of sample, score 11.92): "**What changed.** One parameter: the minimum number of bars between a break and a qualifying retest. Everything else is untouched — 3-touch levels clustered within 0.1× the prior day's ATR, entry at the level price, stop at the break candle"

## entry.limit-after-acceptance

Newer practice: enter by limit after a close and acceptance beyond the prior level, at the nearest naked level to the SFP low. Do not blindly sell prior highs; wait to see the price action.

*hard, observed, not coded.* Sources: tweet curated tweets (2026-06-03); tweet curated tweets (2022-10-21).

Supporting passages:
- [sources/course/tweets/tweets_clean.md:7](../sources/course/tweets/tweets_clean.md) (2022, score 36.91): "Not exactly. At a level like the July '90 high, I would look for an SFP because I don't blindly sell prior highs, I wait to see the price action. If that level had been prior support and it was naked (not retested on the H1 post break), I w"
- [sources/course/tweets/tweets_clean.md:50](../sources/course/tweets/tweets_clean.md) (2026, score 34.16): "H1: Entry via limit offer after close and acceptance below prior support (nearest naked level to the SFP low)"
- [sources/course/curated/advanced-sfp.md:61](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 19.33): "**24/09/2024 18:50** on the return to this 1 touch support level (where you would normally be looking for an SFP to enter) - the trader said he would not buy here, even though the SFP does come. the reason why he would not buy this SFP into"

## entry.offer-the-retest-of-the-spike

After a spike, wait for a daily close back inside, offer the retest of the spike high if it stays naked on H1, and bid the spike low for the exit.

*hard, observed, not coded.* Sources: tweet curated tweets (2026-05-19).

Supporting passages:
- [sources/course/tweets/tweets_clean.md:50](../sources/course/tweets/tweets_clean.md) (2026, score 40.7): "Wait for a daily close back below the spike high, offer the retest of that high if it remains naked on H1, and bid the spike low for the exit."
- [sources/course/tweets/tweets_clean.md:7](../sources/course/tweets/tweets_clean.md) (2022, score 23.83): "Not exactly. At a level like the July '90 high, I would look for an SFP because I don't blindly sell prior highs, I wait to see the price action. If that level had been prior support and it was naked (not retested on the H1 post break), I w"
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 16.85): "* If no HTF bias and no FTA, target the opposite side (e.g., swept major H1 low, TP at major H1 high)"

## entry.blind-bid-then-add

Blind-bid the D1 level, then add on the H1 price action. The first position then has a near-breakeven stop and total risk stays at 1R.

*hard, observed, not coded.* Sources: tweet curated tweets (2026-05-01).

Supporting passages:
- [sources/course/tweets/tweets_clean.md:24](../sources/course/tweets/tweets_clean.md) (2026, score 31.03): "In my opinion, you should have blind bid the D1 level and then added on the H1 PA. This would have given you a near be stop on the 1st position, making your risk still 1R overall (assuming same risk % size on second addition) but adding R t"
- [sources/course/tweets/tweets_clean.md:50](../sources/course/tweets/tweets_clean.md) (2026, score 19.15): "Wait for a daily close back below the spike high, offer the retest of that high if it remains naked on H1, and bid the spike low for the exit."
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 19): "* A level becomes untradeable when the latest touch isn’t respected by price action"

## entry.re-sell-the-same-level

Selling a flipped level again on the retest after taking profits there means never being offside during the whole trending move.

*heuristic, stated, not coded.* Sources: webinar curated/tp-selection.md (2024-09-24).

Supporting passages:
- [sources/course/curated/tp-selection.md:19](../sources/course/curated/tp-selection.md) (tp-selection, score 25.26): "basically meaning i am never offside for the entire trending move (assuming we get the retests)"
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 20.59): "you do NOT trade (in the same direction) if the market has passed its daily ATR. "if we're 70 points down on the day, and the ATR is 70. im not selling. I don't care if its doing a rounded retest on the mother of all levels. I am not sellin"
- [sources/levels-webinar-part1.md:76](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 18.83): "[9:40] stop cells obviously below the level they buy the level if the level breaks they're going to stop out but at the same time there are what you get the"

## entry.entry-types-when-no-lxpb

When there is no LXPB the entry can be an H1 pin or a spike candle at the higher-timeframe level.

*hard, observed, not coded.* Sources: journal curated/lxpb-trades-passed-atr.md (2024-12-16); journal curated/lxpb-trades-return-to-hammer.md (2024-12-22).

Supporting passages:
- [sources/course/curated/lxpb-trades-passed-atr.md:1](../sources/course/curated/lxpb-trades-passed-atr.md) (passed-atr, score 20.15): "**16/12/2024 03:18** no weekly confluencelevel is at daily bearish hammerh1 spike candle rejecting off h1 LXPBpast ATR"
- [sources/course/curated/lxpb-trades-engulfing.md:1](../sources/course/curated/lxpb-trades-engulfing.md) (engulfing, score 16.7): "**26/02/2026 16:26** weekly engulfing KPI currently being disrespected (not closed, thursday)daily zone flippedH1 LXPB just underneath the D1 zone (entry as low as possible)."
- [sources/course/curated/notes-images.md:114](../sources/course/curated/notes-images.md) (notes-images, score 15.85): "in this case, because there was no clear thrust candle (through the whole zone), he will typically move his SL a candle to the left (orange arrow) to make it safer. [image]"

## entry.pull-on-drift-of-doom

Pull a limit when price stair-steps down to it ("drift of doom"), even when the first trouble area is not too close.

*hard, observed, not coded.* Sources: journal curated/lxpb-trades-engulfing.md (2026-02-26); journal curated/lxpb-trades-return-to-hammer.md (2025-05-02).

Supporting passages:
- [sources/course/curated/lxpb-trades-engulfing.md:1](../sources/course/curated/lxpb-trades-engulfing.md) (engulfing, score 24.76): "note: he pulled this limit when price started stair-stepping down to the entry. not because FTA was too close, but just because it was a bit too 'drift of doom'. [image]"
- [sources/course/curated/taking-a-trade-targets.md:1](../sources/course/curated/taking-a-trade-targets.md) (targets, score 16.15): "**22/10/2024 16:18** to choose a Target Price (TP) on a trade, we ask ourselves which is the first area on the chart that might cause some trouble"
- [sources/course/curated/notes-images.md:59](../sources/course/curated/notes-images.md) (notes-images, score 13.44): "so if price is hovering near his level before NFP for example, he will remove the limit - and if applicable - reinsert it about 10 minutes after the news release. [image]"

## entry.counter-bias-warning

Trading against a fresh higher-timeframe bias is allowed only with good levels and a quick exit.

*hard, stated, tested by code.* Sources: webinar curated/trade-management.md (2024-10-18).

Supporting passages:
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 16.99): "* Trading against the daily bias requires good levels and quick exits at first sign of trouble"
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 16.59): "* TP at FTA (First Technical Area) if no higher timeframe bias"
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 15.47): ""if youre trading a H1 level, at 8pm and you see a good level to go short... but its quite evident that when you flip to the daily timeframe, the market is very likely to close as a bullish engulfing - youre probably not going to take that "
