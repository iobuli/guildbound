# Targets and first trouble area (`target`)

Finds where the trade is likely to meet trouble (the FTA, the most recent respected level, the higher-timeframe target, or the other side of a range) and checks the reward-to-risk.

Decides at **prior_bars**, step 7 of the order of operations: Target: the first trouble area. Inputs: stop, level, swings, bias, atr, tuesday. Outputs: target.

## target.fta

Take profit at the First Trouble Area: the first place on the chart where price might meet trouble. Closing there is more consistent than trying to run every trade. In the 2012 webinar the example target was "the next trouble area, about a 30 pip move".

*hard, stated, tested by code.* Sources: lesson curated/taking-a-trade-targets.md (2024-10-22); transcript levels webinar part 1, user-pasted transcript @53:38.

Supporting passages:
- [sources/course/curated/taking-a-trade-targets.md:1](../sources/course/curated/taking-a-trade-targets.md) (targets, score 41.76): "**22/10/2024 16:18** to choose a Target Price (TP) on a trade, we ask ourselves which is the first area on the chart that might cause some trouble"
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 25.95): "* TP at FTA (First Technical Area) if no higher timeframe bias"
- [sources/course/curated/notes-images.md:114](../sources/course/curated/notes-images.md) (notes-images, score 24.3): "**28/10/2024 17:58** in this case, when price broke under the red line (where you would have your limit) - this is not actually considered price thrusting through the level - as 'the level' included the entire blue area."

## target.most-recent-over-first

Price tends to gravitate to the most recent level, so the first level in the way is not always the FTA. He often targets recent price action over older, more obvious levels.

*heuristic, observed, tested by code.* Sources: lesson curated/taking-a-trade-targets.md (2024-10-22); webinar curated/tp-selection.md (2024-09-24); journal curated/lxpb-trades-passed-atr.md (2025-04-03).

Supporting passages:
- [sources/course/curated/taking-a-trade-targets.md:1](../sources/course/curated/taking-a-trade-targets.md) (targets, score 37.05): "in general, price tends to gravitate to the most recent level. so even though price will run into the upper support level first - it is still a safe bet to target the lower (more recent) one. [image]"
- [sources/course/curated/tp-selection.md:56](../sources/course/curated/tp-selection.md) (tp-selection, score 26.8): ""price often tends to gravitate to the last area before the breakout" [image]"
- [sources/course/curated/tp-selection.md:73](../sources/course/curated/tp-selection.md) (tp-selection, score 25.28): "the first level in the way therefore may not always be FTA, as it may not cause any trouble (given the likelihood of price RTOing the more recent PA)"

## target.ignore-tiny-swings

A tiny one-touch swing high is not the FTA. If it were larger or had more taps it would be.

*hard, observed, tested by code.* Sources: webinar curated/tp-selection.md (2024-09-24). Note: Swings under 0.5 H1 ATR deep are ignored here; the source gives no number.

Supporting passages:
- [sources/course/curated/tp-selection.md:39](../sources/course/curated/tp-selection.md) (tp-selection, score 25.41): "its worthwhile to note that the last swing high was not FTA, presumably because it was just a tiny 1 touch high. if it were larger or had more taps, i imagine this would have been the FTA level instead."
- [sources/course/curated/tp-selection.md:73](../sources/course/curated/tp-selection.md) (tp-selection, score 13.96): "a rare example of the trader using the tiny 1 touch level as his FTA: [image]"
- [sources/course/curated/taking-a-trade-h1-sweep.md:36](../sources/course/curated/taking-a-trade-h1-sweep.md) (h1-sweep, score 13.48): "therefore, even when this swing low gets swept - we completely ignore it."

## target.htf-bias-htf-target

With a daily bias the target is on the daily too, and an H1 level in the way is not worried about. "D1 for bias, D1 for TP."

*hard, stated, tested by code.* Sources: webinar curated/tp-selection.md (2024-09-24).

