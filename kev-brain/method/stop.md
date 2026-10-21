# Stop placement (`stop`)

Suggests where the stop belongs (beyond the thrust candle, never into a level, wider on crosses) and audits a stop you propose against the same rules.

Decides at **prior_bars**, step 6 of the order of operations: Stop: beyond the thrust candle, where you would still be bullish. Inputs: level, atr, swings, sfp, tuesday. Outputs: stop.

## stop.beyond-thrust

On an S/R trade the stop goes beyond the thrust candle (the candle that broke the level).

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-30); webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/course/curated/trade-management.md:104](../sources/course/curated/trade-management.md) (trade-management, score 14.17): "his bid on this trade was the LHPB (lighter blue level), and on the candle (big bullish engulfing) that broke above the obvious level (darker blue level), his SL was trailed up below the thrust candle. (edited) [image]"
- [sources/course/curated/trade-management.md:61](../sources/course/curated/trade-management.md) (trade-management, score 12.63): "draw above the candle prior to the thrust candle, and check if the level is good for a SL or not. if not, go another candle back and repeat until you find a good stop placement. [image]"
- [sources/course/curated/notes-images.md:79](../sources/course/curated/notes-images.md) (notes-images, score 12.27): "**23/09/2024 15:48** if this was a X:USD pair, he would have had his stop in the normal place, above the thrust candle. however with this being a crosspair, its more likely to squeeze, so his stop was all the way up at the yellow arrow, abo"

## stop.never-into-a-level

The stop may not sit directly in front of ATR, an S/R level or the next key fib, where price could squeeze to it and reverse. Move it to a candle further left (beyond the level).

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-30); webinar curated/notes-images.md (2024-09-23); journal curated/lxpb-trades-passed-atr.md (2024-12-29). Note: Read here as: if a swing of the right side lies within 0.3 H1 ATR beyond the stop, place the stop just beyond that swing.

Supporting passages:
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 26.83): "it is also not allowed to stop you out into the next key fib level. so SL must be moved to the left again in this scenario too."
- [sources/course/curated/trade-management.md:61](../sources/course/curated/trade-management.md) (trade-management, score 23.97): "however in the image below, you would need to put your stop a few candles to the left of the thrust-bar. as there is a decent S/R level above it that price could easily squeeze up to, stop you out, then reverse to TP off. [image]"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 23.76): "* If price pulls back beyond 61.8% fib level, remove the fib (no longer valid confluence)"

## stop.daily-long-hourly-under

Long off a daily level with an hourly level just underneath: the stop goes under the hourly level, so it is not hit while price finds support there.

*hard, stated, not coded.* Sources: webinar user-pasted notes (2024-09-25).

Supporting passages:
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 19.87): "stoploss must go underneath the hourly level. because while the daily level is more likely to catch a bid, price could easily go a little further into the hourly. and I don't want it to stop me out in the process of finding support."
- [sources/course/general/general_keywords.md:898](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 18.24): "- 12/03/2026 | inside day failure off a daily support level (slightly swerved, but close enough)."
- [sources/course/general/general_full.md:14584](../sources/course/general/general_full.md) (take notes, score 17.97): "**12/03/2026 21:48** inside day failure off a daily support level (slightly swerved, but close enough). [image]"

## stop.second-level-squeeze

He likes a second, minor level between entry and stop as a second layer of protection if price squeezes.

*preference, observed, tested by code.* Sources: webinar curated/notes-images.md (2024-09-10).

Supporting passages:
- [sources/course/curated/notes-images.md:33](../sources/course/curated/notes-images.md) (notes-images, score 32.28): "he likes to have a second, minor level between his entry and his SL (if possible)"
- [sources/course/curated/trade-management.md:1](../sources/course/curated/trade-management.md) (trade-management, score 16.72): "imagining that this ^ opportunity to trail above the engulfing didnt set up - when price got back below his S/R entry, a good place to trail to would be above the 'squeeze point' (you know when you extend your SL just in case price squeezes"
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 11.62): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"

## stop.crosspairs-wider

On cross pairs the stop goes further out (above the S/R level rather than just above the thrust candle), because they squeeze more.

*hard, observed, tested by code.* Sources: webinar curated/notes-images.md (2024-09-23). Note: The 0.25 ATR extra is the builder's number.

Supporting passages:
- [sources/course/curated/notes-images.md:79](../sources/course/curated/notes-images.md) (notes-images, score 22.87): "**23/09/2024 15:48** if this was a X:USD pair, he would have had his stop in the normal place, above the thrust candle. however with this being a crosspair, its more likely to squeeze, so his stop was all the way up at the yellow arrow, abo"
- [sources/course/curated/trade-management.md:79](../sources/course/curated/trade-management.md) (trade-management, score 18.41): "the reason why he said he is okay with putting it above the thrust candle, is because the resistance level is so perfectly defined. every high/low is in the exact same spot - there really shouldnt be any wiggle room for price to squeeze hig"
- [sources/course/curated/trade-management.md:61](../sources/course/curated/trade-management.md) (trade-management, score 17.34): "draw above the candle prior to the thrust candle, and check if the level is good for a SL or not. if not, go another candle back and repeat until you find a good stop placement. [image]"

