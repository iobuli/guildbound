# The user's earlier research pages (pasted text)

Source: pasted by the user on 2026-10-20 together with the course export zip: a backtest forensics write-up (filters that read a close before it printed), 'The Twelve-Bar Blind Spot', 'Solana Wallet Atlas' and notes. Kept verbatim as the user's own analysis; numbers in it were not re-checked here.

Backtest forensics · 13 September 2026

# The close that hadn’t printed yet

Two filters in the Dante system were reading the end of a bar to justify a trade taken in the middle of it. Here is how that was found, what it cost, and what is left once it is removed.

XAUUSD · EURUSD · USDJPY · SOLUSDT H1 · 2009–2026 6,438 trades net 0.10R slippage

## A bar has four prices and only one of them is the last

The system buys at a level. The level is a price, price reaches it mid-bar, the order fills. That part is sound. The problem is what happened next.

Two rules decided whether to *keep* that fill:

**The level-held rule** — take the trade only if the bar *closes* back on the break side.\
**The reclaim gate** — take it only if the bar closes at least 0.75R past the level.

Both are conditions on the **close**. The fill happened at the **level**, earlier in the same hour. At the moment money went on the table, that close did not exist yet. The backtest knew it because the backtest was reading a finished candle out of a file. A live account would have been holding a position and waiting.

The shape of the error

### One real hour, in the order it happened

This is trade from the record. Read it as a person at a desk would have lived through it.

Why this is the same bug as the phantom fill Earlier in this project, entries were booked *at the level* whether or not price ever traded there — roughly 60% of fills at prices that never printed. That was caught only because the Pine script on TradingView came back with 41.84% instead of 61.5%. This is the identical family of mistake: information from the wrong point in time deciding an earlier action. The phantom fill invented a price. This one invents knowledge.

## Two experiments, and they disagree about how bad it is

Each rule was re-run in a form that uses only what is available at the moment of the fill. A real rule survives that test. One of these did not survive at all. The other survived, much smaller.

### Experiment one — the reclaim gate

If the condition reads the close, then the honest entry price **is** the close. Run it that way: the stop stays where it was, so risk is wider and the fill is worse. That is the trade you could actually have got.

Both halves of the record are negative in the causal version. The gate is not a filter — it is a description of bars that had already gone the right way.

| Reclaim gate at 0.75R | Trades | Win | Per year | Sharpe | Years + |
| --- | --- | --- | --- | --- | --- |
| Fill at the level, gated on the close | 2,373 | 90.5% | +$69,377 | 6.71 | 18/18 |
| Fill at the close, gated on the close | 2,097 | 42.8% | −$6,989 | −0.78 | 3/18 |
| Fill at the close, no gate at all | 3,615 | 44.8% | −$14,989 | −1.21 | 1/18 |

Verdict: rejected outright A 90.5% win rate at Sharpe 6.71 was the tell. Nothing in an hourly retest strategy wins nine times in ten. The gate does not predict the bar; it *is* the bar.

### Experiment two — the level-held rule

Here there is a third option, and it matters. You cannot **filter** on the close, but you can **exit** on it. Take every retest fill; if the bar ends up closing on the wrong side, flatten at that close and wear the small loss. That is fully implementable at a live desk.

The edge is real, but roughly six times smaller than the filtered version claimed. Both halves of the record stay positive in rows two and three.

| Level-held rule, 1.5R target | Trades | Win | Per year | Sharpe | Years + |
| --- | --- | --- | --- | --- | --- |
| Used as a filter — what was published | 4,122 | 76.9% | +$71,796 | 5.13 | 18/18 |
| Ignored — take every fill | 6,374 | 58.1% | +$12,789 | 0.73 | 13/18 |
| Used as an exit at the close | 6,816 | 45.1% | +$12,304 | 0.84 | 14/18 |

### The cleanest way to see it

Split the full causal record by whether the entry bar happened to close on the right side. Nobody can make that split in advance. The earlier artifact reported the first row and quietly discarded the second.

At the 4R target. The middle row is not a rounding error — it is the bill.

