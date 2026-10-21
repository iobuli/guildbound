# Swing failure pattern (SFP) (`sfp`)

Finds sweeps of deep swings that close back inside, applies the visit-count, size and invalidation conditions, and says how the entry and stop should be set.

Decides at **prior_bars**, step 0.2 of the order of operations: SFP detector. Inputs: atr, swings. Outputs: sfp.

## sfp.definition

An SFP is essentially a false break of a high or low, but with strict criteria for what it must look like and when it is traded.

*hard, stated, not coded.* Sources: tweet curated tweets (2026-05-22).

Supporting passages:
- [sources/course/tweets/tweets_clean.md:50](../sources/course/tweets/tweets_clean.md) (2026, score 29.43): "Swing Failure Pattern. Essentially a false break of a high or low but I have strict criteria for what it should look like and how and when I will trade it."
- [sources/course/tweets/tweets_clean.md:7](../sources/course/tweets/tweets_clean.md) (2022, score 12.5): "Not sure how you trade but it's worth taking a look at the 5m after an H1 SFP forms. A great low risk, high reward entry can be found selling the original SP with a stop above the thrust candle. I've been trading this way for years in the B"
- [sources/course/curated/levels.md:19](../sources/course/curated/levels.md) (levels, score 12.11): "daily range, price made a false break under the bottom of the range, and got back inside as a shitty SFP (not sweeping a deep low) and a good pinbar. [image]"

## sfp.deep-swing-only

Only sweeps of significant (deep) swings count. Sweeping a shallow swing is ignored.

*hard, stated, tested by code.* Sources: webinar curated/advanced-sfp.md (2024-09-24); lesson curated/taking-a-trade-h1-sweep.md (2024-10-22).

Supporting passages:
- [sources/course/general/general_full.md:14313](../sources/course/general/general_full.md) (take notes, score 19.12): "Say a sweep has closed and its sweeping a super recent swing. But scrolling back you also notice its sweeping a significant swing aswell. Do we prioritise the recent swing that its swept or the significant one? I'm assuming we just prioriti"
- [sources/course/curated/advanced-sfp.md:19](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 18.39): "only sweeps of 'significant / deep' highs and lows count. but how do we define a 'deep swing'? (edited)"
- [sources/course/curated/taking-a-trade-h1-sweep.md:1](../sources/course/curated/taking-a-trade-h1-sweep.md) (h1-sweep, score 17.99): "not just any sweep will do, only sweeps of 'significant / deep' highs and lows count. but how do we define a 'deep swing'?"

## sfp.first-sweep-only

Only the first sweep of a level is tradeable. The second or any later sweep is invalid. A fresh sweep in a new area is fine.

*hard, stated, tested by code.* Sources: lesson curated/taking-a-trade-h1-sweep.md (2024-10-22).

Supporting passages:
- [sources/course/curated/taking-a-trade-h1-sweep.md:62](../sources/course/curated/taking-a-trade-h1-sweep.md) (h1-sweep, score 20.93): "it is very important to note - that we will only trade the first sweep at a certain level"
- [sources/course/curated/advanced-sfp.md:47](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 12.16): "if there is a H1 sweep of a D1 level (but fits the H1 criteria of being 1x ATR) this is still tradeable even if the d1 is not 3x ATR, because its still a perfectly valid H1 setup."
- [sources/course/general/general_keywords.md:467](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 10.28): "- 03/08/2023 | realistic, take the first setup = +2 -1 take the second setup = -1 +2"

## sfp.visit-count

Count visits to the area: the 1st creates the swing, the 2nd needs a sweep, the 3rd needs a sweep AND a MACD divergence, the 4th is a no. Tests on the daily count toward the hourly.

*hard, stated, tested by code.* Sources: webinar curated/advanced-sfp.md (2024-09-24); webinar curated/advanced-sfp.md (2024-09-26). Note: The MACD divergence is flagged (needsDivergence) but not computed here.

Supporting passages:
- [sources/course/curated/advanced-sfp.md:47](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 37.12): "1st time in the area is just to create the swing point.2nd time in the area requires a sweep of the swing point.3rd time in the area requires a sweep and a MACD divergence.4th time back in the area = no. just no."
- [sources/course/curated/advanced-sfp.md:61](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 27.14): "SFP at the level.MACD divergence across all three tests. [image]"
- [sources/course/curated/advanced-sfp.md:91](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 24.34): "**24/09/2024 19:23** this is the daily chart of a support level price was back at for the 3rd time, with an SFP + MACD divergence - signalling longs."

## sfp.time-and-space

There must be time and space between the original swing and the SFP, as with an S/R retest. Swing levels stay valid for about 2 days.

*hard, stated, tested by code.* Sources: webinar user-pasted SFP notes (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 24.9): "* Need time and space between original swing point and the SFP (similar to S/R retest)"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 14.01): "* Fibonacci levels must be within 10% of day’s ATR to count as confluence"
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 12.06): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"

