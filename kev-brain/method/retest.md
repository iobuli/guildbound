# Retest quality and space (`retest`)

Judges the return to a broken level: was there space after the break (and not the immediate retest), did price come in with momentum, and has the order been swerved (price ran away without filling it).

Decides at **prior_bars**, step 4 of the order of operations: Space: did price break and keep going, and is this not the immediate retest?. Inputs: level, atr, bias. Outputs: retest.

## retest.needs-space

There must be space between the break and the retest: price breaks, closes beyond the level on the hourly, and KEEPS GOING. The higher it keeps going, the more space; the example shown is "about the minimum" he wants.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @25:30; transcript levels webinar part 1, user-pasted transcript @28:00. Note: He says he cannot quantify it ("never been able to say you need 10 bars"). The two coded tests below are proxies.

Supporting passages:
- [sources/levels-webinar-part1.md:196](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 23.74): "[25:46] important to me to get that element of space um so when we break a level I don't want to be sitting on the bid until we've continued going higher okay"
- [sources/user-research-pages.md:179](../sources/user-research-pages.md) (Does it survive out of sample, score 22.38): "**What changed.** One parameter: the minimum number of bars between a break and a qualifying retest. Everything else is untouched — 3-touch levels clustered within 0.1× the prior day's ATR, entry at the level price, stop at the break candle"
- [sources/course/curated/notes-images.md:33](../sources/course/curated/notes-images.md) (notes-images, score 19.49): "**16/09/2024 20:10** despite what i would call price being swerved here, and running more than 50% to TP (the h1 lows) - the trader shows this as an obvious short. maybe because the level is so clean (being 4 touch) that its just biddable n"

## retest.never-immediate

Never trade the immediate retest (the "buck kiss": the first close beyond the level followed by a bid). It is not reliable on the hourly, and he does not take it even when there is a daily bias.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @26:03; transcript levels webinar part 1, user-pasted transcript @39:16.

Supporting passages:
- [sources/levels-webinar-part1.md:196](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 31.28): "[25:55] and therefore when we come back we've got an element of space a lot of people do trade what we call the immediate retest some people call it the buck kiss which is when literally the first close"
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 20.55): "* Never trade in same direction if market has passed daily ATR"
- [sources/user-research-pages.md:303](../sources/user-research-pages.md) (Rebuild it yourself, score 19.46): "6. **mhm — 24/09/2024 21:23** daily engulfing for bias, entry on h1 (dante's first example uses a d1 S/R, which is also a h1 S/R) because its a 'fantastic level' (three super clean touches recently). this was NOT the cleanest h1 level, but "

## retest.needs-space-in-time

As a rough proxy for space: 5 bars is not enough and an 11-candle retest is "just about enough", roughly half a day before an H1 entry.

*heuristic, observed, tested by code.* Sources: webinar curated/notes-images.md (2024-09-16).

Supporting passages:
- [sources/course/curated/notes-images.md:33](../sources/course/curated/notes-images.md) (notes-images, score 25.35): "he pointed out another entry on a different chart with an 11 candle retest and said ''i would say this is just about enough space''. so it seems like he wants at least ~half a day of time to pass before his H1 entries. (edited) [image]"
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 17.18): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"
- [sources/user-research-pages.md:179](../sources/user-research-pages.md) (Does it survive out of sample, score 16.97): "**What changed.** One parameter: the minimum number of bars between a break and a qualifying retest. Everything else is untouched — 3-touch levels clustered within 0.1× the prior day's ATR, entry at the level price, stop at the break candle"

## retest.needs-space-in-price

Price must travel at least 0.5 x ATR clear of the level before a retest counts. Described as the single most important rule in the system, and it uses only bars that closed before the fill.

*hard, backtest-reported, tested by code.* Sources: backtest user-supplied forensic page (2026-09-13).

Supporting passages:
- [sources/user-research-pages.md:109](../sources/user-research-pages.md) (What the earlier artifacts said, and what is true, score 51.78): "| The space rule — price must travel 0.5×ATR clear before a retest counts | the source webinar | Stands, and is the single most important rule in the system. It uses only bars that closed before the fill. |"
- [sources/pages/SOL_Trade_Check.md:1](../sources/pages/SOL_Trade_Check.md) (SOL_Trade_Check (page text), score 31.85): "SOL Trade Check One rule did the level survive the fill bar SOLUSDT H1 2020 2026 limit at the level space 0.5 ATR 1.5R 2-bar grace at 2 stop SOL Trade Check The gold config, dropped onto SOL unchanged nothing refitted. Every rule carried ac"
- [sources/user-research-pages.md:179](../sources/user-research-pages.md) (Does it survive out of sample, score 21.08): "**What changed.** One parameter: the minimum number of bars between a break and a qualifying retest. Everything else is untouched — 3-touch levels clustered within 0.1× the prior day's ATR, entry at the level price, stop at the break candle"

## retest.bias-relaxes-space

With a daily bias you do not need to be obsessed with the amount of space, but you still want the break to have happened and price to round back to the area.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @38:49; webinar curated/notes-images.md (2024-09-08).

