# Time, session and mode (`context`)

Works out the session, whether it is a late-Friday or flatten-by-close situation, and whether the trade is a with-bias trade or a quick no-bias scalp.

Decides at **prior_bars**, step 5.1 of the order of operations: Context: session, day of week, flatten-by-close. Inputs: bias. Outputs: context.

## context.friday-afternoon

Context matters: at 2pm on a Friday price is not going to move much, so without a strong bias targets should be conservative.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24). Note: Treated as Friday from 13:00 UTC; the source gives only "2pm".

Supporting passages:
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 31.11): "context matters, at 2pm on a Friday, what are you expecting? price isn't gonna move much. so (if you don't have a strong bias) your targets should be conservative."
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 22.46): "* Time context matters (e.g., 2pm Friday expectations)"
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 14.78): "* Example: “High probability H1 bearish sweep fading a quick upmove on Friday at 2pm”"

## context.flatten-by-close

Forex positions are closed at the end of the week. FTSE and Bund trades are closed at the end of the day.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 27.86): "* Close FTSE/BUND trades at end of day"
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 25.43): "for forex trades, close any positions at the end of the week."
- [sources/course/curated/advanced-sfp.md:47](../sources/course/curated/advanced-sfp.md) (advanced-sfp, score 14.45): "(FTSE) the trader does not care too much about SFPs on the close of market [image]"

## context.no-bias-means-quick

Without a daily bias the trade is in and out quickly. 80% of his trades are like this at 0.8-1R on average.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 26.21): "Without Daily Bias (80% of trades):"
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 19.62): "80% of his trades are quick in and out with 0.8-1R avg."
- [sources/user-research-pages.md:359](../sources/user-research-pages.md) (Rebuild it yourself, score 14.86): "* If daily bias has been hit or several days pass with no action, abandon the setup"

## context.crosspairs-squeeze

Be careful on cross pairs (anything not paired with USD). They seem to squeeze more, so the stop may need to be safer.

*hard, stated, not coded.* Sources: webinar user-pasted notes (2024-09-25); webinar curated/notes-images.md (2024-09-23).

Supporting passages:
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 26.19): "you have to be careful on crosspairs (anything not paired to USD). I guess they squeeze more? so SL is more in danger, you might have to set a safer SL."
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 21.79): "* Be cautious on cross pairs (non-USD pairs) - they may require safer stop losses"
- [sources/levels-webinar-part1.md:499](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 17.01): "[1:08:47] USD in I'm calling a cross cross rate sorry I should have made that"

## context.ltf-against-htf

Do not trade M5 against the H1 bias, just as H1 against the D1 bias is avoided.

*hard, stated, not coded.* Sources: webinar user-pasted notes (2024-10-18).

Supporting passages:
- [sources/user-research-pages.md:474](../sources/user-research-pages.md) (Rebuild it yourself, score 17.98): "you dont really want to be trading m5 against the H1 bias (same as trading H1 against D1 bias)."
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 15.12): "* Avoid trading M5 against H1 bias (just like avoiding H1 against D1 bias)"
- [sources/course/tweets/tweets_clean.md:50](../sources/course/tweets/tweets_clean.md) (2026, score 12.47): "HTF D1 Bias: SFP & bearish spike"

## context.markets-traded

Markets traded: forex majors and crosses (GBPUSD, EURUSD, USDCHF, USDCAD, USDJPY, NZDUSD, AUDUSD, EURCHF, EURGBP, EURJPY, GBPJPY), Bund, spot gold and silver, crude oil, DAX and SPX futures. Later posts also mention Eurostoxx 50, ES and silver.

*hard, observed, not coded.* Sources: webinar curated/notes-images.md (2024-09-30); tweet curated tweets (2026-09-21).

Supporting passages:
- [sources/course/curated/notes-images.md:79](../sources/course/curated/notes-images.md) (notes-images, score 113.11): "Crude Oil (futures)"
- [sources/course/tweets/tweets_clean.md:24](../sources/course/tweets/tweets_clean.md) (2026, score 20.22): "The day I exited my Gold short, Silver closed as a bullish spike, an IDF and SFP, so I went long this morning targeting the stops above the ID high."
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 15.02): "for trades in markets such as the FTSE or BUND, close at the end of the day."

## context.crypto-not-covered

None of the sources cover crypto. Applying the method to SOL assumes forex-style behaviour carries over.

*heuristic, inferred, not coded.* Sources: webinar curated/notes-images.md (2024-09-30). Note: This is the biggest untested assumption in the system.

No strong supporting passage found in the sources (BM25 score < 10 or fewer than 4 shared words).
