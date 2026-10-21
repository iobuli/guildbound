# Higher-timeframe bias (D1 / W1) (`daily-bias`)

Reads closed daily and weekly candles for a pinbar, engulfing, SFP or inside-day failure, cancels opposing signals on the same bar, and expires the bias after a couple of days.

Decides at **prior_bars**, step 3.1 of the order of operations: Confluence factor: daily bias (a bar pattern, not a trend). Inputs: sfp. Outputs: bias.

## daily-bias.types

Daily bias comes from a pinbar, an engulfing candle, an SFP, or (in the later lessons) an inside-day failure. The 2012-era webinar names only the first three.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24); lesson curated/taking-a-trade-daily-biases.md (2024-10-18); transcript levels webinar part 1, user-pasted transcript @37:07.

Supporting passages:
- [sources/course/curated/taking-a-trade-daily-biases.md:1](../sources/course/curated/taking-a-trade-daily-biases.md) (daily-biases, score 22.98): "**18/10/2024 18:16** INSIDE DAY FAILURE"
- [sources/course/general/general_keywords.md:918](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 21.62): "- 23/04/2026 | why did the very next daily candle close above the engulfing KPI (low of the day"
- [sources/course/general/general_keywords.md:898](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 19.34): "- 12/03/2026 | inside day failure off a daily support level (slightly swerved, but close enough)."

## daily-bias.crossbar-cancels

Two opposing biases on one bar (a bullish engulfing with a bearish SFP) cancel out and the market is neutral.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24); webinar curated/notes-images.md (2024-09-08).

Supporting passages:
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 22.85): "crossbars in opposite directions, for example: 'bullish engulfing with bearish SFP' cancel each other out. and the market is neutral."
- [sources/course/curated/notes-images.md:1](../sources/course/curated/notes-images.md) (notes-images, score 18.08): "the trader calls this scenario a 'cross bar' - when there are 2 biases in 1."
- [sources/course/curated/tp-selection.md:1](../sources/course/curated/tp-selection.md) (tp-selection, score 14.43): "**23/09/2024 13:19** bullish d1 engulfing (green arrow)"

## daily-bias.expires

A daily-engulfing bias stops counting after a couple of days if no entry is given.

*hard, stated, tested by code.* Sources: webinar curated/notes-images.md (2024-09-10).

Supporting passages:
- [sources/course/curated/notes-images.md:1](../sources/course/curated/notes-images.md) (notes-images, score 22.29): "however, if entry isnt given after a couple of days, you can no longer use the D1 engulfing as a factor of confluence - and this goes back to being just a standard 2 touch level (that still requires 1 more factor of confluence). (edited)"
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 15.18): ""no daily bias = in and out quickly""
- [sources/course/tweets/tweets_clean.md:50](../sources/course/tweets/tweets_clean.md) (2026, score 13.44): "**2026-06-03** [SFP, bias, levels, stops, trade management]"

## daily-bias.weekly-and-daily-then-h1

W1/D1 give the bias, H1 gives the execution, and he never looks lower.

*hard, stated, not coded.* Sources: tweet curated tweets (2026-09-24); webinar user-pasted notes (2024-09-24).

Supporting passages:
- [sources/course/tweets/tweets_clean.md:77](../sources/course/tweets/tweets_clean.md) (2026, score 20.9): "W1/D1 for the bias, H1 for execution."
- [sources/course/general/general_full.md:14629](../sources/course/general/general_full.md) (take notes, score 17.75): "**02/04/2026 19:22** weekly bias: pinbar + sweep = bullishdaily bias: broke above resistance, coming back to retest it as support = bullishh1: bullish pinbar right at the D1/W1 support levelTP at the next level up (green circle) for +2.1R ["
- [sources/course/general/general_keywords.md:933](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 17.68): "- 29/04/2026 | CHFJPY Direction:W1 bearish pinbar x SFPW1 Low of HammerD1 S/RObjection:the weekly technically hasnt broken under the bearish hammer, as it hasnt yet closed.Location:D1 LXPBExecution:H1 LXPB with TP @FTA [image]"

## daily-bias.only-20-percent

Only about 20% of his trades have a daily bias. The other 80% go straight to H1 and are quick in-and-out trades at 0.8-1R on average.

*heuristic, stated, not coded.* Sources: webinar user-pasted notes (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 31.79): "* Most trades (80%) are quick in/out with 0.8-1R average"
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 20.04): "only about 20% of his trades have a daily bias."
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 14.3): "* If daily bias has been hit or several days pass with no action, abandon the setup"

## daily-bias.against-it

You can trade against the daily bias, but the levels must be good and you must get out at the first sign of trouble. An H1 short is normally skipped if the day looks set to close as a bullish engulfing, unless the target is very close.

*hard, stated, not coded.* Sources: webinar curated/trade-management.md (2024-10-18); webinar user-pasted notes (2024-09-25).

Supporting passages:
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 36.81): "* Trading against the daily bias requires good levels and quick exits at first sign of trouble"
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 32.3): ""if youre trading a H1 level, at 8pm and you see a good level to go short... but its quite evident that when you flip to the daily timeframe, the market is very likely to close as a bullish engulfing - youre probably not going to take that "
- [sources/course/general/general_keywords.md:918](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 24.77): "- 27/03/2026 | weekly bias: none daily bias: double top = bullish h1: broke above its green resistance level. no resistance between here and the double top. entry is the hourly hammer in the green S/R zone. [image]"

## daily-bias.reverse-trading

Start from where price is going and why, then look for an entry that fits that bias.

