# Market regime (`regime`)

Describes the market the levels sit in: daily and weekly trend, where price is in its recent range, how volatile it is against its own past year, and how far it is below its high. Context for the reading only; it gates nothing.

Decides at **prior_bars**, step 0.2 of the order of operations: Regime: trend, volatility, range position (context only). Inputs: atr. Outputs: regime.

## regime.trend-from-the-daily-mean

Trend is the 10-day change of the 20-day mean close, measured in daily ATRs: up at +1 ATR or more, down at -1 or less, otherwise range. The weekly trend compares the last weekly close with the 10-week mean.

*heuristic, inferred, tested by code.* Sources: backtest builder's descriptor: the sources describe HTF context and bias by bar patterns but give no trend definition; this one is only for describing the market.

Supporting passages:
- [sources/levels-webinar-part1.md:584](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 19.18): "[1:19:41] didn't understand the ATR we pull up ATR 20 period overlay"
- [sources/levels-webinar-part1.md:320](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 18.9): "[44:14] been moving on average over the last 20 days but we're concerned with the day in question when we came down to this level"
- [sources/course/general/general_full.md:14210](../sources/course/general/general_full.md) (take notes, score 17.83): "Also this Bullish englufing, I'm assuming the would be with the trend, despite the last 2/3 candles saying down?] [image]"

## regime.volatility-against-its-own-past

Volatility is the daily ATR as a percentage of price, ranked against the past 250 days: below the 33rd percentile is low, above the 67th is high.

*heuristic, inferred, tested by code.* Sources: backtest builder's descriptor: ranks volatility so that crypto and gold read on the same scale.

Supporting passages:
- [sources/course/curated/taking-a-trade-h1-sweep.md:36](../sources/course/curated/taking-a-trade-h1-sweep.md) (h1-sweep, score 17.59): "remembering that we need to measure these swing points against the Daily ATR, we first take our reading - and see that it is 568 pipettes. [image]"
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 17.38): "* Place SL that many pips below SFP low (for longs) or above sweep high (for shorts)"
- [sources/course/curated/lxpb-trades-passed-atr.md:1](../sources/course/curated/lxpb-trades-passed-atr.md) (passed-atr, score 14.77): "note: the daily level is completely irrelevant to this setup, as the hourly LXPB is just too far away.its also worth noting that the TP is below the daily level, just like in this tweet right here, where entry is the LXPB, and youre getting"

## regime.context-only

Regime never vetoes or adds a confluence. It feeds the interpreter so a reading can say what kind of market a level is in.

*hard, inferred, tested by code.* Sources: backtest builder's descriptor: kept out of the decision path until a pre-registered test says it helps.

Supporting passages:
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 12.3): "* Never trade in same direction if market has passed daily ATR"
- [sources/levels-webinar-part1.md:584](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 11.76): "[1:19:33] be uh the ATR if the market is trading past its ATR coming into this area then I would trade it now just for those you"
