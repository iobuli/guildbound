# Where the sources disagree

These are kept, not resolved. Each node still reports the rule it owns.

## swing-depth.in-out-definition vs swing-depth.deep-threshold

Owned by `swing-depth`.

The notes say an H1 swing must be "1x ATR" without saying which ATR; the lesson examples measure it against the DAILY ATR. This build uses the D1 ATR for both (x1 for H1, x3 for D1).

## sfp.enter-at-close vs sfp.enter-at-the-swept-level

Owned by `sfp`.

The 2024 course teaches selling at the SFP candle close. The 2022 and 2026 posts prefer entering at the swept level / nearest naked level. Both are kept; the entry node reports whichever the setup style asks for.

## daily-bias.unclosed-anticipation vs daily-bias.closed-bars-only

Owned by `daily-bias`.

The discretionary journal anticipates unclosed daily candles; the causal rule forbids it. A systematic version cannot act on a candle that has not printed.

## retest.momentum-into-level vs retest.rounded-over-sharp

Owned by `retest`.

The transcript says he wants price to come into the level HARD; the 2024 notes say he prefers a ROUNDED retest over a sharp V. A hard move in and a rounded retest pull opposite ways. Both are preferences, not vetoes, so the node reports each and decides nothing.

## retest.needs-space vs retest.needs-space-in-price

Owned by `retest`.

The trader says space cannot be quantified and shows a chart. The 11-bar and 0.5 ATR figures are proxies from a note-taker and a backtest page; the same backtest material also reports that waiting 1 bar beat waiting 12. Treat the time proxy as unproven.

## news.pull-limit-when-close vs news.no-fills-window

Owned by `news`.

The 2012 webinar reinserts the order five minutes after the release; the 2024 notes say ten; the red-news window itself is fifteen minutes either side. This build uses five for "reinsert" and fifteen for "no fills".

## target.minimum-rr vs target.far-targets-pay

Owned by `target`.

The teacher's discretionary style is quick in-and-out trades near the first trouble area (0.8-1.2R). The causal backtest says the same entries only pay at 3-5R. They are different styles; do not mix their stops and targets.

## causality.twelve-bar-blind-spot vs causality.space-rule-stands

Owned by `causality`.

One page says wait 1 bar after the break, the other says price must travel 0.5 ATR clear first. They can both hold (distance, not time) but the time claim is untested after the lookahead fix.
