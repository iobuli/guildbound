# Trade management (`management`)

Once a position is open, tests the exit and trailing rules against the bars since the fill: evolving R, broken-entry retests, closes beyond the first test, thrust-candle trails, and whether adding is allowed.

Decides at **post_fill**, step 10 of the order of operations: Management: exits, trails, adds. Inputs: stop, target, context. Outputs: mgmt.

## management.evolving-r

RR evolves through a trade. A 1R trade that is 0.5R half way through is still held; only when it gets "ridiculously low" (under 0.5R) does he consider closing. "Don't be a prick for a tick."

*hard, stated, tested by code.* Sources: webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/course/curated/trade-management.md:61](../sources/course/curated/trade-management.md) (trade-management, score 40.85): "its also important to note that he gives an example where he says RR evolves throughout a trade, and a trade that starts out as 1RR and becomes a 0.5RR trade halfway through, he would still hold that. its only when it gets ridiculously low "
- [sources/course/general/general_keywords.md:898](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 15.14): "- 28/03/2025 | just absolutely chadded through the swing point, and theres no way we're closing as a sweep - then i can just"
- [sources/course/curated/notes-images.md:114](../sources/course/curated/notes-images.md) (notes-images, score 14.81): "**28/10/2024 17:58** in this case, when price broke under the red line (where you would have your limit) - this is not actually considered price thrusting through the level - as 'the level' included the entire blue area."

## management.broken-entry-retest-exit

If price closes under your entry, retests the entry from below as resistance, and then closes beneath the initial low, exit the trade.

*hard, stated, tested by code.* Sources: webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/course/curated/trade-management.md:49](../sources/course/curated/trade-management.md) (trade-management, score 27.93): "**24/09/2024 18:51** when price closes under your entry, retests your support entry as resistance, and then closes underneath the initial low - exit the trade. [image]"
- [sources/course/curated/trade-management.md:18](../sources/course/curated/trade-management.md) (trade-management, score 25.17): "**23/09/2024 16:00** he should have closed this trade, due to price breaking under his entry, using it as resistance, and then having a candle close beneath the initial low... but he just got plain lucky. guy was asleep when that candle clo"
- [sources/course/curated/taking-a-trade-3-touch-level.md:1](../sources/course/curated/taking-a-trade-3-touch-level.md) (3-touch-level, score 19.01): "which means that a level that was supporting price, once that level is broken, we expect it to act as the opposite - aka resistance [image]"

## management.close-beyond-first-test

If the trade is a short and an H1 close comes above the highs of the first test of your resistance, you must close it. The same logic applies to a long.

*hard, stated, tested by code.* Sources: webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/course/curated/trade-management.md:32](../sources/course/curated/trade-management.md) (trade-management, score 26.86): "you remain in the trade, but if price makes a H1 close above the previous hourly highs (horizontal white line) from the first test of your resistance, then you must close the trade. [image]"
- [sources/course/curated/trade-management.md:104](../sources/course/curated/trade-management.md) (trade-management, score 18.09): "**25/09/2024 19:32** when this long trade off the yellow line (middle of the screen) comes back for its 2nd touch, and then fails to take out the prior high created from the reaction from the 1st touch - he gets sussed out and puts his SL s"
- [sources/course/general/general_full.md:14352](../sources/course/general/general_full.md) (take notes, score 16.28): "so even though im not looking at those insignificant green highs to get swept... i am focusing on if theyre making the resistance weak"

## management.trail-beyond-thrust

When price breaks a level in your favour, trail the stop beyond the thrust candle that caused the break. These stops still have to make sense: no trailing directly into a level. If it fails, go back another candle.

*heuristic, stated, tested by code.* Sources: webinar curated/trade-management.md (2024-09-24); webinar curated/trade-management.md (2024-11-05).

Supporting passages:
- [sources/course/curated/trade-management.md:61](../sources/course/curated/trade-management.md) (trade-management, score 57.47): "he draws out all of the potential hourly trouble areas on the way to his TP. then as price breaks through a level, he will trail his SL to above the thrust candle that caused the break."
- [sources/course/curated/trade-management.md:104](../sources/course/curated/trade-management.md) (trade-management, score 25.24): "after price breaks down this level - he trails his TP above the thrust candle and gets trailed out in minor profit, only for price to shoot to his TP. [image]"
- [sources/course/curated/trade-management.md:18](../sources/course/curated/trade-management.md) (trade-management, score 24.72): "i rarely see him trail his stop to the thrust candle that caused the initial retest - ive seen him trailing it to the 2nd retest a few times though, right at that "squeeze point"."