| Subset of the same 6,438 trades | Trades | Win | Per year | Sharpe | Years + |
| --- | --- | --- | --- | --- | --- |

Add the two subsets back together and you get the honest number. That is the whole correction in one table: **you may keep the good subset only if you can name it in advance, and you cannot.**

## Real trades, drawn honestly

Every chart here is a genuine setup from the record. The boxed bar is the break, the faint verticals mark the swing touches that built the level, the dot marks the price where the limit filled, and the short bar marks where that same hour closed.

Arrow keys step through a group. **Bars that closed wrong** is the group the earlier artifact threw away. On each one, look at the distance between the dot and the close mark: the dot is when you were filled, the close mark is the last thing that happens in that hour.

## What is left once the lookahead is gone

Refitted on the first half of the record only and confirmed on the second. The surprise is that removing the filter also reverses the target: 1.5R had been chosen on top of an inflated win rate. Causally, the system wants to run its winners a long way.

### Equity, $100,000 at 0.5% risk per trade

### The target sweep, first half against second

A broad plateau from 3R to 5R rather than a single spike, which is what a real parameter looks like. 4R sits in the middle of it.

| Target | Win 1st | Sharpe 1st | Win 2nd | Sharpe 2nd | $/yr 2nd |
| --- | --- | --- | --- | --- | --- |
| 1.0R | 49.4% | −1.44 | 48.9% | −1.19 | −$14,914 |
| 1.5R — the old choice | 45.5% | 0.72 | 44.7% | 0.96 | +$14,909 |
| 2.5R | 37.8% | 2.05 | 37.4% | 2.54 | +$54,692 |
| 4.0R — chosen | 29.1% | 2.38 | 28.8% | 2.73 | +$79,997 |
| 6.0R | 21.4% | 2.02 | 22.1% | 2.63 | +$98,779 |
| 10.0R | 15.0% | 1.95 | 14.3% | 2.03 | +$96,620 |

### Per instrument

SOL was never used to fit a single parameter — the whole configuration was set on gold and the two FX pairs, then dropped on SOL unchanged.

| Instrument | Trades | Win | Per year | Sharpe | Worst run | Years + |
| --- | --- | --- | --- | --- | --- | --- |

What this costs you to sit through A 29% win rate means long stretches of being wrong. The worst losing run in the record is trades in a row. That is a real thing to endure, and it is the price of the far target.

Still unmeasured Slippage is charged at a flat 0.10R everywhere. On gold in thin hours, and on a crypto venue with funding and real depth limits, that number needs measuring rather than assuming. Nothing here should be sized until it is.

## What the earlier artifacts said, and what is true

Five pages were published on this system before this one. Each was honest about what it measured; the measurement underneath two of them was wrong. Corrections, ordered by how much they change.

| Claim | Where | Status |
| --- | --- | --- |
| Rejected setups win 13.1% — the hold rule is a filter worth keeping | The Level-Held Filter | Half right. The 13.1% is real, but you cannot know which setups belong to that group until the bar has ended. It becomes an **exit**, not a filter, and the edge falls from Sharpe 5.13 to 0.84. |
| SOL: 503 trades, 75.7% win, +$27,806/yr, Sharpe 3.41 | SOL Trade Check | Inflated by the same filter. Causally, at 4R: . The transfer to SOL without refitting still holds — it is simply a smaller number. |
| 1.5R is the right target | Direction, Trade by Trade | Reversed. 1.5R was optimal against a 77% win rate that does not exist. At the true 29% the system needs 3–5R, and at 1.0R it loses money outright. |
| Direction from the cluster kind — a broken high means go long | What Direction Is Actually Right | Stands. Unaffected by the fill question, because it was measured on both sides of every setup independently. |
| The space rule — price must travel 0.5×ATR clear before a retest counts | the source webinar | Stands, and is the single most important rule in the system. It uses only bars that closed before the fill. |
| The 2-bar grace window at a 2× stop | Dante Trade Inspector | Stands, but is now nearly irrelevant: at a 4R target it is worth about 0.05 of Sharpe. Keep it; do not lean on it. |
| The reclaim gate — require the bar to close 0.75R past the level | never published | Rejected. It was caught before it reached a page, which is the only reason it is a footnote here rather than a sixth correction. |