## stop.zone-needs-whole-zone

If there is no clear thrust candle through the whole level zone, move the stop one candle to the left to be safe.

*hard, observed, not coded.* Sources: webinar curated/notes-images.md (2024-10-28).

Supporting passages:
- [sources/course/curated/notes-images.md:114](../sources/course/curated/notes-images.md) (notes-images, score 22.86): "in this case, because there was no clear thrust candle (through the whole zone), he will typically move his SL a candle to the left (orange arrow) to make it safer. [image]"
- [sources/course/curated/trade-management.md:61](../sources/course/curated/trade-management.md) (trade-management, score 17.25): "he draws out all of the potential hourly trouble areas on the way to his TP. then as price breaks through a level, he will trail his SL to above the thrust candle that caused the break."
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 15.8): "stop cannot go below the thrust candle if you are being stopped out into ATR. this is just like being stopped out into support. you must move the SL to a candle on the left."

## stop.protected-by-htf

He likes the stop to be protected by higher-timeframe levels. In one trade the stop could not go above the daily level and was protected only by the weekly one.

*preference, observed, not coded.* Sources: journal curated/lxpb-trades-passed-atr.md (2025-04-22); journal curated/lxpb-trades-passed-atr.md (2025-11-13).

Supporting passages:
- [sources/course/curated/lxpb-trades-passed-atr.md:53](../sources/course/curated/lxpb-trades-passed-atr.md) (passed-atr, score 25.97): "note: sl could not be above the daily LXPB for this trade. it was only protected by the W1 level [image]"
- [sources/course/general/general_full.md:14629](../sources/course/general/general_full.md) (take notes, score 20.66): "so instead, as usual, i took the stoploss back to the previous thrust candle. and this is well out of the way, even protected by both HTF (D1 + W1) levels. theres no way im getting 'accidentally stopped out' on this [image]"
- [sources/course/general/general_keywords.md:898](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 18.04): "- 27/03/2026 | way, even protected by both HTF (D1 + W1) levels. theres no way im getting"

## stop.risky-stop-is-fine-if-idea-is-wrong

A stop over double lows is "risky", but if price gets there the idea is wrong anyway.

*hard, observed, not coded.* Sources: journal curated/lxpb-trades-htf-level.md (2026-02-19).

Supporting passages:
- [sources/course/curated/lxpb-trades-htf-level.md:1](../sources/course/curated/lxpb-trades-htf-level.md) (htf-level, score 24.81): "note: the lowest H1 LXPB is being used in this case, due to the trader saying price often squeezes the HTF levels and goes to the H1 levels anyway. no good stop here, as the thrust is too tight. so he just has his stop at the range lows. he"
- [sources/course/curated/advanced-sfp.md:91](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 11.19): "#1 he gets in with a buy stop above the market price - he has his buy stop here drawn on the upper wick of the sweep candle, essentially like how people trade pinbars.#2 he will put a buy stop at the original entry of the original (very bea"
- [sources/course/curated/lxpb-trades-pattern-failure.md:1](../sources/course/curated/lxpb-trades-pattern-failure.md) (pattern-failure, score 11.02): "note: he said that if price had not made a double bottom here, and instead created a LL, its entirely likely he would not have taken this trade. [image]"

## stop.sfp-atr

SFP trades: stop at the sweep extreme plus the 24-period ATR. He also uses a tighter "gap" technique when it still clears the old high.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24); webinar curated/advanced-sfp.md (2024-09-24). Note: The "gap SL technique" is mentioned but never described.

Supporting passages:
- [sources/course/curated/advanced-sfp.md:77](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 34.63): "but not because it needed to be above it - he was just using his gap SL technique (which was a tighter stop than his h1 ATR tech), and felt comfortable having it there even though it was tighter, because it was still above that high."
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 28.06): "* H1 SFP trades use ATR-based stop loss"
- [sources/course/curated/advanced-sfp.md:1](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 17.63): "so what you do is draw in your theoretical ATR based stop in. and if price (on the next bar) reaches ~50% towards this level - put a sell stop in at the original SFP close price. [image]"

## stop.just-beyond-not-one-tick

For failed-breakdown trades on the M5 Bund: the stop goes just under the lows, "not 1 tick under, but just under somewhere"; no ATR or thrust candle needed.

*hard, stated, not coded.* Sources: webinar curated/notes-images.md (2024-09-25).

Supporting passages:
- [sources/course/curated/notes-images.md:79](../sources/course/curated/notes-images.md) (notes-images, score 42.74): "in this case, the market has made its 3 lows. and after the final breakdown, a single green candle is printed. you market buy here with a stop just under the lows (not 1 tick under, but just under somewhere. no ATR or thrust candle necessar"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 22.08): "* Stop loss can’t go below thrust candle if being stopped out into ATR"
- [sources/course/curated/lxpb-trades-htf-level.md:1](../sources/course/curated/lxpb-trades-htf-level.md) (htf-level, score 21.14): "note: the lowest H1 LXPB is being used in this case, due to the trader saying price often squeezes the HTF levels and goes to the H1 levels anyway. no good stop here, as the thrust is too tight. so he just has his stop at the range lows. he"

