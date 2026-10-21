# ATR yardsticks (`atr`)

Measures volatility on D1 and H1. Almost every other node sizes things in ATR, so this is the only place ATR is computed.

Decides at **prior_bars**, step 0 of the order of operations: ATR yardsticks. Inputs: none. Outputs: atr.

## atr.d1-is-the-yardstick

The daily ATR is the yardstick for what counts as a deep swing, how close a fib must be, and whether the market has already "passed ATR".

*hard, stated, not coded.* Sources: lesson curated/taking-a-trade-h1-sweep.md (2024-10-22); webinar curated/advanced-sfp.md (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 28.33): "so if the D1 ATR = 100, and the fib is ≤10 pips away from your level, then it counts as a confluence."
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 26.72): "* Never trade in same direction if market has passed daily ATR"
- [sources/course/curated/advanced-sfp.md:19](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 19.47): "for a D1 swing to be considered 'deep' it must be 3x the size of the current day's ATR. [image]"

## atr.h1-24-for-sfp-stop

The H1 SFP stop uses the 24-period ATR taken at the close of the sweep candle.

*hard, stated, tested by code.* Sources: webinar user-pasted SFP notes (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 21.38): "* Use 24-period ATR value at close of sweep candle"
- [sources/course/curated/advanced-sfp.md:47](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 18): "if there is a H1 sweep of a D1 level (but fits the H1 criteria of being 1x ATR) this is still tradeable even if the d1 is not 3x ATR, because its still a perfectly valid H1 setup."
- [sources/course/curated/advanced-sfp.md:77](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 15.86): "**24/09/2024 19:14** as previously mentioned, if an hourly SFP is created like this (where the sweep-bar is a huge bearish candle that closes really near the low) - then he waits for price to develop a bit more. [image]"

## atr.m5-uses-12

M5 SFPs use a 12-period ATR instead.

*hard, stated, not coded.* Sources: webinar curated/levels.md (2024-09-28). Note: No M5 data in this build, so not computed.

Supporting passages:
- [sources/course/curated/levels.md:65](../sources/course/curated/levels.md) (levels, score 17.96): "the only difference is that his m5 SFPs use a 12-period ATR"
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 14.88): "* Use 24-period ATR value at close of sweep candle"

## atr.d1-period-20

The daily ATR is a 20-period ATR, because there are about 20 trading days in a month: it averages how far the market moves in a day over the last month.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @43:34.

Supporting passages:
- [sources/levels-webinar-part1.md:320](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 34.61): "[43:34] period ATR Okay the reason I use a 20 period ATR is because uh there's 20 trading"
- [sources/levels-webinar-part1.md:584](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 17.98): "[1:19:33] be uh the ATR if the market is trading past its ATR coming into this area then I would trade it now just for those you"
- [sources/levels-webinar-part1.md:337](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 17.22): "[44:45] market is moving on average per day during this trade which was 20 23rd of February"

## atr.use-the-day-before

Read the ATR of the day BEFORE the day in question (hover over the previous daily bar), not the day that triggers the trade.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @44:21. Note: Closed daily bars only, so the forming day is never included. This also keeps it causal.

Supporting passages:
- [sources/levels-webinar-part1.md:320](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 24.87): "[44:21] now we do is we are going to hover over okay the day before not the day that"
- [sources/levels-webinar-part1.md:272](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 13): "[37:31] or bearish engulfing candle now again like I say today I don't want to go into that because we've already gone over it before but if I've got one of those"
- [sources/levels-webinar-part1.md:584](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 12.56): "[1:19:33] be uh the ATR if the market is trading past its ATR coming into this area then I would trade it now just for those you"

## atr.h1-length-unspecified

No source gives an H1 ATR length. This build uses Wilder 14 as a scale only (zone width, swing size). The 24-period value is stated for the SFP stop.

*heuristic, inferred, tested by code.* Sources: webinar curated/advanced-sfp.md.

Supporting passages:
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 25.77): "* H1 SFP trades use ATR-based stop loss"
- [sources/course/curated/advanced-sfp.md:19](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 17.04): "well for that, we use the current D1 ATR value, and the concept of 'swing in' and 'swing out'"
- [sources/course/curated/advanced-sfp.md:47](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 15.83): "if there is a H1 sweep of a D1 level (but fits the H1 criteria of being 1x ATR) this is still tradeable even if the d1 is not 3x ATR, because its still a perfectly valid H1 setup."