Supporting passages:
- [sources/levels-webinar-part1.md:288](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 36.84): "[38:58] got a good two touch level and you've got a daily bias I'm not obsessed with the amount of space that it takes back there but I still you"
- [sources/levels-webinar-part1.md:210](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 20.6): "[27:45] we need how many bars uh does makes up that space and to be honest with you it's not something that you can quantify"
- [sources/levels-webinar-part1.md:225](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 17.96): "[31:14] look for the break we look for the retest with space and then when it comes back to area we're looking to get"

## retest.clean-break-and-go

The nicest trades break through and just keep going. He does not like any interaction with the level after the break: not break, retest, go, and come back.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @29:30.

Supporting passages:
- [sources/levels-webinar-part1.md:225](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 34.83): "[29:38] break through and we just keep going okay you don't really want to see it break through then retest then go then come back okay I don't really like any"
- [sources/levels-webinar-part1.md:46](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 17.28): "[7:12] the last time that price hit that level so ruling out the trades where where price has washed through levels so"
- [sources/user-research-pages.md:418](../sources/user-research-pages.md) (Rebuild it yourself, score 16.71): ""I am in in a trade, I will hold through news. I don't lighten up, I don't cut, I keep my stop where it is based on technical. and I will hold through news.""

## retest.bounce-just-before-the-level

If price bounced just before the level (missed by a few ticks), did not fill the order, and came straight back, do not take the trade: the market is not done with that level.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @28:49; transcript levels webinar part 1, user-pasted transcript @59:00.

Supporting passages:
- [sources/levels-webinar-part1.md:210](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 37.44): "[28:49] of is if the market bounced just before our level I won't take the trade so what"
- [sources/levels-webinar-part1.md:402](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 25.44): "[54:35] on the chart I don't want to be buying it back at that level you know right on pretty much Bas I've missed it by a few ticks because I'm going to be buying as"
- [sources/course/curated/trade-management.md:93](../sources/course/curated/trade-management.md) (trade-management, score 22.36): "take the profits youve got, dont be mad that you couldve had more, and certainly dont hang around just hoping the market comes back to your levels. the trade is done when the trade is done, it doesnt matter if you played it correctly. [imag"

## retest.momentum-into-level

He likes momentum INTO the level. Buying a level the market is falling into hard is safer than buying one it drifts into slowly, though it is not a must. Reasons: pros take profits quickly after a fast drop, amateurs hold, and a sharp move can bounce like an elastic band.

*preference, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @26:41; transcript levels webinar part 1, user-pasted transcript @1:09:15; transcript levels webinar part 1, user-pasted transcript @1:10:37.

Supporting passages:
- [sources/levels-webinar-part1.md:519](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 55.54): "[1:11:32] uh quite quickly you often find if it trades into a good level that it can have the elastic band effect if it's drifting into that level it worries me"
- [sources/levels-webinar-part1.md:196](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 28.25): "[26:41] momentum into my level it's one of the things I really really like to see so if I'm buying a level I like to see the"
- [sources/levels-webinar-part1.md:402](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 19.34): "[53:38] here it's the next it's the next FDA it's the next trouble area it's about 30 pit move that I'm looking for there structurally a strong level if we bounce"

## retest.drift-in-asia-is-fine

A drift into the level is not a problem in the Asian session, where price drifts anyway. In London or the US session a drift would concern him.

*preference, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @1:12:04.

Supporting passages:
- [sources/levels-webinar-part1.md:535](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 28.82): "[1:12:04] probably going to drift you know so drift in the Asian session is not necessarily a problem because that's often what you're going to find is going to"
- [sources/levels-webinar-part1.md:519](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 14.23): "[1:11:55] although again like with anything you do have to take liquidity into into account with this because obviously if you're trading in the Asian session price is"

## retest.rounded-over-sharp

A rounded retest is preferred over a sharp V-shaped one.

*preference, stated, tested by code.* Sources: webinar user-pasted notes (2024-09-24); lesson curated/taking-a-trade-3-touch-level.md (2024-10-21).

Supporting passages:
- [sources/user-research-pages.md:323](../sources/user-research-pages.md) (Rebuild it yourself, score 21.4): "* Rounded retests are preferred over sharp V-shaped retests"
- [sources/user-research-pages.md:303](../sources/user-research-pages.md) (Rebuild it yourself, score 18.59): "3. **mhm — 24/09/2024 20:49** more touches of the line, the better the level is. he also prefers a rounded retest over a sharp V shaped retest."

## retest.missed-the-level-half-way

If the order is missed and price bounces more than half way to the target, pull the order (a 10-15 pip bounce alone does not). The bounce creates a swing point, stops accumulate beyond it, and buying back there means buying as people stop out.