The rule this project keeps re-learning

If a result improves when you add a condition, check what time that condition becomes knowable. Twice now the answer has been “after the money was already on the table.”

### How to not do this a third time

Every condition in the engine now has to answer one question before it is allowed in: **at the exact bar and price where the order fills, is this quantity computable from bars strictly before it, or from that bar’s open and the traded price alone?** The high, low and close of the fill bar are all forbidden at fill time. They become available one bar later — which is exactly what makes the exit version legal and the filter version not.

Dante system · causal build · 4R target, 2-bar grace at a 2× stop, exit at the close on a failed hold · net of 0.10R slippage per trade · 6,438 trades across four instruments, October 2009 to September 2026. Not investment advice; none of this has been traded with money.


**Interactive** · drag the slider S/R breakout–retest · H1 1.5R · structural stop 16 years · 4 instruments

# The Twelve-Bar Blind Spot

The engine refused any retest arriving within `12` bars of a break. Those are the sharpest setups — price closes through the level, snaps straight back, and you fill at the level with the break candle as your stop. **It was throwing away its own best trades.** Move the slider to 1 and watch what returns.

Instrument

Minimum bars to wait after a break

1bar

Account balance · $100,000 at 0.5% risk, fixed size *wait 1* vs dotted wait 12

Profit by calendar year

## The trades it could never see

Every example below retested within **two bars** of the break. Under the old twelve-bar rule none of them existed. The dashed box is the break candle, and the entry marker sits one or two candles later — right where you said the fills looked late.

Example 1

level entry stop target break candle up down

## Does it survive out of sample

Split half

Seven values were swept, so the result is worthless unless it holds in data nothing was fitted to. Each record split in half: **wait 1 wins both halves on all three long-history instruments**, and decays less doing it.

Sharpe from daily returns annualised by √252. Gold's second half at wait 1 (2.66) beats the current setting's *first* half (2.10).

| Instrument | Setting | First half win | First half $/yr | Sharpe | Second half win | Second half $/yr | Sharpe |
| --- | --- | --- | --- | --- | --- | --- | --- |
| XAUUSD | wait 1 | 69.3% | +$15,282 | 2.77 | 68.4% | +$15,044 | 2.66 |
| XAUUSD | wait 12 | 68.9% | +$8,817 | 2.10 | 62.7% | +$7,075 | 1.65 |
| EURUSD | wait 1 | 69.7% | +$13,391 | 2.56 | 68.0% | +$9,590 | 2.13 |
| EURUSD | wait 12 | 64.5% | +$5,729 | 1.51 | 64.7% | +$4,877 | 1.40 |
| USDJPY | wait 1 | 66.8% | +$9,789 | 2.11 | 66.5% | +$8,788 | 1.93 |
| USDJPY | wait 12 | 66.0% | +$6,097 | 1.64 | 64.4% | +$4,351 | 1.31 |

Why the gain is real and not extra risk

Median stop stays at **0.25% of price** at every setting, and worst drawdown barely moves. The money does not come from betting bigger — it comes from more trades at a *higher* win rate. Gold goes from 29 to 49 trades a year and its win rate rises. That is a setup filter being removed, not leverage being added.

Two things still outstanding

The published system audit and every figure in it used wait 12, so all of it needs rerunning before wait 1 becomes the default. And the **same-bar cluster collision** — one price carrying both a long and a short signal — has not been rechecked at the new setting; more trades may mean more collisions.

**What changed.** One parameter: the minimum number of bars between a break and a qualifying retest. Everything else is untouched — 3-touch levels clustered within 0.1× the prior day's ATR, entry at the level price, stop at the break candle's extreme, target 1.5× risk, one position at a time, spread at 1.2 bps of price, and a 0.05% minimum stop.