## management.squeeze-point

After the second return to the entry level and price curving away, he trails beyond the "squeeze point": the high (low) price would reach if it reacted off the level again. A third comeback is unlikely to go well.

*hard, observed, not coded.* Sources: webinar curated/trade-management.md (2024-09-23); webinar curated/trade-management.md (2024-09-25).

Supporting passages:
- [sources/course/curated/trade-management.md:1](../sources/course/curated/trade-management.md) (trade-management, score 31.77): "imagining that this ^ opportunity to trail above the engulfing didnt set up - when price got back below his S/R entry, a good place to trail to would be above the 'squeeze point' (you know when you extend your SL just in case price squeezes"
- [sources/course/curated/trade-management.md:79](../sources/course/curated/trade-management.md) (trade-management, score 26.7): "the reason why he said he is okay with putting it above the thrust candle, is because the resistance level is so perfectly defined. every high/low is in the exact same spot - there really shouldnt be any wiggle room for price to squeeze hig"
- [sources/course/curated/trade-management.md:104](../sources/course/curated/trade-management.md) (trade-management, score 19.09): "after price breaks down this level - he trails his TP above the thrust candle and gets trailed out in minor profit, only for price to shoot to his TP. [image]"

## management.is-this-trade-done

"Is this trade done?" If price came very close to target and then fell back to the entry, it is over: take what is there. If TP is hit without a fill, close the order at once.

*hard, stated, not coded.* Sources: webinar curated/trade-management.md (2024-09-23); webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/course/curated/trade-management.md:1](../sources/course/curated/trade-management.md) (trade-management, score 21.34): "he was asleep when the trade filled and ran all the way up close to his TP. and when he woke up, price was back down at his entry juuust about making candle closes below his support entry."
- [sources/course/general/general_keywords.md:296](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 17.79): "- 18/04/2023 | hits entry, no fill because of spread, straight to TP [image]"
- [sources/course/curated/trade-management.md:49](../sources/course/curated/trade-management.md) (trade-management, score 17.57): "**24/09/2024 17:21** example of him hitting TP, but not getting a fill. and he said he just closed the trade there and then. why hold it? why risk it? the trade is over, take a small L to take a big W."

## management.rejection-at-fta-exit

Exit on a rejection candle at the first trouble area. Take the profits you have and do not hang around hoping the market comes back to your levels.

*hard, observed, not coded.* Sources: webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/course/curated/trade-management.md:93](../sources/course/curated/trade-management.md) (trade-management, score 31.43): "take the profits youve got, dont be mad that you couldve had more, and certainly dont hang around just hoping the market comes back to your levels. the trade is done when the trade is done, it doesnt matter if you played it correctly. [imag"
- [sources/course/curated/taking-a-trade-targets.md:1](../sources/course/curated/taking-a-trade-targets.md) (targets, score 23.92): "**22/10/2024 16:18** to choose a Target Price (TP) on a trade, we ask ourselves which is the first area on the chart that might cause some trouble"
- [sources/course/curated/trade-management.md:141](../sources/course/curated/trade-management.md) (trade-management, score 18.44): "**29/10/2024 18:27** while in this short from a 3 tap level (blue line), you would get out at the bullish sweep - considering we know the SR of these to be high... why hang around in a short, when we are 80% sure price is going to pump? [im"

## management.trapped-exit-at-level

If you are trapped in a trade that clearly will not reach target, get out at the level that is trapping you, with a limit for as small a loss as possible.

*hard, stated, not coded.* Sources: webinar curated/trade-management.md (2024-11-04).

