# Equal highs / lows

`equal-liquidity` · stage **O** · decides at **prior_bars** · rule card (nothing to compute)

Equal highs or lows as a target and as a trade in their own right.

## Ports

**Inputs**
- none

**Outputs**
- none

## Rules

| Rule | Kind | Status | Coded | Sources |
|---|---|---|---|---|
| **equal-liquidity.almost-always**<br>"I almost always try to get in to equal highs/lows." The current daily high has to go first to show it is ready, and a weekly level that has been ground into helps. No other confluences needed. | hard | observed | no | journal: curated/lxpb-trades-equal-swings.md (2026-02-23) |
| **equal-liquidity.sweep-tolerance**<br>A second high can count as sweeping the first by a fraction of a pip (0.3 pips in the example), with an H1 hammer. | hard | observed | no | journal: curated/lxpb-trades-equal-swings.md (2026-03-12) |
| **equal-liquidity.as-a-target**<br>Equal lows are used as the target for H1 shorts (and a double top is the destination in a long). Seen as "good targets" because stops sit there. | hard | observed | no | journal: curated/lxpb-trades-engulfing.md (2026-02-12)<br>tweet: curated tweets (2026-09-21) |
| **equal-liquidity.pattern-for-tops**<br>General pattern for spotting tops: a large run-up, an indecision candle (doji) at the peak, a good pullback, then price running back up but failing to make a new high. | heuristic | observed | no | webinar: curated/notes-images.md (2024-10-19) |