**Accounting.** $100,000 with $500 of risk per trade, no compounding, so a dollar in 2011 counts the same as a dollar in 2025. Gold and USDJPY run from 2009–2010; SOLUSDT covers 2020 onward, which is why its rate is higher and less trustworthy.

**Credit where due.** This came from reading the candle charts and saying the entries looked late. They were.

**Observed links** · not inferred Solana · GeckoTerminal trade tape read-only

# Solana Wallet Atlas

Every trade carries the address that made it. Pull enough tapes and the same wallets keep turning up in different coins — **the same hands, early, in launch after launch.** Each line below is a set of real addresses that demonstrably touched both tokens.

Min shared wallets 2

Min overlap 0%

Circle size is how many distinct wallets traded that coin. Line thickness is what share of the smaller coin's trader set the two have in common. Click a coin to isolate its neighbours; click empty space to release.

## The strongest links

**Overlap** is the share of the smaller coin's entire trader set that also traded the other. **Early** counts how many of those wallets arrived in the opening window of *both* — the column that separates coordination from coincidence.

| Coin A | Coin B | Shared | Early in both | Overlap |
| --- | --- | --- | --- | --- |

## Clusters

Connected components over the surviving edges. A cluster is a set of coins reachable from one another through shared traders — not proof of one operator, but the shape one would leave.

## What this proves, and what it doesn't

Read this

A shared wallet is an observation, not an inference

Every link here is an address that appears in both tapes. That is far stronger than the count-based guesswork a screener does — it is not "these two look similar", it is **this address traded both**. Where the overlap is a large share of a small trader set and the wallets were early in both, a common actor is the straightforward reading.

Three reasons not to call it a bundle

**Bots are everywhere.** Aggregators, MEV searchers and market makers touch hundreds of pools. Any wallet appearing in 12 or more was dropped as infrastructure before edges were drawn — without that filter every coin links to every coin and the graph says nothing.

**The tape is a window, not a ledger.** These are the most recent trades per pool, not the full history. A wallet absent here may still have traded; "early" means early in what was captured.

**The funding graph is invisible.** Which wallets funded each other, dev and sniper holdings, LP burn, mint authority — none of that is in a DEX tape. It needs a Solana RPC and an on-chain walk. Co-trading is evidence of a common actor; it is not evidence of a scheme.

## Rebuild it yourself

Three commands

```
python "D:\dante_system\sol\walletatlas.py" --collect --pools 120
python "D:\dante_system\sol\walletatlas.py" --build
python "D:\dante_system\sol\walletatlas.py" --report
```

Read-only against the public API — no key, no wallet, no orders. --hub sets the infrastructure cutoff, --min-shared the edge threshold, and --window how many blocks after a pool's first trade still counts as early.

**Method.** Pool universe sampled across new, trending (5m/1h/6h/24h) and top-volume endpoints so the set is not one cohort. For each pool, the recent trade tape keeps tx_from_address. That builds a wallet→pool bipartite graph, projected to coin↔coin edges weighted by shared non-hub traders. Same-token pairs (one token, two pools) are dropped as trivially linked.

**Limits.** A snapshot, not a monitor — rerun for current data. Only coins with at least one surviving edge are drawn; isolated pools are counted in the totals but not plotted.

I’ll synthesize the key trading principles and strategies from the document. Here’s a comprehensive summary of the trading concepts mentioned:
Breakout Strategy Rules

* Breakouts need significant size to be valid (multiple pips, not just 0.1 pip)
* Breakouts with confluence (like bullish engulfing patterns) get more weight and trading leniency
* On violent breakouts, price often doesn’t retest the best level but the highest/lowest level

Timeframe and Bias Framework
With Daily Bias (20% of trades):

1. Establish D1 bias using:
   * Pinbars
   * Engulfing patterns
   * SFP (Swing Failure Pattern)
2. Isolate target on D1
3. Drop to H1 for entry:
   * S/R (Support/Resistance) flip
   * H1 SFP
4. Use H1 for stop placement

Without Daily Bias (80% of trades):