*hard, stated, tested by code.* Sources: transcript levels webinar part 1, user-pasted transcript @53:21; transcript levels webinar part 1, user-pasted transcript @54:04; webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/levels-webinar-part1.md:402](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 50.06): "[53:30] I'm going to pull the order so I'm not just going to pull it if we bounce 10 15 Pips but it's getting long at 28 85s I want to get I want to get out of this"
- [sources/course/curated/levels.md:19](../sources/course/curated/levels.md) (levels, score 21.68): "daily range, price made a false break under the bottom of the range, and got back inside as a shitty SFP (not sweeping a deep low) and a good pinbar. [image]"
- [sources/levels-webinar-part1.md:386](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 20.93): "[53:05] order and what do I mean by size of or rally away because this often causes some confusion if it bounces 10 ticks away from your level and comes back do"

## retest.swerve-exception

Exception: with a three-touch level and a daily bias he leaves the order in through a swerve, because the daily target is far away.

*hard, observed, tested by code.* Sources: webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/course/curated/trade-management.md:49](../sources/course/curated/trade-management.md) (trade-management, score 25.26): "he later shows an example of a time where he did not remove his orders. the scenario was a three touch level and a daily bias (bullish engulfing). so because of this daily bias - he was okay leaving risk on the table through a swerve."
- [sources/user-research-pages.md:450](../sources/user-research-pages.md) (Rebuild it yourself, score 19.14): ""if youre trading a H1 level, at 8pm and you see a good level to go short... but its quite evident that when you flip to the daily timeframe, the market is very likely to close as a bullish engulfing - youre probably not going to take that "
- [sources/user-research-pages.md:264](../sources/user-research-pages.md) (Rebuild it yourself, score 18.2): "Without Daily Bias (80% of trades):"

## retest.four-swerves

Four swerves of the level should cause the long to be cancelled.

*hard, observed, tested by code.* Sources: webinar curated/trade-management.md (2024-09-30).

No strong supporting passage found in the sources (BM25 score < 10 or fewer than 4 shared words).

## retest.naked

A level is "naked" while it has not been retested on the H1 since the break.

*hard, stated, tested by code.* Sources: tweet curated tweets (2022-10-21); tweet curated tweets (2026-05-19).

Supporting passages:
- [sources/course/tweets/tweets_clean.md:7](../sources/course/tweets/tweets_clean.md) (2022, score 14.3): "Not exactly. At a level like the July '90 high, I would look for an SFP because I don't blindly sell prior highs, I wait to see the price action. If that level had been prior support and it was naked (not retested on the H1 post break), I w"
- [sources/course/curated/lxpb-trades-what-constitutes-a-daily-bias.md:1](../sources/course/curated/lxpb-trades-what-constitutes-a-daily-bias.md) (what-constitutes-a-daily-bias, score 12.07): "entry would be on the prev day KPI if there was a naked level. since theres not, a bias candle needs to form at the level instead."

## retest.it-can-take-a-long-time

The market sometimes takes a long while to come back. It is still the same breakout; the level does not expire because time has passed.

*hard, stated, not coded.* Sources: transcript levels webinar part 1, user-pasted transcript @18:40; transcript levels webinar part 1, user-pasted transcript @19:17.

Supporting passages:
- [sources/levels-webinar-part1.md:133](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 32.39): "[18:47] long to wait before we come back and we get our trade but sometimes the market takes a long while to come back but we've got to"
- [sources/levels-webinar-part1.md:196](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 15.83): "[26:11] especially because I'm trading on the hourly time frame um the longer it stays above the level and the higher it keeps"
- [sources/levels-webinar-part1.md:288](../sources/levels-webinar-part1.md) (Levels webinar, part 1 (transcript), score 15.37): "[38:11] happened again same story we break the level we come back for the retest and the the rest is history we"

## retest.one-in-ten-misses

With pinpoint levels, about 1 order in 10 never fills because the level was too accurate. That is accepted.

*heuristic, stated, not coded.* Sources: webinar curated/trade-management.md (2024-09-24).

Supporting passages:
- [sources/course/curated/trade-management.md:49](../sources/course/curated/trade-management.md) (trade-management, score 15.71): "he says this is very common if youre pinpoint with your levels. maybe 1/10 of your orders wont fill because youre too accurate with your levels [image]"
- [sources/user-research-pages.md:26](../sources/user-research-pages.md) (One real hour, in the order it happened, score 12.94): "Why this is the same bug as the phantom fill Earlier in this project, entries were booked *at the level* whether or not price ever traded there — roughly 60% of fills at prices that never printed. That was caught only because the Pine scrip"
- [sources/course/general/general_full.md:13421](../sources/course/general/general_full.md) (take notes, score 11.37): "**26/10/2024 16:49** this looking decent for EJ on monday? [image]"

## Conflicts

- `retest.momentum-into-level` vs `retest.rounded-over-sharp`: The transcript says he wants price to come into the level HARD; the 2024 notes say he prefers a ROUNDED retest over a sharp V. A hard move in and a rounded retest pull opposite ways. Both are preferences, not vetoes, so the node reports each and decides nothing.
- `retest.needs-space` vs `retest.needs-space-in-price`: The trader says space cannot be quantified and shows a chart. The 11-bar and 0.5 ATR figures are proxies from a note-taker and a backtest page; the same backtest material also reports that waiting 1 bar beat waiting 12. Treat the time proxy as unproven.
