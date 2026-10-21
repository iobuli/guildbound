# Risk and sizing (`risk`)

Turns the stop distance into a position size for a fixed percentage risk. The rules about adding and not cutting size live here as rules.

Decides at **fill_time**, step 8 of the order of operations: Risk: size from the stop distance. Inputs: stop. Outputs: risk.

## risk.always-one-r

"Always 1R. I never cut size." Cutting size right after a loss makes no sense. 1R can shrink as a share of the account in a drawdown because risk is a fixed percentage.

*hard, stated, not coded.* Sources: tweet curated tweets (2026-09-29).

Supporting passages:
- [sources/course/tweets/tweets_clean.md:77](../sources/course/tweets/tweets_clean.md) (2026, score 51.66): "Having said that, it can drop naturally when in drawdown because 1R can become less of the account if applying fixed percentage risk."
- [sources/user-research-pages.md:136](../sources/user-research-pages.md) (The Twelve-Bar Blind Spot, score 20.41): "Account balance · $100,000 at 0.5% risk, fixed size *wait 1* vs dotted wait 12"
- [sources/course/general/general_keywords.md:359](../sources/course/general/general_keywords.md) (general: keyword snippets (+/-5 words around each trading term), score 16.69): "- 05/06/2023 | i have no setup, but it makes sense"

## risk.size-from-stop

Position size = account risk / stop distance, with risk a fixed percentage of the account.

*hard, inferred, tested by code.* Sources: tweet curated tweets (2026-09-29); backtest user-supplied twelve-bar page. Note: The backtest used 0.5% risk per trade; the percentage in live use is not stated.

Supporting passages:
- [sources/course/tweets/tweets_clean.md:77](../sources/course/tweets/tweets_clean.md) (2026, score 22.76): "Having said that, it can drop naturally when in drawdown because 1R can become less of the account if applying fixed percentage risk."
- [sources/user-research-pages.md:136](../sources/user-research-pages.md) (The Twelve-Bar Blind Spot, score 20.16): "Account balance · $100,000 at 0.5% risk, fixed size *wait 1* vs dotted wait 12"
- [sources/course/tweets/tweets_clean.md:24](../sources/course/tweets/tweets_clean.md) (2026, score 14.77): "In my opinion, you should have blind bid the D1 level and then added on the H1 PA. This would have given you a near be stop on the 1st position, making your risk still 1R overall (assuming same risk % size on second addition) but adding R t"

## risk.adding-keeps-risk-controlled

Add to a trade that is confirming your thesis if the addition fits the plan, the upside is still asymmetric and total risk stays controlled. Adding on H1 price action to a blind D1 bid leaves risk at 1R overall because the first stop is near breakeven.

*hard, stated, not coded.* Sources: tweet curated tweets (2026-06-03); tweet curated tweets (2026-05-01).

Supporting passages:
- [sources/course/tweets/tweets_clean.md:50](../sources/course/tweets/tweets_clean.md) (2026, score 59.71): "You should try to add to a trade that is confirming your thesis, provided the addition fits your plan, the upside is still asymmetric, and risk remains controlled."
- [sources/course/tweets/tweets_clean.md:24](../sources/course/tweets/tweets_clean.md) (2026, score 40.57): "In my opinion, you should have blind bid the D1 level and then added on the H1 PA. This would have given you a near be stop on the 1st position, making your risk still 1R overall (assuming same risk % size on second addition) but adding R t"
- [sources/course/tweets/tweets_clean.md:77](../sources/course/tweets/tweets_clean.md) (2026, score 23.8): "Having said that, it can drop naturally when in drawdown because 1R can become less of the account if applying fixed percentage risk."

## risk.review-in-drawdown

Reviewing strategies in a -10R or -20R drawdown.

*heuristic, observed, not coded.* Sources: webinar curated/notes-images.md (2024-09-30).

Supporting passages:
- [sources/course/curated/notes-images.md:79](../sources/course/curated/notes-images.md) (notes-images, score 17.14): "**30/09/2024 12:47** spread bettingreviewing strategies in -10R or -20R drawdown [image]"

## risk.correlated-setups

Rule 4, "know when to break the rules": he took a second highly correlated setup (breaking his usual rule) only because he had seen exactly one loss under that setup.

*heuristic, stated, not coded.* Sources: webinar user-pasted notes (2024-09-24).

Supporting passages:
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 33.98): "- he said this when he was already short EU, and an SFP into a gap set up on AU. getting into AU was 'a little bit of a rule breaker', but he has only ever seen 1 loss under this setup - so its an obvious time to break that rule of not taki"
- [sources/course/general/general_full.md:7848](../sources/course/general/general_full.md) (take notes, score 16.1): "and you took an L last monday because of it"
- [sources/course/general/general_full.md:13899](../sources/course/general/general_full.md) (take notes, score 15.41): "**16/12/2024 01:01** looking at a few setups for monday:"

## risk.slippage-unmeasured

The backtest charged a flat 0.10R of slippage. That figure is unmeasured, especially for gold in thin hours and a crypto venue with funding and depth limits. "Nothing here should be sized until it is."

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:98](../sources/user-research-pages.md) (Per instrument, score 86.82): "Still unmeasured Slippage is charged at a flat 0.10R everywhere. On gold in thin hours, and on a crypto venue with funding and real depth limits, that number needs measuring rather than assuming. Nothing here should be sized until it is."
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 13.98): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"

## risk.long-losing-runs

A 29% win rate means long stretches of being wrong; the worst losing run is the price of the far target.

*hard, backtest-reported, not coded.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:98](../sources/user-research-pages.md) (Per instrument, score 54.59): "What this costs you to sit through A 29% win rate means long stretches of being wrong. The worst losing run in the record is trades in a row. That is a real thing to endure, and it is the price of the far target."
- [sources/user-research-pages.md:158](../sources/user-research-pages.md) (Does it survive out of sample, score 21.68): "Median stop stays at **0.25% of price** at every setting, and worst drawdown barely moves. The money does not come from betting bigger — it comes from more trades at a *higher* win rate. Gold goes from 29 to 49 trades a year and its win rat"
- [sources/user-research-pages.md:79](../sources/user-research-pages.md) (What is left once the lookahead is gone, score 20.32): "Refitted on the first half of the record only and confirmed on the second. The surprise is that removing the filter also reverses the target: 1.5R had been chosen on top of an inflated win rate. Causally, the system wants to run its winners"