1. Go straight to H1
2. Isolate target on H1
3. Entry on H1
4. Stop on H1

Risk Management Rules

* Most trades (80%) are quick in/out with 0.8-1R average
* Close forex positions at end of week
* Close FTSE/BUND trades at end of day
* Be cautious on cross pairs (non-USD pairs) - they may require safer stop losses
* Stop loss can’t go below thrust candle if being stopped out into ATR
* Never trade in same direction if market has passed daily ATR

Level Analysis

* A level becomes untradeable when the latest touch isn’t respected by price action
* When longing a daily level with hourly level underneath, stop loss must go under hourly level
* Fibonacci levels must be within 10% of day’s ATR to count as confluence
* Fibs are drawn from daily swing points (“deep key levels”)
* If price pulls back beyond 61.8% fib level, remove the fib (no longer valid confluence)

News Trading Protocol

* For red news folders on Forex Factory:
   * Don’t trade 15 minutes before/after news
   * Exception: if price is ~20+ pips away and news may cause RTO (retest opportunity)
   * If already in a trade during news, hold through it
* If price is very close to a level at news release, remove limit until 5 mins after news

Market Context Considerations

* Time context matters (e.g., 2pm Friday expectations)
* Crossbars in opposite directions (e.g., bullish engulfing with bearish SFP) cancel each other out
* Trading against the daily bias requires good levels and quick exits at first sign of trouble
* Avoid trading M5 against H1 bias (just like avoiding H1 against D1 bias)

Trading Philosophy
“Rules to live by when trading:

1. Always follow your rules
2. Always follow your rules
3. Always follow your rules
4. Know when to break the rules.”

Would you like me to elaborate on any specific aspect of these trading concepts?

1. SUPPORT / RESISTANCE
2. **mhm — 24/09/2024 20:39** line goes approx in the middle of the 'SR zone' and is then refined to the area with the most touches, but *heavily* bias more recent candles over older ones.
3. **mhm — 24/09/2024 20:49** more touches of the line, the better the level is. he also prefers a rounded retest over a sharp V shaped retest.
4. **mhm — 24/09/2024 20:57** 3 touches or more (of a flipped level), he will trade the level blind. 2 touches he wants 1 more factor of confluence. 1 touch he needs lots (at least 2 factors) of confluence. factors of confluence:
   * 1). daily bias
   * 2). fib levels from major swings (he likes to draw these from the daily)
   * 3). at/over ATR (it can be just 'close enough' to ATR as well)