## sfp.enter-at-close

On an H1 sweep that closes back inside, sell (buy) at the current price the moment the candle closes.

*hard, stated, tested by code.* Sources: lesson curated/taking-a-trade-h1-sweep.md (2024-10-22).

Supporting passages:
- [sources/course/curated/taking-a-trade-h1-sweep.md:36](../sources/course/curated/taking-a-trade-h1-sweep.md) (h1-sweep, score 19.81): "we sell at the exact current price, the moment that the H1 candle closes as a sweep. (edited) [image]"
- [sources/user-research-pages.md:13](../sources/user-research-pages.md) (A bar has four prices and only one of them is the last, score 16.99): "Both are conditions on the **close**. The fill happened at the **level**, earlier in the same hour. At the moment money went on the table, that close did not exist yet. The backtest knew it because the backtest was reading a finished candle"
- [sources/course/curated/advanced-sfp.md:1](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 15.46): "a gap up.price opening and then continuing to push away from the gap.a sweep of the high.is a 99.999% SR setup. so there is no way you'd want to be picky about the candle close under this scenario."

## sfp.strong-close-wait

If the sweep bar is wide and closes within 20% of its extreme, do not enter at the close. Draw the ATR stop; if price moves about half way toward it and turns back, place a stop order at the original SFP close.

*hard, stated, tested by code.* Sources: webinar curated/advanced-sfp.md (2024-09-24); webinar user-pasted SFP notes (2024-09-24).

Supporting passages:
- [sources/course/curated/advanced-sfp.md:1](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 34.66): "so what you do is draw in your theoretical ATR based stop in. and if price (on the next bar) reaches ~50% towards this level - put a sell stop in at the original SFP close price. [image]"
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 29.65): "* Defined as “wide ranging bar that closes within 20% of the candle extreme”"
- [sources/course/curated/advanced-sfp.md:77](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 26.65): "**24/09/2024 19:14** as previously mentioned, if an hourly SFP is created like this (where the sweep-bar is a huge bearish candle that closes really near the low) - then he waits for price to develop a bit more. [image]"

## sfp.stop-atr24

Stop = the sweep extreme plus the 24-period ATR. The "advanced" entry uses the sweep extreme itself.

*hard, stated, tested by code.* Sources: webinar user-pasted SFP notes (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 23.11): "* Use 24-period ATR value at close of sweep candle"
- [sources/user-research-pages.md:303](../sources/user-research-pages.md) (Rebuild it yourself, score 14.3): "5. **mhm — 24/09/2024 21:08** not super applicable: if drawing a (weekly/daily i think this was) line at THE high/low, move it to S/R above/below (what are the odds price will stop at THE high (assume this high is the only touch on your lin"
- [sources/course/curated/advanced-sfp.md:47](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 13.8): "if there is a H1 sweep of a D1 level (but fits the H1 criteria of being 1x ATR) this is still tradeable even if the d1 is not 3x ATR, because its still a perfectly valid H1 setup."

## sfp.invalidation

If a candle closes beyond the SFP candle itself, the SFP is invalid and an open trade is exited at once.

*hard, stated, tested by code.* Sources: webinar user-pasted SFP notes (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 17.82): "* SFP invalidation: closing below the SFP candle itself (not just the candle being swept)"
- [sources/course/general/general_full.md:14401](../sources/course/general/general_full.md) (take notes, score 12.25): "**26/03/2025 13:31** If we are in a h1 trade and the next candle or two closes as a msb of the swing that was swept. Do you just leave the order to play out or close it?"
- [sources/user-research-pages.md:323](../sources/user-research-pages.md) (Rebuild it yourself, score 11.19): "SFP Identification and Invalidation"

## sfp.small-bars-meaningless

A sweep candle that is tiny (a 6-pip Asian-session bar) is unlikely to mean anything.

*hard, inferred, tested by code.* Sources: webinar curated/advanced-sfp.md (2024-09-24). Note: The 0.5 ATR cut-off is the builder's; the source gives only the example.

Supporting passages:
- [sources/course/curated/advanced-sfp.md:47](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 19.58): "he doesnt mind 'as much' in forex. he cares more about the volatility of the bar itself ''if you look at an SFP candle in the asian session that's only 6 pips wide. its not likely to mean anything.""
- [sources/levels-webinar-part1.md:519](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 18.03): "[1:11:55] although again like with anything you do have to take liquidity into into account with this because obviously if you're trading in the Asian session price is"
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 13.16): "* Defined as “wide ranging bar that closes within 20% of the candle extreme”"

## sfp.distrust-session-close

On the FTSE he does not trust SFPs forming at the close of the market (volatility from closing positions). Less of a concern in forex.

*hard, stated, not coded.* Sources: webinar curated/advanced-sfp.md (2024-09-24).