Supporting passages:
- [sources/course/curated/lxpb-trades-htf-level.md:1](../sources/course/curated/lxpb-trades-htf-level.md) (htf-level, score 18.8): "note: no HTF target needed, in fact the HTF target (triple top) had already been taken and swept. the bullish D1 engulfing was also not super relevant, due to this daily sweep giving contradicting signals. FTA is used here. [image]"
- [sources/course/general/general_full.md:14629](../sources/course/general/general_full.md) (take notes, score 17.59): "so instead, as usual, i took the stoploss back to the previous thrust candle. and this is well out of the way, even protected by both HTF (D1 + W1) levels. theres no way im getting 'accidentally stopped out' on this [image]"
- [sources/course/general/general_keywords.md:898](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 17.55): "- 19/03/2026 | daily level way past ATR, super clean hourly break. obvious thrust candle, obvious LXPB. no brainer [image]"

## target.no-bias-fta-or-range

With no higher-timeframe bias, take profit at the FTA; with no FTA either, aim for the other side of the range. After a false break of a range, price usually goes to the other side.

*hard, stated, tested by code.* Sources: webinar user-pasted SFP notes (2024-09-24); webinar curated/tp-selection.md (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 28.5): "* TP at FTA (First Technical Area) if no higher timeframe bias"
- [sources/course/curated/tp-selection.md:19](../sources/course/curated/tp-selection.md) (tp-selection, score 22.92): "**24/09/2024 16:30** target for this trade would be the range lows - "if you have a range, and it false breaks. the vast majority of the time, it will go to the other side of the range" [image]"
- [sources/course/curated/notes-images.md:1](../sources/course/curated/notes-images.md) (notes-images, score 20.13): "scenario: crossbar - bullish d1 sweep + pinbar. aiming for the other side of the range."

## target.inverse-last

Target the inverse-last level: where the market last found support (for a short) or resistance (for a long).

*hard, stated, not coded.* Sources: webinar curated/tp-selection.md (2024-09-24).

Supporting passages:
- [sources/levels-webinar-part1.md:61](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 22.74): "[7:53] levels that price last found resistance at if we were going to be below L market and we're rallying upwards we're going"
- [sources/course/curated/tp-selection.md:19](../sources/course/curated/tp-selection.md) (tp-selection, score 21.92): ""cracking level of resistance" on the left, but we are looking for the FLIP. i want to put my asks in where the market last found SUPPORT."
- [sources/levels-webinar-part1.md:149](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 20.34): "[20:46] levels that the inverse last we need to sell a prior support level we need to go the opposite way to trade this before"

## target.sections

With a daily bias he splits the take-profit across the hourly levels in the way (five levels: sell a fifth at each) and re-enters each fifth when the level flips and retests.

*hard, observed, not coded.* Sources: webinar curated/tp-selection.md (2024-10-18).

Supporting passages:
- [sources/levels-webinar-part1.md:16](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 18.12): "[2:44] previous levels you'll note that we draw levels in on the weekly time frame the daily time frame and the hourly time frame and and so what that essentially means is that when you're on the hourly"
- [sources/levels-webinar-part1.md:61](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 16.56): "[7:47] concentrate on levels which I call the inverse I.E when we're buying they were"
- [sources/levels-webinar-part1.md:567](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 16.08): "[1:18:08] usually if we're trading very close to a level just before news um I will take the order out and stick it in as soon as"

## target.minimum-rr

Minimum reward-to-risk is about 0.8R, and sometimes 0.5R in a high-probability setup. If the evolving R gets "ridiculously low" he closes.

