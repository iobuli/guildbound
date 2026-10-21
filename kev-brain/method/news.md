# News handling (`news`)

Keeps resting orders from filling into red news: no fills 15 minutes either side, pull a limit if price is close to it, leave it if price is far away, and hold anything already open.

Decides at **fill_time**, step 5 of the order of operations: News: no fills inside red news; pull a nearby limit and put it back after. Inputs: atr. Outputs: news.

## news.no-fills-window

Red-folder news: no trading 15 minutes before or after.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-10-03).

Supporting passages:
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 22.94): "if a news folder is red on forex factory - he will not trade 15 minutes before, or 15 minutes after the news has come out (exception being if price is ~20+ pips away and the news may cause an RTO). if he is already in a trade though, he wil"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 18.59): "* Don’t trade 15 minutes before/after news"
- [sources/course/tweets/tweets_clean.md:1](../sources/course/tweets/tweets_clean.md) (Tweets: trading content only (cleaned), score 13.03): "Left out as having no trading content: a quip before NFP, a sarcastic reply about news and candlesticks (not a rule), and a claim about coining the SFP term."

## news.hold-open-trades

If already in a trade at the release, hold it. Do not lighten up or cut; keep the stop where the technicals put it.

*hard, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24); webinar user-pasted notes (2024-10-03).

Supporting passages:
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 22.67): ""I am in in a trade, I will hold through news. I don't lighten up, I don't cut, I keep my stop where it is based on technical. and I will hold through news.""
- [sources/levels-webinar-part1.md:451](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 15.23): "[1:00:20] 153s you want to keep a a tight stop but what I want to ask myself is am I stopping out into an area where I would"
- [sources/user-research-pages.md:127](../sources/user-research-pages.md) (How to not do this a third time, score 14.42): "Dante system · causal build · 4R target, 2-bar grace at a 2× stop, exit at the close on a failed hold · net of 0.10R slippage per trade · 6,438 trades across four instruments, October 2009 to September 2026. Not investment advice; none of t"

## news.pull-limit-when-close

If price is close to the level at the release (about 10-20 pips on EURUSD), take the order out and put it back after the news. 2012 webinar: give the news five minutes. 2024 notes: about ten.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @1:18:01; webinar curated/notes-images.md (2024-09-19); webinar user-pasted notes (2024-09-24).

Supporting passages:
- [sources/course/curated/notes-images.md:59](../sources/course/curated/notes-images.md) (notes-images, score 40.39): "so if price is hovering near his level before NFP for example, he will remove the limit - and if applicable - reinsert it about 10 minutes after the news release. [image]"
- [sources/levels-webinar-part1.md:567](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 28.59): "[1:18:08] usually if we're trading very close to a level just before news um I will take the order out and stick it in as soon as"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 27.02): "* If price is very close to a level at news release, remove limit until 5 mins after news"

## news.leave-it-when-far

If price is far away (about 60 pips on big news), do not remove the limit. At 20+ pips away the news may even be what tags you in with a retest.

*hard, stated, tested by code.* Sources: webinar curated/notes-images.md (2024-09-19); webinar user-pasted notes (2024-10-03).

Supporting passages:
- [sources/course/curated/notes-images.md:59](../sources/course/curated/notes-images.md) (notes-images, score 33.69): "he then mentions if like 60 pips away from entry on big news, he wont remove the limit."
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 25.85): "* Exception: if price is ~20+ pips away and news may cause RTO (retest opportunity)"
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 20.62): "if a news folder is red on forex factory - he will not trade 15 minutes before, or 15 minutes after the news has come out (exception being if price is ~20+ pips away and the news may cause an RTO). if he is already in a trade though, he wil"

## news.no-m5-through-news

He does not like holding an M5 trade through news.

*hard, stated, not coded.* Sources: webinar user-pasted notes (2024-10-18).

Supporting passages:
- [sources/user-research-pages.md:474](../sources/user-research-pages.md) (Rebuild it yourself, score 19.1): "he implied he doesnt like holding m5 through news:"
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 13.08): "he talks about being in a USDJPY trade through NFP and holding."
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 11.96): "* If already in a trade during news, hold through it"

## news.non-fx-calibration

Pip thresholds only make sense for forex. For other instruments this build substitutes ATR multiples (pull within 1 H1 ATR, leave beyond 3).

*heuristic, inferred, tested by code.* Sources: webinar curated/notes-images.md (2024-09-19).

Supporting passages:
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 20): "* Fibonacci levels must be within 10% of day’s ATR to count as confluence"
- [sources/user-research-pages.md:392](../sources/user-research-pages.md) (Rebuild it yourself, score 12.53): "* Example: 20 pip bearish bar needs to close within 4 pips of lower wick"

## Conflicts

- `news.pull-limit-when-close` vs `news.no-fills-window`: The 2012 webinar reinserts the order five minutes after the release; the 2024 notes say ten; the red-news window itself is fifteen minutes either side. This build uses five for "reinsert" and fifteen for "no fills".
