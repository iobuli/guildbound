# Which side of the market is the level on?

`market-side` · stage **D** · decides at **prior_bars** · computed

The first question in the method: he always buys below the market and sells above it. A level below price can only be a buy; a level above price can only be a sell. That fixes the direction before anything else is looked at.

## Ports

**Inputs**
- none

**Outputs**
- `side`: relation: below|above|unknown, requiredDir: long|short|neutral, valid: boolean, detail: string

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **market-side.buy-below-sell-above**<br>"I'm always buying below market and I'm always selling above market." So the trade is always counter to the short-term trend: buying a dip or selling a rally. A level below price is a buy, a level above is a sell.<br>*Applies to resting limit orders at levels. SFP entries at the candle close are market orders and are exempt.* | hard | stated | yes | transcript: levels webinar part 1, user-pasted transcript @4:43<br>transcript: levels webinar part 1, user-pasted transcript @5:08<br>transcript: levels webinar part 1, user-pasted transcript @1:08:16 |
| **market-side.draw-levels-on-three-timeframes**<br>Levels are drawn on the weekly, daily and hourly charts, and the hourly shows all of them. The question is then which of those to trade. | hard | stated | no | transcript: levels webinar part 1, user-pasted transcript @2:44 |
| **market-side.homework**<br>Homework given: when price rallies, look at the areas above market that were support last; when it falls, look at the areas below market that were resistance last, and trade off those. | hard | observed | no | transcript: levels webinar part 1, user-pasted transcript @1:08:00 |
| **market-side.nothing-on-the-right-side**<br>If no level on the right side of the market qualifies, there is no trade. In the gold example every level above market was resistance last, so there was nothing to sell until price rallied into one. | hard | observed | no | transcript: levels webinar part 1, user-pasted transcript @21:02<br>transcript: levels webinar part 1, user-pasted transcript @1:04:06 |