Supporting passages:
- [sources/course/curated/advanced-sfp.md:47](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 36.15): "even though this was an otherwise beautiful crossbar short, he doesnt generally trust closing PA due to increased volatility (people having to close positions pre EoD) meaning its not real PA."
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 16.36): "* Close forex positions at end of week"
- [sources/levels-webinar-part1.md:320](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 13.25): "[43:57] Overlay this volatility uh this volatility line here shows you whether the Market's volatile"

## sfp.ramp-into-sweep

A short is not ideal if price ramps up bullishly into the sweep. Better if the SFP candle breaks back under prior support, for example as a bearish engulfing.

*preference, observed, not coded.* Sources: webinar curated/advanced-sfp.md (2025-01-21).

Supporting passages:
- [sources/course/curated/advanced-sfp.md:105](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 48.04): "**21/01/2025 17:43** if price ramps up really bullishly into a sweep. its not an ideal short."
- [sources/course/curated/advanced-sfp.md:1](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 21.26): "as previously mentioned, if an SFP bar closes very strongly against you like in the upper example above (bullish SFP, bearish engulfing + closes very close to the level being swept), the correct way to trade this is to wait for further PA t"
- [sources/course/curated/trade-management.md:104](../sources/course/curated/trade-management.md) (trade-management, score 21.21): "after price breaks down this level - he trails his TP above the thrust candle and gets trailed out in minor profit, only for price to shoot to his TP. [image]"

## sfp.opposite-sfp-closes-trades

An H1 SFP against an open trade is a reason to close it, but give price an hour or two to show its hand.

*hard, stated, not coded.* Sources: webinar user-pasted SFP notes (2024-09-24).

Supporting passages:
- [sources/course/curated/advanced-sfp.md:1](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 25.61): "as previously mentioned, if an SFP bar closes very strongly against you like in the upper example above (bullish SFP, bearish engulfing + closes very close to the level being swept), the correct way to trade this is to wait for further PA t"
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 25.1): "* H1 SFP in opposite direction can be reason to close existing trades (but gives price 1-2 hours to “show its hand”)"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 17.83): "* Close FTSE/BUND trades at end of day"

## sfp.enter-at-the-swept-level

Newer practice: enter at the swept level itself or the nearest naked level, not blindly at the SFP close. "Stopped out right on the low that was SFP'd, which is where you should have been looking to enter."

*hard, observed, not coded.* Sources: tweet curated tweets (2026-07-01); tweet curated tweets (2022-10-21).

Supporting passages:
- [sources/course/curated/advanced-sfp.md:61](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 23.48): "**24/09/2024 18:50** on the return to this 1 touch support level (where you would normally be looking for an SFP to enter) - the trader said he would not buy here, even though the SFP does come. the reason why he would not buy this SFP into"
- [sources/course/tweets/tweets_clean.md:50](../sources/course/tweets/tweets_clean.md) (2026, score 18.93): "H1: Entry via limit offer after close and acceptance below prior support (nearest naked level to the SFP low)"
- [sources/course/tweets/tweets_clean.md:7](../sources/course/tweets/tweets_clean.md) (2022, score 18.07): "Not exactly. At a level like the July '90 high, I would look for an SFP because I don't blindly sell prior highs, I wait to see the price action. If that level had been prior support and it was naked (not retested on the H1 post break), I w"

## sfp.average-rr

Average RR on SFP trades is about 1.2 with a high hit rate; he takes them from 0.8R and sometimes 0.5R when probability is high.

*heuristic, stated, not coded.* Sources: webinar user-pasted SFP notes (2024-09-24). Note: Hit rate (~90%) is a self-reported figure, not tested here.

Supporting passages:
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 39.44): "* Takes trades with as low as 0.8R, sometimes even 0.5R in high probability setups"
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 10.29): "| 1.5R is the right target | Direction, Trade by Trade | Reversed. 1.5R was optimal against a 77% win rate that does not exist. At the true 29% the system needs 3–5R, and at 1.0R it loses money outright. |"

## sfp.not-at-inverse-levels

An SFP is not relevant at a level that is the inverse last: an SFP presumes the level was support (resistance) last, and that is the other kind of trade. If you get an SFP at a level, you are in the same-direction case.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @56:53.

Supporting passages:
- [sources/levels-webinar-part1.md:416](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 28.96): "[57:09] presumes that the level with support last if you had an SFP down here then the the touch below the level would have"
- [sources/levels-webinar-part1.md:61](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 21.76): "[8:39] well if you've got a level that was support last why not buy it well what I found is that levels that are"
- [sources/course/curated/tp-selection.md:1](../sources/course/curated/tp-selection.md) (tp-selection, score 20.56): "target was not the equal highs just below the text; it was an inverse-last level, and not even a particularly obvious one."

## Conflicts

- `sfp.enter-at-close` vs `sfp.enter-at-the-swept-level`: The 2024 course teaches selling at the SFP candle close. The 2022 and 2026 posts prefer entering at the swept level / nearest naked level. Both are kept; the entry node reports whichever the setup style asks for.