5. **mhm — 24/09/2024 21:08** not super applicable: if drawing a (weekly/daily i think this was) line at THE high/low, move it to S/R above/below (what are the odds price will stop at THE high (assume this high is the only touch on your line)? odds are it will probe beyond the high... into your resistance line). - this is only relevant if there is PA close enough by. if not, then just draw it at the line itself.
6. **mhm — 24/09/2024 21:23** daily engulfing for bias, entry on h1 (dante's first example uses a d1 S/R, which is also a h1 S/R) because its a 'fantastic level' (three super clean touches recently). this was NOT the cleanest h1 level, but I suppose because the daily had such an obvious level, that made more sense to bid.

Let me add these important Support and Resistance concepts to the trading framework:
Support and Resistance Analysis
Drawing S/R Lines

* Draw S/R lines approximately in the middle of the “zone”
* Refine to the area with the most touches
* Heavily favor recent candles over older ones when determining placement
* When drawing at THE high/low point, consider moving the line to S/R above/below (since price often probes beyond exact highs/lows)

Quality of S/R Levels

* More touches of the line = better quality level
* Rounded retests are preferred over sharp V-shaped retests
* Level quality determines entry criteria:
   * 3+ touches of a flipped level: can trade “blind” (no additional confluence needed)
   * 2 touches: requires 1 additional confluence factor
   * 1 touch: requires at least 2 confluence factors

Confluence Factors

1. Daily bias
2. Fibonacci levels from major swings (preferably drawn from daily)
3. Price at/over ATR (or “close enough” to ATR)

Entry Examples

* Using daily engulfing for bias, then entering on H1
* “Fantastic levels” with three clean recent touches can override other considerations
* Daily S/R that coincides with H1 S/R creates stronger setup

This S/R analysis framework appears to be foundational to the overall trading strategy, especially when combined with the bias, timeframe, and risk management rules from the previous notes.
Would you like me to elaborate on how these S/R principles connect with the other concepts, or would you like information about any specific aspect of the trading approach?
Let me add these SFP (Swing Failure Pattern) concepts to our trading framework:
Swing Failure Pattern (SFP) Trading
SFP Identification and Invalidation

* SFP invalidation: closing below the SFP candle itself (not just the candle being swept)
* If an SFP is invalidated while in trade, exit immediately to minimize loss
* For good sweeps, focus on “which swing high would people get bullish on the break of?”
* Usually this is the main, major swing point

SFP Entry Requirements

* Need time and space between original swing point and the SFP (similar to S/R retest)
* Swing levels remain valid for about 2 days
* Can trade SFPs the next day or potentially day after
* If daily bias has been hit or several days pass with no action, abandon the setup

SFP Stop Loss Placement

* H1 SFP trades use ATR-based stop loss
* Use 24-period ATR value at close of sweep candle
* Place SL that many pips below SFP low (for longs) or above sweep high (for shorts)
* For “advanced technique” entries (waiting for price to probe further beyond the level), use the sweep extreme as SL instead of ATR

SFP Take Profit Targets

* Average RR (risk/reward) is ~1.2
* TP at FTA (First Technical Area) if no higher timeframe bias
* If no HTF bias and no FTA, target the opposite side (e.g., swept major H1 low, TP at major H1 high)
* FTA only applies if there’s a key level in the way

SFP Risk Management

* Success rate is quite high (~90%)
* Takes trades with as low as 0.8R, sometimes even 0.5R in high probability setups
* Example: “High probability H1 bearish sweep fading a quick upmove on Friday at 2pm”
* H1 SFP in opposite direction can be reason to close existing trades (but gives price 1-2 hours to “show its hand”)

Advanced SFP Entry Technique

* For large bearish/bullish candles that close near the level
* Defined as “wide ranging bar that closes within 20% of the candle extreme”
* Example: 20 pip bearish bar needs to close within 4 pips of lower wick
* Wait for price to probe further beyond the level before entering

This SFP framework appears to be a key component of the overall trading strategy, particularly for entries at key levels.
Would you like me to connect these SFP concepts with the previously discussed S/R and bias approaches, or would you like information about any other aspect of the trading method?GENERAL
mhm — 24/09/2024 20:28
breakouts generally need to be fairly big for them to 'count' (ie, no 0.1 pip MSBs). 
this can be (slightly) mitigated if we have good confluence. the example given was an MSB that looks reasonably small. definitely multiple pips, but not a standard large breakout with rounded retest that we typically see. but this breakout was a bullish engulfing. so it is given more weight as a breakout, and more leniency as a trade.
mhm — 24/09/2024 20:36
context matters, at 2pm on a Friday, what are you expecting? price isn't gonna move much. so (if you don't have a strong bias) your targets should be conservative.
mhm — 24/09/2024 20:46
crossbars in opposite directions, for example: 'bullish engulfing with bearish SFP' cancel each other out. and the market is neutral.
mhm — 24/09/2024 20:56
only about 20% of his trades have a daily bias.
80% of his trades are quick in and out with 0.8-1R avg.
"no daily bias = in and out quickly"
mhm — 24/09/2024 21:08
a level becomes untradeable when the latest touch is not respected by PA.
mhm — 24/09/2024 21:18
you have to be careful on crosspairs (anything not paired to USD). I guess they squeeze more? so SL is more in danger, you might have to set a safer SL.
mhm — 24/09/2024 21:26
if price is very close to a level at news release, he removes the limit until 5 mins after the news, and then puts it back in if applicable.
he talks about being in a USDJPY trade through NFP and holding.
"I am in in a trade, I will hold through news. I don't lighten up, I don't cut, I keep my stop where it is based on technical. and I will hold through news."

