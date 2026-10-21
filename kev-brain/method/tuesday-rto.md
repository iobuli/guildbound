# Tuesday weekly-open retest (`tuesday-rto`)

A self-contained setup: on a Tuesday, if the weekly open has not been touched, trade an H1 pinbar toward it with the pin as the stop.

Decides at **prior_bars**, step 0.3 of the order of operations: Tuesday weekly-open setup. Inputs: none. Outputs: tuesday.

## tuesday-rto.rules

Tuesday only. The weekly open must not have been touched since 22:00 on Monday (and not so far on Tuesday). Buy or sell with the weekly open as the target.

*hard, stated, tested by code.* Sources: lesson curated/taking-a-trade-tuesday-rto.md (2025-01-10).

Supporting passages:
- [sources/course/curated/taking-a-trade-tuesday-rto.md:1](../sources/course/curated/taking-a-trade-tuesday-rto.md) (tuesday-rto, score 54.84): "**10/01/2025 12:50** RULES:tuesday onlyweekly open must not have been touched since 22:00 on monday. (no touching it so far on tuesday).buy / sell with a target of the weekly open."
- [sources/course/curated/lxpb-trades-return-to-hammer.md:1](../sources/course/curated/lxpb-trades-return-to-hammer.md) (return-to-hammer, score 15.32): "**22/12/2024 23:57** weekly range (good target)daily pinbarH1 pin at D1 HoH"
- [sources/course/general/general_full.md:1748](../sources/course/general/general_full.md) (general, score 11.38): "as far as weekly sweeps go, theyre only really powerful when theyre WITH MS, a bearish sweep AGAINST bullish MS doesnt do a whole lot most of the time. a bullish sweep WITH bullish MS would be powerful af though [image]"

## tuesday-rto.getting-in

Enter on a multiple-tap level or an H1 pinbar. Weekly open above price: buy a bullish pin. Weekly open below: sell a bearish pin.

*hard, stated, tested by code.* Sources: lesson curated/taking-a-trade-tuesday-rto.md (2025-01-10). Note: Only the pinbar route is coded; the multiple-tap-level route needs a level and is left to the level node.

Supporting passages:
- [sources/course/curated/taking-a-trade-tuesday-rto.md:1](../sources/course/curated/taking-a-trade-tuesday-rto.md) (tuesday-rto, score 51.49): "GETTING IN A TRADE:buy / sell a multiple tap level.buy / sell an hourly pinbar.(weekly open above price = buy a bullish pin)"
- [sources/course/general/general_full.md:14629](../sources/course/general/general_full.md) (take notes, score 24.32): "**02/04/2026 19:22** weekly bias: pinbar + sweep = bullishdaily bias: broke above resistance, coming back to retest it as support = bullishh1: bullish pinbar right at the D1/W1 support levelTP at the next level up (green circle) for +2.1R ["
- [sources/course/general/general_keywords.md:918](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 22.01): "- 27/03/2026 | weekly bias: none daily bias: double top = bullish h1: broke above its green resistance level. no resistance between here and the double top. entry is the hourly hammer in the green S/R zone. [image]"

## tuesday-rto.orders

Buy the top of the pin or sell its low. The stop goes on the other side of the pinbar. Only take trades that are at least 1R.

*hard, stated, tested by code.* Sources: lesson curated/taking-a-trade-tuesday-rto.md (2025-01-10).

Supporting passages:
- [sources/course/curated/taking-a-trade-tuesday-rto.md:1](../sources/course/curated/taking-a-trade-tuesday-rto.md) (tuesday-rto, score 43.41): "SETTING THE ORDERS:buy the top of the pin / sell the low of the pin.put your SL on the other side of the pinbar.only take trades that are minimum 1R (or more)."
- [sources/levels-webinar-part1.md:76](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 17.4): "[9:40] stop cells obviously below the level they buy the level if the level breaks they're going to stop out but at the same time there are what you get the"
- [sources/course/general/general_keywords.md:359](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 16.99): "- 14/06/2023 | I be looking into sell trades now?"

## tuesday-rto.holds-past-tuesday

It does not matter if the trade runs into Wednesday; only the entry day counts.

*hard, stated, not coded.* Sources: lesson curated/taking-a-trade-tuesday-rto.md (2025-01-10).

Supporting passages:
- [sources/course/curated/taking-a-trade-tuesday-rto.md:1](../sources/course/curated/taking-a-trade-tuesday-rto.md) (tuesday-rto, score 20.08): "(note: it doesnt matter that this trade went on into wednesday. all that matters is that the entry was tuesday.)"
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 11.01): "| SOL: 503 trades, 75.7% win, +$27,806/yr, Sharpe 3.41 | SOL Trade Check | Inflated by the same filter. Causally, at 4R: . The transfer to SOL without refitting still holds — it is simply a smaller number. |"
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 10.31): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"

## tuesday-rto.weekly-open-definition

The weekly open is the first bar of the trading week: Sunday 17:00 New York for FX and metals (the 22:00 UK open the sources refer to), Monday 00:00 UTC for crypto. "Since 22:00 on Monday" is read on the London clock.

*hard, inferred, tested by code.* Sources: lesson curated/taking-a-trade-tuesday-rto.md (2025-01-10).

Supporting passages:
- [sources/course/curated/taking-a-trade-tuesday-rto.md:1](../sources/course/curated/taking-a-trade-tuesday-rto.md) (tuesday-rto, score 22.67): "**10/01/2025 12:50** RULES:tuesday onlyweekly open must not have been touched since 22:00 on monday. (no touching it so far on tuesday).buy / sell with a target of the weekly open."
- [sources/course/general/general_full.md:12846](../sources/course/general/general_full.md) (take notes, score 14.49): "**12/06/2024 22:47** Was up Scotland last week now I’m down Southampton"
- [sources/course/general/general_keywords.md:1](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 13.39): "- 19/05/2022 | my first week off from trading in about 3 years"
