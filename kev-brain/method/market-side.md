# Which side of the market is the level on? (`market-side`)

The first question in the method: he always buys below the market and sells above it. A level below price can only be a buy; a level above price can only be a sell. That fixes the direction before anything else is looked at.

Decides at **prior_bars**, step 1 of the order of operations: Which side of the market is the level on? (buy below, sell above). Inputs: none. Outputs: side.

## market-side.buy-below-sell-above

"I'm always buying below market and I'm always selling above market." So the trade is always counter to the short-term trend: buying a dip or selling a rally. A level below price is a buy, a level above is a sell.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @4:43; transcript levels webinar part 1, user-pasted transcript @5:08; transcript levels webinar part 1, user-pasted transcript @1:08:16. Note: Applies to resting limit orders at levels. SFP entries at the candle close are market orders and are exempt.

Supporting passages:
- [sources/levels-webinar-part1.md:31](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 51.55): "[4:59] Market now what that means essentially is that I'm always going counter to the short-term trade short-term Trend I"
- [sources/levels-webinar-part1.md:465](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 22.97): "[1:03:58] level is going to make a fantastic selling opportunity because we're below it and we're rallying up we're looking left and"
- [sources/levels-webinar-part1.md:481](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 22.29): "[1:05:42] when price breaks down through an area like this and we sell the rally into this a below with space and preferably with some momentum coming into it as"

## market-side.draw-levels-on-three-timeframes

Levels are drawn on the weekly, daily and hourly charts, and the hourly shows all of them. The question is then which of those to trade.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @2:44.

Supporting passages:
- [sources/levels-webinar-part1.md:16](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 20.32): "[2:44] previous levels you'll note that we draw levels in on the weekly time frame the daily time frame and the hourly time frame and and so what that essentially means is that when you're on the hourly"
- [sources/levels-webinar-part1.md:241](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 15.12): "[32:01] breakouts they will just on your charts as levels that were the inverse last but I always use the same principle if it's"
- [sources/levels-webinar-part1.md:552](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 14.99): "1:15:001 hour, 15 minutestouches or three plus okay and and the question and is all with the inverse okay with levels that are the inverse"

## market-side.homework

Homework given: when price rallies, look at the areas above market that were support last; when it falls, look at the areas below market that were resistance last, and trade off those.

*hard, observed, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @1:08:00.

Supporting passages:
- [sources/levels-webinar-part1.md:499](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 30.26): "[1:08:24] for areas that structurally strong resistance last and look to trade off of those"
- [sources/levels-webinar-part1.md:451](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 23.36): "[1:00:37] look at this level and say is there any resistance very close to Market which but to this level which price might squeeze towards if I look a little bit"
- [sources/levels-webinar-part1.md:46](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 22.21): "[6:19] was support last okay or it could have been that the market has uh has done neither of those things at your level"

## market-side.nothing-on-the-right-side

If no level on the right side of the market qualifies, there is no trade. In the gold example every level above market was resistance last, so there was nothing to sell until price rallied into one.

*hard, observed, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @21:02; transcript levels webinar part 1, user-pasted transcript @1:04:06.

Supporting passages:
- [sources/levels-webinar-part1.md:149](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 26.24): "[21:02] are now currently in uh gold there is no level above Market that was support last"
- [sources/levels-webinar-part1.md:165](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 19.89): "[21:39] look left on the last touch we want to have seen the market respect it and it didn't it traded right in and out of"
- [sources/course/curated/taking-a-trade-tuesday-rto.md:1](../sources/course/curated/taking-a-trade-tuesday-rto.md) (tuesday-rto, score 17.81): "GETTING IN A TRADE:buy / sell a multiple tap level.buy / sell an hourly pinbar.(weekly open above price = buy a bullish pin)"