## stop.min-distance

A minimum stop of 0.05% of price is used in the backtest.

*hard, backtest-reported, tested by code.* Sources: backtest user-supplied twelve-bar page.

Supporting passages:
- [sources/user-research-pages.md:179](../sources/user-research-pages.md) (Does it survive out of sample, score 13.71): "**What changed.** One parameter: the minimum number of bars between a break and a qualifying retest. Everything else is untouched — 3-touch levels clustered within 0.1× the prior day's ATR, entry at the level price, stop at the break candle"

## stop.grace-two-bars

A 2-bar grace window at a 2x stop is now worth only about 0.05 Sharpe at a 4R target: keep it, do not lean on it.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 40.88): "| The 2-bar grace window at a 2× stop | Dante Trade Inspector | Stands, but is now nearly irrelevant: at a 4R target it is worth about 0.05 of Sharpe. Keep it; do not lean on it. |"
- [sources/user-research-pages.md:127](../sources/user-research-pages.md) (How to not do this a third time, score 21.81): "Dante system · causal build · 4R target, 2-bar grace at a 2× stop, exit at the close on a failed hold · net of 0.10R slippage per trade · 6,438 trades across four instruments, October 2009 to September 2026. Not investment advice; none of t"
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 19.92): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"

## stop.would-i-still-be-bullish

The stop question: am I stopping out into an area where I would STILL be bullish? Resistance flips to support, so look for resistance just beyond the level that price might squeeze to in the worst case, and put the stop just beneath it. Draw the stop and look at what you would be stopped out into.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @1:00:04; transcript levels webinar part 1, user-pasted transcript @1:00:37; transcript levels webinar part 1, user-pasted transcript @1:06:54.

Supporting passages:
- [sources/levels-webinar-part1.md:451](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 57.71): "[1:01:26] level but having a stop there as we said still takes you slightly into what I would call an area where price could squeeze to so I'd want to be a little"
- [sources/levels-webinar-part1.md:481](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 27.61): "[1:06:54] that opened below closed above the level but draw a physical stop there and look what you're stopping out"
- [sources/levels-webinar-part1.md:499](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 24.86): "[1:07:25] that point in time that seems like stop beneath that candle is seems out of the way of the price and you can see that"

## stop.as-tight-as-the-level-allows

Keep the stop as close to market as you can, but no tighter than the squeeze point. There is usually no need for a very wide stop if the level was chosen correctly, and these trades should not show much drawdown.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @1:00:20; transcript levels webinar part 1, user-pasted transcript @1:01:42; transcript levels webinar part 1, user-pasted transcript @1:01:57.

Supporting passages:
- [sources/levels-webinar-part1.md:451](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 38.08): "[1:01:34] bit lower than that but not you know not too low there's usually no need to having really really wide stop these if"
- [sources/levels-webinar-part1.md:402](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 20.26): "[54:11] point and if price rallies away from a swing point and comes back to it what's usually G to happen it's what happens"
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 18.3): "* H1 SFP in opposite direction can be reason to close existing trades (but gives price 1-2 hours to “show its hand”)"

## stop.candle-proxy

The usual stop is beneath the last candle that OPENED below and CLOSED above the level (the thrust candle). If that puts the stop into an area price could squeeze to, move to a candle further left and check again.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @1:01:00; transcript levels webinar part 1, user-pasted transcript @1:01:18; transcript levels webinar part 1, user-pasted transcript @1:07:09.

Supporting passages:
- [sources/levels-webinar-part1.md:451](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 40.3): "[1:01:10] place it beneath the candle that open below and closed above my most common stop placement is basically below a candle open below"
- [sources/course/curated/trade-management.md:61](../sources/course/curated/trade-management.md) (trade-management, score 27.93): "however in the image below, you would need to put your stop a few candles to the left of the thrust-bar. as there is a decent S/R level above it that price could easily squeeze up to, stop you out, then reverse to TP off. [image]"
- [sources/course/curated/trade-management.md:104](../sources/course/curated/trade-management.md) (trade-management, score 26.5): "after price breaks down this level - he trails his TP above the thrust candle and gets trailed out in minor profit, only for price to shoot to his TP. [image]"

## stop.cross-definition

A "cross" is any pair without the USD in it. He uses a wide stop on crosses because they are not clean, and tells people to be careful with them.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @1:06:30; transcript levels webinar part 1, user-pasted transcript @1:07:39; transcript levels webinar part 1, user-pasted transcript @1:08:40.

Supporting passages:
- [sources/levels-webinar-part1.md:499](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 31.97): "[1:07:39] survived on it but again the crosses are something I always tell people to be careful of I'm using them just as an"
- [sources/levels-webinar-part1.md:481](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 16.82): "[1:06:30] the value now this wasn't clean a lot of the crosses aren't clean um I actually made money on this just because I use quite a wide stop on the crosses because"
- [sources/course/curated/advanced-sfp.md:47](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 14.51): "the trader uses a D1 bias over his H1 bias basically only when you cant see the H1 lows without scrolling back massively."
