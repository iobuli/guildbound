# Level scanner (`level-scan`)

Marks the levels on the chart: clusters the swing highs and lows, drops any that price has not yet broken, and for each one on the right side of the market counts the swing touches in its previous role and says which tier it falls in.

Decides at **prior_bars**, step 0.5 of the order of operations: Mark the levels on the chart and count their swing touches. Inputs: atr, swings. Outputs: levels.

## level-scan.mark-levels-first

Start by marking prior support/resistance flips on the weekly, daily and hourly charts, then look at where price currently is and which of those levels sit on the right side of it.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @2:44; transcript levels webinar part 1, user-pasted transcript @5:25; transcript levels webinar part 1, user-pasted transcript @1:02:30.

Supporting passages:
- [sources/levels-webinar-part1.md:31](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 30.72): "[5:33] levels but you should have Lev your charts that Mark out prior support resistance flips especially if you've attended the previous webinar on levels"
- [sources/levels-webinar-part1.md:16](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 24.46): "[2:44] previous levels you'll note that we draw levels in on the weekly time frame the daily time frame and the hourly time frame and and so what that essentially means is that when you're on the hourly"
- [sources/course/general/general_keywords.md:918](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 22.11): "- 21/04/2026 | skipped trade: engulfing failure price above the weekly S/Rdaily closed above the engulfing KPI (low of the candle prior to the bearish engulfing)hourly LXPB right on top of the weekly zoneSL well below the weekly zone to be "

## level-scan.triage-by-touches

Grade every level by its number of swing touches, which fixes the plan for it: 3 or more is traded blind, 2 needs one confluence, 1 needs two.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @23:35; transcript levels webinar part 1, user-pasted transcript @24:25; transcript levels webinar part 1, user-pasted transcript @1:14:52.

Supporting passages:
- [sources/levels-webinar-part1.md:181](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 17.88): "[23:44] I will trade it blind now what I mean by blind is I will literally just leave an order in at that level with a stop and a"
- [sources/levels-webinar-part1.md:552](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 17.43): "[1:15:16] three touches plus the inverse then I will trade it blind what blind means is just bang in a limit order okay I'm just"
- [sources/course/curated/taking-a-trade-2-touch-level.md:1](../sources/course/curated/taking-a-trade-2-touch-level.md) (2-touch-level, score 16.95): "**21/10/2024 15:39** every support / resistance trade needs 3 confluences. but not every S/R level will be a ."

## level-scan.four-touch-support-is-not-a-buy

A level price has hit four times as support is NOT bought: the stops of breakout traders and of failing longs pile up below it. If it breaks, it becomes the great sell on the rally back.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @8:39; transcript levels webinar part 1, user-pasted transcript @12:24; transcript levels webinar part 1, user-pasted transcript @1:03:09. Note: The same-direction case is the subject of a second webinar that is not in the sources provided.

Supporting passages:
- [sources/levels-webinar-part1.md:61](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 37.95): "[9:24] has been hit one two three and it's been hit four times and it starts to come back for time number five a lot of Traders are looking at that thinking more wow it's been hit four times it's"
- [sources/levels-webinar-part1.md:76](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 36.43): "[9:40] stop cells obviously below the level they buy the level if the level breaks they're going to stop out but at the same time there are what you get the"
- [sources/levels-webinar-part1.md:90](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 27.07): "[11:53] if he starts to buy at support level he's going to get joined by by other buyers to see his orders coming in to see price moving up they're going to"

## level-scan.clustering-is-the-builders

The sources mark levels by eye. This scanner clusters swing prices within half an H1 ATR and keeps those within 20% of price. Those two numbers are the builder's.

*heuristic, inferred, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @2:44.

Supporting passages:
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 14.42): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 13.08): "* Fibonacci levels must be within 10% of day’s ATR to count as confluence"
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 13): "fib levels must be within 10% of the day's ATR to count as a confluence."