*hard, observed, not coded.* Sources: journal curated/lxpb-trades-what-constitutes-a-daily-bias.md (2026-02-07).

Supporting passages:
- [sources/course/curated/lxpb-trades-what-constitutes-a-daily-bias.md:1](../sources/course/curated/lxpb-trades-what-constitutes-a-daily-bias.md) (what-constitutes-a-daily-bias, score 18.53): "an example of reverse trading. starting with where its going + why, and then seeing if we can get an entry based on this bias"
- [sources/levels-webinar-part1.md:465](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 13.19): "[1:02:30] by doing is I always look at the market as where it currently is trading so if"
- [sources/course/general/general_keywords.md:700](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 12.75): "- 15/10/2024 | its exactly at the same level as the ATR. so we would be expecting for price to stop going up once it reaches that level (because it, on average, should stop going up from here). [image]"

## daily-bias.candle-definitions

The exact candle definitions (pin wick share, engulfing test) are not given in the sources. This build uses generic textbook thresholds.

*heuristic, inferred, tested by code.* Sources: lesson curated/taking-a-trade-daily-biases.md (2024-10-18).

Supporting passages:
- [sources/course/curated/lxpb-trades-what-constitutes-a-daily-bias.md:1](../sources/course/curated/lxpb-trades-what-constitutes-a-daily-bias.md) (what-constitutes-a-daily-bias, score 11.56): "**07/02/2026 18:50** last bullish candle wasnt an engulfing, but it 'looks like it' and apparently these share the same prev day candle KPI as engulfings. so seeing price break down under this (on the hourly only) with a very clear price ta"
- [sources/course/curated/trade-management.md:79](../sources/course/curated/trade-management.md) (trade-management, score 10.9): "the reason why he said he is okay with putting it above the thrust candle, is because the resistance level is so perfectly defined. every high/low is in the exact same spot - there really shouldnt be any wiggle room for price to squeeze hig"

## daily-bias.unclosed-anticipation

In his journal he sometimes acts on a daily or weekly candle that has not closed yet ("looks like it might close as a bullish engulfing").

*hard, observed, not coded.* Sources: journal curated/lxpb-trades-passed-atr.md (2024-12-21); journal curated/lxpb-trades-what-constitutes-a-daily-bias.md (2026-02-07).

Supporting passages:
- [sources/course/curated/lxpb-trades-passed-atr.md:1](../sources/course/curated/lxpb-trades-passed-atr.md) (passed-atr, score 24.28): "**21/12/2024 03:29** weekly LXPBdaily LXPBH1 LXPBpast ATRfriday 18:00 (end of week reversal)alert daily looks like it might close as a bullish engulfing - but is, as of yet, unclosed [image]"
- [sources/course/general/general_full.md:14656](../sources/course/general/general_full.md) (take notes, score 20.18): "if so, why did the very next daily candle close above the engulfing KPI (low of the day before)?"
- [sources/course/general/general_keywords.md:918](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 19.92): "- 21/04/2026 | skipped trade: engulfing failure price above the weekly S/Rdaily closed above the engulfing KPI (low of the candle prior to the bearish engulfing)hourly LXPB right on top of the weekly zoneSL well below the weekly zone to be "

## daily-bias.closed-bars-only

This build only reads CLOSED daily and weekly bars, so an anticipated but unclosed candle is never used.

*hard, inferred, tested by code.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/course/curated/lxpb-trades-return-to-hammer.md:1](../sources/course/curated/lxpb-trades-return-to-hammer.md) (return-to-hammer, score 16.89): "**21/12/2024 03:27** weekly inside failure (unclosed)daily pinbarH1 LXPB"
- [sources/course/curated/lxpb-trades-pattern-failure.md:1](../sources/course/curated/lxpb-trades-pattern-failure.md) (pattern-failure, score 12.93): "**06/01/2025 12:00** rejected off hammer abovegot back below the weekly HoH (closed on the daily)H1 LXPB confluent with the W1 HoH"
- [sources/course/curated/lxpb-trades-passed-atr.md:1](../sources/course/curated/lxpb-trades-passed-atr.md) (passed-atr, score 12.17): "**16/12/2024 03:18** no weekly confluencelevel is at daily bearish hammerh1 spike candle rejecting off h1 LXPBpast ATR"

## daily-bias.bar-pattern-not-trend

The bias must be a bar pattern on the daily chart, never just the trend: a pinbar, a swing failure (swing low broken and price closes above, or the reverse) or an engulfing. Silver example: a bullish pinbar that was also a swing failure gave "a strong directional bias" because breaking the range low and closing back inside usually takes price to the other side of the range.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @35:16; transcript levels webinar part 1, user-pasted transcript @36:03; transcript levels webinar part 1, user-pasted transcript @37:47.

Supporting passages:
- [sources/levels-webinar-part1.md:272](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 51.23): "[37:23] here so the market breaks a swing low closes above or breaks a swing High closes below or there will be a bullish"
- [sources/levels-webinar-part1.md:257](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 43.74): "[35:42] of crossbar okay we've we've uh we we've put in a swing low Market's broken the swing low closed above Force broken the"
- [sources/course/curated/levels.md:19](../sources/course/curated/levels.md) (levels, score 35.6): "daily range, price made a false break under the bottom of the range, and got back inside as a shitty SFP (not sweeping a deep low) and a good pinbar. [image]"

## Conflicts

- `daily-bias.unclosed-anticipation` vs `daily-bias.closed-bars-only`: The discretionary journal anticipates unclosed daily candles; the causal rule forbids it. A systematic version cannot act on a candle that has not printed.