*hard, stated, tested by code.* Sources: webinar user-pasted SFP notes (2024-09-24); webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 34.51): "* Takes trades with as low as 0.8R, sometimes even 0.5R in high probability setups"
- [sources/course/curated/trade-management.md:49](../sources/course/curated/trade-management.md) (trade-management, score 14.86): "the trader says that he calculates his evolving R usually on bar closes. in this scenario with a bullish h1 pinbar reacting off his entry, it would be possible for him to move his SL up to the low of the pinbar."
- [sources/course/curated/trade-management.md:61](../sources/course/curated/trade-management.md) (trade-management, score 14.75): "its also important to note that he gives an example where he says RR evolves throughout a trade, and a trade that starts out as 1RR and becomes a 0.5RR trade halfway through, he would still hold that. its only when it gets ridiculously low "

## target.take-the-sure-thing

Take a smaller target when it has confluence on the left and the RR is already good enough: "take the sure bet".

*preference, observed, not coded.* Sources: journal curated/lxpb-trades-passed-atr.md (2025-01-03); webinar curated/tp-selection.md (2024-09-24).

Supporting passages:
- [sources/course/curated/lxpb-trades-passed-atr.md:29](../sources/course/curated/lxpb-trades-passed-atr.md) (passed-atr, score 26.19): "note: daily confluence definitely does not seem to be necessary, so long as the hourly level is clean enough.a small level is chosen as a TP, due to its confluence on the left, and the RR already being good enough. "take the sure bet." [ima"
- [sources/course/curated/tp-selection.md:39](../sources/course/curated/tp-selection.md) (tp-selection, score 15.56): "the trader didnt even see the trade to the gap fill. he got out at the SR flip: "lets take the sure thing.""
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 13.31): ""if youre trading a H1 level, at 8pm and you see a good level to go short... but its quite evident that when you flip to the daily timeframe, the market is very likely to close as a bullish engulfing - youre probably not going to take that "

## target.look-further-left

Look further left than you think before picking an FTA: an older level that is still being respected can be the real one.

*hard, observed, not coded.* Sources: webinar curated/tp-selection.md (2024-09-24).

Supporting passages:
- [sources/course/curated/tp-selection.md:56](../sources/course/curated/tp-selection.md) (tp-selection, score 27.4): "his FTA on this was not (as i thought it was going to be) the recent highs just below. but it was a much older S/R level that was still being respected in the market (lower white dashed line). he also said that despite the pink horizontal l"
- [sources/course/curated/tp-selection.md:19](../sources/course/curated/tp-selection.md) (tp-selection, score 14.78): "entry would be on the break of this support line here (upper red) - despite the fact that its messy on the left, that doesnt really matter, as the recent PA is super clearly showing it was support. and recent PA > older PA. [image]"
- [sources/course/curated/tp-selection.md:92](../sources/course/curated/tp-selection.md) (tp-selection, score 14.55): "**28/09/2024 14:07** on this SFP short (he said realistically this short is shit because the highs being swept over on the left arent significant, but ignore that for the sake of the explanation) - the target would be FTA as usual. and FTA "

## target.far-targets-pay

Causal backtest: with the lookahead filters removed, win rate is about 29% and the system wants a far target, 3-5R, with a broad plateau. 1.0R loses money.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:85](../sources/user-research-pages.md) (The target sweep, first half against second, score 38.31): "A broad plateau from 3R to 5R rather than a single spike, which is what a real parameter looks like. 4R sits in the middle of it."
- [sources/user-research-pages.md:79](../sources/user-research-pages.md) (What is left once the lookahead is gone, score 34.76): "Refitted on the first half of the record only and confirmed on the second. The surprise is that removing the filter also reverses the target: 1.5R had been chosen on top of an inflated win rate. Causally, the system wants to run its winners"
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 32.37): "| 1.5R is the right target | Direction, Trade by Trade | Reversed. 1.5R was optimal against a 77% win rate that does not exist. At the true 29% the system needs 3–5R, and at 1.0R it loses money outright. |"

## Conflicts

- `target.minimum-rr` vs `target.far-targets-pay`: The teacher's discretionary style is quick in-and-out trades near the first trouble area (0.8-1.2R). The causal backtest says the same entries only pay at 3-5R. They are different styles; do not mix their stops and targets.