he DOESNT like to be filled on his trades going into news. this is only applicable if price is within ~20 pips (EU as the example) of your entry. he says if you are further than that away when news comes out, then you can actually potentially use the news event to try to tag you in.
mhm — 24/09/2024 21:36
for forex trades, close any positions at the end of the week.
for trades in markets such as the FTSE or BUND, close at the end of the day.
mhm — 24/09/2024 21:50
"rules to live by when trading:
#1 always follow your rules.
#2 always follow your rules.
#3 always follow your rules.
#4 know when to break the rules."

- he said this when he was already short EU, and an SFP into a gap set up on AU. getting into AU was 'a little bit of a rule breaker', but he has only ever seen 1 loss under this setup - so its an obvious time to break that rule of not taking highly correlated setups.
mhm — 24/09/2024 21:58
scenario: im longing a daily level, with an hourly level just underneath.

stoploss must go underneath the hourly level. because while the daily level is more likely to catch a bid, price could easily go a little further into the hourly. and I don't want it to stop me out in the process of finding support.
mhm — 24/09/2024 22:08
on violent breakouts, price often doesn't retest the best level, but the highest/lowest level.
which makes total sense if you see this chart here
mhm — 24/09/2024 22:18
D1 BIAS -->
1). pinbar
2). engulfing
3). SFP
isolate target on D1
drop to H1 for entry:
1). S/R flip for entry
2). H1 SFP for entry
use H1 for stop

NO BIAS -->
straight to H1
isolate target on H1
entry on H1
stop on H1
mhm — 25/09/2024 17:32
"if youre trading a H1 level, at 8pm and you see a good level to go short... but its quite evident that when you flip to the daily timeframe, the market is very likely to close as a bullish engulfing - youre probably not going to take that trade. because you know you're going against the daily bias."
"(unless your target is very close)"
mhm — 30/09/2024 20:31
stop cannot go below the thrust candle if you are being stopped out into ATR. this is just like being stopped out into support. you must move the SL to a candle on the left.

it is also not allowed to stop you out into the next key fib level. so SL must be moved to the left again in this scenario too.
mhm — 30/09/2024 21:37
fib levels must be within 10% of the day's ATR to count as a confluence. 

so if the D1 ATR = 100, and the fib is ≤10 pips away from your level, then it counts as a confluence. 
however if the fib is >10 pips away from your level, then it does not count as a confluence.

if the D1 ATR is 50 pips. then the fib level must be within 5 pips of your level, etc.
mhm — 03/10/2024 20:07
if a news folder is red on forex factory - he will not trade 15 minutes before, or 15 minutes after the news has come out (exception being if price is ~20+ pips away and the news may cause an RTO). if he is already in a trade though, he will stay in the trade through news.
mhm — 03/10/2024 20:18
you do NOT trade (in the same direction) if the market has passed its daily ATR. "if we're 70 points down on the day, and the ATR is 70. im not selling. I don't care if its doing a rounded retest on the mother of all levels. I am not selling."
mhm — 05/10/2024 10:05
fibs:
if we pull back further than the 61.8% fib level, he removes the fib and they can no longer be used as confluence.
he always draws his fibs from daily swing points.
he likes fibs (from different areas) to line up with each other.
mhm — 06/10/2024 14:40
"where I would measure a fib from is sort of like where I would measure an SFP from. only the deep key levels"
mhm — 18/10/2024 15:38
you can take a trade against the daily bias. but your levels must be good, and you cant overstay your welcome in the market - ie, if theres any sign this trade could go south, you should get out.
mhm — 18/10/2024 17:09
you dont really want to be trading m5 against the H1 bias (same as trading H1 against D1 bias).
mhm — 18/10/2024 17:17
he implied he doesnt like holding m5 through news:
"ive been in a bund trade where there are 20 minutes until news, and it hasnt moved [to TP], and i'll be forced to take that even risk. which is just not something that im prepared to do.''
