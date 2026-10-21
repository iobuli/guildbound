# Direction / Objection / Location / Execution (`framework`)

The structure the trader's own journal uses for every trade. The graph's stages follow it: D (bias), O (what is in the way), L (the level), E (the order).

Decides at **prior_bars**. Inputs: none. Outputs: none.

## framework.doel

Newer journal entries are written as Direction (the bias and why), Objection (what stands in the way, such as a target inside a weekly level), Location (the level the trade is built on) and Execution (how the order is placed, usually the H1 LXPB).

*hard, observed, not coded.* Sources: journal curated/lxpb-trades-engulfing.md (2026-05-03); journal curated/lxpb-trades-pattern-failure.md (2026-06-24).

Supporting passages:
- [sources/course/general/general_full.md:14685](../sources/course/general/general_full.md) (take notes, score 36.18): "**29/04/2026 15:00** DOLEDirection:down, weekly inside high taken. perfect equal lows on the hourly.Objection:potential inside day trap (took out the low, could possibly pump back up and close as bullish IDF).given we have bearish inside fa"
- [sources/course/general/general_keywords.md:933](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 35.56): "- 29/04/2026 | CHFJPY Direction:W1 bearish pinbar x SFPW1 Low of HammerD1 S/RObjection:the weekly technically hasnt broken under the bearish hammer, as it hasnt yet closed.Location:D1 LXPBExecution:H1 LXPB with TP @FTA [image]"
- [sources/course/curated/lxpb-trades-return-to-hammer.md:1](../sources/course/curated/lxpb-trades-return-to-hammer.md) (return-to-hammer, score 20.36): "note: the upper hourly LXPB (perfectly confluent with the weekly hammer) was not used in this case, because the hammer just stood out more. its also worth noting that bidding this upper level would not have provided sufficient RR.surprising"

## framework.stacked-lxpb

The passed-ATR trades stack LXPBs across weekly, daily and hourly together with "past ATR". The more timeframes share the same level, the stronger the setup.

*heuristic, observed, not coded.* Sources: journal curated/lxpb-trades-passed-atr.md (2024-12-16); journal curated/lxpb-trades-passed-atr.md (2024-12-21).

Supporting passages:
- [sources/course/curated/lxpb-trades-passed-atr.md:1](../sources/course/curated/lxpb-trades-passed-atr.md) (passed-atr, score 26.21): "note: the daily level is completely irrelevant to this setup, as the hourly LXPB is just too far away.its also worth noting that the TP is below the daily level, just like in this tweet right here, where entry is the LXPB, and youre getting"
- [sources/course/general/general_full.md:14584](../sources/course/general/general_full.md) (take notes, score 21.31): "**19/03/2026 20:09** daily level way past ATR, super clean hourly break. obvious thrust candle, obvious LXPB. no brainer [image]"
- [sources/course/general/general_keywords.md:898](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 20.15): "- 19/03/2026 | daily level way past ATR, super clean hourly break. obvious thrust candle, obvious LXPB. no brainer [image]"

## framework.follow-your-rules

Rules to live by: 1. always follow your rules; 2. always follow your rules; 3. always follow your rules; 4. know when to break the rules.

*heuristic, stated, not coded.* Sources: webinar user-pasted notes (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:303](../sources/user-research-pages.md) (Rebuild it yourself, score 21.56): "1. Always follow your rules"
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 21.51): "#1 always follow your rules."
- [sources/levels-webinar-part1.md:149](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 13.32): "[19:25] gold okay so you can follow this uh follow this trade through live and see how it P out SS L it won't work now but"

## framework.eighty-twenty

About 20% of trades have a daily bias (use D1 for target, H1 for entry and stop); 80% go straight to H1 for target, entry and stop.

*hard, stated, not coded.* Sources: webinar user-pasted notes (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 27.52): "Without Daily Bias (80% of trades):"
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 22.33): "he DOESNT like to be filled on his trades going into news. this is only applicable if price is within ~20 pips (EU as the example) of your entry. he says if you are further than that away when news comes out, then you can actually potential"
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 21.96): ""if youre trading a H1 level, at 8pm and you see a good level to go short... but its quite evident that when you flip to the daily timeframe, the market is very likely to close as a bullish engulfing - youre probably not going to take that "

## framework.order-of-operations

The order the method is applied in: 1. mark levels (W1/D1/H1). 2. Which side of the market is each level on (buy below, sell above). 3. Is it an inverse-last level that price respected last time. 4. Count swing touches. 5. Tier: 3+ blind, 2 needs one confluence, 1 needs two (bias, fib, ATR). 6. Space and momentum, not the immediate retest. 7. Limit order, stop where you would still be bullish, target at the first trouble area. 8. Pull the order if missed and price ran more than half way to target.

*hard, inferred, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @4:43; transcript levels webinar part 1, user-pasted transcript @7:47; transcript levels webinar part 1, user-pasted transcript @23:27; transcript levels webinar part 1, user-pasted transcript @25:30; transcript levels webinar part 1, user-pasted transcript @53:21; transcript levels webinar part 1, user-pasted transcript @1:00:04. Note: Assembled from several places in the transcript; the trader gives the tiers as a summary twice but never lists the whole sequence in one go.

Supporting passages:
- [sources/levels-webinar-part1.md:552](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 49.91): "[1:15:16] three touches plus the inverse then I will trade it blind what blind means is just bang in a limit order okay I'm just"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 48.85): "* Fibonacci levels must be within 10% of day’s ATR to count as confluence"
- [sources/levels-webinar-part1.md:416](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 42.49): "[55:32] how basically we trade levels when we're going the same way as the participants last time these These are the easiest trades you can do when the market is"

## framework.part-two-is-missing

This webinar covers only levels where price is going the OPPOSITE way to the last participants (the inverse). A second webinar covers levels where you go the SAME way (buying support, which includes the SFP) and gives a step-by-step "market blueprint" like a flow chart. That second part is not in the sources provided.

*hard, observed, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @1:22:08; transcript levels webinar part 1, user-pasted transcript @1:22:50; transcript levels webinar part 1, user-pasted transcript @2:11.

Supporting passages:
- [sources/levels-webinar-part1.md:416](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 38.09): "[55:32] how basically we trade levels when we're going the same way as the participants last time these These are the easiest trades you can do when the market is"
- [sources/levels-webinar-part1.md:602](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 37.06): "[1:22:24] going to be a literal plan step by step of what you do at every single level on the chart you know where you know what"
- [sources/levels-webinar-part1.md:105](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 33.42): "[14:07] concentrate on tonight is trading levels where we where participants were last doing the opposite to what we're about"

## framework.limit-orders

Nearly 99% of his trades are limit orders, so he does not need to sit at the screen; a "blind" trade just means a resting limit order with a stop and a target.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @23:44; transcript levels webinar part 1, user-pasted transcript @1:15:25.

Supporting passages:
- [sources/levels-webinar-part1.md:552](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 33.02): "[1:15:33] orders principally because I don't need to sit in front of the PC with a limit order if I know the level's good I don't need to hit it at market I'll just trade"
- [sources/levels-webinar-part1.md:76](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 16.94): "[10:04] be a big breakout and it's going to it's going to collapse so what they will do is they will put a stop sell order into the market below these"
- [sources/levels-webinar-part1.md:181](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 13.95): "[23:44] I will trade it blind now what I mean by blind is I will literally just leave an order in at that level with a stop and a"
