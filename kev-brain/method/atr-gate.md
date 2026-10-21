# Daily ATR gate (`atr-gate`)

Measures how far price has travelled from the HIGH or LOW OF THE DAY, in daily ATRs. If it has fallen an ATR from the day's high, selling is vetoed and buying gains a confluence (and the reverse).

Decides at **fill_time**, step 3.2 of the order of operations: Confluence: has the market already moved its daily ATR?. Inputs: atr. Outputs: gate.

## atr-gate.measure-from-the-days-extreme

The ATR level is measured from the HIGH (or low) OF THE DAY: with a 123-pip ATR and a day high of 128.31, the market has reached its ATR at 127.08. If it falls 101 pips from the high of the day into the level, that counts.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @45:00; transcript levels webinar part 1, user-pasted transcript @46:09; transcript levels webinar part 1, user-pasted transcript @1:19:57.

Supporting passages:
- [sources/levels-webinar-part1.md:337](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 43.9): "[45:32] going to take some profits on the day so we know that as that market is falling if it falls 123 Pips from the high of the day from the highest point of the"
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 20.95): "| Rejected setups win 13.1% — the hold rule is a filter worth keeping | The Level-Held Filter | Half right. The 13.1% is real, but you cannot know which setups belong to that group until the bar has ended. It becomes an **exit**, not a filt"
- [sources/levels-webinar-part1.md:584](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 20.42): "[1:20:04] Pips okay if we fall 101 Pips from the high of day into that area then I would"

## atr-gate.surpassed-atr-is-confluence

A market that is at or past its daily ATR coming into the level is a confluence: it has moved what it normally moves, so people take profits there and it is likely to turn.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @34:41; transcript levels webinar part 1, user-pasted transcript @45:17; transcript levels webinar part 1, user-pasted transcript @1:16:27; lesson curated/taking-a-trade-1-touch-level.md (2024-10-21).

Supporting passages:
- [sources/levels-webinar-part1.md:337](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 28.23): "[45:40] day it's likely to meet its at and people are likely to take profits now if we ho hover over the high of that"
- [sources/levels-webinar-part1.md:519](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 21.09): "[1:10:21] about these uh these sharp moves used to say the quicker or much more likely to take profits at the"
- [sources/levels-webinar-part1.md:354](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 20.85): "[48:12] the market if you take a fib from low to the highest point before the market began its turn back so we have to go up"

## atr-gate.no-same-direction

Do not trade in the same direction once the market has passed its daily ATR. "70 points down on the day and the ATR is 70, I'm not selling, even on a rounded retest of the mother of all levels."

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-10-03).

Supporting passages:
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 44.03): "you do NOT trade (in the same direction) if the market has passed its daily ATR. "if we're 70 points down on the day, and the ATR is 70. im not selling. I don't care if its doing a rounded retest on the mother of all levels. I am not sellin"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 24.59): "* Never trade in same direction if market has passed daily ATR"
- [sources/course/tweets/tweets_clean.md:7](../sources/course/tweets/tweets_clean.md) (2022, score 16.73): "Market breaks resistance on the H1 (right chart) and then pulls back with a rounded retest, Thrust candle allows for a tight stop for over 4R up to the FTA."

## atr-gate.close-enough

Being "close enough" to the ATR counts. In the EURCHF example the ATR line sat 16 pips below the level and still counted, because it matched the lower part of the level zone.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24); transcript levels webinar part 1, user-pasted transcript @47:01. Note: This build treats 90% of the ATR as close enough; the sources give no number.

Supporting passages:
- [sources/levels-webinar-part1.md:354](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 24.59): "[47:19] level that's being provided as Confluence this was the part of the level this is the lower part"
- [sources/user-research-pages.md:303](../sources/user-research-pages.md) (Rebuild it yourself, score 21.06): "5. **mhm — 24/09/2024 21:08** not super applicable: if drawing a (weekly/daily i think this was) line at THE high/low, move it to S/R above/below (what are the odds price will stop at THE high (assume this high is the only touch on your lin"
- [sources/course/curated/levels.md:1](../sources/course/curated/levels.md) (levels, score 19.78): "**24/09/2024 17:21** this counts as a 3 touch level for him. the third 'touch' is just close enough i suppose. he said he took this trade [image]"

## atr-gate.atr-trades

In the passed-ATR trades of the later journal he cares less about a weekly level sitting in the way, and a daily level is not needed if the H1 level is clean.

*heuristic, observed, not coded.* Sources: journal curated/lxpb-trades-passed-atr.md (2025-01-03); journal curated/lxpb-trades-passed-atr.md (2026-02-09).

Supporting passages:
- [sources/course/curated/lxpb-trades-htf-level.md:1](../sources/course/curated/lxpb-trades-htf-level.md) (htf-level, score 16.17): "note: even with the W1 engulfing off the shooting star, and then the D1 pin off the W1 KPI, and thinking price will reach all the way to the bottom of the weekly hammer for hammer failure, the trader was still bid here (no atr needed). he o"
- [sources/course/curated/lxpb-trades-htf-level.md:22](../sources/course/curated/lxpb-trades-htf-level.md) (htf-level, score 16.13): "**03/03/2026 17:21** weekly level (3 taps)daily level (LXPB)weekly high flushed into the level"
- [sources/course/curated/INDEX.md:1](../sources/course/curated/INDEX.md) (Trading knowledge: text index, score 15.83): "| lxpb-trades-passed-atr | 17 | [lxpb-trades-passed-atr.md](lxpb-trades-passed-atr.md) |"