Supporting passages:
- [sources/course/curated/trade-management.md:141](../sources/course/curated/trade-management.md) (trade-management, score 36.97): "long off a weekly / daily level.possible inside week failure (took out the high, low is vulnerable).price dropped below both levels and printed a rejection wick off the daily level.clealry massively dumped into drawdown.when youre trapped i"
- [sources/user-research-pages.md:50](../sources/user-research-pages.md) (Experiment two — the level-held rule, score 16.18): "Here there is a third option, and it matters. You cannot **filter** on the close, but you can **exit** on it. Take every retest fill; if the bar ends up closing on the wrong side, flatten at that close and wear the small loss. That is fully"
- [sources/course/curated/taking-a-trade-targets.md:29](../sources/course/curated/taking-a-trade-targets.md) (targets, score 14.14): "its entirely likely that price will reach that dotted level - and reject entirely. leaving us wishing that we had just taken our easy money and not been greedy, trying to hold on for as much extra money as possible."

## management.opposite-sweep-exit

Get out at a bullish sweep when you are short from a three-tap level and the S/R of the sweep is high: why stay when you are about 80% sure price will pump.

*hard, observed, not coded.* Sources: webinar curated/trade-management.md (2024-10-29).

Supporting passages:
- [sources/course/curated/trade-management.md:141](../sources/course/curated/trade-management.md) (trade-management, score 32.44): "**29/10/2024 18:27** while in this short from a 3 tap level (blue line), you would get out at the bullish sweep - considering we know the SR of these to be high... why hang around in a short, when we are 80% sure price is going to pump? [im"
- [sources/course/curated/taking-a-trade-3-touch-level.md:1](../sources/course/curated/taking-a-trade-3-touch-level.md) (3-touch-level, score 18.09): "on a triple tap level, all 3 confluences are price touching your level: [image]"
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 16.49): "* For “advanced technique” entries (waiting for price to probe further beyond the level), use the sweep extreme as SL instead of ATR"

## management.flatten-by-close

Forex positions are closed at the end of the week; FTSE and Bund trades at the end of the day, partly to avoid being gapped out.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24); webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 32.67): "* Close FTSE/BUND trades at end of day"
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 27.01): "for forex trades, close any positions at the end of the week."
- [sources/course/curated/advanced-sfp.md:47](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 16.31): "(FTSE) the trader does not care too much about SFPs on the close of market [image]"

## management.add-when-confirming

Add to a winner only when the market is confirming, the addition fits the plan, the upside is still asymmetric and risk stays controlled.

*hard, stated, tested by code.* Sources: tweet curated tweets (2026-06-03); tweet curated tweets (2026-05-01). Note: Coded as: at least 1R in profit and evolving R still at least 1.

Supporting passages:
- [sources/course/tweets/tweets_clean.md:50](../sources/course/tweets/tweets_clean.md) (2026, score 43.23): "You should try to add to a trade that is confirming your thesis, provided the addition fits your plan, the upside is still asymmetric, and risk remains controlled."
- [sources/course/tweets/tweets_clean.md:77](../sources/course/tweets/tweets_clean.md) (2026, score 13.9): "Great work. That was a very clear area of engagement on the HTF which then led to a solid bias lower today. Well done on the addition. Love to see people add to trades when they have the market by the balls."

## management.macd-divergence-exit

He has closed a trade on MACD divergence above a key level.

*hard, observed, not coded.* Sources: webinar curated/trade-management.md (2024-09-23). Note: MACD is not computed in this build.

Supporting passages:
- [sources/course/curated/trade-management.md:18](../sources/course/curated/trade-management.md) (trade-management, score 19.33): "**23/09/2024 15:53** he closed this trade due to MACD divergence above a very key level. i'd like to imagine he would have held it if it wasnt a crosspair, but i dont think thats the case. [image]"
- [sources/course/curated/advanced-sfp.md:91](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 13.59): "**24/09/2024 19:23** this is the daily chart of a support level price was back at for the 3rd time, with an SFP + MACD divergence - signalling longs."
- [sources/course/curated/notes-images.md:79](../sources/course/curated/notes-images.md) (notes-images, score 12.06): "**23/09/2024 15:48** if this was a X:USD pair, he would have had his stop in the normal place, above the thrust candle. however with this being a crosspair, its more likely to squeeze, so his stop was all the way up at the yellow arrow, abo"

## management.hold-through-news

Hold open trades through news; do not cut or lighten.

*hard, stated, not coded.* Sources: webinar user-pasted notes (2024-10-03).

Supporting passages:
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 21.36): ""I am in in a trade, I will hold through news. I don't lighten up, I don't cut, I keep my stop where it is based on technical. and I will hold through news.""
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 11.83): "* If already in a trade during news, hold through it"
